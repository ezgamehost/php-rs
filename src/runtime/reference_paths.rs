use super::*;
use std::rc::Weak;
use variables::Cell;

/// Traversal stops at each shared reference. The owner is released before the
/// referent is borrowed, which also permits paths through recursive arrays.
enum Step {
    Follow(Cell, usize),
    Object(Rc<crate::PhpObject>, usize),
    Key(Rc<PhpArray>, usize, bool),
    ConvertedFalse(Rc<PhpArray>, usize),
    InvalidString(Value, &'static str),
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
    if let Value::Indirect(slot) = value {
        return Ok(Step::Follow(slot.clone(), 0));
    }
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
    if let Value::Object(object) = value {
        return Ok(Step::Object(object.clone(), 0));
    }
    let (offset, rest) = keys.split_first_mut().expect("nonterminal array path");
    if matches!(mutation, Mutation::Unset) {
        return match value {
            Value::Array(array) => {
                Rc::make_mut(array);
                let Some(key) = offset.cached_array_key() else {
                    return Ok(Step::Key(array.clone(), 0, true));
                };
                let array = Rc::get_mut(array).unwrap();
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
        if offset.is_append() {
            return Err(Error::new(
                ErrorKind::Error,
                "[] operator not supported for strings",
                line,
            ));
        }
        if !rest.is_empty() {
            return Ok(Step::InvalidString(
                offset.value(line)?.dereferenced(),
                "Cannot use string offset as an array",
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
            _ => Ok(Step::InvalidString(
                offset.value(line)?.dereferenced(),
                "Cannot create references to/from string offsets",
            )),
        };
    }
    if matches!(value, Value::Null | Value::Bool(false)) {
        let was_false = matches!(value, Value::Bool(false));
        let array = Rc::new(PhpArray::default());
        *value = Value::Array(array.clone());
        if was_false {
            return Ok(Step::ConvertedFalse(array, 0));
        }
    }
    let Value::Array(array) = value else {
        return Err(Error::new(
            ErrorKind::Error,
            "Cannot use a scalar value as an array",
            line,
        ));
    };
    Rc::make_mut(array);
    let key = if let Some(key) = offset.cached_array_key() {
        key
    } else if offset.is_append() {
        let key = array.append_key(line)?;
        offset.cache_array_key(key.clone());
        key
    } else {
        return Ok(Step::Key(array.clone(), 0, false));
    };
    let array = Rc::get_mut(array).unwrap();
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
        Step::Object(object, consumed) => Step::Object(object, consumed + 1),
        Step::Key(array, consumed, unset) => Step::Key(array, consumed + 1, unset),
        Step::ConvertedFalse(array, consumed) => Step::ConvertedFalse(array, consumed + 1),
        result => result,
    })
}

/// Borrowed roots are VM temporaries; live variable roots are weak so unsetting
/// the variable during a diagnostic can invalidate the pending write.
enum Root<'a> {
    Value(&'a mut Value),
    Property(Rc<crate::PhpObject>, ArrayKey),
    Cell {
        reference: Weak<RefCell<Value>>,
        _temporary: Option<Cell>,
    },
}
impl Root<'_> {
    fn cell(reference: Cell) -> Self {
        let temporary = (Rc::strong_count(&reference) == 1).then(|| reference.clone());
        Self::Cell {
            reference: Rc::downgrade(&reference),
            _temporary: temporary,
        }
    }
    fn with_mut<T>(&mut self, action: impl FnOnce(&mut Value) -> T) -> Option<T> {
        match self {
            Self::Value(value) => Some(action(value)),
            Self::Property(object, key) => {
                Some(action(object.properties.borrow_mut().get_mut(key)?))
            }
            Self::Cell { reference, .. } => Some(action(&mut reference.upgrade()?.borrow_mut())),
        }
    }
    fn contains_array(&mut self, keys: &[Offset], array: &Rc<PhpArray>) -> bool {
        self.with_mut(|mut value| {
            for offset in keys {
                let Value::Array(parent) = value else {
                    return false;
                };
                let Some(key) = offset.cached_array_key() else {
                    return false;
                };
                // Inspection only: no COW and no borrows cross PHP execution.
                let Some(next) = Rc::get_mut(parent).and_then(|parent| parent.get_mut(&key)) else {
                    return false;
                };
                value = next;
            }
            matches!(value, Value::Array(current) if Rc::ptr_eq(current, array))
        })
        .unwrap_or(false)
    }
}

fn run_step(
    mut root: Root<'_>,
    keys: &mut [Offset],
    mutation: &Mutation,
    line: usize,
    runtime: &mut Runtime,
) -> Result<Step, Error> {
    loop {
        let Some(result) =
            root.with_mut(|value| step(value, keys, mutation, line, &mut runtime.diagnostics))
        else {
            return Ok(Step::Done(Value::Null));
        };
        let (array, consumed, strict_count) = match result? {
            Step::Key(array, consumed, unset) => {
                keys[consumed].array_key(line, unset, runtime)?;
                (array, consumed, !unset)
            }
            Step::ConvertedFalse(array, consumed) => {
                runtime.warn(format!("Deprecated: Automatic conversion of false to array is deprecated on line {line}"))?;
                (array, consumed, false)
            }
            Step::InvalidString(offset, message) => {
                offsets::string_index(&offset, line, Access::Read, runtime)?;
                return Err(Error::new(ErrorKind::Error, message, line));
            }
            result => return Ok(result),
        };
        if !root.contains_array(&keys[..consumed], &array)
            || (strict_count && Rc::strong_count(&array) != 2)
        {
            return Ok(Step::Done(Value::Null));
        }
        // Drop the guard before traversing again; all evaluated keys (including
        // append positions) are cached, so resumption cannot repeat an append.
    }
}

fn finish(
    mut result: Step,
    mut keys: &mut [Offset],
    mutation: &Mutation,
    line: usize,
    runtime: &mut Runtime,
) -> Result<Value, Error> {
    loop {
        match result {
            Step::Key(..) | Step::ConvertedFalse(..) | Step::InvalidString(..) => {
                unreachable!("diagnostic handled before following a path")
            }
            Step::Done(value) => return Ok(value),
            Step::Object(object, consumed) => {
                keys = &mut keys[consumed..];
                runtime.check_array_access(&object, line)?;
                let key = keys[0].argument();
                if keys.len() == 1 {
                    match mutation {
                        Mutation::Write(value) => {
                            runtime.array_access_call(
                                &object,
                                b"offsetSet",
                                vec![key, value.clone()],
                                line,
                            )?;
                            return Ok(value.clone());
                        }
                        Mutation::Unset => {
                            runtime.array_access_call(&object, b"offsetUnset", vec![key], line)?;
                            return Ok(Value::Null);
                        }
                        Mutation::Bind(_) => {
                            return Err(Error::new(
                                ErrorKind::Error,
                                "Cannot assign by reference to an array dimension of an object",
                                line,
                            ));
                        }
                        Mutation::Acquire => {}
                    }
                }
                let value = runtime.array_access_call(&object, b"offsetGet", vec![key], line)?;
                runtime.warn_indirect_array_access(&object, &value, line)?;
                result = Step::Follow(cell(value), 1);
            }
            Step::Follow(reference, consumed) => {
                keys = &mut keys[consumed..];
                if keys.is_empty()
                    && !matches!(
                        *reference.borrow(),
                        Value::Reference(_) | Value::Indirect(_)
                    )
                    && let Mutation::Write(value) = mutation
                {
                    let value = runtime.write_reference_value(
                        &reference,
                        value.clone(),
                        runtime.strict_types,
                        line,
                    )?;
                    return Ok(value);
                }
                if !keys.is_empty()
                    && !matches!(mutation, Mutation::Unset)
                    && matches!(*reference.borrow(), Value::Null | Value::Bool(false))
                {
                    runtime.check_reference_array_init(&reference, line)?;
                }
                result = run_step(Root::cell(reference), keys, mutation, line, runtime)?;
            }
        }
    }
}

pub(super) fn write(
    value: &mut Value,
    keys: &mut [Offset],
    replacement: Value,
    line: usize,
    runtime: &mut Runtime,
) -> Result<Value, Error> {
    let mutation = Mutation::Write(replacement);
    let result = run_step(Root::Value(value), keys, &mutation, line, runtime)?;
    finish(result, keys, &mutation, line, runtime)
}
pub(super) fn write_property(
    object: Rc<crate::PhpObject>,
    key: ArrayKey,
    keys: &mut [Offset],
    replacement: Value,
    line: usize,
    runtime: &mut Runtime,
) -> Result<Value, Error> {
    let mutation = Mutation::Write(replacement);
    let result = run_step(Root::Property(object, key), keys, &mutation, line, runtime)?;
    finish(result, keys, &mutation, line, runtime)
}

pub(super) fn unset_property(
    object: Rc<crate::PhpObject>,
    key: ArrayKey,
    keys: &mut [Offset],
    line: usize,
    runtime: &mut Runtime,
) -> Result<(), Error> {
    let result = run_step(
        Root::Property(object, key),
        keys,
        &Mutation::Unset,
        line,
        runtime,
    )?;
    finish(result, keys, &Mutation::Unset, line, runtime).map(|_| ())
}

pub(super) fn write_cell(
    reference: Cell,
    keys: &mut [Offset],
    replacement: Value,
    line: usize,
    runtime: &mut Runtime,
) -> Result<Value, Error> {
    let mutation = Mutation::Write(replacement);
    finish(Step::Follow(reference, 0), keys, &mutation, line, runtime)
}
pub(super) fn unset(
    value: &mut Value,
    keys: &mut [Offset],
    line: usize,
    runtime: &mut Runtime,
) -> Result<(), Error> {
    let result = run_step(Root::Value(value), keys, &Mutation::Unset, line, runtime)?;
    finish(result, keys, &Mutation::Unset, line, runtime).map(|_| ())
}
pub(super) fn unset_cell(
    reference: Cell,
    keys: &mut [Offset],
    line: usize,
    runtime: &mut Runtime,
) -> Result<(), Error> {
    finish(
        Step::Follow(reference, 0),
        keys,
        &Mutation::Unset,
        line,
        runtime,
    )
    .map(|_| ())
}
pub(super) fn acquire(
    reference: Cell,
    keys: &mut [Offset],
    line: usize,
    runtime: &mut Runtime,
) -> Result<Cell, Error> {
    if keys.is_empty() {
        return Ok(reference);
    }
    let result = finish(
        Step::Follow(reference, 0),
        keys,
        &Mutation::Acquire,
        line,
        runtime,
    )?;
    // An invalidated fetch yields a disposable reference, never a revived slot.
    Ok(match result {
        Value::Reference(reference) => reference,
        value => cell(value),
    })
}
pub(super) fn bind(
    root: Cell,
    keys: &mut [Offset],
    reference: Cell,
    line: usize,
    runtime: &mut Runtime,
) -> Result<(), Error> {
    finish(
        Step::Follow(root, 0),
        keys,
        &Mutation::Bind(reference),
        line,
        runtime,
    )
    .map(|_| ())
}
