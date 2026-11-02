// Altered Rust adaptation of PHP 8.5.0 ext/spl/php_spl.c and spl_functions.c.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum Relation {
    Parents,
    Interfaces,
    Traits,
}

impl Runtime {
    pub(super) fn class_relations(
        &mut self,
        relation: Relation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        // Native parameter parsing checks autoload before validating the mixed
        // first argument. Class-name scalars are not coerced to strings.
        let autoload = args[1]
            .as_ref()
            .map(|value| self.native_bool(value, name, 2, line))
            .transpose()?
            .unwrap_or(true);
        let mut result = PhpArray::default();
        let class = match args[0].as_ref().unwrap() {
            Value::Object(object) => object.class.clone(),
            // Closures have no parents, interfaces or traits, and use a
            // separate runtime representation rather than a Class allocation.
            Value::Closure(_) => return Ok(Value::Array(Rc::new(result))),
            Value::String(class_name) => {
                let bytes = class_name.to_vec();
                let lookup = if autoload {
                    bytes.strip_prefix(b"\\").unwrap_or(&bytes)
                } else {
                    &bytes
                };
                if lookup.eq_ignore_ascii_case(b"Closure") {
                    return Ok(Value::Array(Rc::new(result)));
                }
                let loaded = if autoload {
                    self.load_class(&bytes, line)
                } else if bytes.starts_with(b"\\") {
                    // The no-autoload path directly probes the class table.
                    Ok(None)
                } else {
                    Ok(self.find_class(&bytes))
                };
                let (class, pending) = match loaded {
                    Ok(class) => (class, None),
                    Err(error) if error.kind == ErrorKind::UnwindExit => return Err(error),
                    Err(error) => (None, Some(error)),
                };
                let Some(class) = class else {
                    let label = bytes.split(|b| *b == 0).next().unwrap();
                    // With a pending autoloader exception PHP still emits the
                    // default warning, but suppresses a matching user handler.
                    if pending.is_none()
                        || self.error_handler.is_none()
                        || self.error_handler_reporting & 2 == 0
                    {
                        self.warn(format!(
                            "{name}(): Class {} does not exist{} on line {line}",
                            String::from_utf8_lossy(label),
                            if autoload {
                                " and could not be loaded"
                            } else {
                                ""
                            }
                        ))?;
                    }
                    return pending.map_or(Ok(Value::Bool(false)), Err);
                };
                class
            }
            value => return Err(builtins::type_error(value, name, 1, "object|string", line)),
        };
        let mut classes = Vec::new();
        match relation {
            Relation::Parents => {
                let mut parent = class.parent.clone();
                while let Some(class) = parent {
                    parent = class.parent.clone();
                    classes.push(class);
                }
            }
            Relation::Interfaces => reflection::collect_interfaces(&class, &mut classes),
            Relation::Traits => {
                for name in class
                    .declaration
                    .trait_uses
                    .iter()
                    .flat_map(|use_| &use_.names)
                {
                    if let Some(class) = self.find_class(name) {
                        classes.push(class);
                    }
                }
            }
        }
        for class in classes {
            result.insert(
                ArrayKey::String(class.name.clone()),
                Value::String(class.name.clone().into()),
            );
        }
        Ok(Value::Array(Rc::new(result)))
    }
}
