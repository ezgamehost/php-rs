use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum Action {
    Push,
    Pop,
    Shift,
    Unshift,
}

impl Runtime {
    pub(super) fn array_stack(
        &mut self,
        action: Action,
        name: &str,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        extra: PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let first = args.into_iter().next().unwrap().unwrap();
        builtins::require_array(&first, name, 1, line)?;
        let source = if let Some(reference) = &references[0] {
            drop(first);
            reference.clone()
        } else {
            cell(first)
        };
        mutate(&mut source.borrow_mut(), |array| {
            // Native parsing validates and separates the first argument before
            // rejecting unknown named variadic arguments.
            if extra
                .entries()
                .iter()
                .any(|(key, _)| matches!(key, ArrayKey::String(_)))
            {
                return Err(Error::new(
                    ErrorKind::ArgumentCountError,
                    format!("{name}() does not accept unknown named parameters"),
                    line,
                ));
            }
            match action {
                Action::Pop => Ok(array.pop()),
                Action::Shift => Ok(array.shift()),
                Action::Push => {
                    for (_, value) in extra.entries() {
                        // Earlier appends survive a later exhausted-key error.
                        array.append(value.clone(), line)?;
                    }
                    Ok(Value::Int(array.len() as i64))
                }
                Action::Unshift => {
                    array.unshift(&extra, line)?;
                    Ok(Value::Int(array.len() as i64))
                }
            }
        })
    }
}

fn mutate<T>(value: &mut Value, action: impl FnOnce(&mut PhpArray) -> T) -> T {
    match value {
        Value::Array(array) => action(Rc::make_mut(array)),
        Value::Reference(reference) | Value::Indirect(reference) => {
            mutate(&mut reference.borrow_mut(), action)
        }
        _ => unreachable!("array argument was validated"),
    }
}
