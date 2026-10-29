use crate::{Error, ErrorKind, Value};
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::{Rc, Weak},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArrayKey {
    Int(i64),
    String(Vec<u8>),
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Cursor {
    Current,
    Key,
    Reset,
    End,
    Next,
    Prev,
}

impl Cursor {
    pub(crate) fn moves(self) -> bool {
        !matches!(self, Self::Current | Self::Key)
    }

    pub(crate) fn advance(self, position: usize, len: usize) -> usize {
        if len == 0 {
            return 0;
        }
        match self {
            Self::Reset => 0,
            Self::End => len - 1,
            Self::Next if position < len => position + 1,
            Self::Prev if position > 0 && position < len => position - 1,
            Self::Prev => len,
            _ => position,
        }
    }
}
impl ArrayKey {
    pub(crate) fn from_value(
        value: &Value,
        line: usize,
        warnings: &mut dyn crate::diagnostics::WarningSink,
    ) -> Result<Self, Error> {
        Ok(match value {
            Value::Reference(value) | Value::Indirect(value) => {
                return Self::from_value(&value.borrow(), line, warnings);
            }
            Value::Int(n) => Self::Int(*n),
            Value::Float(_) => Self::Int(value.integer(line, warnings)?),
            Value::Bool(b) => Self::Int(i64::from(*b)),
            Value::Null => {
                warnings.push(format!("Deprecated: Using null as an array offset is deprecated, use an empty string instead on line {line}"));
                Self::String(Vec::new())
            }
            Value::String(bytes) => {
                if let Ok(text) = std::str::from_utf8(&bytes.bytes())
                    && let Ok(number) = text.parse::<i64>()
                    && number.to_string() == text
                {
                    Self::Int(number)
                } else {
                    Self::String(bytes.to_vec())
                }
            }
            Value::Array(_) | Value::Closure(_) | Value::Object(_) => {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    "Illegal offset type",
                    line,
                ));
            }
        })
    }
    pub fn to_value(&self) -> Value {
        match self {
            Self::Int(n) => Value::Int(*n),
            Self::String(s) => Value::String(s.clone().into()),
        }
    }
}

/// Insertion-ordered PHP map. The next append index survives removal, and is
/// deliberately excluded from value equality.
#[derive(Debug, Default)]
pub struct PhpArray {
    entries: Vec<(ArrayKey, Value)>,
    positions: HashMap<ArrayKey, usize>,
    next_index: Option<i64>,
    cursor: usize,
    /// Each copy keeps its own iterator positions. Weak tokens do not keep an
    /// exited loop alive and allow a loop to resume after copy-on-write.
    iterators: RefCell<Vec<(Weak<()>, usize)>>,
}
impl Clone for PhpArray {
    fn clone(&self) -> Self {
        let iterators: Vec<_> = self
            .iterators
            .borrow()
            .iter()
            .filter(|(token, _)| token.strong_count() != 0)
            .cloned()
            .collect();
        let active = !iterators.is_empty();
        Self {
            entries: self
                .entries
                .iter()
                .map(|(key, value)| {
                    let value = value.copy_array_entry();
                    let value = if active {
                        Value::Indirect(Rc::new(RefCell::new(value)))
                    } else {
                        value
                    };
                    (key.clone(), value)
                })
                .collect(),
            positions: self.positions.clone(),
            next_index: self.next_index,
            // zend_array_dup resets an exhausted internal pointer.
            cursor: if self.cursor < self.len() {
                self.cursor
            } else {
                0
            },
            iterators: RefCell::new(iterators),
        }
    }
}
impl PartialEq for PhpArray {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}
impl PhpArray {
    pub(crate) fn splice(
        &mut self,
        start: usize,
        end: usize,
        replacement: &Self,
        result_used: bool,
        line: usize,
    ) -> Result<Self, Error> {
        let mut output = Self::with_capacity(self.len() - (end - start) + replacement.len(), line)?;
        let mut removed = Self::with_capacity(if result_used { end - start } else { 0 }, line)?;
        output.iterators = std::mem::take(&mut self.iterators);
        let iterators = output.iterators.get_mut();
        iterators.retain(|(token, _)| token.strong_count() != 0);
        // Dense numeric arrays take PHP's packed-table path. The runtime does
        // not yet retain the physical bucket history of sparse/mixed tables.
        let packed = self
            .entries
            .iter()
            .enumerate()
            .all(|(index, (key, _))| *key == ArrayKey::Int(index as i64));
        for (_, position) in iterators.iter_mut() {
            if end == self.len() && end > start && *position >= start {
                // Deleting the final bucket clamps even exhausted iterators.
                *position = start;
            } else if packed && (start..end).contains(position) {
                *position = end;
            }
        }
        // PHP remaps tail buckets in order. A position shifted forward can be
        // encountered again. USED_RET also affects the native running position:
        // copying removed entries advances it, while discarding them does not.
        let tail_start = if result_used { end } else { start } + replacement.len();
        let mut next_iterator = iterators
            .iter()
            .map(|(_, position)| *position)
            .filter(|position| *position >= start)
            .min();
        for index in end..self.len() {
            if next_iterator != Some(index) {
                continue;
            }
            for (_, position) in iterators.iter_mut() {
                if *position == index {
                    *position = tail_start + index - end;
                }
            }
            next_iterator = iterators
                .iter()
                .map(|(_, position)| *position)
                .filter(|position| *position > index)
                .min();
        }
        let mut entries = std::mem::take(&mut self.entries).into_iter();
        for (key, value) in entries.by_ref().take(start) {
            splice_insert(&mut output, key, splice_value(value), line)?;
        }
        for (key, value) in entries.by_ref().take(end - start) {
            if result_used {
                splice_insert(&mut removed, key, splice_value(value), line)?;
            }
        }
        for (_, value) in replacement.entries() {
            output.append(splice_value(value.clone()), line)?;
        }
        for (key, value) in entries {
            splice_insert(&mut output, key, splice_value(value), line)?;
        }
        // Replacing the table resets the internal pointer and next append index.
        *self = output;
        Ok(removed)
    }

