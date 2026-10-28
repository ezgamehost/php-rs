use super::{builtins::Builtin, *};
use crate::ast::Argument;

impl Runtime {
    pub(super) fn callback_error(&self, value: &Value, error: &Error) -> String {
        let (class, method) = match value {
            Value::Array(array) => {
                if array.len() != 2 {
                    return "array callback must have exactly two members".into();
                }
                let (Some(base), Some(method)) =
                    (array.get(&ArrayKey::Int(0)), array.get(&ArrayKey::Int(1)))
                else {
                    return "array callback has to contain indices 0 and 1".into();
                };
                let base = base.dereferenced();
                let method = method.dereferenced();
                let class = match &base {
                    Value::String(name) => name.to_vec(),
                    Value::Object(object) => object.class.name.clone(),
                    Value::Closure(_) => b"Closure".to_vec(),
                    _ => return "first array member is not a valid class name or object".into(),
                };
                let Value::String(method) = method else {
                    return "second array member is not a valid method".into();
                };
                (class, method.to_vec())
            }
            Value::String(name) => {
                let name = name.to_vec();
                let Some(index) = name.windows(2).position(|part| part == b"::") else {
                    return format!(
                        "function \"{}\" not found or invalid function name",
                        String::from_utf8_lossy(&name)
                    );
                };
                (name[..index].to_vec(), name[index + 2..].to_vec())
            }
            _ => return "no array or string given".into(),
        };
        let Some(class) = self.find_class(&class) else {
            return format!("class \"{}\" not found", String::from_utf8_lossy(&class));
        };
        let Some(function) = objects::method(&class, &method) else {
            return format!(
                "class {} does not have a method \"{}\"",
                String::from_utf8_lossy(&class.name),
                String::from_utf8_lossy(&method)
            );
        };
        let method = function.name.rsplit(|byte| *byte == b':').next().unwrap();
        let visibility = match function.visibility {
            crate::ast::Visibility::Private => "private",
            crate::ast::Visibility::Protected => "protected",
            crate::ast::Visibility::Public => "public",
        };
        if self.check_method_access(&function, error.line).is_err() {
            format!(
                "cannot access {visibility} method {}::{}()",
                String::from_utf8_lossy(&class.name),
                String::from_utf8_lossy(method)
            )
        } else if !function.is_static {
            format!(
                "non-static method {}::{}() cannot be called statically",
                String::from_utf8_lossy(&class.name),
                String::from_utf8_lossy(method)
            )
        } else {
            error.message.clone()
        }
    }

    pub(super) fn can_inline_callback(
        &self,
        callee: &Expr,
        args: &[Argument],
        builtin: Builtin,
    ) -> bool {
        let static_name = match &callee.kind {
            ExprKind::Constant(_) => true,
            ExprKind::NamespacedName(name) => name.function_fallback.is_none(),
            _ => false,
        };
        static_name
            && args
                .iter()
                .all(|arg| matches!(arg, Argument::Positional(_)))
            && match builtin {
                Builtin::CallUserFunc => !args.is_empty(),
                Builtin::CallUserFuncArray => args.len() == 2,
                _ => false,
            }
    }

    pub(super) fn inline_callback(
        &mut self,
        args: &[Argument],
        builtin: Builtin,
        line: usize,
    ) -> Result<Value, Error> {
        let Argument::Positional(callback) = &args[0] else {
            unreachable!()
        };
        let callback = self.eval(callback)?;
        let name = callback_builtin_name(builtin);
        // PHP's direct-call compiler path resolves the callback before evaluating
        // its arguments, preserves caller strictness, and omits the wrapper frame.
        let mut target = self
            .native_callback(&callback, name, 1, false, line)?
            .unwrap();
        target.reference_values = true;
        if matches!(builtin, Builtin::CallUserFunc) {
            self.invoke_at(target, &args[1..], line, true, false)
        } else {
            let Argument::Positional(array) = &args[1] else {
                unreachable!()
            };
            let value = self.eval(array)?;
            let arguments = callback_arguments(callback_array(&value, line)?, line)?;
            self.invoke_at(target, &arguments, line, true, false)
        }
    }

    pub(super) fn call_user_function(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        extra: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let mut target = self
            .native_callback(
                args[0].as_ref().unwrap(),
                callback_builtin_name(builtin),
                1,
                false,
                line,
            )?
            .unwrap();
        target.reference_values = true;
        let values = if matches!(builtin, Builtin::CallUserFuncArray) {
            callback_array(args[1].as_ref().unwrap(), line)?
        } else {
            extra
        };
        let arguments = callback_arguments(values, line)?;
        // Dynamic, named, unpacked, and namespace-fallback calls use the actual
        // internal wrapper, whose callback arguments are weakly coerced.
        let previous_strict = std::mem::replace(&mut self.strict_types, false);
        let result = self.invoke_at(target, &arguments, line, true, true);
        self.strict_types = previous_strict;
        result
    }
}

fn callback_builtin_name(builtin: Builtin) -> &'static str {
    if matches!(builtin, Builtin::CallUserFunc) {
        "call_user_func"
    } else {
        "call_user_func_array"
    }
}

fn callback_array(value: &Value, line: usize) -> Result<&PhpArray, Error> {
    match value {
        Value::Array(array) => Ok(array),
        _ => Err(Error::new(
            ErrorKind::TypeError,
            format!(
                "call_user_func_array(): Argument #2 ($args) must be of type array, {} given",
                builtins::debug_type(value)
            ),
            line,
        )),
    }
}

pub(super) fn callback_arguments(array: &PhpArray, line: usize) -> Result<Vec<Argument>, Error> {
    let mut named = false;
    array
        .entries()
        .iter()
        .map(|(key, value)| {
            let expr = Expr {
                kind: ExprKind::Literal(value.clone()),
                line,
            };
            match key {
                ArrayKey::String(name) => {
                    named = true;
                    Ok(Argument::Named(name.clone(), expr))
                }
                ArrayKey::Int(_) if named => Err(Error::new(
                    ErrorKind::Error,
                    "Cannot use positional argument after named argument",
                    line,
                )),
                ArrayKey::Int(_) => Ok(Argument::Positional(expr)),
            }
        })
        .collect()
}
