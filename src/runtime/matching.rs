use super::*;
use crate::ast::MatchArm;

impl Runtime {
    pub(super) fn match_expression(
        &mut self,
        subject: &Expr,
        arms: &[MatchArm],
        line: usize,
    ) -> Result<Value, Error> {
        let mut subject = subject;
        while let ExprKind::Group(inner) = &subject.kind {
            subject = inner;
        }
        let variable = matches!(subject.kind, ExprKind::Variable(_)) && !subject.is_globals();
        let value = if variable {
            None
        } else {
            Some(self.eval(subject)?)
        };
        let default = arms.iter().find(|arm| arm.conditions.is_empty());

        // PHP uses a single subject read for a table of constant integer/string
        // keys. Sequential comparisons keep simple variable subjects live.
        if let Some(keys) = constant_keys(arms) {
            let value = match value {
                Some(value) => value,
                None => self.eval(subject)?,
            };
            for (key, arm) in keys {
                if key == value {
                    return self.eval(&arm.result);
                }
            }
            return match default {
                Some(arm) => self.eval(&arm.result),
                None => Err(unhandled(&value, line)),
            };
        }
        for arm in arms {
            for condition in &arm.conditions {
                let candidate = self.eval(condition)?;
                let value = match &value {
                    Some(value) => value.clone(),
                    None => self.eval(subject)?,
                };
                if value.identical(&candidate, line)? {
                    return self.eval(&arm.result);
                }
            }
        }
        if let Some(arm) = default {
            return self.eval(&arm.result);
        }
        let value = value.unwrap_or_else(|| match &subject.kind {
            ExprKind::Variable(name) => self.lookup(name).unwrap_or(Value::Null),
            _ => unreachable!(),
        });
        Err(unhandled(&value, line))
    }
}

fn constant_keys(arms: &[MatchArm]) -> Option<Vec<(Value, &MatchArm)>> {
    if arms.iter().map(|arm| arm.conditions.len()).sum::<usize>() < 2 {
        return None;
    }
    let mut constants = Runtime::default();
    let mut keys = Vec::new();
    for arm in arms {
        for condition in &arm.conditions {
            if !condition.is_constant_expression() {
                return None;
            }
            let value = constants.eval(condition).ok()?;
            if !constants.diagnostics.is_empty()
                || !matches!(value, Value::Int(_) | Value::String(_))
            {
                return None;
            }
            keys.push((value, arm));
        }
    }
    Some(keys)
}

fn unhandled(value: &Value, line: usize) -> Error {
    let description = match value {
        Value::Reference(reference) => return unhandled(&reference.borrow(), line),
        Value::Null => "NULL".into(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(number) => {
            let mut text = String::from_utf8(value.to_bytes()).unwrap();
            if number.is_finite() && !text.contains(['.', 'E']) {
                text.push_str(".0");
            }
            text
        }
        Value::Array(_) => "of type array".into(),
        Value::Closure(_) => "of type Closure".into(),
        Value::Object(object) => format!("of type {}", object.class_name),
        Value::String(value) => {
            let mut description = String::from("'");
            for byte in value.bytes().iter().take(15) {
                match byte {
                    b'\n' => description.push_str("\\n"),
                    b'\r' => description.push_str("\\r"),
                    b'\t' => description.push_str("\\t"),
                    11 => description.push_str("\\v"),
                    12 => description.push_str("\\f"),
                    27 => description.push_str("\\e"),
                    b'\\' => description.push_str("\\\\"),
                    32..=126 => description.push(*byte as char),
                    _ => description.push_str(&format!("\\x{byte:02X}")),
                }
            }
            if value.len() > 15 {
                description.push_str("...");
            }
            description.push('\'');
            description
        }
    };
    Error::new(
        ErrorKind::UnhandledMatchError,
        format!("Unhandled match case {description}"),
        line,
    )
}
