// Altered Rust adaptation of PHP 8.5.0 ext/standard/array.c set operations.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md for its license.
use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum Operation {
    Difference,
    Intersection,
    DifferenceKey,
    DifferenceAssoc,
    IntersectionKey,
    IntersectionAssoc,
}

impl Runtime {
    pub(super) fn array_set(
        &mut self,
        operation: Operation,
        name: &str,
        first: &Value,
        extra: &PhpArray,
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
        if matches!(operation, Operation::Intersection) {
            self.sort_compare_deprecated = false;
        }
        let input = builtins::require_array(first, name, 1, line)?;
        let mut pending = None;
        if matches!(operation, Operation::Difference) {
            return self.array_difference(first, input, extra, line);
        }
        if matches!(operation, Operation::Intersection) {
            return self.array_intersection(input, extra, line);
        }
        let arrays = extra
            .entries()
            .iter()
            .enumerate()
            .map(|(index, (_, value))| builtins::require_array(value, name, index + 2, line))
            .collect::<Result<Vec<_>, _>>()?;
        let intersect = matches!(
            operation,
            Operation::IntersectionKey | Operation::IntersectionAssoc
        );
        let assoc = matches!(
            operation,
            Operation::DifferenceAssoc | Operation::IntersectionAssoc
        );
        let mut output = PhpArray::default();
        for (key, value) in input.entries() {
            let mut keep = true;
            for array in &arrays {
                let equal = if let Some(other) = array.get(key) {
                    !assoc || self.set_compare(value, other, line, &mut pending) == Ordering::Equal
                } else {
                    false
                };
                if equal != intersect {
                    keep = false;
                    break;
                }
            }
            if keep {
                output.insert(key.clone(), value.copy_array_entry());
            }
        }
        pending.map_or_else(|| Ok(Value::Array(Rc::new(output))), Err)
    }

    fn array_difference(
        &mut self,
        first: &Value,
        input: &PhpArray,
        extra: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let mut pending = None;
        // PHP's singleton path converts the needle before validating the
        // remaining arrays, and stops converting candidates after a match.
        if input.len() == 1 {
            let search = self.set_string(&input.entries()[0].1, line, &mut pending);
            let mut found = false;
            for (index, (_, value)) in extra.entries().iter().enumerate() {
                let array = match builtins::require_array(value, "array_diff", index + 2, line) {
                    Ok(array) => array,
                    Err(error) => {
                        self.set_error(error, &mut pending);
                        return Err(pending.unwrap());
                    }
                };
                if !found {
                    for (_, value) in array.entries() {
                        if self.set_string(value, line, &mut pending) == search {
                            found = true;
                            break;
                        }
                    }
                }
            }
            return pending.map_or_else(
                || {
                    Ok(if found {
                        Value::Array(Rc::new(PhpArray::default()))
                    } else {
                        first.clone()
                    })
                },
                Err,
            );
        }
        let arrays = extra
            .entries()
            .iter()
            .enumerate()
            .map(|(index, (_, value))| {
                builtins::require_array(value, "array_diff", index + 2, line)
            })
            .collect::<Result<Vec<_>, _>>()?;
        if input.is_empty() {
            return Ok(Value::Array(Rc::new(PhpArray::default())));
        }
        if arrays.iter().all(|array| array.is_empty()) {
            return Ok(first.clone());
        }
        let mut exclude = HashSet::new();
        for array in arrays {
            for (_, value) in array.entries() {
                exclude.insert(self.set_string(value, line, &mut pending));
            }
        }
        let mut output = PhpArray::default();
        for (key, value) in input.entries() {
            if !exclude.contains(&self.set_string(value, line, &mut pending)) {
                // Unlike the key-based functions, array_diff copies the
                // reference container even when it only has one owner.
                output.insert(key.clone(), value.clone());
            }
        }
        pending.map_or_else(|| Ok(Value::Array(Rc::new(output))), Err)
    }

