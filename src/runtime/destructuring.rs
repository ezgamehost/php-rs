use super::*;

impl Runtime {
    pub(super) fn assign_target(&mut self, target: &Expr, value: Value) -> Result<Value, Error> {
        let ExprKind::Destructure(elements) = &target.kind else {
            let mut location = self.location(target)?;
            return self.write_location(&mut location, value, target.line);
        };
        for (index, element) in elements.iter().enumerate() {
            let ArrayElement::Entry(key, destination) = element else {
                continue;
            };
            let key = match key {
                Some(key) => self.eval(key)?,
                None => Value::Int(index as i64),
            };
            let fetched = match &value {
                Value::Array(_) | Value::Object(_) | Value::Closure(_) => read_offset(
                    value.clone(),
                    &mut Offset::new(Some(key)),
                    destination.line,
                    Access::Read,
                    &mut self.warnings,
                )?,
                Value::Null => Value::Null,
                other => {
                    let name = match other {
                        Value::Bool(_) => "bool",
                        Value::Int(_) => "int",
                        Value::Float(_) => "float",
                        Value::String(_) => "string",
                        _ => unreachable!(),
                    };
                    self.warnings.push(format!(
                        "Cannot use {name} as array on line {}",
                        destination.line
                    ));
                    Value::Null
                }
            };
            self.assign_target(destination, fetched)?;
        }
        Ok(value)
    }
}
