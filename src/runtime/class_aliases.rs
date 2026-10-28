use super::{builtins::Builtin, *};

impl Runtime {
    pub(super) fn class_alias(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = self.native_string(args[0].as_ref().unwrap(), "class_alias", 1, line)?;
        let spelling = self.native_string(args[1].as_ref().unwrap(), "class_alias", 2, line)?;
        let autoload = args[2]
            .as_ref()
            .map(|value| self.native_bool(value, "class_alias", 3, line))
            .transpose()?
            .unwrap_or(true);
        let class = if autoload {
            self.load_class(&name, line)?
        } else {
            self.find_class(&name)
        };
        let Some(class) = class else {
            self.diagnostics.push(format!(
                "Class \"{}\" not found on line {line}",
                String::from_utf8_lossy(&name)
            ));
            return Ok(Value::Bool(false));
        };
        let alias = spelling
            .strip_prefix(b"\\")
            .unwrap_or(&spelling)
            .to_ascii_lowercase();
        if matches!(
            alias.rsplit(|b| *b == b'\\').next().unwrap(),
            b"bool"
                | b"false"
                | b"float"
                | b"int"
                | b"null"
                | b"parent"
                | b"self"
                | b"static"
                | b"string"
                | b"true"
                | b"void"
                | b"never"
                | b"iterable"
                | b"object"
                | b"mixed"
                | b"array"
                | b"callable"
        ) {
            return Err(Error::new(
                ErrorKind::CompileError,
                format!(
                    "Cannot use \"{}\" as a class alias as it is reserved",
                    String::from_utf8_lossy(&alias)
                ),
                line,
            ));
        }
        if alias == b"_" {
            self.diagnostics.push(format!(
                "Deprecated: Using \"_\" as a class alias is deprecated since 8.4 on line {line}"
            ));
        }
        if self.find_class(&alias).is_some() || alias == b"closure" {
            let kind = if class.declaration.is_interface {
                "interface"
            } else if class.declaration.is_trait {
                "trait"
            } else {
                "class"
            };
            self.diagnostics.push(format!(
                "Cannot redeclare {kind} {} on line {line}",
                String::from_utf8_lossy(&spelling)
            ));
            return Ok(Value::Bool(false));
        }
        class.aliases.borrow_mut().push(alias.clone());
        self.classes.insert(alias, class);
        Ok(Value::Bool(true))
    }

    pub(super) fn inspect_inheritance(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let only_subclass = matches!(builtin, Builtin::IsSubclassOf);
        let function = if only_subclass {
            "is_subclass_of"
        } else {
            "is_a"
        };
        let name = self.native_string(args[1].as_ref().unwrap(), function, 2, line)?;
        let allow_string = args[2]
            .as_ref()
            .map(|value| self.native_bool(value, function, 3, line))
            .transpose()?
            .unwrap_or(only_subclass);
        let value = args[0].as_ref().unwrap();
        // Closure has its own value representation and no class-table entry yet.
        let closure = match value {
            Value::Closure(_) => true,
            Value::String(name) if allow_string => {
                let name = name.bytes();
                name.strip_prefix(b"\\")
                    .unwrap_or(&name)
                    .eq_ignore_ascii_case(b"Closure")
            }
            _ => false,
        };
        if closure {
            return Ok(Value::Bool(
                !only_subclass
                    && name
                        .strip_prefix(b"\\")
                        .unwrap_or(&name)
                        .eq_ignore_ascii_case(b"Closure"),
            ));
        }
        let instance = match value {
            Value::Object(object) => Some(object.class.clone()),
            Value::String(name) if allow_string => self.load_class(&name.bytes(), line)?,
            _ => None,
        };
        let result = instance.is_some_and(|instance| {
            self.find_class(&name).is_some_and(|target| {
                (!only_subclass || instance.name != target.name) && instance.is_a(&target.name)
            })
        });
        Ok(Value::Bool(result))
    }
}
