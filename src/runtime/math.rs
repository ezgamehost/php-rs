use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum Math {
    Abs,
    Ceil,
    Floor,
    Pow,
    Fpow,
    Fdiv,
    Fmod,
    Intdiv,
    Sqrt,
    IsFinite,
    IsInfinite,
    IsNan,
}

impl Math {
    pub(super) fn parameters(self) -> &'static [(&'static str, bool, bool)] {
        match self {
            Self::Pow | Self::Fpow => &[("num", false, false), ("exponent", false, false)],
            Self::Fdiv | Self::Fmod | Self::Intdiv => {
                &[("num1", false, false), ("num2", false, false)]
            }
            _ => &[("num", false, false)],
        }
    }
}

impl Runtime {
    pub(super) fn math(
        &mut self,
        kind: Math,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        if matches!(kind, Math::Pow) {
            return power(
                first,
                args[1].as_ref().unwrap(),
                line,
                &mut self.diagnostics,
            );
        }
        if matches!(kind, Math::Intdiv) {
            let a = self.native_int(first, name, 1, line)?;
            let b = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
            return if b == 0 {
                Err(Error::new(
                    ErrorKind::DivisionByZeroError,
                    "Division by zero",
                    line,
                ))
            } else if a == i64::MIN && b == -1 {
                Err(Error::new(
                    ErrorKind::ArithmeticError,
                    "Division of PHP_INT_MIN by -1 is not an integer",
                    line,
                ))
            } else {
                Ok(Value::Int(a / b))
            };
        }
        if matches!(kind, Math::Abs | Math::Ceil | Math::Floor) {
            let number = self.native_number(first, name, 1, "int|float", line)?;
            return Ok(match (kind, number) {
                (Math::Abs, Value::Int(n)) => n
                    .checked_abs()
                    .map(Value::Int)
                    .unwrap_or(Value::Float(-(n as f64))),
                (Math::Abs, Value::Float(n)) => Value::Float(n.abs()),
                (Math::Ceil, n) => Value::Float(n.as_float().ceil()),
                (Math::Floor, n) => Value::Float(n.as_float().floor()),
                _ => unreachable!(),
            });
        }
        let a = self.native_float(first, name, 1, line)?;
        Ok(match kind {
            Math::Sqrt => Value::Float(a.sqrt()),
            Math::IsFinite => Value::Bool(a.is_finite()),
            Math::IsInfinite => Value::Bool(a.is_infinite()),
            Math::IsNan => Value::Bool(a.is_nan()),
            Math::Fpow | Math::Fdiv | Math::Fmod => {
                let b = self.native_float(args[1].as_ref().unwrap(), name, 2, line)?;
                Value::Float(match kind {
                    Math::Fpow => a.powf(b),
                    Math::Fdiv => a / b,
                    Math::Fmod => a % b,
                    _ => unreachable!(),
                })
            }
            _ => unreachable!(),
        })
    }

    pub(super) fn native_float(
        &mut self,
        value: &Value,
        name: &str,
        position: usize,
        line: usize,
    ) -> Result<f64, Error> {
        self.native_number(value, name, position, "float", line)
            .map(|n| n.as_float())
    }

    pub(super) fn native_number(
        &mut self,
        value: &Value,
        name: &str,
        position: usize,
        expected: &str,
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(value, Value::Int(_) | Value::Float(_)) {
            return Ok(value.clone());
        }
        if !self.strict_types {
            match value {
                Value::Null => {
                    self.warn_null(value, name, position, expected, line);
                    return Ok(Value::Int(0));
                }
                Value::Bool(n) => return Ok(Value::Int(i64::from(*n))),
                Value::String(bytes) => {
                    if let Some(number) = crate::value::numeric_string(&bytes.bytes()) {
                        return Ok(number);
                    }
                }
                _ => {}
            }
        }
        Err(builtins::type_error(value, name, position, expected, line))
    }
}

// pow() and ** share PHP's operator conversions, integer result rules, and
// zero-base deprecation. fpow() uses native float arguments and IEEE semantics.
pub(super) fn power(
    left: &Value,
    right: &Value,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Value, Error> {
    let invalid = |_| {
        Error::new(
            ErrorKind::TypeError,
            format!(
                "Unsupported operand types: {} ** {}",
                builtins::debug_type(left),
                builtins::debug_type(right)
            ),
            line,
        )
    };
    let a = left.number(line, warnings).map_err(invalid)?;
    let b = right.number(line, warnings).map_err(invalid)?;
    if let (Value::Int(base), Value::Int(exponent)) = (&a, &b)
        && *exponent >= 0
    {
        let mut result = 1_i64;
        let mut base = *base;
        let mut exponent = *exponent;
        // Keep PHP's multiplication order when promotion to float is necessary;
        // recomputing pow(original_base, original_exponent) can round differently.
        while exponent > 0 {
            if exponent % 2 != 0 {
                exponent -= 1;
                match result.checked_mul(base) {
                    Some(product) => result = product,
                    None => {
                        return Ok(Value::Float(
                            (result as f64 * base as f64) * (base as f64).powf(exponent as f64),
                        ));
                    }
                }
            } else {
                exponent /= 2;
                match base.checked_mul(base) {
                    Some(square) => base = square,
                    None => {
                        return Ok(Value::Float(
                            result as f64 * (base as f64 * base as f64).powf(exponent as f64),
                        ));
                    }
                }
            }
        }
        return Ok(Value::Int(result));
    }
    let a = a.as_float();
    let b = b.as_float();
    if a == 0.0 && b < 0.0 {
        warnings.push(format!(
            "Deprecated: Power of base 0 and negative exponent is deprecated on line {line}"
        ));
    }
    Ok(Value::Float(a.powf(b)))
}
