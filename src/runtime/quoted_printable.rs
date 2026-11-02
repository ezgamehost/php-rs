// Altered Rust adaptation of PHP 8.5.0 ext/standard/quot_print.c.
// Copyright (c) The PHP Group; original author Kirill Maximov.
// See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn quoted_printable(
        &mut self,
        encode: bool,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(value, name, 1, line)?;
        if bytes.is_empty() {
            return Ok(Value::String(bytes.into()));
        }
        let capacity = if encode {
            bytes
                .len()
                .checked_mul(3)
                .and_then(|length| bytes.len().checked_add(length / 66 + 1))
                .and_then(|length| length.checked_mul(3))
                .ok_or_else(|| Error::new(ErrorKind::Error, "Encoded string size overflow", line))?
        } else {
            bytes.len()
        };
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate encoded string", line))?;
        if encode {
            encode_bytes(&bytes, &mut output);
        } else {
            decode_bytes(&bytes, &mut output);
        }
        Ok(Value::String(output.into()))
    }
}

fn encode_bytes(input: &[u8], output: &mut Vec<u8>) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut column = 0usize;
    let mut index = 0;
    while index < input.len() {
        let byte = input[index];
        let next = input.get(index + 1).copied();
        index += 1;
        if byte == b'\r' && next == Some(b'\n') {
            output.extend_from_slice(b"\r\n");
            index += 1;
            column = 0;
        } else if byte.is_ascii_control()
            || byte >= 0x80
            || byte == b'='
            || (byte == b' ' && next == Some(b'\r'))
        {
            column += 3;
            // PHP reserves room according to the byte's UTF-8 range, including
            // continuation bytes. Bytes above F4 never trigger this wrap check.
            let wrap = match byte {
                0..=0x7f => column > 75,
                0x80..=0xdf => column + 3 > 75,
                0xe0..=0xef => column + 6 > 75,
                0xf0..=0xf4 => column + 9 > 75,
                _ => false,
            };
            if wrap {
                output.extend_from_slice(b"=\r\n");
                column = 3;
            }
            output.extend_from_slice(&[b'=', HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]]);
        } else {
            column += 1;
            if column > 75 {
                output.extend_from_slice(b"=\r\n");
                column = 1;
            }
            output.push(byte);
        }
    }
}

fn decode_bytes(input: &[u8], output: &mut Vec<u8>) {
    // Native decoding stops at an input NUL. An escaped =00 is emitted normally.
    let input = input.split(|&byte| byte == 0).next().unwrap();
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'=' {
            let digits = input
                .get(index + 1)
                .and_then(|byte| (*byte as char).to_digit(16))
                .zip(
                    input
                        .get(index + 2)
                        .and_then(|byte| (*byte as char).to_digit(16)),
                );
            if let Some((high, low)) = digits {
                output.push((high * 16 + low) as u8);
                index += 3;
                continue;
            }
            let mut end = index + 1;
            while matches!(input.get(end), Some(b' ' | b'\t')) {
                end += 1;
            }
            match input.get(end) {
                None => break,
                Some(b'\r') => {
                    index = end
                        + if input.get(end + 1) == Some(&b'\n') {
                            2
                        } else {
                            1
                        };
                    continue;
                }
                Some(b'\n') => {
                    index = end + 1;
                    continue;
                }
                _ => {}
            }
        }
        output.push(input[index]);
        index += 1;
    }
}
