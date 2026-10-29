use super::*;

impl PhpArray {
    pub(crate) fn pop(&mut self) -> Value {
        let Some((key, value)) = self.entries.pop() else {
            return Value::Null;
        };
        self.positions.remove(&key);
        if let ArrayKey::Int(index) = key
            && self.next_index.and_then(|next| next.checked_sub(1)) == Some(index)
        {
            self.next_index = Some(index);
        }
        let len = self.len();
        for (_, position) in self.iterators.get_mut() {
            *position = (*position).min(len);
        }
        self.cursor = 0;
        value.dereferenced()
    }

    pub(crate) fn shift(&mut self) -> Value {
        if self.is_empty() {
            return Value::Null;
        }
        // Packed compaction leaves an exhausted iterator at the old end. Mixed
        // table rehashing migrates it to the new end. As with splicing, physical
        // history of sparse tables is not represented by our dense entry vector.
        let packed = self
            .entries
            .iter()
            .enumerate()
            .all(|(index, (key, _))| *key == ArrayKey::Int(index as i64));
        let old_len = self.len();
        let (_, value) = self.entries.remove(0);
        for (_, position) in self.iterators.get_mut() {
            if old_len == 1 {
                *position = 0;
            } else if *position > 0 && (!packed || *position < old_len) {
                *position -= 1;
            }
        }
        self.positions.clear();
        let mut numeric = 0;
        for (position, (key, _)) in self.entries.iter_mut().enumerate() {
            if matches!(key, ArrayKey::Int(_)) {
                *key = ArrayKey::Int(numeric);
                numeric += 1;
            }
            self.positions.insert(key.clone(), position);
        }
        self.next_index = Some(numeric);
        self.cursor = 0;
        value.dereferenced()
    }

    pub(crate) fn unshift(&mut self, values: &Self, line: usize) -> Result<(), Error> {
        // Even an empty prepend renumbers numeric keys and resets the cursor.
        let mut output = Self::with_capacity(self.len() + values.len(), line)?;
        output.iterators = std::mem::take(&mut self.iterators);
        output
            .iterators
            .get_mut()
            .retain(|(token, _)| token.strong_count() != 0);
        for (_, position) in output.iterators.get_mut() {
            *position += values.len();
        }
        for (_, value) in values.entries() {
            output.append(splice_value(value.clone()), line)?;
        }
        for (key, value) in std::mem::take(&mut self.entries) {
            splice_insert(&mut output, key, splice_value(value), line)?;
        }
        *self = output;
        Ok(())
    }
}
