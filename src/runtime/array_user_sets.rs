// Altered Rust adaptation of PHP 8.5.0 ext/standard/array.c set operations.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md for its license.
use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum Operation {
    DiffKeys,
    IntersectKeys,
    DiffAssoc,
    IntersectAssoc,
    DiffValues,
    IntersectValues,
    DiffAssocValues,
    IntersectAssocValues,
    DiffBoth,
    IntersectBoth,
}

impl Operation {
    pub(super) fn callbacks(self) -> usize {
        if matches!(self, Self::DiffBoth | Self::IntersectBoth) {
            2
        } else {
            1
        }
    }
    fn intersect(self) -> bool {
        matches!(
            self,
            Self::IntersectKeys
                | Self::IntersectAssoc
                | Self::IntersectValues
                | Self::IntersectAssocValues
                | Self::IntersectBoth
        )
    }
    fn direct(self) -> bool {
        matches!(self, Self::DiffAssocValues | Self::IntersectAssocValues)
    }
    fn keys(self) -> bool {
        !matches!(self, Self::DiffValues | Self::IntersectValues)
    }
    fn assoc(self) -> bool {
        self.keys() && !matches!(self, Self::DiffKeys | Self::IntersectKeys)
    }
    fn data_callback(self) -> bool {
        matches!(
            self,
            Self::DiffValues
                | Self::IntersectValues
                | Self::DiffAssocValues
                | Self::IntersectAssocValues
                | Self::DiffBoth
                | Self::IntersectBoth
        )
    }
}

struct Comparisons<'a> {
    operation: Operation,
    name: &'a str,
    line: usize,
    key: Option<functions::CallTarget>,
    data: Option<functions::CallTarget>,
    pending: Option<Error>,
}

impl Comparisons<'_> {
    fn compare(
        &mut self,
        runtime: &mut Runtime,
        a: &(ArrayKey, Value),
        b: &(ArrayKey, Value),
    ) -> Ordering {
        if self.operation.keys() {
            self.call(runtime, true, [a.0.to_value(), b.0.to_value()])
        } else {
            self.values(runtime, &a.1, &b.1)
        }
    }
    fn values(&mut self, runtime: &mut Runtime, a: &Value, b: &Value) -> Ordering {
        if self.data.is_some() {
            self.call(runtime, false, [a.clone(), b.clone()])
        } else {
            runtime.set_compare(a, b, self.line, &mut self.pending)
        }
    }
    fn call(&mut self, runtime: &mut Runtime, key: bool, values: [Value; 2]) -> Ordering {
        if self.pending.is_some() {
            return Ordering::Equal;
        }
        let callback = if key {
            self.key.as_ref()
        } else {
            self.data.as_ref()
        }
        .unwrap();
        let result = if self.operation.direct() {
            // Exact-key associative variants use zval_user_compare, which
            // casts bool directly without deprecation or swapped retry.
            runtime
                .call_callback(callback, &values, self.line)
                .and_then(|value| runtime.user_sort_result(value, self.line))
        } else {
            runtime.user_sort_compare(callback, values, self.name, self.line)
        };
        match result {
            Ok(order) => order,
            Err(error) => {
                runtime.set_error(error, &mut self.pending);
                Ordering::Equal
            }
        }
    }
    fn finish(self, output: PhpArray) -> Result<Value, Error> {
        self.pending
            .map_or_else(|| Ok(Value::Array(Rc::new(output))), Err)
    }
}

impl Runtime {
    pub(super) fn array_user_set(
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
        // Native parsing resolves the trailing callback(s) before validating
        // any array, including the first argument.
        let array_count = extra.len() + 1 - operation.callbacks();
        let mut callbacks = Vec::new();
        for index in array_count - 1..extra.len() {
            let callback = self
                .native_callback(&extra.entries()[index].1, name, index + 2, false, line)
                .map_err(|mut error| {
                    if error.kind == ErrorKind::TypeError && error.throwable.is_none() {
                        error.message = error.message.replacen(" ($callback)", "", 1);
                    }
                    error
                })?
                .unwrap();
            callbacks.push(callback);
        }
        let key = if operation.keys() && !operation.direct() {
            callbacks.pop()
        } else {
            None
        };
        let data = if operation.data_callback() {
            callbacks.pop()
        } else {
            None
        };
        let mut compare = Comparisons {
            operation,
            name,
            line,
            key,
            data,
            pending: None,
        };
        let values: Vec<_> = std::iter::once(first)
            .chain(extra.entries()[..array_count - 1].iter().map(|(_, v)| v))
            .collect();
        let mut arrays = Vec::new();
        let mut orders = Vec::new();
        if !operation.direct() {
            // The sorted variants reset the request-wide deprecation flag
            // after callback parsing, and do not restore it on return.
            self.sort_compare_deprecated = false;
        }
        for (index, value) in values.into_iter().enumerate() {
            let array = match builtins::require_array(value, name, index + 1, line) {
                Ok(array) => array,
                Err(error) => {
                    self.set_error(error, &mut compare.pending);
                    return Err(compare.pending.unwrap());
                }
            };
            arrays.push(array);
            if !operation.direct() {
                let mut order: Vec<_> = (0..array.len()).collect();
                sorting::algorithm::sort(&mut order, &mut |&a, &b| {
                    compare.compare(self, &array.entries()[a], &array.entries()[b])
                });
                if compare
                    .pending
                    .as_ref()
                    .is_some_and(|error| error.kind == ErrorKind::UnwindExit)
                {
                    return Err(compare.pending.unwrap());
                }
                orders.push(order);
            }
        }
        if operation.direct() {
            let mut output = PhpArray::default();
            for (key, value) in arrays[0].entries() {
                let mut keep = true;
                for array in &arrays[1..] {
                    let equal = array
                        .get(key)
                        .is_some_and(|other| compare.values(self, value, other) == Ordering::Equal);
                    if equal != operation.intersect() {
                        keep = false;
                        break;
                    }
                }
                if keep {
                    output.insert(key.clone(), value.copy_array_entry());
                }
            }
            return compare.finish(output);
        }
        if operation.intersect() {
            self.user_set_intersection(&arrays, &orders, compare)
        } else {
            self.user_set_difference(&arrays, &orders, compare)
        }
    }

