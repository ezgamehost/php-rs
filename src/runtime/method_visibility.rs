use super::*;
use crate::{ast::Visibility, object::Class};

impl Runtime {
    pub(super) fn resolve_native_callback(
        &mut self,
        value: &Value,
        line: usize,
    ) -> Result<functions::CallTarget, Error> {
        let class_method = match value {
            Value::String(name) => {
                let name = name.bytes();
                name.windows(2)
                    .position(|part| part == b"::")
                    .map(|index| (name[..index].to_vec(), name[index + 2..].to_vec()))
            }
            Value::Array(array) if array.len() == 2 => {
                match (array.get(&ArrayKey::Int(0)), array.get(&ArrayKey::Int(1))) {
                    (Some(Value::String(class)), Some(Value::String(method))) => {
                        Some((class.to_vec(), method.to_vec()))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some((class, method)) = class_method {
            // PHP's internal callback validation can bind the active object
            // within the named class's scope. Ordinary variable calls cannot.
            let allow_this = self
                .class_context
                .as_ref()
                .is_some_and(|context| context.scope.is_a(&class));
            let target = self.resolve_class_method(&class, &method, false, allow_this, line)?;
            if method.eq_ignore_ascii_case(b"__construct") && target.magic_name.is_some() {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot call constructor",
                    line,
                ));
            }
            Ok(target)
        } else {
            self.resolve_callable(value.clone(), line)
        }
    }

    pub(super) fn check_method_access(
        &self,
        method: &FunctionDecl,
        line: usize,
    ) -> Result<(), Error> {
        if method.visibility == Visibility::Public {
            return Ok(());
        }
        let declaring = self
            .find_class(declaring_name(method))
            .expect("method class is linked");
        let mut prototype = declaring.clone();
        if method.visibility == Visibility::Protected {
            let name = method.name.rsplit(|byte| *byte == b':').next().unwrap();
            while let Some(parent) = &prototype.parent {
                let Some(ancestor) = objects::method(parent, name) else {
                    break;
                };
                if ancestor.visibility == Visibility::Private {
                    break;
                }
                prototype = self
                    .find_class(declaring_name(&ancestor))
                    .expect("ancestor method class is linked");
            }
        }
        if self.class_context.as_ref().is_some_and(|context| {
            context.scope.name.eq_ignore_ascii_case(&declaring.name)
                || (method.visibility == Visibility::Protected
                    && (context.scope.is_a(&prototype.name) || prototype.is_a(&context.scope.name)))
        }) {
            return Ok(());
        }
        let visibility = if method.visibility == Visibility::Private {
            "private"
        } else {
            "protected"
        };
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Call to {visibility} method {}() from {}",
                String::from_utf8_lossy(&method.name),
                self.class_context.as_ref().map_or_else(
                    || "global scope".to_owned(),
                    |c| format!("scope {}", String::from_utf8_lossy(&c.scope.name))
                )
            ),
            line,
        ))
    }

    pub(super) fn instance_method(&self, class: &Class, name: &[u8]) -> Option<Rc<FunctionDecl>> {
        // A private method belongs to its declaring scope, even when a child
        // declares another method with the same name.
        if let Some(context) = &self.class_context
            && class.is_a(&context.scope.name)
            && let Some(method) = context.scope.declaration.methods.iter().find(|method| {
                method.visibility == Visibility::Private
                    && method
                        .name
                        .rsplit(|b| *b == b':')
                        .next()
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
        {
            return Some(method.clone());
        }
        objects::method(class, name)
    }
}

pub(super) fn declaring_name(method: &FunctionDecl) -> &[u8] {
    let end = method
        .name
        .windows(2)
        .position(|part| part == b"::")
        .expect("qualified method name");
    &method.name[..end]
}
