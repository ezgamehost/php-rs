use super::*;
use crate::{PhpObject, array::Cursor};

impl Runtime {
    pub(super) fn array_cursor(
        &mut self,
        action: Cursor,
        name: &str,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        line: usize,
        result_used: bool,
    ) -> Result<Value, Error> {
        let mut value = args.into_iter().next().unwrap().unwrap();
        match &value {
            Value::Array(array) => {
                if !action.moves() {
                    return Ok(cursor_result(action, array.cursor_entry()));
                }
                if let Some(reference) = &references[0] {
                    // Discard the argument snapshot before separating: its Rc is
                    // not a PHP alias and must not trigger copy-on-write.
                    drop(value);
                    return Ok(move_array_cursor(&mut reference.borrow_mut(), action));
                }
                Ok(move_array_cursor(&mut value, action))
            }
            Value::Object(object) => {
                self.diagnostics.push(format!(
                    "Deprecated: {name}(): Calling {name}() on an object is deprecated on line {line}"
                ));
                Ok(object_cursor(object, action, result_used))
            }
            Value::Closure(_) => {
                self.diagnostics.push(format!(
                    "Deprecated: {name}(): Calling {name}() on an object is deprecated on line {line}"
                ));
                Ok(cursor_result(action, None))
            }
            _ => Err(builtins::type_error(&value, name, 1, "array|object", line)),
        }
    }
}

fn cursor_result(action: Cursor, entry: Option<&(ArrayKey, Value)>) -> Value {
    if matches!(action, Cursor::Key) {
        entry.map_or(Value::Null, |(key, _)| key.to_value())
    } else {
        entry.map_or(Value::Bool(false), |(_, value)| value.dereferenced())
    }
}

fn move_array_cursor(value: &mut Value, action: Cursor) -> Value {
    match value {
        Value::Array(array) => {
            let array = Rc::make_mut(array);
            array.move_cursor(action);
            cursor_result(action, array.cursor_entry())
        }
        Value::Reference(reference) | Value::Indirect(reference) => {
            move_array_cursor(&mut reference.borrow_mut(), action)
        }
        _ => unreachable!(),
    }
}

fn object_cursor(object: &PhpObject, action: Cursor, result_used: bool) -> Value {
    let mut cursor = object.property_cursor.borrow_mut();
    object.sync_property_cursor(&mut cursor);
    let properties = object.properties.borrow();
    let len = cursor.keys.len();
    cursor.position = action.advance(cursor.position, len);
    if action.moves() && !result_used {
        return Value::Null;
    }
    let forward = !matches!(action, Cursor::End | Cursor::Prev);
    while cursor.position < len {
        let key = &cursor.keys[cursor.position];
        if let Some(value) = properties.get(key) {
            return if matches!(action, Cursor::Key) {
                key.to_value()
            } else {
                value.dereferenced()
            };
        }
        cursor.position = if forward {
            cursor.position + 1
        } else {
            cursor.position.checked_sub(1).unwrap_or(len)
        };
    }
    cursor_result(action, None)
}
