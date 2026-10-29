// Altered Rust adaptation of PHP 8.5.0 ext/standard/string.c wordwrap.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md for its license.
use super::*;

impl Runtime {
    pub(super) fn wordwrap(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        const NAME: &str = "wordwrap";
        let text = self.native_string(args[0].as_ref().unwrap(), NAME, 1, line)?;
        let width = args[1]
            .as_ref()
            .map(|v| self.native_int(v, NAME, 2, line))
            .transpose()?
            .unwrap_or(75);
        let separator = args[2]
            .as_ref()
            .map(|v| self.native_string(v, NAME, 3, line))
            .transpose()?
            .unwrap_or_else(|| b"\n".to_vec());
        let cut = args[3]
            .as_ref()
            .map(|v| self.native_bool(v, NAME, 4, line))
            .transpose()?
            .unwrap_or(false);
        if text.is_empty() {
            return Ok(Value::String(text.into()));
        }
        if separator.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "wordwrap(): Argument #3 ($break) must not be empty",
                line,
            ));
        }
        if width == 0 && cut {
            return Err(Error::new(
                ErrorKind::ValueError,
                "wordwrap(): Argument #4 ($cut_long_words) cannot be true when argument #2 ($width) is 0",
                line,
            ));
        }
        let mut start = 0;
        let mut space = 0;
        if separator.len() == 1 && !cut {
            let mut output = text.clone();
            for (current, &byte) in text.iter().enumerate() {
                if byte == separator[0] {
                    start = current + 1;
                    space = start;
                } else if byte == b' ' {
                    if (current - start) as i64 >= width {
                        output[current] = separator[0];
                        start = current + 1;
                    }
                    space = current;
                } else if (current - start) as i64 >= width && start != space {
                    output[space] = separator[0];
                    start = space + 1;
                }
            }
            return Ok(Value::String(output.into()));
        }
        let mut output = Vec::new();
        let mut current = 0;
        while current < text.len() {
            // Native strncmp stops at matching NUL bytes. A separator ending
            // exactly at EOF does not enter this existing-break branch.
            if separator.len() < text.len() - current
                && matches_separator(&text[current..], &separator)
            {
                append(
                    &mut output,
                    &text[start..current + separator.len()],
                    &[],
                    line,
                )?;
                current += separator.len() - 1;
                start = current + 1;
                space = start;
            } else if text[current] == b' ' {
                if (current - start) as i64 >= width {
                    append(&mut output, &text[start..current], &separator, line)?;
                    start = current + 1;
                }
                space = current;
            } else if (current - start) as i64 >= width && cut && start >= space {
                append(&mut output, &text[start..current], &separator, line)?;
                start = current;
                space = current;
            } else if (current - start) as i64 >= width && start < space {
                append(&mut output, &text[start..space], &separator, line)?;
                start = space + 1;
                space = start;
            }
            current += 1;
        }
        append(&mut output, &text[start..], &[], line)?;
        Ok(Value::String(output.into()))
    }
}

fn matches_separator(text: &[u8], separator: &[u8]) -> bool {
    for (&a, &b) in text.iter().zip(separator) {
        if a != b {
            return false;
        }
        if a == 0 {
            break;
        }
    }
    true
}

fn append(output: &mut Vec<u8>, text: &[u8], separator: &[u8], line: usize) -> Result<(), Error> {
    let extra = text
        .len()
        .checked_add(separator.len())
        .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
    output
        .try_reserve(extra)
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
    output.extend_from_slice(text);
    output.extend_from_slice(separator);
    Ok(())
}
