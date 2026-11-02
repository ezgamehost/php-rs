// Altered Rust adaptation of PHP 8.5.0 ext/standard/uuencode.c.
// Copyright (c) The PHP Group; original author Ilia Alshanetsky.
// Portions derive from Berkeley uuencode/uudecode, copyright (c) 1983, 1993
// The Regents of the University of California. All rights reserved.
// See THIRD_PARTY_NOTICES.md for the complete licenses and acknowledgement.
use super::*;

impl Runtime {
    pub(super) fn uuencode(
        &mut self,
        encode: bool,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(value, name, 1, line)?;
        if encode {
            Ok(Value::String(encode_bytes(&bytes, line)?.into()))
        } else if let Some(result) = decode_bytes(&bytes) {
            Ok(Value::String(result.into()))
        } else {
            // PHP's diagnostic says data, although the parameter is string.
            self.warn(format!(
                "{name}(): Argument #1 ($data) is not a valid uuencoded string on line {line}"
            ))?;
            Ok(Value::Bool(false))
        }
    }
}

fn encode_bytes(input: &[u8], line: usize) -> Result<Vec<u8>, Error> {
    let capacity = (input.len() / 2)
        .checked_mul(3)
        .and_then(|length| length.checked_add(46))
        .ok_or_else(|| Error::new(ErrorKind::Error, "Encoded string size overflow", line))?;
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity)
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate encoded string", line))?;
    for row in input.chunks(45) {
        output.push(encode_byte(row.len() as u8));
        for group in row.chunks(3) {
            let a = group[0];
            let b = group.get(1).copied().unwrap_or(0);
            let c = group.get(2).copied().unwrap_or(0);
            output.extend([
                encode_byte(a >> 2),
                encode_byte(((a << 4) & 0x30) | (b >> 4)),
                encode_byte(((b << 2) & 0x3c) | (c >> 6)),
                encode_byte(c & 0x3f),
            ]);
        }
        output.push(b'\n');
    }
    output.extend_from_slice(b"`\n");
    Ok(output)
}

fn encode_byte(byte: u8) -> u8 {
    if byte == 0 { b'`' } else { byte + b' ' }
}

fn decode_byte(byte: u8) -> u8 {
    byte.wrapping_sub(b' ') & 0x3f
}

fn decode_bytes(input: &[u8]) -> Option<Vec<u8>> {
    if input.is_empty() {
        return None;
    }
    let mut output = Vec::new();
    let mut cursor = 0;
    let mut total = 0;
    while cursor < input.len() {
        let length = decode_byte(input[cursor]) as usize;
        cursor += 1;
        if length == 0 {
            break;
        }
        if length > input.len() {
            return None;
        }
        total += length;
        // For all possible six-bit lengths, this integer expression equals
        // PHP's floor(length * 1.33). Full lines have a special exact size.
        let end = cursor + if length == 45 { 60 } else { length * 133 / 100 };
        if end > input.len() {
            return None;
        }
        while cursor < end {
            let group = input.get(cursor..cursor + 4)?;
            let [a, b, c, d] = std::array::from_fn(|index| decode_byte(group[index]));
            output.extend([(a << 2) | (b >> 4), (b << 4) | (c >> 2), (c << 6) | d]);
            cursor += 4;
        }
        if length < 45 {
            break;
        }
        // Native decoding skips one byte without validating a newline, and
        // ignores trailing bytes after a short line or zero-length terminator.
        cursor += 1;
    }
    // Decoded groups include padding. Every accepted line supplies at least
    // its declared length, so truncation cannot expose uninitialized bytes.
    if total > output.len() {
        return None;
    }
    output.truncate(total);
    Some(output)
}
