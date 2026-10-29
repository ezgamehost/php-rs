use super::*;
use crate::ast::CastKind;
use builtins::{Builtin, require_array};

impl Runtime {
    pub(super) fn array_join(
        &mut self,
        kind: Builtin,
        name: &str,
        args: &[Option<Value>],
        extra: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
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
        let replace = matches!(kind, Builtin::ArrayReplace(_));
        let mut arrays = Vec::new();
        if replace {
            arrays.push(require_array(args[0].as_ref().unwrap(), name, 1, line)?);
        }
        for (index, (_, value)) in extra.entries().iter().enumerate() {
            arrays.push(require_array(
                value,
                name,
                index + 1 + usize::from(replace),
                line,
            )?);
        }
        let Some(first) = arrays.first() else {
            return Ok(Value::Array(Rc::new(PhpArray::default())));
        };
        let mut result = if replace {
            (*first).clone()
        } else {
            let capacity = arrays.iter().map(|array| array.len()).sum();
            let mut result = PhpArray::with_capacity(capacity, line)?;
            self.join_array_entries(
                &mut result,
                first,
                Builtin::ArrayMerge(false),
                &mut HashSet::new(),
                line,
            )?;
            result
        };
        for array in arrays.iter().skip(1) {
            self.join_array_entries(&mut result, array, kind, &mut HashSet::new(), line)?;
        }
        Ok(Value::Array(Rc::new(result)))
    }

    fn join_array_entries(
        &mut self,
        dest: &mut PhpArray,
        src: &PhpArray,
        kind: Builtin,
        active: &mut HashSet<usize>,
        line: usize,
    ) -> Result<(), Error> {
        let replace = matches!(kind, Builtin::ArrayReplace(_));
        let recursive = matches!(
            kind,
            Builtin::ArrayMerge(true) | Builtin::ArrayReplace(true)
        );
        for (key, entry) in src.entries() {
            if !replace && matches!(key, ArrayKey::Int(_)) {
                dest.append(entry.copy_array_entry(), line)?;
                continue;
            }
            let previous = dest.get(key);
            if !recursive || previous.is_none() {
                dest.insert(key.clone(), entry.copy_array_entry());
                continue;
            }
            let previous = previous.unwrap().dereferenced();
            let mut incoming = entry.dereferenced();
            if replace
                && !(matches!(previous, Value::Array(_)) && matches!(incoming, Value::Array(_)))
            {
                dest.insert(key.clone(), entry.copy_array_entry());
                continue;
            }
            // Merge protects the original destination; replace protects the
            // source and its separated destination. The latter's new identity
            // cannot occur in either input. Do not mark the old destination:
            // it can also be a later source without forming a cycle.
            let dest_id = match &previous {
                Value::Array(array) => Some(Rc::as_ptr(array) as usize),
                _ => None,
            };
            let src_id = match &incoming {
                Value::Array(array) if replace => Some(Rc::as_ptr(array) as usize),
                _ => None,
            };
            if dest_id.is_some_and(|id| active.contains(&id))
                || src_id.is_some_and(|id| active.contains(&id))
            {
                return Err(Error::new(ErrorKind::Error, "Recursion detected", line));
            }
            let converted = if matches!(previous, Value::Null) {
                let mut array = PhpArray::default();
                array.append(Value::Null, line)?;
                Value::Array(Rc::new(array))
            } else {
                casting::cast(CastKind::Array, previous, line, &mut self.diagnostics)?
            };
            let Value::Array(array) = converted else {
                unreachable!()
            };
            let mut nested = (*array).clone();
            if matches!(incoming, Value::Object(_) | Value::Closure(_)) {
                incoming = casting::cast(CastKind::Array, incoming, line, &mut self.diagnostics)?;
            }
            if let Value::Array(array) = incoming {
                let protected = if replace { src_id } else { dest_id };
                if let Some(id) = protected {
                    active.insert(id);
                }
                self.join_array_entries(&mut nested, &array, kind, active, line)?;
                if let Some(id) = protected {
                    active.remove(&id);
                }
            } else {
                nested.append(incoming, line)?;
            }
            dest.insert(key.clone(), Value::Array(Rc::new(nested)));
        }
        Ok(())
    }
}
