// Altered Rust adaptation of PHP 8.5.0 ext/standard/url.c.
// Copyright (c) The PHP Group; original author Jim Winstead.
// See THIRD_PARTY_NOTICES.md.
use super::*;

const NAMES: [&[u8]; 8] = [
    b"scheme",
    b"host",
    b"port",
    b"user",
    b"pass",
    b"path",
    b"query",
    b"fragment",
];
type Parts = [Option<Value>; 8];

impl Runtime {
    pub(super) fn url_codec(
        &mut self,
        encode: bool,
        raw: bool,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        let input = self.native_string(value, name, 1, line)?;
        let capacity = input
            .len()
            .checked_mul(if encode { 3 } else { 1 })
            .ok_or_else(|| Error::new(ErrorKind::Error, "URL string size overflow", line))?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate URL string", line))?;
        let mut index = 0;
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        while index < input.len() {
            let byte = input[index];
            index += 1;
            if encode {
                if byte == b' ' && !raw {
                    output.push(b'+');
                } else if byte.is_ascii_alphanumeric()
                    || b"-_.".contains(&byte)
                    || (raw && byte == b'~')
                {
                    output.push(byte);
                } else {
                    output.extend_from_slice(&[
                        b'%',
                        HEX[(byte >> 4) as usize],
                        HEX[(byte & 15) as usize],
                    ]);
                }
            } else if byte == b'+' && !raw {
                output.push(b' ');
            } else if byte == b'%'
                && let Some((high, low)) = input
                    .get(index)
                    .and_then(|byte| (*byte as char).to_digit(16))
                    .zip(
                        input
                            .get(index + 1)
                            .and_then(|byte| (*byte as char).to_digit(16)),
                    )
            {
                output.push((high * 16 + low) as u8);
                index += 2;
            } else {
                output.push(byte);
            }
        }
        Ok(Value::String(output.into()))
    }

    pub(super) fn parse_url(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let input = self.native_string(args[0].as_ref().unwrap(), "parse_url", 1, line)?;
        let component = args[1]
            .as_ref()
            .map(|value| self.native_int(value, "parse_url", 2, line))
            .transpose()?
            .unwrap_or(-1);
        let Some(mut parts) = parse(&input) else {
            return Ok(Value::Bool(false));
        };
        // PHP validates the component only after successfully parsing the URL.
        if component >= 0 {
            if component >= NAMES.len() as i64 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!(
                        "parse_url(): Argument #2 ($component) must be a valid URL component identifier, {component} given"
                    ),
                    line,
                ));
            }
            return Ok(parts[component as usize].take().unwrap_or(Value::Null));
        }
        let mut result = PhpArray::default();
        for (name, value) in NAMES.into_iter().zip(parts) {
            if let Some(value) = value {
                result.insert(ArrayKey::String(name.to_vec()), value);
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }
}

fn component(input: &[u8]) -> Value {
    Value::String(
        input
            .iter()
            .map(|&b| if b.is_ascii_control() { b'_' } else { b })
            .collect::<Vec<_>>()
            .into(),
    )
}

enum Start {
    Port(usize),
    Host(usize),
    Path(usize),
    Complete,
}

fn start(input: &[u8], parts: &mut Parts) -> Start {
    let relative = input.starts_with(b"//");
    let Some(colon) = input.iter().position(|&b| b == b':') else {
        return if relative {
            Start::Host(2)
        } else {
            Start::Path(0)
        };
    };
    if colon == 0 {
        return Start::Port(colon);
    }
    if !input[..colon]
        .iter()
        .all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(b))
    {
        if colon + 1 < input.len()
            && colon
                < input
                    .iter()
                    .position(|b| b"?#".contains(b))
                    .unwrap_or(input.len())
        {
            return Start::Port(colon);
        }
        return if relative {
            Start::Host(2)
        } else {
            Start::Path(0)
        };
    }
    if colon + 1 == input.len() {
        parts[0] = Some(component(&input[..colon]));
        return Start::Complete;
    }
    if input[colon + 1] != b'/' {
        let digits = input[colon + 1..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let end = colon + 1 + digits;
        if (end == input.len() || input[end] == b'/') && digits < 6 {
            return Start::Port(colon);
        }
        parts[0] = Some(component(&input[..colon]));
        return Start::Path(colon + 1);
    }
    parts[0] = Some(component(&input[..colon]));
    if input.get(colon + 2) != Some(&b'/') {
        return Start::Path(colon + 1);
    }
    if input[..colon].eq_ignore_ascii_case(b"file") && input.get(colon + 3) == Some(&b'/') {
        // Native file URLs remove the extra slash before a Windows drive letter.
        return Start::Path(
            colon
                + if input.get(colon + 5) == Some(&b':') {
                    4
                } else {
                    3
                },
        );
    }
    Start::Host(colon + 3)
}

