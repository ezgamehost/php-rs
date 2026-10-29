use super::*;

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
            &mut self.diagnostics,
        )?;
        let Value::Array(replacement) = replacement else {
            unreachable!()
        };
        let removed = with_array(&mut source.borrow_mut(), |array| {
            let (start, end) = substrings::substring_range(array.len(), offset, length);
            Rc::make_mut(array).splice(start, end, &replacement, result_used, line)
        })?;
        Ok(Value::Array(Rc::new(removed)))
    }
}

fn with_array<T>(value: &mut Value, action: impl FnOnce(&mut Rc<PhpArray>) -> T) -> T {
    match value {
        Value::Array(array) => action(array),
        Value::Reference(reference) | Value::Indirect(reference) => {
            with_array(&mut reference.borrow_mut(), action)
        }
        _ => unreachable!("array argument was validated before index coercion"),
    }
}
