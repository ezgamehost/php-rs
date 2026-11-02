// Altered Rust adaptation of PHP 8.5.0 ext/standard/file.c and string.c.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md for its license.
use super::*;

impl Runtime {
    pub(super) fn stream_csv(
        &mut self,
        write: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let stream = args[0].as_ref().unwrap();
        self.validate_stream(stream, name, line)?;
        let fields = if write {
            let value = args[1].as_ref().unwrap();
            let Value::Array(fields) = value else {
                return Err(builtins::type_error(value, name, 2, "array", line));
            };
            Some(fields)
        } else {
            None
        };
        let length = if write {
            None
        } else {
            args[1]
                .as_ref()
                .filter(|v| !matches!(v, Value::Null))
                .map(|v| self.native_int_type(v, name, 2, line, "?int"))
                .transpose()?
        };
        let separator = args[2]
            .as_ref()
            .map(|v| self.native_string(v, name, 3, line))
            .transpose()?
            .unwrap_or_else(|| b",".to_vec());
        let enclosure = args[3]
            .as_ref()
            .map(|v| self.native_string(v, name, 4, line))
            .transpose()?
            .unwrap_or_else(|| b"\"".to_vec());
        let escape = args[4]
            .as_ref()
            .map(|v| self.native_string(v, name, 5, line))
            .transpose()?;
        let eol = if write {
            args[5]
                .as_ref()
                .filter(|v| !matches!(v, Value::Null))
                .map(|v| self.native_string_type(v, name, 6, line, "?string"))
                .transpose()?
        } else {
            None
        };
        for (position, parameter, bytes) in
            [(3, "separator", &separator), (4, "enclosure", &enclosure)]
        {
            if bytes.len() != 1 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!(
                        "{name}(): Argument #{position} (${parameter}) must be a single character"
                    ),
                    line,
                ));
            }
        }
        let escape = if let Some(escape) = escape {
            if escape.len() > 1 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!("{name}(): Argument #5 ($escape) must be empty or a single character"),
                    line,
                ));
            }
            escape.first().copied()
        } else {
            self.warn(format!("Deprecated: {name}(): the $escape parameter must be provided as its default value will change on line {line}"))?;
            Some(b'\\')
        };
        let (separator, enclosure) = (separator[0], enclosure[0]);
        if let Some(fields) = fields {
            let mut output = Vec::new();
            let count = fields.len();
            let mut index = 0;
            let mut pending = None;
            while let Some((_, field)) = fields.entry(index) {
                // Native CSV conversion keeps building and writes the record
                // after a conversion exception; later object callbacks stop.
                let text = self.set_string(&field, line, &mut pending);
                if pending
                    .as_ref()
                    .is_some_and(|e| e.kind == ErrorKind::UnwindExit)
                {
                    return Err(pending.unwrap());
                }
                let quoted = text.iter().any(|&b| {
                    b == separator
                        || b == enclosure
                        || Some(b) == escape
                        || matches!(b, b'\n' | b'\r' | b'\t' | b' ')
                });
                if quoted {
                    output.push(enclosure);
                    let mut escaped = false;
                    for byte in text {
                        if Some(byte) == escape {
                            escaped = true;
                        } else if !escaped && byte == enclosure {
                            output.push(enclosure);
                        } else {
                            escaped = false;
                        }
                        output.push(byte);
                    }
                    output.push(enclosure);
                } else {
                    output.extend_from_slice(&text);
                }
                index += 1;
                if index != count {
                    output.push(separator);
                }
            }
            output.extend_from_slice(eol.as_deref().unwrap_or(b"\n"));
            let written = self.stream_operation(
                streams::Operation::Write,
                name,
                &[
                    Some(stream.clone()),
                    Some(Value::String(output.into())),
                    None,
                ],
                line,
            );
            if let Err(error) = &written {
                self.set_error(error.clone(), &mut pending);
            }
            return pending.map_or(written, Err);
        }
        if length.is_some_and(|n| n < 0 || n == i64::MAX) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!(
                    "{name}(): Argument #2 ($length) must be between 0 and {}",
                    i64::MAX - 1
                ),
                line,
            ));
        }
        let limit = length.filter(|n| *n != 0).map(|n| n as usize);
        let Some(text) = self.stream_read_line(stream, name, limit, line)? else {
            return Ok(Value::Bool(false));
        };
        parse_record(text, separator, enclosure, escape, line, || {
            self.stream_read_line(stream, name, None, line)
        })
    }

    pub(super) fn str_getcsv(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        const NAME: &str = "str_getcsv";
        let text = self.native_string(args[0].as_ref().unwrap(), NAME, 1, line)?;
        let separator = args[1]
            .as_ref()
            .map(|v| self.native_string(v, NAME, 2, line))
            .transpose()?
            .unwrap_or_else(|| b",".to_vec());
        let enclosure = args[2]
            .as_ref()
            .map(|v| self.native_string(v, NAME, 3, line))
            .transpose()?
            .unwrap_or_else(|| b"\"".to_vec());
        let escape = args[3]
            .as_ref()
            .map(|v| self.native_string(v, NAME, 4, line))
            .transpose()?;
        for (position, name, bytes) in [(2, "separator", &separator), (3, "enclosure", &enclosure)]
        {
            if bytes.len() != 1 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!("{NAME}(): Argument #{position} (${name}) must be a single character"),
                    line,
                ));
            }
        }
        let escape = if let Some(escape) = escape {
            if escape.len() > 1 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "str_getcsv(): Argument #4 ($escape) must be empty or a single character",
                    line,
                ));
            }
            escape.first().copied()
        } else {
            self.warn(format!("Deprecated: str_getcsv(): the $escape parameter must be provided as its default value will change on line {line}"))?;
            Some(b'\\')
        };
        parse_record(text, separator[0], enclosure[0], escape, line, || Ok(None))
    }
}

