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
            let Value::String(bytes) = result else {
                unreachable!("__toString has a string return type")
            };
            return Ok(bytes);
        }
        string_bytes(value, line, &mut self.warnings)
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
            return Ok(Value::String(bytes));
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
        binary(op, left, right, line, &mut self.warnings)
    }
}

pub(super) fn throwable_string(object: &Rc<crate::PhpObject>) -> Vec<u8> {
    let mut chain = Vec::new();
    let mut current = Some(object.clone());
    let mut seen = HashSet::new();
    while let Some(object) = current {
        if !seen.insert(object.object_id) {
            break;
        }
        let data = object.throwable.borrow();
        let data = data.as_ref().expect("throwable metadata");
        let mut text = object.class_name.as_bytes().to_vec();
        if !data.message.is_empty() {
            text.extend_from_slice(b": ");
            text.extend_from_slice(&data.message);
            if matches!(
                object.class_name.as_str(),
                "TypeError" | "ArgumentCountError"
            ) && data
                .message
                .windows(b", called in ".len())
                .any(|w| w == b", called in ")
            {
                text.extend_from_slice(b" and defined");
            }
        }
        text.extend_from_slice(b" in ");
        text.extend_from_slice(&data.file);
        text.extend_from_slice(format!(":{}\nStack trace:\n", data.line).as_bytes());
        text.extend_from_slice(&traces::trace_string(&data.trace));
        chain.push(text);
        current = data.previous.clone();
    }
    let mut result = Vec::new();
    for (index, text) in chain.iter().rev().enumerate() {
        if index != 0 {
            result.extend_from_slice(b"\n\nNext ");
        }
        result.extend_from_slice(text);
    }
    result
}
