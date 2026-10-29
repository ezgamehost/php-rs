// Altered Rust adaptation of PHP 8.5.0 ext/standard/array.c transformations.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md for its license.
use super::*;

impl Runtime {
    pub(super) fn array_change_key_case(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let input =
            builtins::require_array(args[0].as_ref().unwrap(), "array_change_key_case", 1, line)?;
        let upper = match &args[1] {
            Some(value) => self.native_int(value, "array_change_key_case", 2, line)? != 0,
            None => false,
        };
        let mut output = PhpArray::default();
        for (key, value) in input.entries() {
            let key = match key {
                ArrayKey::Int(_) => key.clone(),
                ArrayKey::String(bytes) => {
                    let mut bytes = bytes.clone();
                    if upper {
                        bytes.make_ascii_uppercase();
                    } else {
                        bytes.make_ascii_lowercase();
                    }
                    ArrayKey::String(bytes)
                }
            };
            // A later colliding key replaces the value in the original slot.
            output.insert(key, value.clone());
        }
        Ok(Value::Array(Rc::new(output)))
    }

    pub(super) fn array_unique(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        let input = builtins::require_array(first, "array_unique", 1, line)?;
        let flags = match &args[1] {
            Some(value) => self.native_int(value, "array_unique", 2, line)?,
            None => 2,
        };
        if input.len() <= 1 {
            return Ok(first.clone());
        }
        let mut pending = None;
        if flags == 2 {
            let mut seen = HashSet::new();
            let mut output = PhpArray::default();
            for (key, value) in input.entries() {
                if seen.insert(self.set_string(value, line, &mut pending)) {
                    output.insert(key.clone(), value.copy_array_entry());
                }
            }
            return pending.map_or_else(|| Ok(Value::Array(Rc::new(output))), Err);
        }
        // Other modes sort bucket indices, then delete duplicates from a
        // pre-existing copy. This preserves the copy's append index and cursor.
        let mut output = input.clone();
        let mut order: Vec<_> = (0..input.len()).collect();
        sorting::algorithm::sort(&mut order, &mut |&a, &b| {
            self.sort_compare(
                &input.entries()[a].1,
                &input.entries()[b].1,
                flags,
                line,
                &mut pending,
            )
        });
        if let Some(error) = pending {
            return Err(error);
        }
        let mut kept = order[0];
        for &current in &order[1..] {
            if self.sort_compare(
                &input.entries()[kept].1,
                &input.entries()[current].1,
                flags,
                line,
                &mut pending,
            ) != Ordering::Equal
            {
                kept = current;
            } else {
                // Equal values may be unstable in the sorted index list;
                // retain whichever appeared first in the original array.
                let removed = kept.max(current);
                kept = kept.min(current);
                output.remove(&input.entries()[removed].0);
            }
        }
        pending.map_or_else(|| Ok(Value::Array(Rc::new(output))), Err)
    }
}