fn parse_record(
    mut text: Vec<u8>,
    separator: u8,
    enclosure: u8,
    escape: Option<u8>,
    line: usize,
    mut next_line: impl FnMut() -> Result<Option<Vec<u8>>, Error>,
) -> Result<Value, Error> {
    let mut length = text.len();
    let mut limit = trailing_line_end(&text);
    // Native parsing can inspect the terminating NUL once after an opening
    // enclosure at EOF. Keep it explicit, including the NUL-escape behavior.
    text.push(0);
    let mut values = PhpArray::default();
    let mut cursor = 0;
    loop {
        let mut width = character_width(&text, cursor, limit);
        if width == 1 {
            let mut next = cursor;
            while next < length && text[next] != separator && ascii_space(text[next]) {
                next += 1;
            }
            if next < limit && text[next] == enclosure {
                cursor = next;
            }
        }
        if values.is_empty() && cursor == limit {
            values.append(Value::Null, line)?;
            break;
        }
        let mut field = Vec::new();
        if width != 0 && text[cursor] == enclosure {
            let mut state = 0;
            cursor += 1;
            let mut hunk = cursor;
            loop {
                if width == 0 {
                    if state == 2 {
                        field.extend_from_slice(&text[hunk..cursor - 1]);
                        hunk = cursor;
                    } else {
                        if state == 1 {
                            field.extend_from_slice(&text[hunk..cursor]);
                            hunk = cursor;
                        }
                        if hunk != limit {
                            field.extend_from_slice(&text[hunk..cursor]);
                            hunk = cursor;
                        }
                        field.extend_from_slice(&text[limit..length]);
                        if let Some(next) = next_line()? {
                            text = next;
                            length = text.len();
                            limit = trailing_line_end(&text);
                            text.push(0);
                            cursor = 0;
                            hunk = 0;
                            state = 0;
                            width = character_width(&text, cursor, limit);
                            continue;
                        }
                        if cursor > limit {
                            if hunk == cursor {
                                hunk -= 1;
                            }
                            cursor -= 1;
                        }
                    }
                    break;
                } else if width <= 1 {
                    match state {
                        1 => {
                            cursor += 1;
                            state = 0;
                        }
                        2 => {
                            if text[cursor] != enclosure {
                                field.extend_from_slice(&text[hunk..cursor - 1]);
                                hunk = cursor;
                                break;
                            }
                            field.extend_from_slice(&text[hunk..cursor]);
                            cursor += 1;
                            hunk = cursor;
                            state = 0;
                        }
                        _ => {
                            if text[cursor] == enclosure {
                                state = 2;
                            } else if escape == Some(text[cursor]) {
                                state = 1;
                            }
                            cursor += 1;
                        }
                    }
                } else {
                    if state == 2 {
                        field.extend_from_slice(&text[hunk..cursor - 1]);
                        hunk = cursor;
                        break;
                    }
                    cursor += width as usize;
                    if state == 1 {
                        field.extend_from_slice(&text[hunk..cursor]);
                        hunk = cursor;
                        state = 0;
                    }
                }
                width = character_width(&text, cursor, limit);
            }
            // Bytes after a closing enclosure remain in the field until the
            // next separator, even when they are spaces or more quote bytes.
            while width != 0 {
                width = width.max(1);
                if width == 1 && text[cursor] == separator {
                    break;
                }
                cursor += width as usize;
                width = character_width(&text, cursor, limit);
            }
            field.extend_from_slice(&text[hunk..cursor]);
            cursor += width as usize;
        } else {
            let start = cursor;
            while width != 0 {
                width = width.max(1);
                if width == 1 && text[cursor] == separator {
                    break;
                }
                cursor += width as usize;
                width = character_width(&text, cursor, limit);
            }
            let bytes = &text[start..cursor];
            field.extend_from_slice(&bytes[..trailing_line_end(bytes)]);
            if text[cursor] == separator {
                cursor += 1;
            }
        }
        values.append(Value::String(field.into()), line)?;
        if width == 0 {
            break;
        }
    }
    Ok(Value::Array(Rc::new(values)))
}

fn ascii_space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

// Follow the reference CLI's UTF-8 character locale. Invalid byte sequences reset
// the native conversion state and advance one byte; they do not update the
// remembered characters used to recognize a trailing CR/LF pair.
fn character_width(bytes: &[u8], cursor: usize, limit: usize) -> isize {
    if cursor >= limit {
        return 0;
    }
    let width = match bytes[cursor] {
        0..=0x7f => return 1,
        0xc2..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf4 => 4,
        _ => return -1,
    };
    if width <= limit - cursor && std::str::from_utf8(&bytes[cursor..cursor + width]).is_ok() {
        width as isize
    } else {
        -1
    }
}

fn trailing_line_end(bytes: &[u8]) -> usize {
    let mut cursor = 0;
    let mut last = [0; 2];
    while cursor < bytes.len() {
        let width = character_width(bytes, cursor, bytes.len());
        if width > 0 {
            last = [last[1], bytes[cursor]];
        }
        cursor += width.max(1) as usize;
    }
    match last {
        [b'\r', b'\n'] => cursor - 2,
        [_, b'\r' | b'\n'] => cursor - 1,
        _ => cursor,
    }
}