    fn array_intersection(
        &mut self,
        input: &PhpArray,
        extra: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let mut arrays = vec![input];
        let mut orders = Vec::new();
        let mut pending = None;
        // Each input is sorted before the next input is type-checked. Use
        // Zend's comparison order because string conversions can run PHP.
        for index in 0..=extra.len() {
            if index > 0 {
                match builtins::require_array(
                    &extra.entries()[index - 1].1,
                    "array_intersect",
                    index + 1,
                    line,
                ) {
                    Ok(array) => arrays.push(array),
                    Err(error) => {
                        self.set_error(error, &mut pending);
                        return Err(pending.unwrap());
                    }
                }
            }
            let array = arrays[index];
            let mut order: Vec<_> = (0..array.len()).collect();
            sorting::algorithm::sort(&mut order, &mut |&a, &b| {
                self.set_compare(
                    &array.entries()[a].1,
                    &array.entries()[b].1,
                    line,
                    &mut pending,
                )
            });
            if pending
                .as_ref()
                .is_some_and(|error| error.kind == ErrorKind::UnwindExit)
            {
                return Err(pending.unwrap());
            }
            orders.push(order);
        }
        let mut output = input.clone();
        let mut positions = vec![0; arrays.len()];
        let mut comparison = Ordering::Equal;
        'scan: while positions[0] < orders[0].len() {
            let current = orders[0][positions[0]];
            let mut mismatch = 0;
            for index in 1..arrays.len() {
                while positions[index] < orders[index].len() {
                    comparison = self.set_compare(
                        &input.entries()[current].1,
                        &arrays[index].entries()[orders[index][positions[index]]].1,
                        line,
                        &mut pending,
                    );
                    if comparison != Ordering::Greater {
                        break;
                    }
                    positions[index] += 1;
                }
                if positions[index] == orders[index].len() {
                    for &remaining in &orders[0][positions[0]..] {
                        output.remove(&input.entries()[remaining].0);
                    }
                    break 'scan;
                }
                if comparison != Ordering::Equal {
                    mismatch = index;
                    break;
                }
                positions[index] += 1;
            }
            if comparison != Ordering::Equal {
                loop {
                    output.remove(&input.entries()[orders[0][positions[0]]].0);
                    positions[0] += 1;
                    if positions[0] == orders[0].len() {
                        break 'scan;
                    }
                    if self.set_compare(
                        &input.entries()[orders[0][positions[0]]].1,
                        &arrays[mismatch].entries()[orders[mismatch][positions[mismatch]]].1,
                        line,
                        &mut pending,
                    ) != Ordering::Less
                    {
                        break;
                    }
                }
            } else {
                loop {
                    positions[0] += 1;
                    if positions[0] == orders[0].len() {
                        break 'scan;
                    }
                    if self.set_compare(
                        &input.entries()[orders[0][positions[0] - 1]].1,
                        &input.entries()[orders[0][positions[0]]].1,
                        line,
                        &mut pending,
                    ) != Ordering::Equal
                    {
                        break;
                    }
                }
            }
        }
        pending.map_or_else(|| Ok(Value::Array(Rc::new(output))), Err)
    }

    pub(super) fn set_compare(
        &mut self,
        a: &Value,
        b: &Value,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Ordering {
        let a = self.set_string(a, line, pending);
        let b = self.set_string(b, line, pending);
        a.cmp(&b)
    }

    fn set_string(&mut self, value: &Value, line: usize, pending: &mut Option<Error>) -> Vec<u8> {
        let value = value.dereferenced();
        if pending
            .as_ref()
            .is_some_and(|error| error.kind == ErrorKind::UnwindExit)
            || (pending.is_some() && matches!(value, Value::Object(_) | Value::Closure(_)))
        {
            return Vec::new();
        }
        match self.string_bytes(&value, line) {
            Ok(bytes) => bytes,
            Err(error) => {
                self.set_error(error, pending);
                Vec::new()
            }
        }
    }

    pub(super) fn set_error(&mut self, error: Error, pending: &mut Option<Error>) {
        // Native argument checks and object casts leave an existing exception
        // intact while the surrounding C loop finishes.
        if pending.is_none() {
            *pending = Some(self.materialize_error(error));
        }
    }
}
