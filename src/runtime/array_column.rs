use super::*;

impl Runtime {
    pub(super) fn array_column(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let input = builtins::require_array(args[0].as_ref().unwrap(), "array_column", 1, line)?;
        let column = self.column_selector(args[1].as_ref().unwrap(), 2, line)?;
        let index = self.column_selector(args[2].as_ref().unwrap_or(&Value::Null), 3, line)?;
        let mut output = PhpArray::with_capacity(input.len(), line)?;
        let mut pending: Option<Error> = None;
        for (_, row) in input.entries() {
            let row = row.dereferenced();
            let result = (|| {
                let value = if let Some(column) = &column {
                    let Some(value) = self.column_value(&row, column, pending.is_some(), line)?
                    else {
                        return Ok(());
                    };
                    value
                } else {
                    row.clone()
                };
                let key = index
                    .as_ref()
                    .map(|index| self.column_value(&row, index, pending.is_some(), line))
                    .transpose()?
                    .flatten();
                if let Some(key) = key {
                    if matches!(key, Value::Array(_) | Value::Object(_) | Value::Closure(_)) {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            format!(
                                "Cannot access offset of type {} on array",
                                builtins::debug_type(&key)
                            ),
                            line,
                        ));
                    }
                    let key = ArrayKey::from_value(&key, line, &mut self.diagnostics)?;
                    output.insert(key, value);
                } else {
                    // The native function silently drops an unindexed row if a
                    // preceding explicit key has exhausted the append index.
                    let _ = output.append(value, line);
                }
                Ok(())
            })();
            if let Err(error) = result {
                if error.kind == ErrorKind::UnwindExit {
                    return Err(error);
                }
                let error = self.materialize_error(error);
                if let Some(previous) = pending.take()
                    && let (Some(current), Some(previous)) = (&error.throwable, previous.throwable)
                {
                    throwables::append_previous(current, &previous);
                }
                pending = Some(error);
            }
        }
        pending.map_or_else(|| Ok(Value::Array(Rc::new(output))), Err)
    }

    fn column_selector(
        &mut self,
        value: &Value,
        position: usize,
        line: usize,
    ) -> Result<Option<Value>, Error> {
        const TYPE: &str = "string|int|null";
        match value {
            Value::Null => Ok(None),
            Value::String(_) | Value::Int(_) => Ok(Some(value.clone())),
            Value::Bool(_) | Value::Float(_) if !self.strict_types => {
                // PHP first attempts integer coercion, then string coercion for
                // floating-point values outside the integer range.
                match self.native_int_type(value, "array_column", position, line, TYPE) {
                    Ok(value) => Ok(Some(Value::Int(value))),
                    Err(_) => self
                        .native_string_type(value, "array_column", position, line, TYPE)
                        .map(|value| Some(Value::String(value.into()))),
                }
            }
            _ => self
                .native_string_type(value, "array_column", position, line, TYPE)
                .map(|value| Some(Value::String(value.into()))),
        }
    }

    fn column_value(
        &mut self,
        row: &Value,
        selector: &Value,
        pending: bool,
        line: usize,
    ) -> Result<Option<Value>, Error> {
        match row {
            Value::Array(array) => {
                let key = ArrayKey::from_value(selector, line, &mut self.diagnostics)?;
                Ok(array.get(&key).map(Value::dereferenced))
            }
            Value::Object(object) => {
                let name = selector.to_bytes();
                // Existence includes accessible initialized null properties.
                // The separate isset probe invokes magic only when necessary,
                // and skips never-initialized typed properties.
                let exists = name.first() != Some(&0)
                    && self.check_instance_property_access(object, &name, true, line)?
                    && object
                        .properties
                        .borrow()
                        .get(&self.instance_property_key(object, &name))
                        .is_some();
                let exists = exists
                    || (!pending
                        && self
                            .read_property_value(object, &name, line, Access::Probe)?
                            .is_truthy());
                if exists {
                    self.read_property_value(object, &name, line, Access::Read)
                        .map(|value| Some(value.dereferenced()))
                } else {
                    Ok(None)
                }
            }
            _ => Ok(None),
        }
    }
}