    /// Apply a bucket permutation without copying reference values. PHP leaves
    /// live foreach positions at their bucket offsets and resets only the IAP.
    pub(crate) fn reorder(&mut self, order: &[usize], renumber: bool) {
        if self.len() <= 1 && !(renumber && !self.is_empty()) {
            return;
        }
        let mut old: Vec<_> = std::mem::take(&mut self.entries)
            .into_iter()
            .map(Some)
            .collect();
        self.positions.clear();
        for (position, &index) in order.iter().enumerate() {
            let (key, value) = old[index].take().unwrap();
            let key = if renumber {
                ArrayKey::Int(position as i64)
            } else {
                key
            };
            self.positions.insert(key.clone(), position);
            self.entries.push((key, value));
        }
        if renumber {
            self.next_index = Some(self.len() as i64);
        }
        self.cursor = 0;
    }

    pub(crate) fn reorder_multisort(&mut self, order: &[usize]) {
        if self.is_empty() {
            return;
        }
        self.reorder(order, false);
        self.positions.clear();
        let mut numeric = 0;
        for (position, (key, _)) in self.entries.iter_mut().enumerate() {
            if matches!(key, ArrayKey::Int(_)) {
                *key = ArrayKey::Int(numeric);
                numeric += 1;
            }
            self.positions.insert(key.clone(), position);
        }
        // PHP reserves the next append index using the entire row count,
        // including string keys, rather than just the renumbered integers.
        self.next_index = Some(self.len() as i64);
        self.cursor = 0;
    }

    pub(crate) fn cursor_entry(&self) -> Option<&(ArrayKey, Value)> {
        self.entries.get(self.cursor)
    }

    pub(crate) fn move_cursor(&mut self, action: Cursor) {
        self.cursor = action.advance(self.cursor, self.len());
    }

    pub(crate) fn with_capacity(capacity: usize, line: usize) -> Result<Self, Error> {
        // PHP's 64-bit HashTable cannot round a larger requested capacity to
        // a valid table size. Reject it before asking the host allocator.
        if capacity > 0x40000000 {
            return Err(Error::new(
                ErrorKind::Error,
                "Possible integer overflow in memory allocation",
                line,
            ));
        }
        let mut array = Self::default();
        array
            .entries
            .try_reserve_exact(capacity)
            .and_then(|()| array.positions.try_reserve(capacity))
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate array storage", line))?;
        Ok(array)
    }

    pub(crate) fn start_reference_iteration(&mut self, token: &Rc<()>) {
        self.start_reference_iteration_at(token, 0);
    }

    pub(crate) fn cursor_position(&self) -> usize {
        self.cursor
    }

