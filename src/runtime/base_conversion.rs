use super::*;

const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";

impl Runtime {
    pub(super) fn base_conversion(
        &mut self,
        kind: builtins::Builtin,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        if let builtins::Builtin::BaseEncode(base) = kind {
            let number = self.native_int(first, name, 1, line)?;
            return Ok(Value::String(encode_integer(number as u64, base).into()));
        }
        let number = self.native_string(first, name, 1, line)?;
        if let builtins::Builtin::BaseDecode(base) = kind {
            return Ok(decode(&number, base, line, &mut self.diagnostics));
        }
        let from = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
        let to = self.native_int(args[2].as_ref().unwrap(), name, 3, line)?;
        // Native argument conversions precede both base-range checks.
        for (base, position, label) in [(from, 2, "from_base"), (to, 3, "to_base")] {
            if !(2..=36).contains(&base) {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!(
                        "base_convert(): Argument #{position} (${label}) must be between 2 and 36 (inclusive)"
                    ),
                    line,
                ));
            }
        }
        let value = decode(&number, from as u8, line, &mut self.diagnostics);
        let bytes = match value {
            Value::Int(n) => encode_integer(n as u64, to as u8),
            Value::Float(n) => encode_float(n, to as u8, line)?,
            _ => unreachable!(),
        };
        Ok(Value::String(bytes.into()))
    }
}

fn decode(bytes: &[u8], base: u8, line: usize, warnings: &mut dyn WarningSink) -> Value {
    let mut bytes = bytes;
    // C's isspace() also includes vertical tab, unlike Rust's ASCII whitespace.
    let whitespace = |byte: &u8| matches!(byte, b' ' | b'\t'..=b'\r');
    while bytes.first().is_some_and(whitespace) {
        bytes = &bytes[1..];
    }
    while bytes.last().is_some_and(whitespace) {
        bytes = &bytes[..bytes.len() - 1];
    }
    if bytes.len() >= 2
        && bytes[0] == b'0'
        && matches!(
            (base, bytes[1]),
            (2, b'b' | b'B') | (8, b'o' | b'O') | (16, b'x' | b'X')
        )
    {
        bytes = &bytes[2..];
    }
    let mut integer = 0_i64;
    let mut float = None;
    let mut invalid = false;
    for &byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'z' => byte - b'a' + 10,
            b'A'..=b'Z' => byte - b'A' + 10,
            _ => u8::MAX,
        };
        if digit >= base {
            invalid = true;
            continue;
        }
        if let Some(n) = float {
            float = Some(n * f64::from(base) + f64::from(digit));
        } else if let Some(n) = integer
            .checked_mul(i64::from(base))
            .and_then(|n| n.checked_add(i64::from(digit)))
        {
            integer = n;
        } else {
            float = Some(integer as f64 * f64::from(base) + f64::from(digit));
        }
    }
    if invalid {
        warnings.push(format!("Deprecated: Invalid characters passed for attempted conversion, these have been ignored on line {line}"));
    }
    float.map(Value::Float).unwrap_or(Value::Int(integer))
}

fn encode_integer(mut value: u64, base: u8) -> Vec<u8> {
    let mut bytes = Vec::new();
    loop {
        bytes.push(DIGITS[(value % u64::from(base)) as usize]);
        value /= u64::from(base);
        if value == 0 {
            break;
        }
    }
    bytes.reverse();
    bytes
}

fn encode_float(value: f64, base: u8, line: usize) -> Result<Vec<u8>, Error> {
    if value.is_infinite() {
        return Err(Error::new(
            ErrorKind::ValueError,
            format!("An infinite value cannot be converted to base {base}"),
            line,
        ));
    }
    let mut value = value.floor();
    let mut bytes = Vec::new();
    // PHP uses a 64-digit buffer and successive floating division (without
    // flooring each quotient), including its truncation/rounding behavior.
    loop {
        bytes.push(DIGITS[(value % f64::from(base)) as usize]);
        value /= f64::from(base);
        if value < 1.0 || bytes.len() == 64 {
            break;
        }
    }
    bytes.reverse();
    Ok(bytes)
}
