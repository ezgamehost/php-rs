// Altered Rust adaptation of PHP 8.5.0 ext/standard/{levenshtein,string}.c.
// Copyright (c) The PHP Group. See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn levenshtein(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let left = self.native_string(args[0].as_ref().unwrap(), "levenshtein", 1, line)?;
        let right = self.native_string(args[1].as_ref().unwrap(), "levenshtein", 2, line)?;
        let mut costs = [1; 3];
        for (index, cost) in costs.iter_mut().enumerate() {
            if let Some(value) = &args[index + 2] {
                *cost = self.native_int(value, "levenshtein", index + 3, line)?;
            }
        }
        let [insertion, replacement, deletion] = costs;
        if left.is_empty() {
            return Ok(Value::Int((right.len() as i64).wrapping_mul(insertion)));
        }
        if right.is_empty() {
            return Ok(Value::Int((left.len() as i64).wrapping_mul(deletion)));
        }
        let (mut left, mut right) = (left.as_slice(), right.as_slice());
        if left.len() < right.len() && insertion == replacement && replacement == deletion {
            std::mem::swap(&mut left, &mut right);
        }
        let mut previous = distance_row(right.len() + 1, line)?;
        let mut current = distance_row(right.len() + 1, line)?;
        for (index, cell) in previous.iter_mut().enumerate() {
            *cell = (index as i64).wrapping_mul(insertion);
        }
        for &byte in left {
            current[0] = previous[0].wrapping_add(deletion);
            for (index, &other) in right.iter().enumerate() {
                current[index + 1] = previous[index]
                    .wrapping_add(if byte == other { 0 } else { replacement })
                    .min(previous[index + 1].wrapping_add(deletion))
                    .min(current[index].wrapping_add(insertion));
            }
            std::mem::swap(&mut previous, &mut current);
        }
        Ok(Value::Int(previous[right.len()]))
    }

    pub(super) fn similar_text(
        &mut self,
        args: &[Option<Value>],
        references: &[Option<variables::Cell>],
        line: usize,
    ) -> Result<Value, Error> {
        let left = self.native_string(args[0].as_ref().unwrap(), "similar_text", 1, line)?;
        let right = self.native_string(args[1].as_ref().unwrap(), "similar_text", 2, line)?;
        let count = similar_count(&left, &right);
        if let Some(reference) = &references[2] {
            let percent = if left.is_empty() && right.is_empty() {
                0.0
            } else {
                count as f64 * 200.0 / (left.len() + right.len()) as f64
            };
            self.write_reference_value(reference, Value::Float(percent), self.strict_types, line)?;
        }
        Ok(Value::Int(count as i64))
    }
}

fn distance_row(length: usize, line: usize) -> Result<Vec<i64>, Error> {
    let mut row = Vec::new();
    row.try_reserve_exact(length)
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate Levenshtein row", line))?;
    row.resize(length, 0);
    Ok(row)
}

fn similar_count(left: &[u8], right: &[u8]) -> usize {
    let mut pending = vec![(left, right)];
    let mut total = 0;
    while let Some((left, right)) = pending.pop() {
        let (mut start_left, mut start_right, mut longest, mut improvements) = (0, 0, 0, 0);
        for i in 0..left.len() {
            for j in 0..right.len() {
                let mut length = 0;
                while i + length < left.len()
                    && j + length < right.len()
                    && left[i + length] == right[j + length]
                {
                    length += 1;
                }
                if length > longest {
                    (start_left, start_right, longest) = (i, j, length);
                    improvements += 1;
                }
            }
        }
        if longest == 0 {
            continue;
        }
        total += longest;
        // Retain PHP's first-match tie breaking and prefix recursion condition.
        // An explicit stack avoids consuming the embedding thread's call stack.
        if start_left != 0 && start_right != 0 && improvements > 1 {
            pending.push((&left[..start_left], &right[..start_right]));
        }
        if start_left + longest < left.len() && start_right + longest < right.len() {
            pending.push((
                &left[start_left + longest..],
                &right[start_right + longest..],
            ));
        }
    }
    total
}