    pub(crate) fn start_reference_iteration_at(&mut self, token: &Rc<()>, position: usize) {
        self.iterators
            .get_mut()
            .retain(|(token, _)| token.strong_count() != 0);
        if self
            .iterators
            .get_mut()
            .iter()
            .any(|(existing, _)| existing.ptr_eq(&Rc::downgrade(token)))
        {
            return;
        }
        self.iterators
            .get_mut()
            .push((Rc::downgrade(token), position));
        for (_, value) in &mut self.entries {
            if !matches!(value, Value::Indirect(_)) {
                *value =
                    Value::Indirect(Rc::new(RefCell::new(std::mem::replace(value, Value::Null))));
            }
        }
    }

    pub(crate) fn has_iterator(&self, token: &Rc<()>) -> bool {
        self.iterators
            .borrow()
            .iter()
            .any(|(existing, _)| existing.ptr_eq(&Rc::downgrade(token)))
    }

    pub(crate) fn next_reference(&self, token: &Rc<()>) -> Option<(ArrayKey, Rc<RefCell<Value>>)> {
        let mut iterators = self.iterators.borrow_mut();
        let (_, position) = iterators
            .iter_mut()
            .find(|(existing, _)| existing.ptr_eq(&Rc::downgrade(token)))?;
        let (key, slot) = self.entries.get(*position)?;
        *position += 1;
        let Value::Indirect(slot) = slot else {
            unreachable!("reference iteration uses mutable buckets")
        };
        let mut value = slot.borrow_mut();
        let reference = match &mut *value {
            Value::Reference(reference) => reference.clone(),
            value => {
                let reference = Rc::new(RefCell::new(std::mem::replace(value, Value::Null)));
                *value = Value::Reference(reference.clone());
                reference
            }
        };
        Some((key.clone(), reference))
    }

    pub fn entries(&self) -> &[(ArrayKey, Value)] {
        &self.entries
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn get(&self, key: &ArrayKey) -> Option<&Value> {
        self.positions.get(key).map(|i| &self.entries[*i].1)
    }
    pub(crate) fn get_mut(&mut self, key: &ArrayKey) -> Option<&mut Value> {
        self.positions.get(key).map(|i| &mut self.entries[*i].1)
    }
    pub fn insert(&mut self, key: ArrayKey, value: Value) {
        let value = if self
            .iterators
            .get_mut()
            .iter()
            .any(|(token, _)| token.strong_count() != 0)
        {
            Value::Indirect(Rc::new(RefCell::new(value)))
        } else {
            value
        };
        if let Some(index) = self.positions.get(&key) {
            self.entries[*index].1 = value;
            return;
        }
        if let ArrayKey::Int(n) = &key
            && self.next_index.is_none_or(|next| *n >= next)
        {
            self.next_index = Some(n.saturating_add(1));
        }
        self.positions.insert(key.clone(), self.entries.len());
        self.entries.push((key, value));
    }
    pub(crate) fn append_key(&self, line: usize) -> Result<ArrayKey, Error> {
        let key = ArrayKey::Int(self.next_index.unwrap_or(0));
        if self.positions.contains_key(&key) {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot add element to the array as the next element is already occupied",
                line,
            ));
        }
        Ok(key)
    }
    pub fn append(&mut self, value: Value, line: usize) -> Result<(), Error> {
        let key = self.append_key(line)?;
        self.insert(key, value);
        Ok(())
    }
    pub fn remove(&mut self, key: &ArrayKey) {
        if let Some(index) = self.positions.remove(key) {
            self.entries.remove(index);
            if self.cursor > index {
                self.cursor -= 1;
            }
            for (_, position) in self.iterators.get_mut() {
                if *position > index {
                    *position -= 1;
                }
            }
            for i in index..self.entries.len() {
                self.positions.insert(self.entries[i].0.clone(), i);
            }
        }
    }
}

// Native splicing preserves reference containers, even when their other aliases
// have gone away. Indirect cells are only our mutable bucket implementation.
fn splice_value(value: Value) -> Value {
    match value {
        Value::Indirect(slot) => splice_value(slot.borrow().clone()),
        _ => value,
    }
}

fn splice_insert(
    array: &mut PhpArray,
    key: ArrayKey,
    value: Value,
    line: usize,
) -> Result<(), Error> {
    match key {
        ArrayKey::Int(_) => array.append(value, line),
        key => {
            array.insert(key, value);
            Ok(())
        }
    }
}
