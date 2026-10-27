use super::*;
use crate::{
    ast::LambdaDecl,
    closure::{CapturedValue, Environment},
};

impl Runtime {
    pub(super) fn acquire_lambda(&mut self, lambda: &LambdaDecl) -> Result<Value, Error> {
        let mut captures = Vec::new();
        for capture in &lambda.captures {
            if capture.name == b"this" {
                continue;
            }
            let value = if capture.by_reference {
                CapturedValue::Reference(self.scope.get_or_insert(&capture.name, Value::Null))
            } else if let Some(value) = self.lookup(&capture.name) {
                CapturedValue::Value(value)
            } else if lambda.arrow {
                continue;
            } else {
                self.warnings.push(format!(
                    "Undefined variable ${} on line {}",
                    String::from_utf8_lossy(&capture.name),
                    lambda.function.line
                ));
                CapturedValue::Value(Value::Null)
            };
            captures.push((capture.name.clone(), value));
        }
        self.next_object_id += 1;
        Ok(Value::Closure(Rc::new(crate::Closure {
            name: lambda.function.name.clone(),
            context: self.class_context.clone(),
            this: if !lambda.is_static {
                match self.lookup(b"this") {
                    Some(Value::Object(object)) => Some(object),
                    _ => None,
                }
            } else {
                None
            },
            function: Some(lambda.function.clone()),
            object_id: self.next_object_id,
            environment: Some(Environment {
                captures,
                statics: RefCell::new(HashMap::new()),
            }),
        })))
    }
}
