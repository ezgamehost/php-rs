use super::{builtins::Builtin, *};
use crate::object::Class;

impl Runtime {
    pub(super) fn class_inspection(
        &mut self,
        builtin: Builtin,
        value: &Value,
        line: usize,
    ) -> Result<Value, Error> {
        let methods = matches!(builtin, Builtin::GetClassMethods);
        let class_name = if methods {
            match value {
                Value::Object(object) => object.class.name.clone(),
                Value::Closure(_) => b"Closure".to_vec(),
                Value::String(name) => name.to_vec(),
                _ => return Err(class_methods_error(value, line)),
            }
        } else {
            // The legacy class-name parameter converts values to strings even
            // in strict mode, then validates/autoloads the resulting name.
            self.string_bytes(value, line)?
        };
        // Closures use their own runtime value representation, without a Class.
        if class_name
            .strip_prefix(b"\\")
            .unwrap_or(&class_name)
            .eq_ignore_ascii_case(b"Closure")
        {
            let mut result = PhpArray::default();
            if methods {
                for name in ["bind", "bindTo", "call", "fromCallable", "getCurrent"] {
                    result.append(Value::String(name.as_bytes().to_vec().into()), line)?;
                }
            }
            return Ok(Value::Array(Rc::new(result)));
        }
        let class = match value {
            Value::Object(object) if methods => Some(object.class.clone()),
            _ => self.load_class(&class_name, line)?,
        };
        let class = class.ok_or_else(|| if methods {
            class_methods_error(value, line)
        } else {
            Error::new(ErrorKind::TypeError, format!(
                "get_class_vars(): Argument #1 ($class) must be a valid class name, {} given",
                String::from_utf8_lossy(&class_name).split('\0').next().unwrap(),
            ), line)
        })?;
        let mut result = PhpArray::default();
        if methods {
            for method in ordered_methods(&class) {
                if self.check_method_access(&method, line).is_ok() {
                    let name = method.name.rsplit(|byte| *byte == b':').next().unwrap();
                    result.append(Value::String(name.to_vec().into()), line)?;
                }
            }
        } else {
            self.initialize_class(&class)?;
            let mut properties = Vec::new();
            let mut seen = HashSet::new();
            let mut owner = Some(class.clone());
            while let Some(current) = owner {
                for (is_static, declarations) in [
                    (false, &current.declaration.properties),
                    (true, &current.declaration.static_properties),
                ] {
                    for property in declarations {
                        // A child declaration shadows the parent name across
                        // both storage kinds, even when it is inaccessible.
                        if seen.insert(property.name.clone()) {
                            properties.push((current.clone(), property.clone(), is_static));
                        }
                    }
                }
                owner = current.parent.clone();
            }
            // Instance defaults precede static declaration defaults. Mutable
            // static values and uninitialized-property errors do not apply.
            for is_static in [false, true] {
                for (owner, property, storage) in &properties {
                    if *storage == is_static
                        && self.can_access_static_visibility(owner, property.visibility)
                    {
                        let value = self
                            .property_default(owner, property)?
                            .unwrap_or(Value::Null);
                        result.insert(ArrayKey::String(property.name.clone()), value);
                    }
                }
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }
}

fn class_methods_error(value: &Value, line: usize) -> Error {
    Error::new(
        ErrorKind::TypeError,
        format!(
            "get_class_methods(): Argument #1 ($object_or_class) must be an object or a valid class name, {} given",
            match value {
                Value::Bool(true) => "true",
                Value::Bool(false) => "false",
                _ => builtins::debug_type(value),
            },
        ),
        line,
    )
}

fn ordered_methods(class: &Class) -> Vec<Rc<FunctionDecl>> {
    let mut result = class.declaration.methods.clone();
    let mut seen: HashSet<_> = result
        .iter()
        .map(|method| {
            method
                .name
                .rsplit(|byte| *byte == b':')
                .next()
                .unwrap()
                .to_ascii_lowercase()
        })
        .collect();
    let inherited = class
        .parent
        .as_ref()
        .map_or_else(Vec::new, |parent| ordered_methods(parent));
    for method in inherited.iter().chain(class.methods.iter()) {
        let name = method
            .name
            .rsplit(|byte| *byte == b':')
            .next()
            .unwrap()
            .to_ascii_lowercase();
        if seen.insert(name) {
            result.push(method.clone());
        }
    }
    result
}
