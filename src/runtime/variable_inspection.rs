use super::*;

impl Runtime {
    pub(super) fn defined_variables(&self, dynamic: bool, line: usize) -> Result<Value, Error> {
        forbid_dynamic("get_defined_vars", dynamic, line)?;
        let mut result = PhpArray::default();
        for (name, variable) in self.scope.iter() {
            // $this is an execution-context value, not a compiled local slot.
            if name == b"this" {
                continue;
            }
            let value = if Rc::strong_count(variable) > 1 {
                Value::Reference(variable.clone())
            } else {
                variable.borrow().clone()
            };
            // The symbol table preserves even numeric names as string keys.
            result.insert(ArrayKey::String(name.clone()), value);
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn compact_variables(
        &mut self,
        args: &[Option<Value>],
        extra: &PhpArray,
        dynamic: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if extra
            .entries()
            .iter()
            .any(|(key, _)| matches!(key, ArrayKey::String(_)))
        {
            return Err(Error::new(
                ErrorKind::ArgumentCountError,
                "compact() does not accept unknown named parameters",
                line,
            ));
        }
        forbid_dynamic("compact", dynamic, line)?;
        let mut state = CompactState {
            result: PhpArray::default(),
            active: HashSet::new(),
            error: None,
        };
        let values = std::iter::once(args[0].as_ref().unwrap())
            .chain(extra.entries().iter().map(|(_, value)| value));
        for (index, value) in values.enumerate() {
            self.compact_names(value, index + 1, line, &mut state);
        }
        match state.error {
            Some(error) => Err(error),
            None => Ok(Value::Array(Rc::new(state.result))),
        }
    }

    fn compact_names(
        &mut self,
        names: &Value,
        position: usize,
        line: usize,
        state: &mut CompactState,
    ) {
        match names.dereferenced() {
            Value::String(name) => {
                let name = name.to_vec();
                if let Some(variable) = self.scope.get(&name) {
                    state
                        .result
                        .insert(ArrayKey::String(name), variable.borrow().dereferenced());
                } else if name != b"this" {
                    self.diagnostics.push(format!(
                        "Warning: compact(): Undefined variable ${} on line {line}",
                        String::from_utf8_lossy(&name)
                    ));
                }
            }
            Value::Array(array) => {
                let identity = Rc::as_ptr(&array) as usize;
                if !state.active.insert(identity) {
                    let error = self.materialize_error(Error::new(
                        ErrorKind::Error,
                        "Recursion detected",
                        line,
                    ));
                    // PHP continues the name scan after recursion errors. Each
                    // subsequent error chains the previously pending exception.
                    if let Some(previous) = state.error.take()
                        && let Some(current) = &error.throwable
                    {
                        current.throwable.borrow_mut().as_mut().unwrap().previous =
                            previous.throwable;
                    }
                    state.error = Some(error);
                    return;
                }
                for (_, value) in array.entries() {
                    self.compact_names(value, position, line, state);
                }
                state.active.remove(&identity);
            }
            value => {
                let kind = match value {
                    Value::Bool(true) => "true",
                    Value::Bool(false) => "false",
                    _ => builtins::debug_type(&value),
                };
                self.diagnostics.push(format!("Warning: compact(): Argument #{position} must be string or array of strings, {kind} given on line {line}"));
            }
        }
    }
}

struct CompactState {
    result: PhpArray,
    active: HashSet<usize>,
    error: Option<Error>,
}

fn forbid_dynamic(name: &str, dynamic: bool, line: usize) -> Result<(), Error> {
    if dynamic {
        Err(Error::new(
            ErrorKind::Error,
            format!("Cannot call {name}() dynamically"),
            line,
        ))
    } else {
        Ok(())
    }
}
