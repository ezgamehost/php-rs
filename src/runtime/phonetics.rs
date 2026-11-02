// Altered Rust adaptation of PHP 8.5.0 ext/standard/{soundex,metaphone}.c.
// Copyright (c) The PHP Group. Authors include Bjørn Borud and Thies C. Arntzen;
// Metaphone derives from Michael G Schwern's Text-Metaphone implementation.
// See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn phonetic(
        &mut self,
        metaphone: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        if !metaphone {
            return Ok(Value::String(soundex(&bytes).to_vec().into()));
        }
        let limit = args[1]
            .as_ref()
            .map(|value| self.native_int(value, name, 2, line))
            .transpose()?
            .unwrap_or(0);
        if limit < 0 {
            return Err(Error::new(
                ErrorKind::ValueError,
                "metaphone(): Argument #2 ($max_phonemes) must be greater than or equal to 0",
                line,
            ));
        }
        Ok(Value::String(
            metaphone_bytes(&bytes, limit as usize, line)?.into(),
        ))
    }
}

fn soundex(input: &[u8]) -> [u8; 4] {
    const CODES: &[u8; 26] = b"\x00123\x0012\x00\x0022455\x0012623\x001\x002\x002";
    let mut output = [b'0'; 4];
    let mut used = 0;
    let mut previous = 0;
    for byte in input.iter().map(u8::to_ascii_uppercase) {
        if !byte.is_ascii_uppercase() {
            continue;
        }
        let code = CODES[(byte - b'A') as usize];
        if used == 0 {
            output[0] = byte;
            used = 1;
        } else if code != previous && code != 0 {
            output[used] = code;
            used += 1;
            if used == output.len() {
                break;
            }
        }
        previous = code;
    }
    output
}

fn vowel(byte: u8) -> bool {
    b"AEIOU".contains(&byte)
}

fn soft(byte: u8) -> bool {
    b"EIY".contains(&byte)
}

fn metaphone_bytes(input: &[u8], limit: usize, line: usize) -> Result<Vec<u8>, Error> {
    let capacity = if limit == 0 { input.len() } else { limit };
    let mut output = Vec::new();
    output
        .try_reserve_exact(capacity.saturating_add(1))
        .map_err(|_| {
            Error::new(
                ErrorKind::Error,
                "Unable to allocate metaphone output",
                line,
            )
        })?;
    // PHP's Metaphone scans C-string termination; Soundex uses the byte length.
    let word = input.split(|&byte| byte == 0).next().unwrap();
    let Some(mut index) = word.iter().position(u8::is_ascii_alphabetic) else {
        return Ok(output);
    };
    let at = |index: usize| word.get(index).copied().unwrap_or(0).to_ascii_uppercase();
    let first = at(index);
    let next = at(index + 1);
    match first {
        b'A' => {
            output.push(if next == b'E' { b'E' } else { b'A' });
            index += if next == b'E' { 2 } else { 1 };
        }
        b'G' | b'K' | b'P' if next == b'N' => {
            output.push(b'N');
            index += 2;
        }
        b'W' if next == b'R' || next == b'H' || vowel(next) => {
            output.push(if next == b'R' { b'R' } else { b'W' });
            index += 2;
        }
        b'X' => {
            output.push(b'S');
            index += 1;
        }
        b'E' | b'I' | b'O' | b'U' => {
            output.push(first);
            index += 1;
        }
        _ => {}
    }
    // The limit is checked between input letters, so X's two-byte expansion
    // can exceed it by one. PHP always uses the traditional algorithm.
    while index < word.len() && (limit == 0 || output.len() < limit) {
        let byte = at(index);
        let previous = index.checked_sub(1).map_or(0, at);
        let next = at(index + 1);
        let after_next = at(index + 2);
        let mut skip = 0;
        if !byte.is_ascii_uppercase() || (byte == previous && byte != b'C') {
            index += 1;
            continue;
        }
        match byte {
            b'B' if previous != b'M' => output.push(b'B'),
            b'C' => {
                if soft(next) {
                    if next == b'I' && after_next == b'A' {
                        output.push(b'X');
                    } else if previous != b'S' {
                        output.push(b'S');
                    }
                } else if next == b'H' {
                    output.push(b'X');
                    skip = 1;
                } else {
                    output.push(b'K');
                }
            }
            b'D' => {
                if next == b'G' && soft(after_next) {
                    output.push(b'J');
                    skip = 1;
                } else {
                    output.push(b'T');
                }
            }
            b'G' => {
                if next == b'H' {
                    let back3 = index.checked_sub(3).map_or(0, at);
                    let back4 = index.checked_sub(4).map_or(0, at);
                    if !b"BDH".contains(&back3) && back4 != b'H' {
                        output.push(b'F');
                        skip = 1;
                    }
                } else if next == b'N' {
                    if after_next.is_ascii_uppercase()
                        && !(after_next == b'E' && at(index + 3) == b'D')
                    {
                        output.push(b'K');
                    }
                } else if soft(next) && previous != b'G' {
                    output.push(b'J');
                } else {
                    output.push(b'K');
                }
            }
            b'H' if vowel(next) && !b"CGPST".contains(&previous) => output.push(b'H'),
            b'K' if previous != b'C' => output.push(b'K'),
            b'P' => output.push(if next == b'H' { b'F' } else { b'P' }),
            b'Q' => output.push(b'K'),
            b'S' => {
                if next == b'I' && matches!(after_next, b'O' | b'A') {
                    output.push(b'X');
                } else if next == b'H' {
                    output.push(b'X');
                    skip = 1;
                } else {
                    output.push(b'S');
                }
            }
            b'T' => {
                if next == b'I' && matches!(after_next, b'O' | b'A') {
                    output.push(b'X');
                } else if next == b'H' {
                    output.push(b'0');
                    skip = 1;
                } else if !(next == b'C' && after_next == b'H') {
                    output.push(b'T');
                }
            }
            b'V' => output.push(b'F'),
            b'W' | b'Y' if vowel(next) => output.push(byte),
            b'X' => output.extend_from_slice(b"KS"),
            b'Z' => output.push(b'S'),
            b'F' | b'J' | b'L' | b'M' | b'N' | b'R' => output.push(byte),
            _ => {}
        }
        index += skip + 1;
    }
    Ok(output)
}
