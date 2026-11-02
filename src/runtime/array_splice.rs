// Altered Rust adaptation of PHP 8.5.0's php_splice destruction and mutation
// checks in ext/standard/array.c. See THIRD_PARTY_NOTICES.md.
use super::*;
use sorting::with_array;

impl Runtime {
    pub(super) fn array_splice(
        &mut self,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        line: usize,
        result_used: bool,
    ) -> Result<Value, Error> {
        let mut args = args.into_iter();
        let first = args.next().unwrap().unwrap();
        builtins::require_array(&first, "array_splice", 1, line)?;
        let source = if let Some(reference) = &references[0] {
            drop(first);
            reference.clone()
        } else {
            cell(first)
        };
        // Native argument parsing separates the array before coercing either
        // index. The bound argument's snapshot must not itself cause a copy.
        with_array(&mut source.borrow_mut(), |array| {
            Rc::make_mut(array);
        });
        let offset = self.native_int(&args.next().unwrap().unwrap(), "array_splice", 2, line)?;
        let length = args
            .next()
            .flatten()
            .filter(|value| !matches!(value, Value::Null))
            .map(|value| self.native_int_type(&value, "array_splice", 3, line, "?int"))
            .transpose()?;
        let replacement = casting::cast(
            crate::ast::CastKind::Array,
            args.next().flatten().unwrap_or(Value::Null),
            line,
            self,
        )?;
        let Value::Array(replacement) = replacement else {
            unreachable!()
        };
        let may_run_destructors = !result_used
            && with_array(&mut source.borrow_mut(), |array| {
                let (start, end) = substrings::substring_range(array.len(), offset, length);
                array.entries()[start..end].iter().any(|(_, value)| {
                    matches!(
                        value,
                        Value::Object(_)
                            | Value::Closure(_)
                            | Value::Array(_)
                            | Value::Reference(_)
                            | Value::Indirect(_)
                    )
                })
            })
            .ok_or_else(|| modified(line))?;
        // Scalar-only removals cannot call PHP; keep their linear bulk path.
        if may_run_destructors {
            return self.discard_array_splice(&source, offset, length, &replacement, line);
        }
        let removed = with_array(&mut source.borrow_mut(), |array| {
            let (start, end) = substrings::substring_range(array.len(), offset, length);
            Rc::make_mut(array).splice(start, end, &replacement, result_used, line)
        })
        .ok_or_else(|| modified(line))??;
        Ok(Value::Array(Rc::new(removed)))
    }

    fn discard_array_splice(
        &mut self,
        source: &variables::Cell,
        offset: i64,
        length: Option<i64>,
        replacement: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let (mut guard, plan) = with_array(&mut source.borrow_mut(), |array| {
            // Establish exclusive table ownership after coercion callbacks
            // and before exposing the native mutation guard.
            Rc::make_mut(array);
            let (start, end) = substrings::substring_range(array.len(), offset, length);
            (
                SpliceGuard {
                    array: array.clone(),
                },
                array.plan_discard_splice(start, end, replacement.len()),
            )
        })
        .ok_or_else(|| modified(line))?;
        let mut pending = Ok(());
        for key in &plan.keys {
            drop(guard.take(key));
            // A native splice keeps the original table alive while each
            // deletion runs PHP. User writes therefore separate that table.
            // Complete later deletions even if a destructor throws.
            pending = self.drain_destructors(pending, line);
            if pending
                .as_ref()
                .is_err_and(|error| error.kind == ErrorKind::UnwindExit)
            {
                return pending.map(|()| Value::Null);
            }
        }
        if Rc::strong_count(&guard.array) > 1 {
            guard.finish(plan, replacement, line)?;
            drop(guard);
        } else {
            drop(guard);
            pending = self.drain_destructors(pending, line);
            if pending
                .as_ref()
                .is_err_and(|error| error.kind == ErrorKind::UnwindExit)
            {
                return pending.map(|()| Value::Null);
            }
            let error = self.materialize_error(modified(line));
            if let Err(previous) = pending
                && let (Some(current), Some(previous)) = (&error.throwable, previous.throwable)
            {
                throwables::append_previous(current, &previous);
            }
            return Err(error);
        }
        pending.map(|()| Value::Array(Rc::new(PhpArray::default())))
    }
}

/// PHP permits its native splice to mutate the retained table without COW.
/// This guard is private: only `take` and `finish` may write through it.
/// Construction separates the input before any guard/trace aliases exist.
struct SpliceGuard {
    array: Rc<PhpArray>,
}

impl SpliceGuard {
    fn take(&mut self, key: &ArrayKey) -> Option<Value> {
        // SAFETY: The table was uniquely owned when the guard was created.
        // Later aliases can only arise during synchronous PHP callbacks, which
        // have returned before this method runs. No borrowed table/entry views
        // escape those callbacks. Rc is single-threaded and all aliases retain
        // exactly PhpArray. `take` only moves a Value and updates table metadata;
        // it executes no PHP and returns before the moved Value is released.
        // Thus no other Rc is dereferenced while this mutable borrow exists,
        // as required by Rc::get_mut_unchecked (nightly; equivalent below).
        unsafe { (&mut *Rc::as_ptr(&self.array).cast_mut()).take(key) }
    }

    fn finish(
        &mut self,
        plan: crate::array::DiscardSplice,
        replacement: &PhpArray,
        line: usize,
    ) -> Result<(), Error> {
        // SAFETY: Same exclusive access window as `take`. The replacement table
        // predates the guard and is distinct because input parsing separated it.
        // No PHP executes while rebuilding; object drops only enqueue work.
        unsafe {
            (&mut *Rc::as_ptr(&self.array).cast_mut()).finish_discard_splice(
                plan,
                replacement,
                line,
            )
        }
    }
}

fn modified(line: usize) -> Error {
    Error::new(
        ErrorKind::Error,
        "Array was modified during array_splice operation",
        line,
    )
}
