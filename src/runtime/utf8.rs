// Altered Rust adaptation of PHP 8.5.0 ext/standard/string.c's UTF-8/Latin-1
// conversions and html.c's UTF-8 character reader. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn utf8_latin1(
        &mut self,
        encode: bool,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(value, name, 1, line)?;
        let capacity = bytes
            .len()
            .checked_mul(if encode { 2 } else { 1 })
            .ok_or_else(|| Error::new(ErrorKind::Error, "Converted string size overflow", line))?;
        let mut output = Vec::new();
        output.try_reserve_exact(capacity).map_err(|_| {
            Error::new(
                ErrorKind::Error,
                "Unable to allocate converted string",
                line,
            )
        })?;
        if encode {
            for byte in bytes {
                if byte < 0x80 {
                    output.push(byte);
                } else {
                    output.extend_from_slice(&[0xc0 | (byte >> 6), 0x80 | (byte & 0x3f)]);
                }
            }
        } else {
            let mut cursor = 0;
            while cursor < bytes.len() {
                output.push(
                    next_char(&bytes, &mut cursor)
                        .filter(|&code| code <= 0xff)
                        .map_or(b'?', |code| code as u8),
                );
            }
        }
        Ok(Value::String(output.into()))
    }
}

// Advances past one character or illegal sequence. PHP retains any subsequent
// ASCII byte or valid UTF-8 leading byte for the next iteration, but consumes
// other invalid bytes within the expected width of the current sequence.
pub(super) fn next_char(input: &[u8], cursor: &mut usize) -> Option<u32> {
    let remaining = &input[*cursor..];
    let first = remaining[0];
    let (width, minimum, mask) = match first {
        0..=0x7f => {
            *cursor += 1;
            return Some(u32::from(first));
        }
        0xc2..=0xdf => (2, 0x80, 0x1f),
        0xe0..=0xef => (3, 0x800, 0x0f),
        0xf0..=0xf4 => (4, 0x10000, 0x07),
        _ => {
            *cursor += 1;
            return None;
        }
    };
    if remaining.len() < width
        || remaining[1..width]
            .iter()
            .any(|byte| !(0x80..=0xbf).contains(byte))
    {
        let advance = (1..width)
            .find(|&index| {
                remaining
                    .get(index)
                    .is_none_or(|&byte| byte < 0x80 || (0xc2..=0xf4).contains(&byte))
            })
            .unwrap_or(width);
        *cursor += advance;
        return None;
    }
    *cursor += width;
    let mut code = u32::from(first & mask);
    for byte in &remaining[1..width] {
        code = (code << 6) | u32::from(byte & 0x3f);
    }
    (code >= minimum && code <= 0x10ffff && !(0xd800..=0xdfff).contains(&code)).then_some(code)
}
