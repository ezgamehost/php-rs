// Altered Rust adaptation of PHP 8.5.0 ext/standard/pack.c.
// Copyright (c) The PHP Group. Original author Chris Schneider.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::ast::CastKind;

impl Runtime {
    pub(super) fn pack(
        &mut self,
        args: &[Option<Value>],
        extra: PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let format = self.native_string(args[0].as_ref().unwrap(), "pack", 1, line)?;
        if extra
            .entries()
            .iter()
            .any(|(key, _)| matches!(key, ArrayKey::String(_)))
        {
            return Err(Error::new(
                ErrorKind::ArgumentCountError,
                "pack() does not accept unknown named parameters",
                line,
            ));
        }
        let mut values: Vec<_> = extra
            .entries()
            .iter()
            .map(|(_, value)| value.dereferenced())
            .collect();
        drop(extra);
        let previous = self.native_warning_suppressed;
        let mut pending = None;
        let result = self.pack_values(&format, &mut values, &mut pending, line);
        self.native_warning_suppressed = previous;
        self.finish_packing(result, pending)
    }

    fn packing_error(&mut self, error: Error, pending: &mut Option<Error>) -> Result<(), Error> {
        if error.kind == ErrorKind::UnwindExit {
            return Err(error);
        }
        self.set_error(error, pending);
        self.native_warning_suppressed = true;
        Ok(())
    }

