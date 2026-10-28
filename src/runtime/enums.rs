use super::*;
use crate::{
    PhpObject,
    ast::{ClassConstant, ClassDecl, NativeMethod},
    object::Class,
};

impl Runtime {
    pub(super) fn prepare_enum(
        &self,
        mut declaration: Rc<ClassDecl>,
    ) -> Result<Rc<ClassDecl>, Error> {
        let decl = &declaration;
        for name in &decl.interfaces {
            if let Some(interface) = self.find_class(name) {
                if interface.is_a(b"UnitEnum") && !decl.is_enum && !decl.is_interface {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Non-enum class {} cannot implement interface {}",
                            String::from_utf8_lossy(&decl.name),
                            interface.display_name()
                        ),
                        decl.line,
                    ));
                }
                if decl.is_enum && decl.enum_backing.is_none() && interface.is_a(b"BackedEnum") {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Non-backed enum {} cannot implement interface BackedEnum",
                            String::from_utf8_lossy(&decl.name)
                        ),
                        decl.line,
                    ));
                }
            }
        }
        if !decl.is_enum {
            return Ok(declaration);
        }
        if !decl.properties.is_empty() || !decl.static_properties.is_empty() {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Enum {} cannot include properties",
                    String::from_utf8_lossy(&decl.name)
                ),
                decl.line,
            ));
        }
        for function in &decl.methods {
            let method = method_name(function).to_ascii_lowercase();
            if matches!(
                method.as_slice(),
                b"__construct"
                    | b"__destruct"
                    | b"__clone"
                    | b"__get"
                    | b"__set"
                    | b"__unset"
                    | b"__isset"
                    | b"__tostring"
                    | b"__debuginfo"
                    | b"__serialize"
                    | b"__unserialize"
                    | b"__sleep"
                    | b"__wakeup"
                    | b"__set_state"
            ) {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Enum {} cannot include magic method {}",
                        String::from_utf8_lossy(&decl.name),
                        String::from_utf8_lossy(method_name(function))
                    ),
                    function.line,
                ));
            }
            if method == b"cases"
                || (decl.enum_backing.is_some()
                    && matches!(method.as_slice(), b"from" | b"tryfrom"))
            {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Cannot redeclare {}()",
                        String::from_utf8_lossy(&function.name)
                    ),
                    function.line,
                ));
            }
        }
        let backing = decl.enum_backing.as_ref().map_or(String::new(), |ty| format!("public readonly {ty} $value; public static function from(int|string $value): static {{}} public static function tryFrom(int|string $value): ?static {{}}"));
        let code = format!(
            "<?php class EnumMethods {{ public readonly string $name; public static function cases(): array {{}} {backing} }}"
        );
        let (mut statements, _, _) =
            parser::parse(lexer::lex(code.as_bytes()).expect("enum source")).expect("enum methods");
        let Stmt::Class(template) = statements.remove(0) else {
            unreachable!()
        };
        let decl = Rc::make_mut(&mut declaration);
        decl.properties = template.properties.clone();
        for property in &mut decl.properties {
            property.declaring_class = decl.name.clone();
        }
        for function in &template.methods {
            let mut function = (**function).clone();
            let short = method_name(&function).to_vec();
            function.name = decl.name.clone();
            function.name.extend(b"::");
            function.name.extend(short);
            function.native = Some(NativeMethod::Enum);
            decl.methods.push(Rc::new(function));
        }
        decl.interfaces.insert(0, b"UnitEnum".to_vec());
        if decl.enum_backing.is_some() {
            decl.interfaces.insert(1, b"BackedEnum".to_vec());
        }
        Ok(declaration)
    }

    fn enum_backing_values(
        &mut self,
        class: &Rc<Class>,
        line: usize,
    ) -> Result<Vec<(Vec<u8>, Value)>, Error> {
        if let Some(values) = self.enum_values.get(&class.name) {
            return Ok(values.clone());
        }
        let expected = class.declaration.enum_backing.as_ref().unwrap();
        let outer = self.class_context.replace(crate::object::ClassContext {
            scope: class.clone(),
            called: class.clone(),
        });
        let result = (|| {
            let mut values: Vec<(Vec<u8>, Value)> = Vec::new();
            for constant in class
                .declaration
                .constants
                .iter()
                .filter(|c| c.is_enum_case)
            {
                let value = self.eval_default(&constant.value)?;
                if !matches!(
                    (&value, expected.as_str()),
                    (Value::Int(_), "int") | (Value::String(_), "string")
                ) {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "Enum case type {} does not match enum backing type {expected}",
                            builtins::debug_type(&value)
                        ),
                        line,
                    ));
                }
                if let Some((previous, _)) = values.iter().find(|(_, v)| *v == value) {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Duplicate value in enum {} for cases {} and {}",
                            class.display_name(),
                            String::from_utf8_lossy(previous),
                            String::from_utf8_lossy(&constant.name)
                        ),
                        line,
                    ));
                }
                values.push((constant.name.clone(), value));
            }
            Ok(values)
        })();
        self.class_context = outer;
        let values = result?;
        self.enum_values.insert(class.name.clone(), values.clone());
        Ok(values)
    }

    pub(super) fn enum_case(
        &mut self,
        class: &Rc<Class>,
        case: &ClassConstant,
        line: usize,
    ) -> Result<Value, Error> {
        let mut properties = PhpArray::default();
        properties.insert(
            ArrayKey::String(b"name".to_vec()),
            Value::String(case.name.clone().into()),
        );
        if class.declaration.enum_backing.is_some() {
            let values = self.enum_backing_values(class, line)?;
            let value = values
                .into_iter()
                .find(|(name, _)| *name == case.name)
                .unwrap()
                .1;
            properties.insert(ArrayKey::String(b"value".to_vec()), value);
        }
        self.next_object_id += 1;
        Ok(Value::Object(Rc::new(PhpObject {
            class: class.clone(),
            class_name: class.display_name().into_owned(),
            object_id: self.next_object_id,
            properties: RefCell::new(properties),
            unset_properties: RefCell::default(),
            reinitializable: RefCell::default(),
            throwable: RefCell::default(),
            reflection: RefCell::default(),
        })))
    }

    pub(super) fn native_enum_method(&mut self, line: usize) -> Result<Value, Error> {
        let class = self.class_context.as_ref().unwrap().scope.clone();
        let method = self
            .current_function
            .as_ref()
            .unwrap()
            .rsplit(|b| *b == b':')
            .next()
            .unwrap();
        if method == b"cases" {
            let mut result = PhpArray::default();
            for case in class
                .declaration
                .constants
                .iter()
                .filter(|c| c.is_enum_case)
            {
                result.append(
                    self.evaluate_class_constant(&class, &case.name, line)?,
                    line,
                )?;
            }
            return Ok(Value::Array(Rc::new(result)));
        }
        let try_from = method == b"tryfrom";
        let value = self.lookup(b"value").unwrap();
        let value = if class.declaration.enum_backing.as_deref() == Some("string") {
            Value::String(value.to_bytes().into())
        } else {
            value
        };
        let values = self.enum_backing_values(&class, line)?;
        if let Some((name, _)) = values.into_iter().find(|(_, v)| *v == value) {
            return self.evaluate_class_constant(&class, &name, line);
        }
        if try_from {
            return Ok(Value::Null);
        }
        let label = match &value {
            Value::String(s) => format!("\"{}\"", String::from_utf8_lossy(&s.to_vec())),
            _ => String::from_utf8_lossy(&value.to_bytes()).into_owned(),
        };
        Err(Error::new(
            ErrorKind::ValueError,
            format!(
                "{label} is not a valid backing value for enum {}",
                class.display_name()
            ),
            line,
        ))
    }
}

fn method_name(function: &FunctionDecl) -> &[u8] {
    function.name.rsplit(|b| *b == b':').next().unwrap()
}
