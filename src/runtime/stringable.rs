use super::*;

impl Runtime {
    pub(super) fn can_stringify(&self, value: &Value) -> bool {
        matches!(value, Value::Object(object) if objects::method(&object.class, b"__toString").is_some())
    }

    pub(super) fn string_bytes(&mut self, value: &Value, line: usize) -> Result<Vec<u8>, Error> {
        if let Value::Object(object) = value
            && let Some(method) = objects::method(&object.class, b"__toString")
        {
            let target = self.unchecked_method_target(
                method,
                object.class.clone(),
                Some(object.clone()),
                line,
            )?;
            let internal = self.trace_frames.last().is_some_and(|f| f.internal);
            let result = self.invoke_at(target, &[], line, false, internal)?;
            let Value::String(bytes) = result.dereferenced() else {
                unreachable!("__toString has a string return type")
            };
            return Ok(bytes.to_vec());
        }
        string_bytes(value, line, self)
    }

    pub(super) fn binary(
        &mut self,
        op: &str,
        left: Value,
        right: Value,
        line: usize,
    ) -> Result<Value, Error> {
        if op == "." {
            let mut bytes = self.string_bytes(&left, line)?;
            bytes.extend(self.string_bytes(&right, line)?);
            return Ok(Value::String(bytes.into()));
        }
        if matches!(op, "==" | "!=" | "<>" | "<" | "<=" | ">" | ">=" | "<=>") {
            let order = self.compare(&left, &right, line)?;
            return Ok(match op {
                "==" => Value::Bool(order == Some(Ordering::Equal)),
                "!=" | "<>" => Value::Bool(order != Some(Ordering::Equal)),
                "<" => Value::Bool(order == Some(Ordering::Less)),
                "<=" => Value::Bool(matches!(order, Some(Ordering::Less | Ordering::Equal))),
                ">" => Value::Bool(order == Some(Ordering::Greater)),
                ">=" => Value::Bool(matches!(order, Some(Ordering::Greater | Ordering::Equal))),
                _ => Value::Int(match order {
                    Some(Ordering::Less) => -1,
                    Some(Ordering::Equal) => 0,
                    _ => 1,
                }),
            });
        }
        binary(op, left, right, line, self)
    }
}
