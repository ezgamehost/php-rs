use super::*;

impl Runtime {
    pub(super) fn inspect_callable(
        &mut self,
        args: &[Option<Value>],
        references: &[Option<variables::Cell>],
        line: usize,
    ) -> Result<Value, Error> {
        let value = args[0].as_ref().unwrap();
        let syntax_only = args[1]
            .as_ref()
            .map(|value| self.native_bool(value, "is_callable", 2, line))
            .transpose()?
            .unwrap_or(false);
        let result = if syntax_only {
            Ok(match value {
                Value::String(_) | Value::Closure(_) => true,
                Value::Array(array) => method_parts(array).is_some(),
                Value::Object(object) => objects::method(&object.class, b"__invoke").is_some(),
                _ => false,
            })
        } else {
            self.check_callable(value, line)
        };
        // PHP also produces the descriptive name when validation fails or an
        // autoloader throws. Compute it only when the output argument is supplied.
        if args[2].is_some() {
            let name = Value::String(self.callable_name(value, line)?.into());
            if let Some(reference) = &references[2] {
                self.write_reference_value(reference, name, self.strict_types, line)?;
            }
        }
        result.map(Value::Bool)
    }

    fn check_callable(&mut self, value: &Value, line: usize) -> Result<bool, Error> {
        let mut normalized = value.clone();
        let class_method = match value {
            Value::String(name) => {
                let name = name.bytes();
                name.windows(2)
                    .rposition(|part| part == b"::")
                    .map(|index| (name[..index].to_vec(), name[index + 2..].to_vec()))
            }
            Value::Array(array) => method_parts(array).and_then(|(base, name)| {
                if let Value::String(class) = base {
                    Some((class.to_vec(), name))
                } else {
                    None
                }
            }),
            _ => None,
        };
        if let Some((class, method)) = class_method {
            let keyword = class.to_ascii_lowercase();
            let name = if matches!(keyword.as_slice(), b"self" | b"parent" | b"static") {
                let name = match self.contextual_class_name(&class, line) {
                    Ok(name) => name,
                    Err(_) => return Ok(false),
                };
                self.diagnostics.push(format!(
                    "Deprecated: Use of \"{}\" in callables is deprecated on line {line}",
                    String::from_utf8_lossy(&keyword),
                ));
                name
            } else {
                class
            };
            let mut array = PhpArray::default();
            array.append(Value::String(name.into()), line)?;
            array.append(Value::String(method.into()), line)?;
            normalized = Value::Array(Rc::new(array));
        }

        if let Value::Array(array) = &normalized
            && let Some((base, method)) = method_parts(array)
            && let Some(separator) = method.windows(2).rposition(|part| part == b"::")
        {
            return self.qualified_callable(base, &method, separator, line);
        }
        match self.resolve_native_callback(&normalized, line) {
            Ok(target) => Ok(target
                .function
                .as_ref()
                .is_none_or(|function| !function.is_abstract)),
            Err(error) if error.throwable.is_some() || error.kind == ErrorKind::UnwindExit => {
                Err(error)
            }
            Err(_) => Ok(false),
        }
    }

    fn qualified_callable(
        &mut self,
        base: Value,
        method: &[u8],
        separator: usize,
        line: usize,
    ) -> Result<bool, Error> {
        if separator == 0 {
            return Ok(false);
        }
        let (class, object) = match base {
            Value::Object(object) => (object.class.clone(), Some(object)),
            Value::String(name) => {
                let Some(class) = self.load_class(&name.bytes(), line)? else {
                    return Ok(false);
                };
                let object = if self
                    .class_context
                    .as_ref()
                    .is_some_and(|context| context.scope.is_a(&class.name))
                {
                    match self.lookup(b"this") {
                        Some(Value::Object(object)) if object.class.is_a(&class.name) => {
                            Some(object)
                        }
                        _ => None,
                    }
                } else {
                    None
                };
                (class, object)
            }
            _ => return Ok(false),
        };
        let qualifier = &method[..separator];
        let target = match qualifier.to_ascii_lowercase().as_slice() {
            b"self" => Some(class.clone()),
            b"parent" => class.parent.clone(),
            b"static" => self
                .class_context
                .as_ref()
                .map(|context| context.called.clone()),
            _ => self.load_class(qualifier, line)?,
        };
        let Some(target) = target.filter(|target| class.is_a(&target.name)) else {
            return Ok(false);
        };
        self.diagnostics.push(format!(
            "Deprecated: Callables of the form [\"{}\", \"{}\"] are deprecated on line {line}",
            class.display_name(),
            String::from_utf8_lossy(method),
        ));
        if let Some(function) = objects::method(&target, &method[separator + 2..])
            && self.check_method_access(&function, line).is_ok()
        {
            return Ok(!function.is_abstract && (function.is_static || object.is_some()));
        }
        Ok(if object.is_some() && Rc::ptr_eq(&target, &class) {
            objects::method(&target, b"__call").is_some()
        } else {
            objects::method(&target, b"__callStatic").is_some()
        })
    }

    fn callable_name(&mut self, value: &Value, line: usize) -> Result<Vec<u8>, Error> {
        Ok(match value {
            Value::String(name) => name.to_vec(),
            Value::Array(array) => {
                let Some((base, method)) = method_parts(array) else {
                    return Ok(b"Array".to_vec());
                };
                let mut name = match base {
                    Value::String(name) => name.to_vec(),
                    Value::Object(object) => object.class.name.clone(),
                    Value::Closure(_) => b"Closure".to_vec(),
                    _ => unreachable!(),
                };
                name.extend(b"::");
                name.extend(method);
                name
            }
            Value::Closure(closure) => {
                if closure.magic_name.is_some() {
                    closure.name.clone()
                } else {
                    closure
                        .function
                        .as_ref()
                        .map_or_else(|| closure.name.clone(), |function| function.name.clone())
                }
            }
            Value::Object(object) => {
                let mut name = object.class.name.clone();
                name.extend(b"::__invoke");
                name
            }
            _ => self.string_bytes(value, line)?,
        })
    }
}

fn method_parts(array: &PhpArray) -> Option<(Value, Vec<u8>)> {
    if array.len() != 2 {
        return None;
    }
    let base = array.get(&ArrayKey::Int(0))?.dereferenced();
    let Value::String(method) = array.get(&ArrayKey::Int(1))?.dereferenced() else {
        return None;
    };
    if !matches!(
        base,
        Value::String(_) | Value::Object(_) | Value::Closure(_)
    ) {
        return None;
    }
    Some((base, method.to_vec()))
}
