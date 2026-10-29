// Base64 decoding rules adapted from PHP 8.5.0 ext/standard/base64.c.
// Copyright (c) The PHP Group. Altered Rust implementation; see
// THIRD_PARTY_NOTICES.md for the PHP license.
use super::*;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

impl Runtime {
    pub(super) fn base64(
        &mut self,
        encode: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let strict = if encode {
            false
        } else {
            args[1]
                .as_ref()
                .map(|v| self.native_bool(v, name, 2, line))
                .transpose()?
                .unwrap_or(false)
        };
        let capacity = if encode {
            bytes
                .len()
                .div_ceil(3)
                .checked_mul(4)
                .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?
        } else {
            bytes.len() / 4 * 3 + bytes.len() % 4
        };
        let mut output = Vec::new();
        output
            .try_reserve_exact(capacity)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
        if encode {
            for chunk in bytes.chunks(3) {
                let a = chunk[0];
                let b = chunk.get(1).copied().unwrap_or(0);
                let c = chunk.get(2).copied().unwrap_or(0);
                output.push(ALPHABET[(a >> 2) as usize]);
                output.push(ALPHABET[(((a & 3) << 4) | (b >> 4)) as usize]);
                output.push(if chunk.len() > 1 {
                    ALPHABET[(((b & 15) << 2) | (c >> 6)) as usize]
                } else {
                    b'='
                });
                output.push(if chunk.len() > 2 {
                    ALPHABET[(c & 63) as usize]
                } else {
                    b'='
                });
            }
        } else {
            let mut padding = 0usize;
            let mut digits = 0usize;
            let mut pending = 0u8;
            for byte in bytes {
                let digit = match byte {
                    b'A'..=b'Z' => byte - b'A',
                    b'a'..=b'z' => byte - b'a' + 26,
                    b'0'..=b'9' => byte - b'0' + 52,
                    b'+' => 62,
                    b'/' => 63,
                    b'=' => {
                        padding += 1;
                        continue;
                    }
                    b' ' | b'\t' | b'\r' | b'\n' => continue,
                    _ if !strict => continue,
                    _ => return Ok(Value::Bool(false)),
                };
                if strict && padding != 0 {
                    return Ok(Value::Bool(false));
                }
                match digits % 4 {
                    0 => pending = digit << 2,
                    1 => {
                        output.push(pending | (digit >> 4));
                        pending = digit << 4;
                    }
                    2 => {
                        output.push(pending | (digit >> 2));
                        pending = digit << 6;
                    }
                    _ => output.push(pending | digit),
                }
                digits += 1;
            }
            // Missing padding is accepted, but an isolated final digit is not.
            // PHP does not require unused bits in the final digit to be zero.
            if strict
                && (digits % 4 == 1
                    || (padding != 0 && (padding > 2 || !(digits + padding).is_multiple_of(4))))
            {
                return Ok(Value::Bool(false));
            }
        }
        Ok(Value::String(output.into()))
    }
}
