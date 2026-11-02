use super::*;
use crate::{PhpObject, PhpString, string::StringBuffer};
use std::rc::Weak;

/// A pending write retains the original string, but does not keep a variable or
/// array alive or force array separation while PHP conversion runs.
enum Storage {
    Cell(Weak<RefCell<Value>>),
    Property(Rc<PhpObject>, ArrayKey),
}
struct PendingWrite {
    buffer: StringBuffer,
    offset: Value,
    original_len: usize,
    path: Vec<(usize, ArrayKey)>,
}
enum Preparation {
    Reference(variables::Cell, usize),
    Ordinary,
    ArrayKey {
        array: Rc<PhpArray>,
        path: Vec<(usize, ArrayKey)>,
    },
    Character(PendingWrite),
}

impl Storage {
    fn prepare(&self, keys: &mut [Offset], line: usize) -> Result<Preparation, Error> {
        match self {
            Self::Cell(cell) => match cell.upgrade() {
                Some(cell) => prepare(&mut cell.borrow_mut(), keys, line, &mut Vec::new()),
                None => Ok(Preparation::Ordinary),
            },
            Self::Property(object, key) => match object.properties.borrow_mut().get_mut(key) {
                Some(value) => prepare(value, keys, line, &mut Vec::new()),
                None => Ok(Preparation::Ordinary),
            },
        }
    }

    fn with_current_value<T>(
        &self,
        path: &[(usize, ArrayKey)],
        action: impl FnOnce(&Value) -> T,
    ) -> Option<T> {
        fn follow<'a>(mut value: &'a Value, path: &[(usize, ArrayKey)]) -> Option<&'a Value> {
            for (identity, key) in path {
                let Value::Array(array) = value else {
                    return None;
                };
                if Rc::as_ptr(array) as usize != *identity {
                    return None;
                }
                value = array.get(key)?;
            }
            Some(value)
        }
        match self {
            Self::Cell(cell) => Some(action(follow(&cell.upgrade()?.borrow(), path)?)),
            Self::Property(object, key) => {
                Some(action(follow(object.properties.borrow().get(key)?, path)?))
            }
        }
    }

    fn with_current_buffer<T>(
        &self,
        path: &[(usize, ArrayKey)],
        action: impl FnOnce(&PhpString) -> T,
    ) -> Option<T> {
        self.with_current_value(path, |value| match value {
            Value::String(buffer) => Some(action(buffer)),
            _ => None,
        })
        .flatten()
    }
}

impl Runtime {
    pub(super) fn string_offset_assignment(
        &mut self,
        location: &mut Location,
        replacement: &Value,
        line: usize,
    ) -> Result<Option<Value>, Error> {
        if location.keys.is_empty() {
            return Ok(None);
        }
        let storage = if let Some(object) = &location.object {
            Storage::Property(
                object.clone(),
                self.instance_property_key(object, &location.name),
            )
        } else {
            let stored = if let Some(class) = &location.static_class {
                let stored = self.static_cell(class, &location.name, line, false)?;
                self.check_static_set_access(location, true, line)?;
                stored
            } else {
                self.location_scope(location).get(&location.name).cloned()
            };
            let Some(stored) = stored else {
                return Ok(None);
            };
            Storage::Cell(Rc::downgrade(&stored))
        };
        self.assign_string_storage(storage, &mut location.keys, replacement, line)
    }

    pub(super) fn write_overloaded_path(
        &mut self,
        value: Value,
        keys: &mut [Offset],
        replacement: Value,
        line: usize,
    ) -> Result<Value, Error> {
        let temporary = cell(value);
        if let Some(result) = self.assign_string_storage(
            Storage::Cell(Rc::downgrade(&temporary)),
            keys,
            &replacement,
            line,
        )? {
            return Ok(result);
        }
        reference_paths::write_cell(temporary, keys, replacement, line, self)
    }

