use super::*;
use variables::Cell;

/// Traversal stops at each shared reference. The owner is released before the
/// referent is borrowed, which also permits paths through recursive arrays.
enum Step {
    Follow(Cell, usize),
    Done(Value),
}
enum Mutation {
    Write(Value),
    Unset,
    Acquire,
    Bind(Cell),
}

fn step(
    value: &mut Value,
    keys: &mut [Offset],
    mutation: &Mutation,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Step, Error> {
    if keys.is_empty() {
        match mutation {
            Mutation::Bind(reference) => {
                *value = Value::Reference(reference.clone());
                return Ok(Step::Done(Value::Reference(reference.clone())));
            }
            Mutation::Acquire => {
                let reference = match value {
                    Value::Reference(reference) => reference.clone(),
                    _ => {
                        let reference = cell(std::mem::replace(value, Value::Null));
                        *value = Value::Reference(reference.clone());
                        reference
                    }
                };
                return Ok(Step::Done(Value::Reference(reference)));
            }
            Mutation::Unset => return Ok(Step::Done(Value::Null)),
            Mutation::Write(new_value) if !matches!(value, Value::Reference(_)) => {
                *value = new_value.clone();
                return Ok(Step::Done(new_value.clone()));
            }
            Mutation::Write(_) => {}
        }
    }
    if let Value::Reference(reference) = value {
        return Ok(Step::Follow(reference.clone(), 0));
    }
    let (offset, rest) = keys.split_first_mut().expect("nonterminal array path");
    if matches!(mutation, Mutation::Unset) {
        return match value {
            Value::Array(array) => {
                let key = offset.array_key(line, true, warnings)?;
                let array = Rc::make_mut(array);
                if rest.is_empty() {
                    array.remove(&key);
                    Ok(Step::Done(Value::Null))
                } else if let Some(value) = array.get_mut(&key) {
                    advance(step(value, rest, mutation, line, warnings)?)
                } else {
                    Ok(Step::Done(Value::Null))
                }
            }
            Value::Null => Ok(Step::Done(Value::Null)),
            Value::String(_) => Err(Error::new(
                ErrorKind::Error,
                "Cannot unset string offsets",
                line,
            )),
            _ => Err(Error::new(
                ErrorKind::Error,
                "Cannot unset offset in a non-array variable",
                line,
            )),
        };
    }
    if let Value::String(bytes) = value {
        if !rest.is_empty() {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot use string offset as an array",
                line,
            ));
        }
        if offset.is_append() {
            return Err(Error::new(
                ErrorKind::Error,
                "[] operator not supported for strings",
                line,
            ));
        }
        return match mutation {
            Mutation::Write(new_value) => offsets::write_string(
                &mut bytes.make_mut(),
                offset.value(line)?,
                new_value.clone(),
                line,
                warnings,
            )
            .map(Step::Done),
            _ => Err(Error::new(
                ErrorKind::Error,
                "Cannot create references to/from string offsets",
                line,
            )),
        };
    }
    if matches!(value, Value::Null | Value::Bool(false)) {
        if matches!(value, Value::Bool(false)) {
            warnings.push(format!(
                "Deprecated: Automatic conversion of false to array on line {line}"
            ));
        }
        *value = Value::Array(Rc::new(PhpArray::default()));
    }
    let Value::Array(array) = value else {
        return Err(Error::new(
            ErrorKind::Error,
            "Cannot use a scalar value as an array",
            line,
        ));
    };
    let array = Rc::make_mut(array);
    let key = if offset.is_append() {
        array.append_key(line)?
    } else {
        offset.array_key(line, false, warnings)?
    };
    if array.get(&key).is_none() {
        array.insert(key.clone(), Value::Null);
    }
    advance(step(
        array.get_mut(&key).unwrap(),
        rest,
        mutation,
        line,
        warnings,
    )?)
}

fn advance(result: Step) -> Result<Step, Error> {
    Ok(match result {
        Step::Follow(reference, consumed) => Step::Follow(reference, consumed + 1),
        result => result,
    })
}

fn finish(
    mut result: Step,
    mut keys: &mut [Offset],
    mutation: &Mutation,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Value, Error> {
    loop {
        match result {
            Step::Done(value) => return Ok(value),
            Step::Follow(reference, consumed) => {
                keys = &mut keys[consumed..];
                result = step(&mut reference.borrow_mut(), keys, mutation, line, warnings)?;
            }
        }
    }
}

pub(super) fn write(
    value: &mut Value,
    keys: &mut [Offset],
    replacement: Value,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Value, Error> {
    let mutation = Mutation::Write(replacement);
    let result = step(value, keys, &mutation, line, warnings)?;
    finish(result, keys, &mutation, line, warnings)
}
pub(super) fn write_cell(
    reference: Cell,
    keys: &mut [Offset],
    replacement: Value,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Value, Error> {
    let mutation = Mutation::Write(replacement);
    finish(Step::Follow(reference, 0), keys, &mutation, line, warnings)
}
pub(super) fn unset(
    value: &mut Value,
    keys: &mut [Offset],
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<(), Error> {
    let result = step(value, keys, &Mutation::Unset, line, warnings)?;
    finish(result, keys, &Mutation::Unset, line, warnings).map(|_| ())
}
pub(super) fn unset_cell(
    reference: Cell,
    keys: &mut [Offset],
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<(), Error> {
    finish(
        Step::Follow(reference, 0),
        keys,
        &Mutation::Unset,
        line,
        warnings,
    )
    .map(|_| ())
}
pub(super) fn acquire(
    reference: Cell,
    keys: &mut [Offset],
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<Cell, Error> {
    if keys.is_empty() {
        return Ok(reference);
    }
    let result = finish(
        Step::Follow(reference, 0),
        keys,
        &Mutation::Acquire,
        line,
        warnings,
    )?;
    let Value::Reference(reference) = result else {
        unreachable!()
    };
    Ok(reference)
}
pub(super) fn bind(
    root: Cell,
    keys: &mut [Offset],
    reference: Cell,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> Result<(), Error> {
    finish(
        Step::Follow(root, 0),
        keys,
        &Mutation::Bind(reference),
        line,
        warnings,
    )
    .map(|_| ())
}