fn parse(input: &[u8]) -> Option<Parts> {
    let mut parts = std::array::from_fn(|_| None);
    let mut state = start(input, &mut parts);
    if let Start::Port(colon) = state {
        let digits = input[colon + 1..]
            .iter()
            .take(6)
            .take_while(|b| b.is_ascii_digit())
            .count();
        let end = colon + 1 + digits;
        if (1..6).contains(&digits) && (end == input.len() || input[end] == b'/') {
            parts[2] = Some(Value::Int(port(&input[colon + 1..end])?));
            state = Start::Host(if input.starts_with(b"//") { 2 } else { 0 });
        } else if digits == 0 && end == input.len() {
            return None;
        } else {
            state = if input.starts_with(b"//") {
                Start::Host(2)
            } else {
                Start::Path(0)
            };
        }
    }
    if let Start::Host(mut begin) = state {
        let end = begin
            + input[begin..]
                .iter()
                .position(|b| b"/?#".contains(b))
                .unwrap_or(input.len() - begin);
        if let Some(at) = input[begin..end]
            .iter()
            .rposition(|&b| b == b'@')
            .map(|p| begin + p)
        {
            if let Some(colon) = input[begin..at]
                .iter()
                .position(|&b| b == b':')
                .map(|p| begin + p)
            {
                parts[3] = Some(component(&input[begin..colon]));
                parts[4] = Some(component(&input[colon + 1..at]));
            } else {
                parts[3] = Some(component(&input[begin..at]));
            }
            begin = at + 1;
        }
        let colon = if input.get(begin) == Some(&b'[') && end > begin && input[end - 1] == b']' {
            None
        } else {
            input[begin..end]
                .iter()
                .rposition(|&b| b == b':')
                .map(|p| begin + p)
        };
        let host_end = if let Some(colon) = colon {
            if !matches!(parts[2], Some(Value::Int(value)) if value != 0) {
                let bytes = &input[colon + 1..end];
                if bytes.len() > 5 {
                    return None;
                }
                if !bytes.is_empty() {
                    parts[2] = Some(Value::Int(port(bytes)?));
                }
            }
            colon
        } else {
            end
        };
        if host_end == begin {
            return None;
        }
        parts[1] = Some(component(&input[begin..host_end]));
        if end == input.len() {
            return Some(parts);
        }
        state = Start::Path(end);
    }
    if let Start::Path(begin) = state {
        let mut end = input.len();
        if let Some(hash) = input[begin..end]
            .iter()
            .position(|&b| b == b'#')
            .map(|p| begin + p)
        {
            parts[7] = Some(component(&input[hash + 1..end]));
            end = hash;
        }
        if let Some(query) = input[begin..end]
            .iter()
            .position(|&b| b == b'?')
            .map(|p| begin + p)
        {
            parts[6] = Some(component(&input[query + 1..end]));
            end = query;
        }
        if begin < end || begin == input.len() {
            parts[5] = Some(component(&input[begin..end]));
        }
    }
    Some(parts)
}

// Native host ports use strtol: leading ASCII whitespace, a sign and trailing
// non-digits are accepted. The caller enforces the five-byte field limit.
fn port(input: &[u8]) -> Option<i64> {
    let mut index = 0;
    while input
        .get(index)
        .is_some_and(|b| b" \t\n\r\x0b\x0c".contains(b))
    {
        index += 1;
    }
    let negative = input.get(index) == Some(&b'-');
    if matches!(input.get(index), Some(b'+' | b'-')) {
        index += 1;
    }
    let start = index;
    let mut value = 0;
    while let Some(byte) = input.get(index).filter(|b| b.is_ascii_digit()) {
        value = value * 10 + i64::from(byte - b'0');
        index += 1;
    }
    if index == start || value > 65535 || (negative && value != 0) {
        None
    } else {
        Some(value)
    }
}
