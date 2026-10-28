use super::{builtins::Builtin, *};

impl Runtime {
    pub(super) fn string_builtin(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        match builtin {
            Builtin::Bin2Hex => {
                let bytes = self.native_string(first, "bin2hex", 1, line)?;
                let length = bytes
                    .len()
                    .checked_mul(2)
                    .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
                let mut result = Vec::new();
                result.try_reserve_exact(length).map_err(|_| {
                    Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
                })?;
                let digits = b"0123456789abcdef";
                for byte in bytes {
                    result.push(digits[(byte >> 4) as usize]);
                    result.push(digits[(byte & 15) as usize]);
                }
                Ok(Value::String(result.into()))
            }
            Builtin::StrLower | Builtin::StrUpper => {
                let upper = matches!(builtin, Builtin::StrUpper);
                let name = if upper { "strtoupper" } else { "strtolower" };
                let mut bytes = self.native_string(first, name, 1, line)?;
                if upper {
                    bytes.make_ascii_uppercase();
                } else {
                    bytes.make_ascii_lowercase();
                }
                Ok(Value::String(bytes.into()))
            }
            Builtin::Trim(mode, name) => {
                let bytes = self.native_string(first, name, 1, line)?;
                let characters = match &args[1] {
                    Some(value) => self.native_string(value, name, 2, line)?,
                    None => b" \t\n\r\x0b\0".to_vec(),
                };
                let mask = character_mask(&characters, name, line, &mut self.diagnostics);
                let mut start = 0;
                let mut end = bytes.len();
                if mode & 1 != 0 {
                    while start < end && mask[bytes[start] as usize] {
                        start += 1;
                    }
                }
                if mode & 2 != 0 {
                    while start < end && mask[bytes[end - 1] as usize] {
                        end -= 1;
                    }
                }
                Ok(Value::String(bytes[start..end].to_vec().into()))
            }
            Builtin::Chr => {
                let code = self.native_int(first, "chr", 1, line)?;
                if !(0..=255).contains(&code) {
                    self.diagnostics.push(format!("Deprecated: chr(): Providing a value not in-between 0 and 255 is deprecated, this is because a byte value must be in the [0, 255] interval. The value used will be constrained using % 256 on line {line}"));
                }
                Ok(Value::String(vec![code as u8].into()))
            }
            Builtin::Ord => {
                let bytes = self.native_string(first, "ord", 1, line)?;
                if bytes.len() != 1 {
                    let message = if bytes.is_empty() {
                        "Providing an empty string is deprecated"
                    } else {
                        "Providing a string that is not one byte long is deprecated. Use ord($str[0]) instead"
                    };
                    self.diagnostics
                        .push(format!("Deprecated: ord(): {message} on line {line}"));
                }
                Ok(Value::Int(bytes.first().copied().unwrap_or(0) as i64))
            }
            Builtin::StrRev => {
                let mut bytes = self.native_string(first, "strrev", 1, line)?;
                bytes.reverse();
                Ok(Value::String(bytes.into()))
            }
            Builtin::StrRepeat => {
                let bytes = self.native_string(first, "str_repeat", 1, line)?;
                let times = self.native_int(args[1].as_ref().unwrap(), "str_repeat", 2, line)?;
                if times < 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "str_repeat(): Argument #2 ($times) must be greater than or equal to 0",
                        line,
                    ));
                }
                if bytes.is_empty() || times == 0 {
                    return Ok(Value::String(Vec::new().into()));
                }
                let length = bytes
                    .len()
                    .checked_mul(times as usize)
                    .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
                let mut result = Vec::new();
                result.try_reserve_exact(length).map_err(|_| {
                    Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
                })?;
                result.extend_from_slice(&bytes);
                while result.len() < length {
                    let count = result.len().min(length - result.len());
                    result.extend_from_within(..count);
                }
                Ok(Value::String(result.into()))
            }
            _ => unreachable!(),
        }
    }
}

fn character_mask(
    bytes: &[u8],
    function: &str,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> [bool; 256] {
    let mut mask = [false; 256];
    let mut index = 0;
    while index < bytes.len() {
        if index + 3 < bytes.len()
            && bytes[index + 1..index + 3] == *b".."
            && bytes[index] <= bytes[index + 3]
        {
            mask[bytes[index] as usize..=bytes[index + 3] as usize].fill(true);
            index += 4;
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"..") {
            let detail = if index == 0 {
                ", no character to the left of '..'"
            } else if index + 2 >= bytes.len() {
                ", no character to the right of '..'"
            } else if bytes[index - 1] > bytes[index + 2] {
                ", '..'-range needs to be incrementing"
            } else {
                ""
            };
            warnings.push(format!(
                "{function}(): Invalid '..'-range{detail} on line {line}"
            ));
        } else {
            mask[bytes[index] as usize] = true;
        }
        index += 1;
    }
    mask
}
