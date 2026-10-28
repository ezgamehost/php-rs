use super::*;

impl Runtime {
    pub(super) fn substring(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(args[0].as_ref().unwrap(), "substr", 1, line)?;
        let offset = self.native_int(args[1].as_ref().unwrap(), "substr", 2, line)?;
        let length = args[2]
            .as_ref()
            .filter(|value| !matches!(value, Value::Null))
            .map(|value| self.native_int_type(value, "substr", 3, line, "?int"))
            .transpose()?;
        let (start, end) = substring_range(bytes.len(), offset, length);
        Ok(Value::String(bytes[start..end].to_vec().into()))
    }

    pub(super) fn substring_replace(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let subject =
            self.native_array_or_string(args[0].as_ref().unwrap(), "substr_replace", 1, line)?;
        let replacement =
            self.native_array_or_string(args[1].as_ref().unwrap(), "substr_replace", 2, line)?;
        let offset = self.substring_index(args[2].as_ref().unwrap(), 3, false, line)?;
        let length =
            self.substring_index(args[3].as_ref().unwrap_or(&Value::Null), 4, true, line)?;
        if matches!(subject, Value::String(_)) {
            for (value, position, name) in [(&offset, 3, "offset"), (&length, 4, "length")] {
                if matches!(value, Value::Array(_)) {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "substr_replace(): Argument #{position} (${name}) cannot be an array when working on a single string"
                        ),
                        line,
                    ));
                }
            }
        }
        let replace = |runtime: &mut Self, value: &Value, index| {
            let bytes = runtime.string_bytes(&value.dereferenced(), line)?;
            let offset = runtime
                .substring_array_index(&offset, index, Some(0), line)?
                .unwrap();
            let length = runtime.substring_array_index(&length, index, None, line)?;
            let (start, end) = substring_range(bytes.len(), offset, length);
            let replacement = match &replacement {
                Value::Array(array) => match array.entries().get(index) {
                    Some((_, value)) => runtime.string_bytes(&value.dereferenced(), line)?,
                    None => Vec::new(),
                },
                Value::String(bytes) => bytes.to_vec(),
                _ => unreachable!(),
            };
            let length = (bytes.len() - (end - start))
                .checked_add(replacement.len())
                .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
            let mut result = Vec::new();
            result.try_reserve_exact(length).map_err(|_| {
                Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
            })?;
            result.extend_from_slice(&bytes[..start]);
            result.extend_from_slice(&replacement);
            result.extend_from_slice(&bytes[end..]);
            Ok::<_, Error>(Value::String(result.into()))
        };
        if let Value::Array(array) = &subject {
            let mut result = PhpArray::default();
            for (index, (key, value)) in array.entries().iter().enumerate() {
                result.insert(key.clone(), replace(self, value, index)?);
            }
            Ok(Value::Array(Rc::new(result)))
        } else {
            replace(self, &subject, 0)
        }
    }

    fn substring_index(
        &mut self,
        value: &Value,
        position: usize,
        nullable: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(value, Value::Array(_)) || (nullable && matches!(value, Value::Null)) {
            return Ok(value.clone());
        }
        let kind = if nullable {
            "array|int|null"
        } else {
            "array|int"
        };
        self.native_int_type(value, "substr_replace", position, line, kind)
            .map(Value::Int)
    }

    fn substring_array_index(
        &mut self,
        value: &Value,
        index: usize,
        missing: Option<i64>,
        line: usize,
    ) -> Result<Option<i64>, Error> {
        match value {
            Value::Array(array) => match array.entries().get(index) {
                Some((_, value)) => {
                    // Elements use explicit integer conversion even with strict_types.
                    let Value::Int(value) = casting::cast(
                        crate::ast::CastKind::Int,
                        value.dereferenced(),
                        line,
                        &mut self.diagnostics,
                    )?
                    else {
                        unreachable!()
                    };
                    Ok(Some(value))
                }
                None => Ok(missing),
            },
            Value::Int(value) => Ok(Some(*value)),
            Value::Null => Ok(None),
            _ => unreachable!(),
        }
    }
}

pub(super) fn substring_range(size: usize, offset: i64, length: Option<i64>) -> (usize, usize) {
    let start = if offset < 0 {
        (size as u64).saturating_sub(offset.unsigned_abs()) as usize
    } else {
        (offset as u64).min(size as u64) as usize
    };
    let remaining = size - start;
    let length = match length {
        Some(length) if length < 0 => {
            (remaining as u64).saturating_sub(length.unsigned_abs()) as usize
        }
        Some(length) => (length as u64).min(remaining as u64) as usize,
        None => remaining,
    };
    (start, start + length)
}
