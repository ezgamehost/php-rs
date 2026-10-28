use super::*;

impl Runtime {
    pub(super) fn translate_string(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let mut bytes = self.native_string(args[0].as_ref().unwrap(), "strtr", 1, line)?;
        if let Some(to) = &args[2] {
            let from = self.native_string(args[1].as_ref().unwrap(), "strtr", 2, line)?;
            let to = self.native_string(to, "strtr", 3, line)?;
            let mut table = std::array::from_fn::<_, 256, _>(|index| index as u8);
            for (from, to) in from.iter().zip(to) {
                table[*from as usize] = to;
            }
            for byte in &mut bytes {
                *byte = table[*byte as usize];
            }
            return Ok(Value::String(bytes.into()));
        }
        let from = builtins::require_array(args[1].as_ref().unwrap(), "strtr", 2, line)?;
        if bytes.is_empty() || from.is_empty() {
            return Ok(Value::String(bytes.into()));
        }
        // The single-entry PHP path converts its replacement once, even if it does
        // not match. The multi-entry path converts each replacement only on a match.
        let cached = if from.len() == 1 {
            Some(self.string_bytes(&from.entries()[0].1.dereferenced(), line)?)
        } else {
            None
        };
        let mut patterns = Vec::new();
        for (key, value) in from.entries() {
            let key = key.to_value().to_bytes();
            if key.is_empty() {
                self.diagnostics.push(format!(
                    "strtr(): Ignoring replacement of empty string on line {line}"
                ));
            } else if key.len() <= bytes.len() {
                patterns.push((key, value));
            }
        }
        patterns.sort_by_key(|(key, _)| std::cmp::Reverse(key.len()));
        let mut result = Vec::new();
        let mut cursor = 0;
        let mut copied = 0;
        while cursor < bytes.len() {
            if let Some((key, value)) = patterns
                .iter()
                .find(|(key, _)| bytes[cursor..].starts_with(key))
            {
                append(&mut result, &bytes[copied..cursor], line)?;
                if let Some(cached) = &cached {
                    append(&mut result, cached, line)?;
                } else {
                    let replacement = self.string_bytes(&value.dereferenced(), line)?;
                    append(&mut result, &replacement, line)?;
                }
                cursor += key.len();
                copied = cursor;
            } else {
                cursor += 1;
            }
        }
        append(&mut result, &bytes[copied..], line)?;
        Ok(Value::String(result.into()))
    }

    pub(super) fn escape_slashes(
        &mut self,
        add: bool,
        c_style: bool,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = match (add, c_style) {
            (true, false) => "addslashes",
            (false, false) => "stripslashes",
            (true, true) => "addcslashes",
            (false, true) => "stripcslashes",
        };
        let bytes = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let mask = if add && c_style {
            let characters = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
            if bytes.is_empty() || characters.is_empty() {
                return Ok(Value::String(bytes.into()));
            }
            string_builtins::character_mask(&characters, name, line, &mut self.diagnostics)
        } else {
            std::array::from_fn(|index| matches!(index as u8, b'\'' | b'"' | b'\\' | 0))
        };
        let maximum = bytes
            .len()
            .checked_mul(if add { if c_style { 4 } else { 2 } } else { 1 })
            .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(maximum)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
        if add {
            for byte in bytes {
                if !mask[byte as usize] {
                    result.push(byte);
                    continue;
                }
                result.push(b'\\');
                if !c_style {
                    result.push(if byte == 0 { b'0' } else { byte });
                } else if let Some(escape) = control_escape(byte) {
                    result.push(escape);
                } else if !(32..=126).contains(&byte) {
                    result.extend_from_slice(&[
                        b'0' + (byte >> 6),
                        b'0' + ((byte >> 3) & 7),
                        b'0' + (byte & 7),
                    ]);
                } else {
                    result.push(byte);
                }
            }
        } else {
            let mut cursor = 0;
            while cursor < bytes.len() {
                let byte = bytes[cursor];
                cursor += 1;
                if byte != b'\\' {
                    result.push(byte);
                    continue;
                }
                let Some(&next) = bytes.get(cursor) else {
                    if c_style {
                        result.push(b'\\');
                    }
                    break;
                };
                cursor += 1;
                if !c_style {
                    result.push(if next == b'0' { 0 } else { next });
                    continue;
                }
                let control = match next {
                    b'a' => Some(7),
                    b'b' => Some(8),
                    b'f' => Some(12),
                    b'n' => Some(10),
                    b'r' => Some(13),
                    b't' => Some(9),
                    b'v' => Some(11),
                    _ => None,
                };
                if let Some(byte) = control {
                    result.push(byte);
                } else if next == b'x' && bytes.get(cursor).is_some_and(u8::is_ascii_hexdigit) {
                    let mut value = 0;
                    for _ in 0..2 {
                        let Some(digit) = bytes
                            .get(cursor)
                            .and_then(|byte| (*byte as char).to_digit(16))
                        else {
                            break;
                        };
                        value = value * 16 + digit;
                        cursor += 1;
                    }
                    result.push(value as u8);
                } else if (b'0'..=b'7').contains(&next) {
                    let mut value = u16::from(next - b'0');
                    for _ in 0..2 {
                        let Some(&byte) = bytes
                            .get(cursor)
                            .filter(|byte| (b'0'..=b'7').contains(byte))
                        else {
                            break;
                        };
                        value = value * 8 + u16::from(byte - b'0');
                        cursor += 1;
                    }
                    result.push(value as u8);
                } else {
                    result.push(next);
                }
            }
        }
        Ok(Value::String(result.into()))
    }
}

fn control_escape(byte: u8) -> Option<u8> {
    match byte {
        7 => Some(b'a'),
        8 => Some(b'b'),
        9 => Some(b't'),
        10 => Some(b'n'),
        11 => Some(b'v'),
        12 => Some(b'f'),
        13 => Some(b'r'),
        _ => None,
    }
}

fn append(result: &mut Vec<u8>, bytes: &[u8], line: usize) -> Result<(), Error> {
    result
        .try_reserve(bytes.len())
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
    result.extend_from_slice(bytes);
    Ok(())
}
