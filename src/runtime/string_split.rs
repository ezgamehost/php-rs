use super::{builtins::Builtin, *};

impl Runtime {
    pub(super) fn explode_string(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let separator = self.native_string(args[0].as_ref().unwrap(), "explode", 1, line)?;
        let bytes = self.native_string(args[1].as_ref().unwrap(), "explode", 2, line)?;
        let limit = args[2]
            .as_ref()
            .map(|value| self.native_int(value, "explode", 3, line))
            .transpose()?
            .unwrap_or(i64::MAX);
        if separator.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "explode(): Argument #1 ($separator) must not be empty",
                line,
            ));
        }
        let mut pieces = Vec::new();
        let mut cursor = 0;
        let maximum = if limit < 0 {
            u64::MAX
        } else {
            limit.max(1) as u64
        };
        while (pieces.len() as u64) < maximum - 1 {
            let Some(offset) = bytes[cursor..]
                .windows(separator.len())
                .position(|window| window == separator)
            else {
                break;
            };
            pieces.push(&bytes[cursor..cursor + offset]);
            cursor += offset + separator.len();
        }
        pieces.push(&bytes[cursor..]);
        if limit < 0 {
            pieces.truncate((pieces.len() as u64).saturating_sub(limit.unsigned_abs()) as usize);
        }
        let mut result = PhpArray::default();
        for piece in pieces {
            result.append(Value::String(piece.to_vec().into()), line)?;
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn implode_strings(
        &mut self,
        name: &str,
        direct_positional: bool,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        let missing_array = || {
            Error::new(
                ErrorKind::TypeError,
                format!(
                    "{name}(): If argument #1 ($separator) is of type string, argument #2 ($array) must be of type array, null given"
                ),
                line,
            )
        };
        // Only implode's direct positional call has PHP's specialized argument parser;
        // the join alias, named/unpacked calls, and callbacks use the general handler.
        let (separator, pieces) = if name == "implode" && direct_positional && args[1].is_none() {
            let Value::Array(array) = first else {
                return Err(missing_array());
            };
            (Vec::new(), array.clone())
        } else {
            let separator = if name == "implode" && direct_positional {
                Value::String(self.native_string(first, name, 1, line)?.into())
            } else {
                self.native_array_or_string(first, name, 1, line)?
            };
            let pieces = match &args[1] {
                None | Some(Value::Null) => None,
                Some(Value::Array(array)) => Some(array.clone()),
                Some(value) => return Err(builtins::type_error(value, name, 2, "?array", line)),
            };
            match (separator, pieces) {
                (Value::Array(array), None) => (Vec::new(), array),
                (Value::String(separator), Some(pieces)) => (separator.to_vec(), pieces),
                (Value::String(_), None) => return Err(missing_array()),
                (Value::Array(_), Some(_)) => {
                    return Err(builtins::type_error(first, name, 1, "string", line));
                }
                _ => unreachable!(),
            }
        };
        let mut strings = Vec::new();
        let mut size = separator
            .len()
            .checked_mul(pieces.len().saturating_sub(1))
            .ok_or_else(|| overflow(line))?;
        for (_, value) in pieces.entries() {
            let bytes = self.string_bytes(&value.dereferenced(), line)?;
            size = size
                .checked_add(bytes.len())
                .ok_or_else(|| overflow(line))?;
            strings.push(bytes);
        }
        let mut result = allocate(size, line)?;
        for (index, bytes) in strings.into_iter().enumerate() {
            if index != 0 {
                result.extend_from_slice(&separator);
            }
            result.extend_from_slice(&bytes);
        }
        Ok(Value::String(result.into()))
    }

    pub(super) fn split_chunks(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let chunk_split = matches!(builtin, Builtin::ChunkSplit);
        let name = if chunk_split {
            "chunk_split"
        } else {
            "str_split"
        };
        let bytes = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let length = args[1]
            .as_ref()
            .map(|value| self.native_int(value, name, 2, line))
            .transpose()?
            .unwrap_or(if chunk_split { 76 } else { 1 });
        let separator = if chunk_split {
            args[2]
                .as_ref()
                .map(|value| self.native_string(value, name, 3, line))
                .transpose()?
                .unwrap_or_else(|| b"\r\n".to_vec())
        } else {
            Vec::new()
        };
        if length <= 0 {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #2 ($length) must be greater than 0"),
                line,
            ));
        }
        let length = length as usize;
        if chunk_split {
            let count = bytes.len().div_ceil(length).max(1);
            let size = separator
                .len()
                .checked_mul(count)
                .and_then(|extra| bytes.len().checked_add(extra))
                .ok_or_else(|| overflow(line))?;
            let mut result = allocate(size, line)?;
            if bytes.is_empty() {
                result.extend_from_slice(&separator);
            } else {
                for chunk in bytes.chunks(length) {
                    result.extend_from_slice(chunk);
                    result.extend_from_slice(&separator);
                }
            }
            Ok(Value::String(result.into()))
        } else {
            let mut result = PhpArray::default();
            for chunk in bytes.chunks(length) {
                result.append(Value::String(chunk.to_vec().into()), line)?;
            }
            Ok(Value::Array(Rc::new(result)))
        }
    }
}

fn overflow(line: usize) -> Error {
    Error::new(ErrorKind::Error, "String size overflow", line)
}

fn allocate(size: usize, line: usize) -> Result<Vec<u8>, Error> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(size)
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
    Ok(result)
}
