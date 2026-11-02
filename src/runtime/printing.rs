use super::*;
use crate::PhpObject;

#[derive(Default)]
struct PrintState {
    output: Vec<u8>,
    seen: HashSet<(bool, usize)>,
    pending: Option<Error>,
}

impl Runtime {
    pub(super) fn print_r(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        let return_output = args[1]
            .as_ref()
            .map(|value| self.native_bool(value, "print_r", 2, line))
            .transpose()?
            .unwrap_or(false);
        let mut state = PrintState::default();
        self.print_value(args[0].as_ref().unwrap(), 0, &mut state, line)?;
        if return_output {
            if let Some(error) = state.pending {
                return Err(error);
            }
            Ok(Value::String(state.output.into()))
        } else {
            self.emit_output(&state.output)?;
            if let Some(error) = state.pending {
                return Err(error);
            }
            Ok(Value::Bool(true))
        }
    }

    pub(super) fn debug_properties(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Result<PhpArray, Error> {
        let Some(method) = objects::method(&object.class, b"__debugInfo") else {
            return Ok(object.export_properties());
        };
        if let Some(error) = pending.take() {
            // PHP cannot enter another hook with an exception already pending.
            self.report_pending_exception(error)?;
            return Err(self.native_fatal(b"__debuginfo() must return an array", line, false));
        }
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        let value = match self.invoke_at(target, &[], line, false, true) {
            Ok(value) => value.dereferenced(),
            Err(error) if error.kind == ErrorKind::UnwindExit => return Err(error),
            Err(error) => {
                self.report_pending_exception(error)?;
                return Err(self.native_fatal(b"__debuginfo() must return an array", line, false));
            }
        };
        match value {
            Value::Array(properties) => Ok((*properties).clone()),
            Value::Null => {
                self.defer_warning(format!("Deprecated: Returning null from {}::__debugInfo() is deprecated, return an empty array instead on line {line}", object.class_name), pending)?;
                Ok(PhpArray::default())
            }
            _ => Err(self.native_fatal(b"__debuginfo() must return an array", line, true)),
        }
    }

    fn print_value(
        &mut self,
        value: &Value,
        indent: usize,
        state: &mut PrintState,
        line: usize,
    ) -> Result<(), Error> {
        // Release reference borrows before invoking __debugInfo(), which may
        // modify the same reference or property table while rendering it.
        match value.dereferenced() {
            Value::Array(array) => {
                state.output.extend_from_slice(b"Array\n");
                let identity = (false, Rc::as_ptr(&array) as usize);
                if !state.seen.insert(identity) {
                    state.output.extend_from_slice(b" *RECURSION*");
                    return Ok(());
                }
                self.print_table(&array, false, indent, state, line)?;
                state.seen.remove(&identity);
            }
            Value::Object(object) => {
                state
                    .output
                    .extend_from_slice(object.class_name.split('\0').next().unwrap().as_bytes());
                if object.class.declaration.is_enum {
                    state.output.extend(b" Enum");
                    if let Some(backing) = &object.class.declaration.enum_backing {
                        state.output.push(b':');
                        state.output.extend(backing.bytes());
                    }
                    state.output.push(b'\n');
                } else {
                    state.output.extend_from_slice(b" Object\n");
                }
                let identity = (true, object.object_id);
                if !state.seen.insert(identity) {
                    state.output.extend_from_slice(b" *RECURSION*");
                    return Ok(());
                }
                let properties = self.debug_properties(&object, line, &mut state.pending)?;
                self.print_table(&properties, true, indent, state, line)?;
                state.seen.remove(&identity);
            }
            Value::Closure(closure) => {
                state.output.extend_from_slice(b"Closure Object\n");
                let identity = (true, closure.object_id);
                if !state.seen.insert(identity) {
                    state.output.extend_from_slice(b" *RECURSION*");
                    return Ok(());
                }
                let properties = builtins::closure_debug_properties(&closure);
                self.print_table(&properties, true, indent, state, line)?;
                state.seen.remove(&identity);
            }
            value => {
                value.warn_nan("string", line, self)?;
                state.output.extend(value.to_bytes());
            }
        }
        Ok(())
    }

    fn print_table(
        &mut self,
        table: &PhpArray,
        object: bool,
        indent: usize,
        state: &mut PrintState,
        line: usize,
    ) -> Result<(), Error> {
        state.output.extend(std::iter::repeat_n(b' ', indent));
        state.output.extend_from_slice(b"(\n");
        for (ref key, ref value) in table.iter() {
            state.output.extend(std::iter::repeat_n(b' ', indent + 4));
            state.output.push(b'[');
            let raw = key.to_value().to_bytes();
            if object && matches!(key, ArrayKey::String(_)) {
                match crate::object::property_name(&raw) {
                    Ok(name) => {
                        state.output.extend_from_slice(name.name);
                        if let Some(scope) = name.scope {
                            if scope == b"*" {
                                state.output.extend_from_slice(b":protected");
                            } else {
                                state.output.push(b':');
                                state.output.extend_from_slice(scope);
                                state.output.extend_from_slice(b":private");
                            }
                        }
                    }
                    Err(message) => {
                        self.defer_warning(
                            format!("Notice: {message} on line {line}"),
                            &mut state.pending,
                        )?;
                        state.output.extend_from_slice(&raw);
                    }
                }
            } else {
                state.output.extend_from_slice(&raw);
            }
            state.output.extend_from_slice(b"] => ");
            self.print_value(value, indent + 8, state, line)?;
            state.output.push(b'\n');
        }
        state.output.extend(std::iter::repeat_n(b' ', indent));
        state.output.extend_from_slice(b")\n");
        Ok(())
    }
}
