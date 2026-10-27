use super::*;
use crate::{
    ast::CastKind,
    value::{float_to_int, numeric_prefix},
};

pub(super) fn cast(
    kind: CastKind,
    value: Value,
    line: usize,
    warnings: &mut Vec<String>,
) -> Result<Value, Error> {
    Ok(match kind {
        CastKind::Void => Value::Null,
        CastKind::Bool => Value::Bool(value.to_bool(line, warnings)),
        CastKind::String => Value::String(string_bytes(&value, line, warnings)),
        CastKind::Array => match value {
            Value::Array(_) => value,
            value => {
                value.warn_nan("array", line, warnings);
                let mut array = PhpArray::default();
                if value != Value::Null {
                    array.append(value, line)?;
                }
                Value::Array(Rc::new(array))
            }
        },
        CastKind::Int => {
            Value::Int(match &value {
                Value::Int(number) => *number,
                Value::Float(number) => {
                    if out_of_range(*number) {
                        warnings.push(format!("The float {} is not representable as an int, cast occurred on line {line}", String::from_utf8_lossy(&value.to_bytes())));
                    }
                    float_to_int(*number)
                }
                Value::String(bytes) => match numeric_prefix(bytes) {
                    Some((Value::Int(number), _)) => number,
                    Some((Value::Float(number), _)) => {
                        if out_of_range(number) {
                            warnings.push(format!("The float-string \"{}\" is not representable as an int, cast occurred on line {line}", String::from_utf8_lossy(bytes)));
                        }
                        if number.is_finite() { number as i64 } else { 0 }
                    }
                    _ => 0,
                },
                _ => i64::from(value.is_truthy()),
            })
        }
        CastKind::Float => Value::Float(match &value {
            Value::Float(number) => *number,
            Value::Int(number) => *number as f64,
            Value::String(bytes) => match numeric_prefix(bytes) {
                Some((number, _)) => {
                    let number = number.as_float();
                    if number == 0.0
                        && bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'-')
                    {
                        -0.0
                    } else {
                        number
                    }
                }
                _ => 0.0,
            },
            _ => {
                if value.is_truthy() {
                    1.0
                } else {
                    0.0
                }
            }
        }),
    })
}

fn out_of_range(number: f64) -> bool {
    !number.is_finite() || !(-9223372036854775808.0..9223372036854775808.0).contains(&number)
}
