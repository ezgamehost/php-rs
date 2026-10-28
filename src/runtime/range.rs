use super::*;
use crate::value::numeric_string;

const MAX_SIZE: u64 = 0x40000000;

#[derive(Clone, Copy)]
enum Boundary {
    Int(i64),
    Float(f64),
    Character(u8),
    Digit(u8),
}
impl Boundary {
    fn character(self) -> Option<u8> {
        match self {
            Self::Character(byte) | Self::Digit(byte) => Some(byte),
            _ => None,
        }
    }
    fn integer(self) -> i64 {
        match self {
            Self::Int(n) => n,
            Self::Digit(byte) => i64::from(byte - b'0'),
            Self::Character(_) => 0,
            Self::Float(_) => unreachable!(),
        }
    }
    fn number(self) -> f64 {
        match self {
            Self::Float(n) => n,
            other => other.integer() as f64,
        }
    }
}

impl Runtime {
    pub(super) fn range(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        // Native argument conversion finishes before semantic step/endpoint checks.
        let start = self.range_argument(args[0].as_ref().unwrap(), 1, true, line)?;
        let end = self.range_argument(args[1].as_ref().unwrap(), 2, true, line)?;
        let step =
            self.range_argument(args[2].as_ref().unwrap_or(&Value::Int(1)), 3, false, line)?;
        let (step, integer_step, negative, float_step) = match step {
            Value::Int(step) => {
                if step == i64::MIN {
                    return Err(argument_error(
                        3,
                        "must be greater than -9223372036854775808",
                        line,
                    ));
                }
                (
                    step.unsigned_abs() as f64,
                    step.unsigned_abs(),
                    step < 0,
                    false,
                )
            }
            Value::Float(step) => {
                finite(step, 3, line)?;
                let magnitude = step.abs();
                let fractional = magnitude.fract() != 0.0 || magnitude >= 9223372036854775808.0;
                (magnitude, magnitude as u64, step < 0.0, fractional)
            }
            _ => unreachable!(),
        };
        if step == 0.0 {
            return Err(argument_error(3, "cannot be 0", line));
        }
        let start = self.range_boundary(start, 1, line)?;
        let end = self.range_boundary(end, 2, line)?;
        match (start.character(), end.character()) {
            (Some(start_byte), Some(end_byte)) if !float_step => {
                return integer_range(
                    i64::from(start_byte),
                    i64::from(end_byte),
                    integer_step,
                    negative,
                    true,
                    line,
                );
            }
            (Some(_), Some(_)) => {
                if matches!(start, Boundary::Character(_)) || matches!(end, Boundary::Character(_))
                {
                    self.range_warning("Argument #3 ($step) must be of type int when generating an array of characters, inputs converted to 0", line);
                }
            }
            (Some(_), None) if matches!(start, Boundary::Character(_)) => {
                self.range_warning("Argument #2 ($end) must be a single byte string if argument #1 ($start) is a single byte string, argument #1 ($start) converted to 0", line);
            }
            (None, Some(_)) if matches!(end, Boundary::Character(_)) => {
                self.range_warning("Argument #1 ($start) must be a single byte string if argument #2 ($end) is a single byte string, argument #2 ($end) converted to 0", line);
            }
            _ => {}
        }
        if float_step || matches!(start, Boundary::Float(_)) || matches!(end, Boundary::Float(_)) {
            float_range(start.number(), end.number(), step, negative, line)
        } else {
            integer_range(
                start.integer(),
                end.integer(),
                integer_step,
                negative,
                false,
                line,
            )
        }
    }

    fn range_argument(
        &mut self,
        value: &Value,
        position: usize,
        string: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(value, Value::Int(_) | Value::Float(_))
            || string && matches!(value, Value::String(_))
        {
            return Ok(value.clone());
        }
        let kind = if string {
            "string|int|float"
        } else {
            "int|float"
        };
        if !self.strict_types {
            match value {
                Value::Null => {
                    self.warn_null(value, "range", position, kind, line);
                    return Ok(Value::Int(0));
                }
                Value::Bool(value) => return Ok(Value::Int(i64::from(*value))),
                Value::String(bytes) => {
                    if let Some(number) = numeric_string(&bytes.bytes()) {
                        return Ok(number);
                    }
                }
                Value::Object(_) if string && self.can_stringify(value) => {
                    return self
                        .string_bytes(value, line)
                        .map(|bytes| Value::String(bytes.into()));
                }
                _ => {}
            }
        }
        Err(builtins::type_error(value, "range", position, kind, line))
    }

