use super::*;
use crate::PhpObject;

impl Runtime {
    pub(super) fn print_r(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        let return_output = args[1]
            .as_ref()
            .map(|value| self.native_bool(value, "print_r", 2, line))
            .transpose()?
            .unwrap_or(false);
        let mut output = Vec::new();
        self.print_value(
            args[0].as_ref().unwrap(),
            0,
            &mut output,
            &mut HashSet::new(),
            line,
        )?;
        if return_output {
            Ok(Value::String(output.into()))
        } else {
            self.emit_output(&output);
            Ok(Value::Bool(true))
        }
    }

    fn debug_properties(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<PhpArray, Error> {
        let Some(method) = objects::method(&object.class, b"__debugInfo") else {
            return Ok(object.snapshot());
        };
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        match self.invoke(target, &[], line, false)?.dereferenced() {
            Value::Array(properties) => Ok((*properties).clone()),
            Value::Null => {
                self.diagnostics.push(format!("Deprecated: Returning null from {}::__debugInfo() is deprecated, return an empty array instead on line {line}", object.class_name));
                Ok(PhpArray::default())
            }
            _ => Err(Error::new(
                ErrorKind::Error,
                "__debugInfo() must return an array",
                line,
            )),
        }
    }

    fn print_value(
        &mut self,
        value: &Value,
        indent: usize,
        output: &mut Vec<u8>,
        seen: &mut HashSet<(bool, usize)>,
        line: usize,
    ) -> Result<(), Error> {
        // Release reference borrows before invoking __debugInfo(), which may
        // modify the same reference or property table while rendering it.
        match value.dereferenced() {
            Value::Array(array) => {
                output.extend_from_slice(b"Array\n");
                let identity = (false, Rc::as_ptr(&array) as usize);
                if !seen.insert(identity) {
                    output.extend_from_slice(b" *RECURSION*");
                    return Ok(());
                }
                self.print_table(&array, false, indent, output, seen, line)?;
                seen.remove(&identity);
            }
            Value::Object(object) => {
                output.extend_from_slice(object.class_name.split('\0').next().unwrap().as_bytes());
                if object.class.declaration.is_enum {
                    output.extend(b" Enum");
                    if let Some(backing) = &object.class.declaration.enum_backing {
                        output.push(b':');
                        output.extend(backing.bytes());
                    }
                    output.push(b'\n');
                } else {
                    output.extend_from_slice(b" Object\n");
                }
                let identity = (true, object.object_id);
                if !seen.insert(identity) {
                    output.extend_from_slice(b" *RECURSION*");
                    return Ok(());
                }
                let properties = self.debug_properties(&object, line)?;
                self.print_table(&properties, true, indent, output, seen, line)?;
                seen.remove(&identity);
            }
            Value::Closure(closure) => {
                output.extend_from_slice(b"Closure Object\n");
                let identity = (true, closure.object_id);
                if !seen.insert(identity) {
                    output.extend_from_slice(b" *RECURSION*");
                    return Ok(());
                }
                let properties = builtins::closure_debug_properties(&closure);
                self.print_table(&properties, true, indent, output, seen, line)?;
                seen.remove(&identity);
            }
            value => {
                value.warn_nan("string", line, &mut self.diagnostics);
                output.extend(value.to_bytes());
            }
        }
        Ok(())
    }

    fn print_table(
        &mut self,
        table: &PhpArray,
        object: bool,
        indent: usize,
        output: &mut Vec<u8>,
        seen: &mut HashSet<(bool, usize)>,
        line: usize,
    ) -> Result<(), Error> {
        output.extend(std::iter::repeat_n(b' ', indent));
        output.extend_from_slice(b"(\n");
        for (key, value) in table.entries() {
            output.extend(std::iter::repeat_n(b' ', indent + 4));
            output.push(b'[');
            let raw = key.to_value().to_bytes();
            if object && matches!(key, ArrayKey::String(_)) {
                match crate::object::property_name(&raw) {
                    Ok(name) => {
                        output.extend_from_slice(name.name);
                        if let Some(scope) = name.scope {
                            if scope == b"*" {
                                output.extend_from_slice(b":protected");
                            } else {
                                output.push(b':');
                                output.extend_from_slice(scope);
                                output.extend_from_slice(b":private");
                            }
                        }
                    }
                    Err(message) => {
                        self.diagnostics
                            .push(format!("Notice: {message} on line {line}"));
                        output.extend_from_slice(&raw);
                    }
                }
            } else {
                output.extend_from_slice(&raw);
            }
            output.extend_from_slice(b"] => ");
            self.print_value(value, indent + 8, output, seen, line)?;
            output.push(b'\n');
        }
        output.extend(std::iter::repeat_n(b' ', indent));
        output.extend_from_slice(b")\n");
        Ok(())
    }
}