    fn assign_string_storage(
        &mut self,
        mut storage: Storage,
        mut keys: &mut [Offset],
        replacement: &Value,
        line: usize,
    ) -> Result<Option<Value>, Error> {
        let pending = loop {
            match storage.prepare(keys, line)? {
                Preparation::Ordinary => return Ok(None),
                Preparation::ArrayKey { array, path } => {
                    // Conversion can run PHP. Keep only the array guard across
                    // that call, then abandon a replaced or newly shared table.
                    keys[path.len()].array_key(line, false, self)?;
                    let current = storage.with_current_value(&path, |value| {
                        matches!(value, Value::Array(current) if Rc::ptr_eq(current, &array))
                    }).unwrap_or(false);
                    if !current || Rc::strong_count(&array) != 2 {
                        return Ok(Some(Value::Null));
                    }
                }
                Preparation::Character(pending) => break pending,
                Preparation::Reference(reference, consumed) => {
                    storage = Storage::Cell(Rc::downgrade(&reference));
                    keys = &mut keys[consumed..];
                }
            }
        };
        // No variable, property, array, or string is borrowed across PHP calls.
        let index = offsets::string_index(&pending.offset, line, Access::Read, self)?.unwrap();
        if !pending.buffer.retained() {
            return Ok(Some(Value::Null));
        }
        let Some(position) = offsets::position(index, pending.original_len) else {
            self.warn(format!("Illegal string offset {index} on line {line}"))?;
            return Ok(Some(Value::Null));
        };
        let bytes = self.string_bytes(replacement, line)?;
        if !pending.buffer.retained() {
            return Ok(Some(Value::Null));
        }
        let Some(byte) = bytes.first().copied() else {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot assign an empty string to a string offset",
                line,
            ));
        };
        if bytes.len() > 1 {
            self.warn(format!(
                "Only the first byte will be assigned to the string offset on line {line}"
            ))?;
            if !pending.buffer.retained() {
                return Ok(Some(Value::Null));
            }
        }
        if position >= pending.original_len {
            // Extension resumes with the original buffer, including copies
            // retained by PHP code during conversion, then restores that buffer
            // to the destination value slot if it still exists.
            offsets::assign_character(&mut pending.buffer.pending_bytes(), position, byte, line)?;
            storage.with_current_buffer(&pending.path, |current| {
                current.replace_buffer(&pending.buffer)
            });
        } else {
            let current = storage
                .with_current_buffer(&pending.path, PhpString::retain)
                .unwrap_or(pending.buffer);
            let mut current = current.pending_bytes();
            // PHP's in-bounds decision uses the pre-conversion length. Avoid
            // extending a shorter replacement (or writing beyond its storage).
            if position < current.len() {
                current[position] = byte;
            }
        }
        Ok(Some(Value::String(vec![byte].into())))
    }
}

fn prepare(
    value: &mut Value,
    keys: &mut [Offset],
    line: usize,
    path: &mut Vec<(usize, ArrayKey)>,
) -> Result<Preparation, Error> {
    let Some((offset, rest)) = keys.split_first_mut() else {
        return Ok(Preparation::Ordinary);
    };
    match value {
        Value::Reference(reference) | Value::Indirect(reference) => {
            Ok(Preparation::Reference(reference.clone(), 0))
        }
        Value::String(buffer) => {
            if !rest.is_empty() {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot use string offset as an array",
                    line,
                ));
            }
            if offset.is_append() {
                return Err(Error::new(
                    ErrorKind::Error,
                    "[] operator not supported for strings",
                    line,
                ));
            }
            buffer.separate();
            Ok(Preparation::Character(PendingWrite {
                buffer: buffer.retain(),
                offset: offset.value(line)?.dereferenced(),
                original_len: buffer.len(),
                path: path.clone(),
            }))
        }
        Value::Array(array) if !offset.is_append() => {
            // PHP separates the table before key conversion can call a handler.
            Rc::make_mut(array);
            let Some(key) = offset.cached_array_key() else {
                return Ok(Preparation::ArrayKey {
                    array: array.clone(),
                    path: path.clone(),
                });
            };
            if array.get(&key).is_none() {
                return Ok(Preparation::Ordinary);
            }
            // Record the identity after copy-on-write separation.
            Rc::make_mut(array);
            path.push((Rc::as_ptr(array) as usize, key.clone()));
            let result = prepare(
                Rc::get_mut(array).unwrap().get_mut(&key).unwrap(),
                rest,
                line,
                path,
            );
            path.pop();
            result.map(|result| match result {
                Preparation::Reference(reference, consumed) => {
                    Preparation::Reference(reference, consumed + 1)
                }
                result => result,
            })
        }
        _ => Ok(Preparation::Ordinary),
    }
}
