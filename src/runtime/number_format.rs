use super::*;

impl Runtime {
    pub(super) fn number_format(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let mut value = self.native_number(
            args[0].as_ref().unwrap(),
            "number_format",
            1,
            "int|float",
            line,
        )?;
        let decimals = args[1]
            .as_ref()
            .map_or(Ok(0), |n| self.native_int(n, "number_format", 2, line))?;
        let decimal = match args[2].as_ref() {
            None | Some(Value::Null) => b".".to_vec(),
            Some(n) => self.native_string_type(n, "number_format", 3, line, "?string")?,
        };
        let thousands = match args[3].as_ref() {
            None | Some(Value::Null) => b",".to_vec(),
            Some(n) => self.native_string_type(n, "number_format", 4, line, "?string")?,
        };
        // Doubles at this magnitude no longer have a fractional part. PHP uses
        // its integer formatter when they fit a long, preserving decimal rounding.
        if let Value::Float(n) = value
            && n.abs() >= 4503599627370496.0
            && (-9223372036854775808.0..9223372036854775808.0).contains(&n)
        {
            value = Value::Int(n as i64);
        }
        let (digits, fraction, decimals, negative) = match value {
            Value::Int(n) => {
                let mut magnitude = n.unsigned_abs();
                if decimals < -19 {
                    magnitude = 0;
                } else if decimals < 0 {
                    let power = 10_u64.pow((-decimals) as u32);
                    let rest = magnitude % power;
                    magnitude -= rest;
                    if rest >= power / 2 {
                        magnitude += power;
                    }
                }
                (
                    magnitude.to_string().into_bytes(),
                    Vec::new(),
                    decimals.max(0) as usize,
                    n < 0 && magnitude != 0,
                )
            }
            Value::Float(n) => {
                if !n.is_finite() {
                    return Ok(Value::String(
                        if n.is_nan() {
                            b"nan".to_vec()
                        } else {
                            b"inf".to_vec()
                        }
                        .into(),
                    ));
                }
                let places = decimals.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
                let rounded = rounding::decimal_round(n.abs(), places, 1);
                let decimals = places.max(0) as usize;
                // php_conv_fp limits fixed conversion to 318 decimal places;
                // number_format pads any remaining requested places with zeros.
                let text = format!("{rounded:.*}", decimals.min(318));
                let (digits, fraction) = text.split_once('.').unwrap_or((&text, ""));
                (
                    digits.as_bytes().to_vec(),
                    fraction.as_bytes().to_vec(),
                    decimals,
                    n < 0.0 && rounded != 0.0,
                )
            }
            _ => unreachable!(),
        };
        let separators = (digits.len() - 1) / 3;
        let length = separators
            .checked_mul(thousands.len())
            .and_then(|n| n.checked_add(digits.len()))
            .and_then(|n| n.checked_add(usize::from(negative)))
            .and_then(|n| n.checked_add(decimals))
            .and_then(|n| n.checked_add(if decimals > 0 { decimal.len() } else { 0 }))
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    "Possible integer overflow in memory allocation",
                    line,
                )
            })?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
        if negative {
            output.push(b'-');
        }
        for (index, byte) in digits.iter().enumerate() {
            if index != 0 && (digits.len() - index).is_multiple_of(3) {
                output.extend_from_slice(&thousands);
            }
            output.push(*byte);
        }
        if decimals > 0 {
            output.extend_from_slice(&decimal);
            output.extend_from_slice(&fraction);
            output.resize(length, b'0');
        }
        Ok(Value::String(output.into()))
    }
}
