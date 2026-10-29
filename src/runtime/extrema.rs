use super::*;

#[derive(Clone, Copy)]
enum NumericMode {
    Integer,
    Float,
    Generic,
}

impl Runtime {
    pub(super) fn extremum(
        &mut self,
        maximum: bool,
        name: &str,
        first: &Value,
        extra: &PhpArray,
        direct_positional: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if extra
            .entries()
            .iter()
            .any(|(key, _)| matches!(key, ArrayKey::String(_)))
        {
            return Err(Error::new(
                ErrorKind::ArgumentCountError,
                format!("{name}() does not accept unknown named parameters"),
                line,
            ));
        }
        if extra.is_empty() {
            let array = builtins::require_array(first, name, 1, line)?;
            let Some(mut best) = array.entries().first().map(|(_, value)| value) else {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!("{name}(): Argument #1 ($value) must contain at least one element"),
                    line,
                ));
            };
            for (_, candidate) in array.entries() {
                // Hash-table extrema compare the incumbent to the candidate,
                // unlike the variadic path. Uncomparable values map to PHP's 1.
                let order = self
                    .compare(best, candidate, line)?
                    .unwrap_or(Ordering::Greater);
                if order
                    == if maximum {
                        Ordering::Less
                    } else {
                        Ordering::Greater
                    }
                {
                    best = candidate;
                }
            }
            return Ok(best.dereferenced());
        }
        if direct_positional && extra.len() == 1 {
            let second = &extra.entries()[0].1;
            let order = if numeric_pair(first, second) {
                if let (Value::Int(a), Value::Int(b)) = (first, second) {
                    Some(a.cmp(b))
                } else {
                    first.as_float().partial_cmp(&second.as_float())
                }
            } else {
                Some(
                    self.compare(first, second, line)?
                        .unwrap_or(Ordering::Greater),
                )
            };
            // PHP's frameless min chooses the right argument on equality;
            // frameless max keeps the left. Both choose right for numeric NaN.
            let left = if maximum {
                matches!(order, Some(Ordering::Greater | Ordering::Equal))
            } else {
                order == Some(Ordering::Less)
            };
            return Ok(if left { first } else { second }.dereferenced());
        }
        let mut best = first;
        let mut mode = match first {
            Value::Int(_) => NumericMode::Integer,
            Value::Float(_) => NumericMode::Float,
            _ => NumericMode::Generic,
        };
        for (_, candidate) in extra.entries() {
            let order = match (mode, candidate) {
                (NumericMode::Integer, Value::Int(n)) => {
                    let Value::Int(best) = best else {
                        unreachable!()
                    };
                    Some(n.cmp(best))
                }
                (NumericMode::Integer, Value::Float(_)) if exact_float_integer(best) => {
                    mode = NumericMode::Float;
                    candidate.as_float().partial_cmp(&best.as_float())
                }
                (NumericMode::Float, Value::Float(_)) => {
                    candidate.as_float().partial_cmp(&best.as_float())
                }
                (NumericMode::Float, Value::Int(_)) if exact_float_integer(candidate) => {
                    candidate.as_float().partial_cmp(&best.as_float())
                }
                _ => {
                    // Once PHP falls back to generic comparison it stays there.
                    mode = NumericMode::Generic;
                    Some(
                        self.compare(candidate, best, line)?
                            .unwrap_or(Ordering::Greater),
                    )
                }
            };
            if order
                == Some(if maximum {
                    Ordering::Greater
                } else {
                    Ordering::Less
                })
            {
                best = candidate;
            }
        }
        Ok(best.dereferenced())
    }
}

fn exact_float_integer(value: &Value) -> bool {
    matches!(value, Value::Int(n) if crate::value::float_to_int(*n as f64) == *n)
}

fn numeric_pair(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Int(_), Value::Int(_)) | (Value::Float(_), Value::Float(_)) => true,
        (Value::Int(_), Value::Float(_)) => exact_float_integer(left),
        (Value::Float(_), Value::Int(_)) => exact_float_integer(right),
        _ => false,
    }
}