    fn user_set_difference(
        &mut self,
        arrays: &[&PhpArray],
        orders: &[Vec<usize>],
        mut compare: Comparisons<'_>,
    ) -> Result<Value, Error> {
        let mut output = arrays[0].clone();
        let mut positions = vec![0; arrays.len()];
        'scan: while positions[0] < orders[0].len() {
            let current = &arrays[0].entries()[orders[0][positions[0]]];
            let mut comparison = Ordering::Greater;
            for index in 1..arrays.len() {
                // Key-comparator difference scans the entire other array for
                // each key, even when the comparator imposes a partial order.
                let mut position = positions[index];
                while position < orders[index].len() {
                    comparison = compare.compare(
                        self,
                        current,
                        &arrays[index].entries()[orders[index][position]],
                    );
                    if if compare.operation.keys() {
                        comparison == Ordering::Equal
                    } else {
                        comparison != Ordering::Greater
                    } {
                        break;
                    }
                    position += 1;
                }
                if !compare.operation.keys() {
                    positions[index] = position;
                }
                if comparison == Ordering::Equal {
                    if !compare.operation.keys() {
                        if position < orders[index].len() {
                            positions[index] += 1;
                        }
                        break;
                    }
                    if compare.operation.assoc() {
                        if position < orders[index].len() {
                            if compare.values(
                                self,
                                &current.1,
                                &arrays[index].entries()[orders[index][position]].1,
                            ) != Ordering::Equal
                            {
                                comparison = Ordering::Less;
                            } else {
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
            }
            loop {
                if comparison == Ordering::Equal {
                    output.remove(&arrays[0].entries()[orders[0][positions[0]]].0);
                }
                positions[0] += 1;
                if positions[0] == orders[0].len() {
                    break 'scan;
                }
                if compare.operation.keys()
                    || compare.compare(
                        self,
                        &arrays[0].entries()[orders[0][positions[0] - 1]],
                        &arrays[0].entries()[orders[0][positions[0]]],
                    ) != Ordering::Equal
                {
                    break;
                }
            }
        }
        compare.finish(output)
    }

    fn user_set_intersection(
        &mut self,
        arrays: &[&PhpArray],
        orders: &[Vec<usize>],
        mut compare: Comparisons<'_>,
    ) -> Result<Value, Error> {
        let mut output = arrays[0].clone();
        let mut positions = vec![0; arrays.len()];
        let mut comparison = Ordering::Equal;
        'scan: while positions[0] < orders[0].len() {
            let current = &arrays[0].entries()[orders[0][positions[0]]];
            let mut mismatch = 0;
            for index in 1..arrays.len() {
                while positions[index] < orders[index].len() {
                    comparison = compare.compare(
                        self,
                        current,
                        &arrays[index].entries()[orders[index][positions[index]]],
                    );
                    if comparison != Ordering::Greater {
                        break;
                    }
                    positions[index] += 1;
                }
                if comparison == Ordering::Equal
                    && positions[index] < orders[index].len()
                    && compare.operation.assoc()
                    && compare.values(
                        self,
                        &current.1,
                        &arrays[index].entries()[orders[index][positions[index]]].1,
                    ) != Ordering::Equal
                {
                    comparison = Ordering::Greater;
                }
                if positions[index] == orders[index].len() {
                    for &remaining in &orders[0][positions[0]..] {
                        output.remove(&arrays[0].entries()[remaining].0);
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
                    output.remove(&arrays[0].entries()[orders[0][positions[0]]].0);
                    positions[0] += 1;
                    if positions[0] == orders[0].len() {
                        break 'scan;
                    }
                    if compare.operation.keys()
                        || compare.compare(
                            self,
                            &arrays[0].entries()[orders[0][positions[0]]],
                            &arrays[mismatch].entries()[orders[mismatch][positions[mismatch]]],
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
                    if compare.operation.keys()
                        || compare.compare(
                            self,
                            &arrays[0].entries()[orders[0][positions[0] - 1]],
                            &arrays[0].entries()[orders[0][positions[0]]],
                        ) != Ordering::Equal
                    {
                        break;
                    }
                }
            }
        }
        compare.finish(output)
    }
}
