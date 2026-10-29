use super::*;
use builtins::{Builtin, require_array};

impl Runtime {
    pub(super) fn array_inspection(
        &mut self,
        kind: Builtin,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let array = require_array(args[0].as_ref().unwrap(), name, 1, line)?;
        match kind {
            Builtin::ArrayEdge(last, key) => {
                let entry = if last {
                    array.entries().last()
                } else {
                    array.entries().first()
                };
                Ok(entry.map_or(Value::Null, |(k, v)| {
                    if key { k.to_value() } else { v.dereferenced() }
                }))
            }
            Builtin::ArrayIsList => {
                Ok(Value::Bool(array.entries().iter().enumerate().all(
                    |(index, (key, _))| *key == ArrayKey::Int(index as i64),
                )))
            }
            Builtin::ArrayFlip | Builtin::ArrayCountValues => {
                let flip = matches!(kind, Builtin::ArrayFlip);
                let mut result = if flip {
                    PhpArray::with_capacity(array.len(), line)?
                } else {
                    PhpArray::default()
                };
                for (key, value) in array.entries() {
                    let value = value.dereferenced();
                    if !matches!(value, Value::Int(_) | Value::String(_)) {
                        self.diagnostics.push(format!("{name}(): Can only {} string and integer values, entry skipped on line {line}", if flip { "flip" } else { "count" }));
                        continue;
                    }
                    let value_key = ArrayKey::from_value(&value, line, &mut self.diagnostics)?;
                    let entry = if flip {
                        key.to_value()
                    } else {
                        let count = match result.get(&value_key) {
                            Some(Value::Int(count)) => *count,
                            None => 0,
                            _ => unreachable!(),
                        };
                        Value::Int(count + 1)
                    };
                    result.insert(value_key, entry);
                }
                Ok(Value::Array(Rc::new(result)))
            }
            Builtin::ArrayCombine => {
                let values = require_array(args[1].as_ref().unwrap(), name, 2, line)?;
                if array.len() != values.len() {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "array_combine(): Argument #1 ($keys) and argument #2 ($values) must have the same number of elements",
                        line,
                    ));
                }
                let mut result = PhpArray::with_capacity(array.len(), line)?;
                for ((_, key), (_, value)) in array.entries().iter().zip(values.entries()) {
                    let key = key.dereferenced();
                    let key = match key {
                        Value::Int(n) => ArrayKey::Int(n),
                        value => {
                            let bytes = self.string_bytes(&value, line)?;
                            ArrayKey::from_value(
                                &Value::String(bytes.into()),
                                line,
                                &mut self.diagnostics,
                            )?
                        }
                    };
                    result.insert(key, value.copy_array_entry());
                }
                Ok(Value::Array(Rc::new(result)))
            }
            _ => unreachable!(),
        }
    }
}