    fn packing_warning(
        &mut self,
        name: &str,
        message: String,
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<(), Error> {
        if let Err(error) = self.warn(format!("{name}(): {message} on line {line}")) {
            self.packing_error(error, pending)?;
        }
        Ok(())
    }

    fn pack_values(
        &mut self,
        format: &[u8],
        values: &mut [Value],
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<Value, Error> {
        let mut formats = Vec::new();
        let mut cursor = 0;
        let mut argument = 0;
        while cursor < format.len() {
            let code = format[cursor];
            cursor += 1;
            // atoi narrows the host long to C int; negative results act as '*'.
            let mut count = repeat(format, &mut cursor) as i32 as i64;
            match code {
                b'x' | b'X' | b'@' => {
                    if count < 0 {
                        self.packing_warning(
                            "pack",
                            format!("Type {}: '*' ignored", code as char),
                            pending,
                            line,
                        )?;
                        count = 1;
                    }
                }
                b'a' | b'A' | b'Z' | b'h' | b'H' => {
                    let Some(value) = values.get_mut(argument) else {
                        return Err(format_error(
                            format!("Type {}: not enough arguments", code as char),
                            line,
                        ));
                    };
                    if count < 0 {
                        if pending.is_some()
                            && matches!(value, Value::Object(_) | Value::Closure(_))
                        {
                            return Err(pending.take().unwrap());
                        }
                        let bytes = self.string_bytes(value, line)?;
                        count = bytes.len() as i32 as i64;
                        if code == b'Z' {
                            count = (count as i32).wrapping_add(1) as i64;
                        }
                        *value = Value::String(bytes.into());
                        if let Some(frame) = self.trace_frames.last_mut() {
                            frame.replace_native_argument(argument + 1, value.clone());
                        }
                        // The converted argument stays a string if releasing
                        // its previous object throws. PHP continues parsing.
                        if let Err(error) = self.drain_destructors(Ok(()), line) {
                            self.packing_error(error, pending)?;
                        }
                    }
                    argument += 1;
                }
                _ if width(code).is_some() => {
                    if count < 0 {
                        count = (values.len() - argument) as i64;
                    }
                    if count > i32::MAX as i64 - argument as i64
                        || count as usize > values.len() - argument
                    {
                        return Err(format_error(
                            format!("Type {}: too few arguments", code as char),
                            line,
                        ));
                    }
                    argument += count as usize;
                }
                _ => {
                    return Err(format_error(
                        format!("Type {}: unknown format code", code as char),
                        line,
                    ));
                }
            }
            formats.push((code, count));
        }
        if argument < values.len() {
            self.packing_warning(
                "pack",
                format!("{} arguments unused", values.len() - argument),
                pending,
                line,
            )?;
        }
        let mut position = 0_i64;
        let mut maximum = 0;
        for &(code, count) in &formats {
            match code {
                b'X' => {
                    position -= count;
                    if position < 0 {
                        self.packing_warning(
                            "pack",
                            "Type X: outside of string".into(),
                            pending,
                            line,
                        )?;
                        position = 0;
                    }
                }
                b'@' => position = count,
                _ => {
                    let amount = if matches!(code, b'h' | b'H') {
                        count / 2 + count % 2
                    } else {
                        count * width(code).unwrap_or(1) as i64
                    };
                    if amount < 0 || amount > i32::MAX as i64 - position {
                        return Err(format_error(
                            format!("Type {}: integer overflow in format string", code as char),
                            line,
                        ));
                    }
                    position += amount;
                }
            }
            maximum = maximum.max(position);
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(maximum as usize)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate packed string", line))?;
        output.resize(maximum as usize, 0);
        let mut position = 0_usize;
        let mut argument = 0;
        for (code, count) in formats {
            let count = count as usize;
            match code {
                b'a' | b'A' | b'Z' | b'h' | b'H' => {
                    let bytes = if pending.is_some()
                        && matches!(values[argument], Value::Object(_) | Value::Closure(_))
                    {
                        Vec::new()
                    } else {
                        match self.string_bytes(&values[argument], line) {
                            Ok(bytes) => bytes,
                            Err(error) => {
                                self.packing_error(error, pending)?;
                                Vec::new()
                            }
                        }
                    };
                    argument += 1;
                    if matches!(code, b'h' | b'H') {
                        let count = if count > bytes.len() {
                            self.packing_warning(
                                "pack",
                                format!("Type {}: not enough characters in string", code as char),
                                pending,
                                line,
                            )?;
                            bytes.len()
                        } else {
                            count
                        };
                        for (index, &digit) in bytes[..count].iter().enumerate() {
                            let nibble = match digit {
                                b'0'..=b'9' => digit - b'0',
                                b'a'..=b'f' => digit - b'a' + 10,
                                b'A'..=b'F' => digit - b'A' + 10,
                                _ => {
                                    self.packing_warning(
                                        "pack",
                                        format!(
                                            "Type {}: illegal hex digit {}",
                                            code as char, digit as char
                                        ),
                                        pending,
                                        line,
                                    )?;
                                    0
                                }
                            };
                            if index % 2 == 0 {
                                output[position + index / 2] = 0;
                            }
                            let shift = if (index % 2 == 0) == (code == b'H') {
                                4
                            } else {
                                0
                            };
                            output[position + index / 2] |= nibble << shift;
                        }
                        position += count.div_ceil(2);
                    } else {
                        output[position..position + count].fill(if code == b'A' {
                            b' '
                        } else {
                            0
                        });
                        let copied = bytes.len().min(if code == b'Z' {
                            count.saturating_sub(1)
                        } else {
                            count
                        });
                        output[position..position + copied].copy_from_slice(&bytes[..copied]);
                        position += count;
                    }
                }
                b'x' => {
                    output[position..position + count].fill(0);
                    position += count;
                }
                b'X' => position = position.saturating_sub(count),
                b'@' => {
                    if count > position {
                        output[position..count].fill(0);
                    }
                    position = count;
                }
                _ => {
                    let size = width(code).unwrap();
                    for value in &values[argument..argument + count] {
                        let float = matches!(code, b'f' | b'g' | b'G' | b'd' | b'e' | b'E');
                        let kind = if float {
                            CastKind::Float
                        } else {
                            CastKind::Int
                        };
                        let converted = match casting::cast(kind, value.clone(), line, self) {
                            Ok(value) => value,
                            Err(error) => {
                                self.packing_error(error, pending)?;
                                casting::cast(kind, value.clone(), line, self)?
                            }
                        };
                        let bits = match converted {
                            Value::Int(number) => number as u64,
                            Value::Float(number) if size == 4 => (number as f32).to_bits() as u64,
                            Value::Float(number) => number.to_bits(),
                            _ => unreachable!(),
                        };
                        let bytes = if little_endian(code) {
                            bits.to_le_bytes()
                        } else {
                            bits.to_be_bytes()
                        };
                        let bytes = if little_endian(code) {
                            &bytes[..size]
                        } else {
                            &bytes[8 - size..]
                        };
                        output[position..position + size].copy_from_slice(bytes);
                        position += size;
                    }
                    argument += count;
                }
            }
        }
        output.truncate(position);
        Ok(Value::String(output.into()))
    }

    pub(super) fn unpack(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        let format = self.native_string(args[0].as_ref().unwrap(), "unpack", 1, line)?;
        let input = self.native_string(args[1].as_ref().unwrap(), "unpack", 2, line)?;
        let offset = args[2]
            .as_ref()
            .map(|value| self.native_int(value, "unpack", 3, line))
            .transpose()?
            .unwrap_or(0);
        if offset < 0 || offset as usize > input.len() {
            return Err(format_error(
                "unpack(): Argument #3 ($offset) must be contained in argument #2 ($data)".into(),
                line,
            ));
        }
        let previous = self.native_warning_suppressed;
        let mut pending = None;
        let result = self.unpack_bytes(&format, &input[offset as usize..], &mut pending, line);
        self.native_warning_suppressed = previous;
        self.finish_packing(result, pending)
    }

    fn unpack_bytes(
        &mut self,
        format: &[u8],
        input: &[u8],
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<Value, Error> {
        let mut result = PhpArray::default();
        let mut cursor = 0;
        let mut position = 0_i64;
        while cursor < format.len() {
            let code = format[cursor];
            cursor += 1;
            let original = repeat(format, &mut cursor);
            if original > i32::MAX as i64 {
                self.packing_warning(
                    "unpack",
                    format!("Type {}: integer overflow", code as char),
                    pending,
                    line,
                )?;
                return Ok(Value::Bool(false));
            }
            let start = cursor;
            while cursor < format.len() && format[cursor] != b'/' {
                cursor += 1;
            }
            let name = &format[start..cursor.min(start + 200)];
            if cursor < format.len() {
                cursor += 1;
            }
            let mut count = original;
            let mut size = match code {
                b'X' => {
                    if count < 0 {
                        self.packing_warning(
                            "unpack",
                            "Type X: '*' ignored".into(),
                            pending,
                            line,
                        )?;
                        count = 1;
                    }
                    -1
                }
                b'@' => 0,
                b'a' | b'A' | b'Z' => {
                    count = 1;
                    original
                }
                b'h' | b'H' => {
                    count = 1;
                    if original > 0 {
                        (original + 1) / 2
                    } else {
                        original
                    }
                }
                b'x' => 1,
                _ => width(code).ok_or_else(|| {
                    format_error(format!("Invalid format type {}", code as char), line)
                })? as i64,
            };
            let mut index = 0_i64;
            while index != count {
                if size != 0 && size != -1 && i32::MAX as i64 - size + 1 < position {
                    self.packing_warning(
                        "unpack",
                        format!("Type {}: integer overflow", code as char),
                        pending,
                        line,
                    )?;
                    return Ok(Value::Bool(false));
                }
                if position + size > input.len() as i64 {
                    if count < 0 {
                        break;
                    }
                    let remaining = input.len() as i64 - position;
                    self.packing_warning("unpack", format!("Type {}: not enough input values, need {size} values but only {remaining} {} provided", code as char, if remaining == 1 { "was" } else { "were" }), pending, line)?;
                    return Ok(Value::Bool(false));
                }
                let offset = position as usize;
                let value = match code {
                    b'x' | b'X' => None,
                    b'@' => {
                        if count <= input.len() as i64 {
                            position = count;
                        } else {
                            self.packing_warning(
                                "unpack",
                                "Type @: outside of string".into(),
                                pending,
                                line,
                            )?;
                        }
                        index = count - 1;
                        None
                    }
                    b'a' | b'A' | b'Z' => {
                        size = if size < 0 {
                            (input.len() - offset) as i64
                        } else {
                            size
                        };
                        let mut bytes = &input[offset..offset + size as usize];
                        if code == b'A' {
                            while bytes.last().is_some_and(|b| b"\0 \t\r\n".contains(b)) {
                                bytes = &bytes[..bytes.len() - 1];
                            }
                        } else if code == b'Z' {
                            bytes = bytes.split(|&b| b == 0).next().unwrap();
                        }
                        Some(Value::String(bytes.to_vec().into()))
                    }
                    b'h' | b'H' => {
                        if size > i32::MAX as i64 / 2 {
                            return Err(format_error(
                                format!(
                                    "unpack(): Argument #1 ($format) repeater must be less than or equal to {}",
                                    i32::MAX / 2
                                ),
                                line,
                            ));
                        }
                        let available = (input.len() - offset) * 2;
                        let mut length = if size < 0 {
                            available
                        } else {
                            available.min(size as usize * 2)
                        };
                        if length > 0 && original > 0 {
                            length -= original as usize % 2;
                        }
                        let mut bytes = Vec::with_capacity(length);
                        for n in 0..length {
                            let shift = if (n % 2 == 0) == (code == b'H') { 4 } else { 0 };
                            bytes.push(
                                b"0123456789abcdef"
                                    [((input[offset + n / 2] >> shift) & 15) as usize],
                            );
                        }
                        Some(Value::String(bytes.into()))
                    }
                    _ => Some(unpack_number(code, &input[offset..offset + size as usize])),
                };
                if let Some(value) = value {
                    let key = if name.is_empty() {
                        ArrayKey::Int(index + 1)
                    } else {
                        let mut name = name.to_vec();
                        if count != 1 {
                            name.extend_from_slice((index + 1).to_string().as_bytes());
                        }
                        ArrayKey::from_value(&Value::String(name.into()), line, self)?
                    };
                    result.insert(key, value);
                }
                position += size;
                if position < 0 {
                    if size != -1 {
                        self.packing_warning(
                            "unpack",
                            format!("Type {}: outside of string", code as char),
                            pending,
                            line,
                        )?;
                    }
                    position = 0;
                }
                index += 1;
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }
    fn finish_packing(
        &mut self,
        result: Result<Value, Error>,
        pending: Option<Error>,
    ) -> Result<Value, Error> {
        match (result, pending) {
            (Err(error), _) if error.kind == ErrorKind::UnwindExit => Err(error),
            (Err(error), Some(previous)) => {
                let error = self.materialize_error(error);
                if let (Some(current), Some(previous)) = (&error.throwable, previous.throwable) {
                    throwables::append_previous(current, &previous);
                }
                Err(error)
            }
            (Ok(_), Some(error)) => Err(error),
            (result, None) => result,
        }
    }
}

fn format_error(message: String, line: usize) -> Error {
    Error::new(
        ErrorKind::ValueError,
        message.split('\0').next().unwrap(),
        line,
    )
}

fn repeat(format: &[u8], cursor: &mut usize) -> i64 {
    if format.get(*cursor) == Some(&b'*') {
        *cursor += 1;
        return -1;
    }
    if !format.get(*cursor).is_some_and(u8::is_ascii_digit) {
        return 1;
    }
    let mut count = 0_i64;
    while let Some(&digit) = format.get(*cursor).filter(|b| b.is_ascii_digit()) {
        count = count
            .saturating_mul(10)
            .saturating_add((digit - b'0') as i64);
        *cursor += 1;
    }
    count
}

fn width(code: u8) -> Option<usize> {
    Some(match code {
        b'c' | b'C' => 1,
        b's' | b'S' | b'n' | b'v' => 2,
        b'i' | b'I' => std::mem::size_of::<std::ffi::c_int>(),
        b'l' | b'L' | b'N' | b'V' | b'f' | b'g' | b'G' => 4,
        b'q' | b'Q' | b'J' | b'P' | b'd' | b'e' | b'E' => 8,
        _ => return None,
    })
}

fn little_endian(code: u8) -> bool {
    match code {
        b'n' | b'N' | b'J' | b'G' | b'E' => false,
        b'v' | b'V' | b'P' | b'g' | b'e' => true,
        _ => cfg!(target_endian = "little"),
    }
}

fn unpack_number(code: u8, bytes: &[u8]) -> Value {
    let mut storage = [0; 8];
    let bits = if little_endian(code) {
        storage[..bytes.len()].copy_from_slice(bytes);
        u64::from_le_bytes(storage)
    } else {
        storage[8 - bytes.len()..].copy_from_slice(bytes);
        u64::from_be_bytes(storage)
    };
    match code {
        b'f' | b'g' | b'G' => Value::Float(f32::from_bits(bits as u32) as f64),
        b'd' | b'e' | b'E' => Value::Float(f64::from_bits(bits)),
        b'c' | b's' | b'i' | b'l' => {
            let shift = 64 - bytes.len() * 8;
            Value::Int((bits << shift) as i64 >> shift)
        }
        _ => Value::Int(bits as i64),
    }
}
