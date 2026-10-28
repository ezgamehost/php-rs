use crate::{Error, ErrorKind, Value};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArrayKey {
    Int(i64),
    String(Vec<u8>),
}
impl ArrayKey {
    pub(crate) fn from_value(
        value: &Value,
        line: usize,
        warnings: &mut Vec<String>,
    ) -> Result<Self, Error> {
        Ok(match value {
            Value::Int(n) => Self::Int(*n),
            Value::Float(_) => Self::Int(value.integer(line, warnings)?),
            Value::Bool(b) => Self::Int(i64::from(*b)),
            Value::Null => {
                warnings.push(format!("Deprecated: Using null as an array offset is deprecated, use an empty string instead on line {line}"));
                Self::String(Vec::new())
            }
            Value::String(bytes) => {
                if let Ok(text) = std::str::from_utf8(bytes)
                    && let Ok(number) = text.parse::<i64>()
                    && number.to_string() == text
                {
                    Self::Int(number)
                } else {
                    Self::String(bytes.clone())
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
            Self::String(s) => Value::String(s.clone()),
        }
    }
}

/// Insertion-ordered PHP map. The next append index survives removal, and is
/// deliberately excluded from value equality.
#[derive(Debug, Clone, Default)]
pub struct PhpArray {
    entries: Vec<(ArrayKey, Value)>,
    positions: HashMap<ArrayKey, usize>,
    next_index: Option<i64>,
}
impl PartialEq for PhpArray {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
    }
}
impl PhpArray {
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
            for i in index..self.entries.len() {
                self.positions.insert(self.entries[i].0.clone(), i);
            }
        }
    }
}