    fn range_boundary(
        &mut self,
        value: Value,
        position: usize,
        line: usize,
    ) -> Result<Boundary, Error> {
        match value {
            Value::Int(value) => Ok(Boundary::Int(value)),
            Value::Float(value) => {
                finite(value, position, line)?;
                Ok(Boundary::Float(value))
            }
            Value::String(bytes) => {
                let bytes = bytes.bytes();
                if bytes.is_empty() {
                    self.range_warning(
                        &format!(
                            "Argument #{position} (${}) must not be empty, casted to 0",
                            parameter(position)
                        ),
                        line,
                    );
                    return Ok(Boundary::Int(0));
                }
                match numeric_string(&bytes) {
                    Some(Value::Int(value)) => Ok(if bytes.len() == 1 {
                        Boundary::Digit(bytes[0])
                    } else {
                        Boundary::Int(value)
                    }),
                    Some(Value::Float(value)) => {
                        finite(value, position, line)?;
                        Ok(Boundary::Float(value))
                    }
                    None => {
                        if bytes.len() != 1 {
                            self.range_warning(&format!("Argument #{position} (${}) must be a single byte, subsequent bytes are ignored", parameter(position)), line);
                        }
                        Ok(Boundary::Character(bytes[0]))
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }
    }

    fn range_warning(&mut self, message: &str, line: usize) {
        self.diagnostics
            .push(format!("range(): {message} on line {line}"));
    }
}

fn integer_range(
    start: i64,
    end: i64,
    step: u64,
    negative: bool,
    characters: bool,
    line: usize,
) -> Result<Value, Error> {
    if start < end && negative {
        return Err(negative_step(line));
    }
    let distance = (i128::from(start) - i128::from(end)).unsigned_abs() as u64;
    if distance != 0 && distance < step {
        return Err(boundary_error(line));
    }
    let intervals = distance / step;
    if intervals >= MAX_SIZE - 1 {
        return Err(Error::new(
            ErrorKind::ValueError,
            format!(
                "The supplied range exceeds the maximum array size by {} elements: start={}, end={}, step={step}. Calculated size: {intervals}. Maximum size: {MAX_SIZE}.",
                intervals - (MAX_SIZE - 1),
                start.min(end),
                start.max(end)
            ),
            line,
        ));
    }
    let size = intervals as usize + 1;
    let mut result = PhpArray::with_capacity(size, line)?;
    for index in 0..size {
        let distance = index as i128 * i128::from(step);
        let value = if start > end {
            i128::from(start) - distance
        } else {
            i128::from(start) + distance
        };
        result.append(
            if characters {
                Value::String(vec![value as u8].into())
            } else {
                Value::Int(value as i64)
            },
            line,
        )?;
    }
    Ok(Value::Array(Rc::new(result)))
}

fn float_range(
    start: f64,
    end: f64,
    step: f64,
    negative: bool,
    line: usize,
) -> Result<Value, Error> {
    if start < end && negative {
        return Err(negative_step(line));
    }
    let distance = (start - end).abs();
    if distance != 0.0 && distance < step {
        return Err(boundary_error(line));
    }
    let size = distance / step + 1.0;
    if size >= MAX_SIZE as f64 {
        return Err(Error::new(
            ErrorKind::ValueError,
            format!(
                "The supplied range exceeds the maximum array size by {:.1} elements: start={:.1}, end={:.1}, step={step:.1}. Max size: {MAX_SIZE}",
                size - MAX_SIZE as f64,
                start.min(end),
                start.max(end)
            ),
            line,
        ));
    }
    let size = size.round() as usize;
    let mut result = PhpArray::with_capacity(size, line)?;
    for index in 0..size {
        let element = if index == 0 {
            start
        } else if start > end {
            start - index as f64 * step
        } else {
            start + index as f64 * step
        };
        if start > end && element < end || start < end && element > end {
            break;
        }
        result.append(Value::Float(element), line)?;
    }
    Ok(Value::Array(Rc::new(result)))
}

fn parameter(position: usize) -> &'static str {
    match position {
        1 => "start",
        2 => "end",
        3 => "step",
        _ => unreachable!(),
    }
}
fn argument_error(position: usize, message: &str, line: usize) -> Error {
    Error::new(
        ErrorKind::ValueError,
        format!(
            "range(): Argument #{position} (${}) {message}",
            parameter(position)
        ),
        line,
    )
}
fn finite(value: f64, position: usize, line: usize) -> Result<(), Error> {
    if value.is_infinite() {
        Err(argument_error(
            position,
            "must be a finite number, INF provided",
            line,
        ))
    } else if value.is_nan() {
        Err(argument_error(
            position,
            "must be a finite number, NAN provided",
            line,
        ))
    } else {
        Ok(())
    }
}
fn negative_step(line: usize) -> Error {
    argument_error(3, "must be greater than 0 for increasing ranges", line)
}
fn boundary_error(line: usize) -> Error {
    argument_error(
        3,
        "must be less than the range spanned by argument #1 ($start) and argument #2 ($end)",
        line,
    )
}
