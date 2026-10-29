use super::*;
use crate::value::numeric_string;

impl Runtime {
    pub(super) fn compare(
        &mut self,
        left: &Value,
        right: &Value,
        line: usize,
    ) -> Result<Option<Ordering>, Error> {
        self.compare_inner(left, right, line, &mut HashSet::new())
    }

    fn compare_inner(
        &mut self,
        left: &Value,
        right: &Value,
        line: usize,
        seen: &mut HashSet<(usize, usize)>,
    ) -> Result<Option<Ordering>, Error> {
        use Value::*;
        if matches!(left, Value::Reference(_) | Value::Indirect(_))
            || matches!(right, Value::Reference(_) | Value::Indirect(_))
        {
            return self.compare_inner(&left.dereferenced(), &right.dereferenced(), line, seen);
        }
        Ok(match (left, right) {
            (Reference(_) | Indirect(_), _) | (_, Reference(_) | Indirect(_)) => unreachable!(),
            (Bool(_), _) | (_, Bool(_)) => left.is_truthy().partial_cmp(&right.is_truthy()),
            (Null, String(s)) => b"".as_slice().partial_cmp(s.bytes().as_slice()),
            (String(s), Null) => s.bytes().as_slice().partial_cmp(b"".as_slice()),
            (Null, _) | (_, Null) => left.is_truthy().partial_cmp(&right.is_truthy()),
            (Object(a), Object(b)) => {
                if Rc::ptr_eq(a, b) {
                    return Ok(Some(Ordering::Equal));
                }
                if a.class.declaration.is_enum
                    || b.class.declaration.is_enum
                    || !a.class.name.eq_ignore_ascii_case(&b.class.name)
                {
                    return Ok(None);
                }
                let pair = (Rc::as_ptr(a) as usize, Rc::as_ptr(b) as usize);
                if !seen.insert(pair) {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Nesting level too deep - recursive dependency?",
                        line,
                    ));
                }
                let result = self.compare_arrays(&a.snapshot(), &b.snapshot(), line, seen);
                seen.remove(&pair);
                result?
            }
            (Object(_), String(_)) if self.can_stringify(left) => {
                let converted = String(self.string_bytes(left, line)?.into());
                self.compare_inner(&converted, right, line, seen)?
            }
            (String(_), Object(_)) if self.can_stringify(right) => {
                let converted = String(self.string_bytes(right, line)?.into());
                self.compare_inner(left, &converted, line, seen)?
            }
            (Object(_) | Closure(_), Int(_) | Float(_)) => {
                let kind = if matches!(right, Int(_)) {
                    "int"
                } else {
                    "float"
                };
                self.diagnostics.push(format!(
                    "Notice: Object of class {} could not be converted to {kind} on line {line}",
                    builtins::debug_type(left)
                ));
                self.compare_inner(&Int(1), right, line, seen)?
            }
            (Int(_) | Float(_), Object(_) | Closure(_)) => {
                let kind = if matches!(left, Int(_)) {
                    "int"
                } else {
                    "float"
                };
                self.diagnostics.push(format!(
                    "Notice: Object of class {} could not be converted to {kind} on line {line}",
                    builtins::debug_type(right)
                ));
                self.compare_inner(left, &Int(1), line, seen)?
            }
            (Object(_), Closure(_)) | (Closure(_), Object(_)) => None,
            (Object(_), _) => Some(Ordering::Greater),
            (_, Object(_)) => Some(Ordering::Less),
            (Closure(a), Closure(b)) => a.same_function(b).then_some(Ordering::Equal),
            (Closure(_), _) => Some(Ordering::Greater),
            (_, Closure(_)) => Some(Ordering::Less),
            (Array(a), Array(b)) => {
                if Rc::ptr_eq(a, b) {
                    return Ok(Some(Ordering::Equal));
                }
                let pair = (Rc::as_ptr(a) as usize, 0);
                if !seen.insert(pair) {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Nesting level too deep - recursive dependency?",
                        line,
                    ));
                }
                let result = self.compare_arrays(a, b, line, seen);
                seen.remove(&pair);
                result?
            }
            (Array(_), _) => Some(Ordering::Greater),
            (_, Array(_)) => Some(Ordering::Less),
            (Resource(resource), _) => {
                let other = resource_number(right);
                self.compare_inner(&Int(resource.id), &other, line, seen)?
            }
            (_, Resource(resource)) => {
                let other = resource_number(left);
                self.compare_inner(&other, &Int(resource.id), line, seen)?
            }
            (Int(a), Int(b)) => a.partial_cmp(b),
            (Int(_) | Float(_), Int(_) | Float(_)) => {
                left.as_float().partial_cmp(&right.as_float())
            }
            (Float(n), String(_)) | (String(_), Float(n)) if n.is_nan() => None,
            (String(a), String(b)) => {
                match (numeric_string(&a.bytes()), numeric_string(&b.bytes())) {
                    (Some(a), Some(b)) => self.compare_inner(&a, &b, line, seen)?,
                    _ => a.bytes().partial_cmp(&b.bytes()),
                }
            }
            (String(a), _) => match numeric_string(&a.bytes()) {
                Some(a) => self.compare_inner(&a, right, line, seen)?,
                None => a.bytes().partial_cmp(&right.to_bytes()),
            },
            (_, String(b)) => match numeric_string(&b.bytes()) {
                Some(b) => self.compare_inner(left, &b, line, seen)?,
                None => left.to_bytes().partial_cmp(&b.bytes()),
            },
        })
    }

    fn compare_arrays(
        &mut self,
        left: &PhpArray,
        right: &PhpArray,
        line: usize,
        seen: &mut HashSet<(usize, usize)>,
    ) -> Result<Option<Ordering>, Error> {
        if left.len() != right.len() {
            return Ok(left.len().partial_cmp(&right.len()));
        }
        for (key, value) in left.entries() {
            let Some(other) = right.get(key) else {
                return Ok(Some(Ordering::Greater));
            };
            let order = self.compare_inner(value, other, line, seen)?;
            if order != Some(Ordering::Equal) {
                return Ok(order);
            }
        }
        Ok(Some(Ordering::Equal))
    }
}

fn resource_number(value: &Value) -> Value {
    match value {
        Value::Resource(resource) => Value::Int(resource.id),
        Value::String(bytes) => {
            crate::value::numeric_prefix(&bytes.bytes()).map_or(Value::Int(0), |(number, _)| number)
        }
        _ => value.clone(),
    }
}
