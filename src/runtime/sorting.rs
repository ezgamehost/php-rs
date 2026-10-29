use super::*;
mod algorithm;
mod natural;

#[derive(Debug, Clone, Copy)]
pub(super) enum Sort {
    Values,
    ReverseValues,
    Associative,
    ReverseAssociative,
    Keys,
    ReverseKeys,
    Natural,
    NaturalCase,
}
impl Sort {
    fn keys(self) -> bool {
        matches!(self, Self::Keys | Self::ReverseKeys)
    }
    fn reverse(self) -> bool {
        matches!(
            self,
            Self::ReverseValues | Self::ReverseAssociative | Self::ReverseKeys
        )
    }
    fn renumber(self) -> bool {
        matches!(self, Self::Values | Self::ReverseValues)
    }
}

impl Runtime {
    pub(super) fn array_sort(
        &mut self,
        kind: Sort,
        name: &str,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        line: usize,
    ) -> Result<Value, Error> {
        let mut args = args.into_iter();
        let first = args.next().unwrap().unwrap();
        builtins::require_array(&first, name, 1, line)?;
        let source = if let Some(reference) = &references[0] {
            drop(first);
            reference.clone()
        } else {
            cell(first)
        };
        // Native array-by-reference parsing separates before parsing the flags.
        let array = with_array(&mut source.borrow_mut(), |array| {
            Rc::make_mut(array);
            array.clone()
        })
        .unwrap();
        let flags = match kind {
            Sort::Natural => 6,
            Sort::NaturalCase => 14,
            _ => match args.next().flatten() {
                Some(value) => self.native_int(&value, name, 2, line)?,
                None => 0,
            },
        };
        let mut order: Vec<_> = (0..array.len()).collect();
        let mut error = None;
        algorithm::sort(&mut order, &mut |&a, &b| {
            let (a_key, a_value) = &array.entries()[a];
            let (b_key, b_value) = &array.entries()[b];
            let comparison = if kind.keys() {
                // Numeric key sorting preserves exact integer comparison.
                if flags & !8 == 1
                    && let (ArrayKey::Int(a), ArrayKey::Int(b)) = (a_key, b_key)
                {
                    a.cmp(b)
                } else {
                    self.sort_compare(
                        &a_key.to_value(),
                        &b_key.to_value(),
                        flags,
                        line,
                        &mut error,
                    )
                }
            } else {
                self.sort_compare(a_value, b_value, flags, line, &mut error)
            };
            let comparison = if kind.reverse() {
                comparison.reverse()
            } else {
                comparison
            };
            // Stable ties retain insertion order in both ascending and descending sorts.
            comparison.then_with(|| a.cmp(&b))
        });
        let identity = Rc::as_ptr(&array);
        drop(array);
        with_array(&mut source.borrow_mut(), |array| {
            // A Stringable conversion can replace or separate the caller's array.
            // The sort belongs to the original table, not that replacement.
            if Rc::as_ptr(array) == identity {
                Rc::make_mut(array).reorder(&order, kind.renumber());
            }
        });
        match error {
            Some(error) => Err(error),
            None => Ok(Value::Bool(true)),
        }
    }

    fn sort_compare(
        &mut self,
        a: &Value,
        b: &Value,
        flags: i64,
        line: usize,
        error: &mut Option<Error>,
    ) -> Ordering {
        let a = a.dereferenced();
        let b = b.dereferenced();
        match flags & !8 {
            1 => {
                let a = self.sort_number(&a, line);
                let b = self.sort_number(&b, line);
                a.partial_cmp(&b).unwrap_or(Ordering::Greater)
            }
            2 | 5 | 6 => {
                let a = self.sort_string(&a, line, error);
                let b = self.sort_string(&b, line, error);
                match flags & !8 {
                    6 => natural::compare(&a, &b, flags & 8 != 0),
                    // The runtime currently has only the C locale. strcoll in
                    // that locale is byte ordering up to the first NUL.
                    5 => a
                        .split(|&c| c == 0)
                        .next()
                        .cmp(&b.split(|&c| c == 0).next()),
                    _ if flags & 8 != 0 => a
                        .iter()
                        .map(u8::to_ascii_lowercase)
                        .cmp(b.iter().map(u8::to_ascii_lowercase)),
                    _ => a.cmp(&b),
                }
            }
            _ => {
                let failed_cast = match (&a, &b) {
                    (Value::Object(_), Value::String(_)) if self.can_stringify(&a) => {
                        Some(Ordering::Greater)
                    }
                    (Value::String(_), Value::Object(_)) if self.can_stringify(&b) => {
                        Some(Ordering::Less)
                    }
                    _ => None,
                };
                if error.is_some()
                    && let Some(order) = failed_cast
                {
                    return order;
                }
                let comparison = match self.compare(&a, &b, line) {
                    Ok(comparison) => comparison,
                    Err(failure) => {
                        if error.is_none() {
                            *error = Some(failure);
                        }
                        return failed_cast.unwrap_or(Ordering::Greater);
                    }
                };
                comparison.unwrap_or_else(|| {
                    // PHP groups enum cases using object identity for regular sorts.
                    if let Value::Object(right) = &b
                        && right.class.declaration.is_enum
                    {
                        if let Value::Object(left) = &a
                            && left.class.declaration.is_enum
                        {
                            return Rc::as_ptr(left).cmp(&Rc::as_ptr(right));
                        }
                        return Ordering::Less;
                    }
                    Ordering::Greater
                })
            }
        }
    }

    fn sort_string(&mut self, value: &Value, line: usize, error: &mut Option<Error>) -> Vec<u8> {
        // zend_sort continues after a conversion throws. Failed conversions
        // produce empty strings, and pending exceptions prevent further user
        // __toString calls, while scalar conversions still take place.
        if error.is_some() && matches!(value, Value::Object(_) | Value::Closure(_)) {
            return Vec::new();
        }
        match self.string_bytes(value, line) {
            Ok(bytes) => bytes,
            Err(failure) => {
                if error.is_none() {
                    *error = Some(failure);
                }
                Vec::new()
            }
        }
    }

    fn sort_number(&mut self, value: &Value, line: usize) -> f64 {
        match value {
            Value::Float(n) => *n,
            Value::Int(n) => *n as f64,
            Value::String(s) => {
                crate::value::numeric_prefix(&s.bytes()).map_or(0.0, |(n, _)| n.as_float())
            }
            Value::Object(_) | Value::Closure(_) => {
                self.diagnostics.push(format!(
                    "Warning: Object of class {} could not be converted to float on line {line}",
                    builtins::debug_type(value)
                ));
                1.0
            }
            _ => {
                if value.is_truthy() {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

fn with_array<T>(value: &mut Value, action: impl FnOnce(&mut Rc<PhpArray>) -> T) -> Option<T> {
    match value {
        Value::Array(array) => Some(action(array)),
        Value::Reference(reference) | Value::Indirect(reference) => {
            with_array(&mut reference.borrow_mut(), action)
        }
        _ => None,
    }
}
