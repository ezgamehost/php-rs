use super::*;
use builtins::{Builtin, require_array};

impl Runtime {
    pub(super) fn array_segments(
        &mut self,
        builtin: Builtin,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        let result = match builtin {
            Builtin::ArrayFill => {
                let start = self.native_int(first, name, 1, line)?;
                let count = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                if count < 0 {
                    return Err(value_error(
                        name,
                        2,
                        "count",
                        "must be greater than or equal to 0",
                        line,
                    ));
                }
                if count > i32::MAX as i64 {
                    return Err(value_error(name, 2, "count", "is too large", line));
                }
                if count > 0 && start.checked_add(count - 1).is_none() {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Cannot add element to the array as the next element is already occupied",
                        line,
                    ));
                }
                let mut array = PhpArray::with_capacity(count as usize, line)?;
                for offset in 0..count {
                    array.insert(
                        ArrayKey::Int(start + offset),
                        args[2].as_ref().unwrap().clone(),
                    );
                }
                array
            }
            Builtin::ArrayFillKeys => {
                let keys = require_array(first, name, 1, line)?;
                let value = args[1].as_ref().unwrap();
                let mut array = PhpArray::with_capacity(keys.len(), line)?;
                for (_, key) in keys.entries() {
                    let key = key.dereferenced();
                    let key = match key {
                        Value::Int(key) => ArrayKey::Int(key),
                        key => {
                            let text = self.string_bytes(&key, line)?;
                            ArrayKey::from_value(
                                &Value::String(text.into()),
                                line,
                                &mut self.diagnostics,
                            )?
                        }
                    };
                    array.insert(key, value.clone());
                }
                array
            }
            Builtin::ArraySlice => {
                let input = require_array(first, name, 1, line)?;
                let offset = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                let length = match args[2].as_ref() {
                    None | Some(Value::Null) => None,
                    Some(value) => Some(self.native_int_type(value, name, 3, line, "?int")?),
                };
                let preserve = args[3]
                    .as_ref()
                    .map(|value| self.native_bool(value, name, 4, line))
                    .transpose()?
                    .unwrap_or(false);
                let (start, end) = substrings::substring_range(input.len(), offset, length);
                let mut array = PhpArray::with_capacity(end - start, line)?;
                for (key, value) in &input.entries()[start..end] {
                    copy_entry(&mut array, key, value.copy_array_entry(), preserve, line)?;
                }
                array
            }
            Builtin::ArrayReverse => {
                let input = require_array(first, name, 1, line)?;
                let preserve = args[1]
                    .as_ref()
                    .map(|value| self.native_bool(value, name, 2, line))
                    .transpose()?
                    .unwrap_or(false);
                let mut array = PhpArray::with_capacity(input.len(), line)?;
                for (key, value) in input.entries().iter().rev() {
                    copy_entry(&mut array, key, value.copy_array_entry(), preserve, line)?;
                }
                array
            }
            Builtin::ArrayChunk => {
                let input = require_array(first, name, 1, line)?;
                let length = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                let preserve = args[2]
                    .as_ref()
                    .map(|value| self.native_bool(value, name, 3, line))
                    .transpose()?
                    .unwrap_or(false);
                if length <= 0 {
                    return Err(value_error(
                        name,
                        2,
                        "length",
                        "must be greater than 0",
                        line,
                    ));
                }
                let mut array =
                    PhpArray::with_capacity(input.len().div_ceil(length as usize), line)?;
                for entries in input.entries().chunks(length as usize) {
                    let mut chunk = PhpArray::with_capacity(entries.len(), line)?;
                    for (key, value) in entries {
                        if preserve {
                            chunk.insert(key.clone(), value.copy_array_entry());
                        } else {
                            chunk.append(value.copy_array_entry(), line)?;
                        }
                    }
                    array.append(Value::Array(Rc::new(chunk)), line)?;
                }
                array
            }
            Builtin::ArrayPad => {
                let input = require_array(first, name, 1, line)?;
                let length = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                let size = length.unsigned_abs();
                if size > 0x40000000 {
                    return Err(value_error(
                        name,
                        2,
                        "length",
                        "must not exceed the maximum allowed array size",
                        line,
                    ));
                }
                if size as usize <= input.len() {
                    return Ok(first.clone());
                }
                let size = size as usize;
                let pads = size - input.len();
                let value = args[2].as_ref().unwrap();
                let mut array = PhpArray::with_capacity(size, line)?;
                if length < 0 {
                    for _ in 0..pads {
                        array.append(value.clone(), line)?;
                    }
                }
                for (key, value) in input.entries() {
                    copy_entry(&mut array, key, shared_entry(value), false, line)?;
                }
                if length > 0 {
                    for _ in 0..pads {
                        array.append(value.clone(), line)?;
                    }
                }
                array
            }
            _ => unreachable!(),
        };
        Ok(Value::Array(Rc::new(result)))
    }
}

fn copy_entry(
    array: &mut PhpArray,
    key: &ArrayKey,
    value: Value,
    preserve: bool,
    line: usize,
) -> Result<(), Error> {
    if preserve || matches!(key, ArrayKey::String(_)) {
        array.insert(key.clone(), value);
        Ok(())
    } else {
        array.append(value, line)
    }
}

fn shared_entry(value: &Value) -> Value {
    match value {
        Value::Indirect(slot) => shared_entry(&slot.borrow()),
        value => value.clone(),
    }
}

fn value_error(name: &str, position: usize, parameter: &str, message: &str, line: usize) -> Error {
    Error::new(
        ErrorKind::ValueError,
        format!("{name}(): Argument #{position} (${parameter}) {message}"),
        line,
    )
}
