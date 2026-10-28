use super::*;

impl Runtime {
    pub(super) fn strip_tags(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(args[0].as_ref().unwrap(), "strip_tags", 1, line)?;
        let allowed = match &args[1] {
            None | Some(Value::Null) => Value::Null,
            Some(value) => {
                self.native_array_or_string_type(value, "strip_tags", 2, line, "array|string|null")?
            }
        };
        let mut allowed = match allowed {
            Value::Array(array) => {
                let mut tags = Vec::new();
                for (_, value) in array.entries() {
                    let name = self.string_bytes(&value.dereferenced(), line)?;
                    tags.push(b'<');
                    tags.extend_from_slice(&name);
                    tags.push(b'>');
                }
                tags
            }
            Value::String(bytes) => bytes.to_vec(),
            Value::Null => Vec::new(),
            _ => unreachable!(),
        };
        allowed.make_ascii_lowercase();
        // PHP uses a NUL-terminated search for the normalized allowed tag.
        if let Some(end) = allowed.iter().position(|byte| *byte == 0) {
            allowed.truncate(end);
        }
        Ok(Value::String(strip(&bytes, &allowed).into()))
    }
}

#[derive(Clone, Copy)]
enum State {
    Text,
    Tag,
    Php,
    Declaration,
    Comment,
}

fn strip(bytes: &[u8], allowed: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut tag = Vec::new();
    let mut state = State::Text;
    let mut last = 0;
    let mut quote = 0;
    let mut depth = 0usize;
    let mut brackets = 0i64;
    let mut xml = false;
    let capture = !allowed.is_empty();
    for (index, &byte) in bytes.iter().enumerate() {
        let previous = index.checked_sub(1).map_or(0, |index| bytes[index]);
        match state {
            State::Text => match byte {
                0 => (),
                b'<' => {
                    if quote != 0 {
                        continue;
                    }
                    if bytes.get(index + 1).is_some_and(|byte| space(*byte)) {
                        result.push(byte);
                    } else {
                        last = b'<';
                        state = State::Tag;
                        if capture {
                            tag.push(byte);
                        }
                    }
                }
                b'>' if depth != 0 => depth -= 1,
                b'>' if quote != 0 => (),
                _ => result.push(byte),
            },
            State::Tag => match byte {
                0 => (),
                b'<' => {
                    if quote != 0 {
                        continue;
                    }
                    if bytes.get(index + 1).is_some_and(|byte| space(*byte)) {
                        if capture {
                            tag.push(byte);
                        }
                    } else {
                        depth += 1;
                    }
                }
                b'>' => {
                    if depth != 0 {
                        depth -= 1;
                        continue;
                    }
                    if quote != 0 {
                        continue;
                    }
                    last = b'>';
                    if xml && previous == b'-' {
                        continue;
                    }
                    quote = 0;
                    state = State::Text;
                    xml = false;
                    if capture {
                        tag.push(byte);
                        if tag_allowed(&tag, allowed) {
                            result.extend_from_slice(&tag);
                        }
                        tag.clear();
                    }
                }
                b'\'' | b'"' => {
                    toggle_quote(&mut quote, byte);
                    if capture {
                        tag.push(byte);
                    }
                }
                b'!' if previous == b'<' => {
                    state = State::Declaration;
                    last = byte;
                }
                b'?' if previous == b'<' => {
                    state = State::Php;
                    brackets = 0;
                }
                _ => {
                    if capture {
                        tag.push(byte);
                    }
                }
            },
            State::Php => match byte {
                b'(' | b')' if !matches!(last, b'\'' | b'"') => {
                    last = byte;
                    brackets += if byte == b'(' { 1 } else { -1 };
                }
                b'>' => {
                    if depth != 0 {
                        depth -= 1;
                        continue;
                    }
                    if quote != 0 {
                        continue;
                    }
                    if brackets == 0 && last != b'"' && previous == b'?' {
                        quote = 0;
                        state = State::Text;
                        tag.clear();
                    }
                }
                b'\'' | b'"' if previous != b'\\' => {
                    if last == byte {
                        last = 0;
                    } else if last != b'\\' {
                        last = byte;
                    }
                    toggle_quote(&mut quote, byte);
                }
                b'l' | b'L'
                    if index > 4 && bytes[index - 4..=index].eq_ignore_ascii_case(b"<?xml") =>
                {
                    state = State::Tag;
                    xml = true;
                }
                _ => (),
            },
            State::Declaration => match byte {
                b'>' => {
                    if depth != 0 {
                        depth -= 1;
                        continue;
                    }
                    if quote != 0 {
                        continue;
                    }
                    quote = 0;
                    state = State::Text;
                    tag.clear();
                }
                b'\'' | b'"' if previous != b'\\' => toggle_quote(&mut quote, byte),
                b'-' if index >= 2 && previous == b'-' && bytes[index - 2] == b'!' => {
                    state = State::Comment
                }
                b'e' | b'E'
                    if index > 6 && bytes[index - 6..=index].eq_ignore_ascii_case(b"doctype") =>
                {
                    state = State::Tag
                }
                _ => (),
            },
            State::Comment => {
                if byte == b'>' && quote == 0 && index >= 2 && &bytes[index - 2..index] == b"--" {
                    quote = 0;
                    state = State::Text;
                    tag.clear();
                }
            }
        }
    }
    result
}

fn toggle_quote(quote: &mut u8, byte: u8) {
    if *quote == 0 {
        *quote = byte;
    } else if *quote == byte {
        *quote = 0;
    }
}

fn space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t'..=b'\r')
}

fn tag_allowed(tag: &[u8], allowed: &[u8]) -> bool {
    let mut normalized = Vec::new();
    let mut started = false;
    for (index, &byte) in tag.iter().enumerate() {
        match byte {
            b'<' => normalized.push(byte),
            b'>' => break,
            byte if space(byte) => {
                if started {
                    break;
                }
            }
            _ => {
                started = true;
                if byte != b'/'
                    || (index.checked_sub(1).is_none_or(|index| tag[index] != b'<')
                        && tag.get(index + 1) != Some(&b'>'))
                {
                    normalized.push(byte.to_ascii_lowercase());
                }
            }
        }
    }
    normalized.push(b'>');
    allowed
        .windows(normalized.len())
        .any(|window| window == normalized)
}
