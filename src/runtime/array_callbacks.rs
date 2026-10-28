use super::builtins::require_array;
use super::*;

impl Runtime {
    pub(super) fn array_map(
        &mut self,
        args: &[Option<Value>],
        extra: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        if extra
            .entries()
            .iter()
            .any(|(key, _)| matches!(key, ArrayKey::String(_)))
        {
            return Err(Error::new(
                ErrorKind::ArgumentCountError,
                "array_map() does not accept unknown named parameters",
                line,
            ));
        }
        let callback =
            self.native_callback(args[0].as_ref().unwrap(), "array_map", 1, true, line)?;
        let first = args[1].as_ref().unwrap();
        let mut arrays = vec![require_array(first, "array_map", 2, line)?];
        for (index, (_, value)) in extra.entries().iter().enumerate() {
            arrays.push(require_array(value, "array_map", index + 3, line)?);
        }
        if arrays.len() == 1 && (callback.is_none() || arrays[0].is_empty()) {
            return Ok(first.clone());
        }
        let count = arrays.iter().map(|array| array.len()).max().unwrap_or(0);
        let mut result = PhpArray::default();
        for index in 0..count {
            let values: Vec<_> = arrays
                .iter()
                .map(|array| {
                    array
                        .entries()
                        .get(index)
                        .map_or(Value::Null, |(_, value)| value.clone())
                })
                .collect();
            let value = if let Some(callback) = &callback {
                self.call_callback(callback, &values, line)?
            } else {
                let mut row = PhpArray::default();
                for value in values {
                    row.append(value, line)?;
                }
                Value::Array(Rc::new(row))
            };
            if arrays.len() == 1 {
                result.insert(arrays[0].entries()[index].0.clone(), value);
            } else {
                result.append(value, line)?;
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn array_filter(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let array = require_array(args[0].as_ref().unwrap(), "array_filter", 1, line)?;
        let callback = self.native_callback(
            args[1].as_ref().unwrap_or(&Value::Null),
            "array_filter",
            2,
            true,
            line,
        )?;
        let mode = match &args[2] {
            Some(mode) => self.native_int(mode, "array_filter", 3, line)?,
            None => 0,
        };
        let mut result = PhpArray::default();
        for (key, value) in array.entries() {
            let keep = if let Some(callback) = &callback {
                let values = match mode {
                    1 => vec![value.clone(), key.to_value()],
                    2 => vec![key.to_value()],
                    _ => vec![value.clone()],
                };
                self.call_callback(callback, &values, line)?
                    .to_bool(line, &mut self.diagnostics)
            } else {
                value.to_bool(line, &mut self.diagnostics)
            };
            if keep {
                result.insert(key.clone(), value.clone());
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn array_reduce(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let array = require_array(args[0].as_ref().unwrap(), "array_reduce", 1, line)?;
        let callback = self
            .native_callback(args[1].as_ref().unwrap(), "array_reduce", 2, false, line)?
            .unwrap();
        let mut carry = args[2].clone().unwrap_or(Value::Null);
        for (_, value) in array.entries() {
            carry = self.call_callback(&callback, &[carry, value.clone()], line)?;
        }
        Ok(carry)
    }
}
