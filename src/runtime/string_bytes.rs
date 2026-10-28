use super::{builtins::Builtin, *};

impl Runtime {
    pub(super) fn byte_string_builtin(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = match builtin {
            Builtin::Hex2Bin => "hex2bin",
            Builtin::FirstCase(true) => "ucfirst",
            Builtin::FirstCase(false) => "lcfirst",
            Builtin::Ucwords => "ucwords",
            Builtin::Nl2Br => "nl2br",
            Builtin::CountChars => "count_chars",
            Builtin::StrPad => "str_pad",
            Builtin::StrPbrk => "strpbrk",
            _ => unreachable!(),
        };
        let mut bytes = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        match builtin {
            Builtin::Hex2Bin => {
                let (pairs, remainder) = bytes.as_chunks::<2>();
                if !remainder.is_empty() {
                    self.diagnostics.push(format!("hex2bin(): Hexadecimal input string must have an even length on line {line}"));
                    return Ok(Value::Bool(false));
                }
                let mut result = Vec::with_capacity(pairs.len());
                for pair in pairs {
                    let digits = (pair[0] as char)
                        .to_digit(16)
                        .zip((pair[1] as char).to_digit(16));
                    let Some((high, low)) = digits else {
                        self.diagnostics.push(format!(
                            "hex2bin(): Input string must be hexadecimal string on line {line}"
                        ));
                        return Ok(Value::Bool(false));
                    };
                    result.push((high * 16 + low) as u8);
                }
                Ok(Value::String(result.into()))
            }
            Builtin::FirstCase(upper) => {
                if let Some(first) = bytes.first_mut() {
                    *first = if upper {
                        first.to_ascii_uppercase()
                    } else {
                        first.to_ascii_lowercase()
                    };
                }
                Ok(Value::String(bytes.into()))
            }
            Builtin::Ucwords => {
                let separators = args[1]
                    .as_ref()
                    .map(|value| self.native_string(value, name, 2, line))
                    .transpose()?
                    .unwrap_or_else(|| b" \t\r\n\x0c\x0b".to_vec());
                if !bytes.is_empty() {
                    let mask = string_builtins::character_mask(
                        &separators,
                        name,
                        line,
                        &mut self.diagnostics,
                    );
                    bytes[0] = bytes[0].to_ascii_uppercase();
                    for index in 1..bytes.len() {
                        // PHP tests the preceding byte after any earlier capitalization.
                        if mask[bytes[index - 1] as usize] {
                            bytes[index] = bytes[index].to_ascii_uppercase();
                        }
                    }
                }
                Ok(Value::String(bytes.into()))
            }
            Builtin::Nl2Br => {
                let xhtml = args[1]
                    .as_ref()
                    .map(|value| self.native_bool(value, name, 2, line))
                    .transpose()?
                    .unwrap_or(true);
                let tag: &[u8] = if xhtml { b"<br />" } else { b"<br>" };
                let mut count = 0usize;
                let mut index = 0;
                while index < bytes.len() {
                    let width = newline_width(&bytes, index);
                    if width != 0 {
                        count += 1;
                    }
                    index += width.max(1);
                }
                let size = count
                    .checked_mul(tag.len())
                    .and_then(|extra| bytes.len().checked_add(extra))
                    .ok_or_else(|| overflow(line))?;
                let mut result = allocate(size, line)?;
                index = 0;
                while index < bytes.len() {
                    let width = newline_width(&bytes, index);
                    if width != 0 {
                        result.extend_from_slice(tag);
                    }
                    let width = width.max(1);
                    result.extend_from_slice(&bytes[index..index + width]);
                    index += width;
                }
                Ok(Value::String(result.into()))
            }
            Builtin::CountChars => {
                let mode = args[1]
                    .as_ref()
                    .map(|value| self.native_int(value, name, 2, line))
                    .transpose()?
                    .unwrap_or(0);
                if !(0..=4).contains(&mode) {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "count_chars(): Argument #2 ($mode) must be between 0 and 4 (inclusive)",
                        line,
                    ));
                }
                let mut counts = [0i64; 256];
                for byte in bytes {
                    counts[byte as usize] += 1;
                }
                let mut array = PhpArray::default();
                let mut string = Vec::new();
                for (byte, count) in counts.into_iter().enumerate() {
                    let selected = mode == 0
                        || if mode == 1 || mode == 3 {
                            count != 0
                        } else {
                            count == 0
                        };
                    if selected {
                        if mode < 3 {
                            array.insert(ArrayKey::Int(byte as i64), Value::Int(count));
                        } else {
                            string.push(byte as u8);
                        }
                    }
                }
                Ok(if mode < 3 {
                    Value::Array(Rc::new(array))
                } else {
                    Value::String(string.into())
                })
            }
            Builtin::StrPbrk => {
                let characters = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
                if characters.is_empty() {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "strpbrk(): Argument #2 ($characters) must be a non-empty string",
                        line,
                    ));
                }
                let mut mask = [false; 256];
                for byte in characters {
                    mask[byte as usize] = true;
                }
                Ok(bytes
                    .iter()
                    .position(|byte| mask[*byte as usize])
                    .map_or(Value::Bool(false), |index| {
                        Value::String(bytes[index..].to_vec().into())
                    }))
            }
            Builtin::StrPad => {
                let length = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                let padding = args[2]
                    .as_ref()
                    .map(|value| self.native_string(value, name, 3, line))
                    .transpose()?
                    .unwrap_or_else(|| b" ".to_vec());
                let side = args[3]
                    .as_ref()
                    .map(|value| self.native_int(value, name, 4, line))
                    .transpose()?
                    .unwrap_or(1);
                if length < 0 || length as u64 <= bytes.len() as u64 {
                    return Ok(Value::String(bytes.into()));
                }
                if padding.is_empty() {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "str_pad(): Argument #3 ($pad_string) must not be empty",
                        line,
                    ));
                }
                if !(0..=2).contains(&side) {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "str_pad(): Argument #4 ($pad_type) must be STR_PAD_LEFT, STR_PAD_RIGHT, or STR_PAD_BOTH",
                        line,
                    ));
                }
                let extra = length as usize - bytes.len();
                let left = match side {
                    0 => extra,
                    1 => 0,
                    _ => extra / 2,
                };
                let mut result = allocate(length as usize, line)?;
                result.extend(padding.iter().copied().cycle().take(left));
                result.extend_from_slice(&bytes);
                result.extend(padding.iter().copied().cycle().take(extra - left));
                Ok(Value::String(result.into()))
            }
            _ => unreachable!(),
        }
    }
}

fn newline_width(bytes: &[u8], index: usize) -> usize {
    let byte = bytes[index];
    if !matches!(byte, b'\r' | b'\n') {
        return 0;
    }
    if bytes
        .get(index + 1)
        .is_some_and(|next| matches!(*next, b'\r' | b'\n') && *next != byte)
    {
        2
    } else {
        1
    }
}

fn overflow(line: usize) -> Error {
    Error::new(ErrorKind::Error, "String size overflow", line)
}

fn allocate(size: usize, line: usize) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(size)
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
    Ok(bytes)
}
