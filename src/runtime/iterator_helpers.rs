// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_iterators.c's iterator
// helpers and Zend/zend_API.c's array_set_zval_key. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::PhpObject;

impl Runtime {
    fn visit_iterator(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
        mut visit: impl FnMut(&mut Runtime, &Rc<PhpObject>) -> Result<bool, Error>,
    ) -> Result<(), Error> {
        let iterator = self.resolve_iterator(object, false, line)?;
        let wrapper = self.allocate_object_handle();
        let result = (|| {
            drop(self.iterator_method(&iterator, b"rewind", line)?);
            self.drain_destructors(Ok(()), line)?;
            loop {
                let valid = self.iterator_method(&iterator, b"valid", line)?;
                let more = valid.to_bool(line, self);
                drop(valid);
                if !self.drain_destructors(more, line)? {
                    break;
                }
                if !visit(self, &iterator)? {
                    break;
                }
                drop(self.iterator_method(&iterator, b"next", line)?);
                self.drain_destructors(Ok(()), line)?;
            }
            Ok(())
        })();
        let cleanup = self.recursive_iterator_release(&iterator, line);
        let result = result.and(cleanup);
        drop(iterator);
        let result = self.drain_destructors(result, line);
        drop(wrapper);
        self.drain_destructors(result, line)
    }

    pub(super) fn iterator_to_array(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        const NAME: &str = "iterator_to_array";
        let value = args[0].as_ref().unwrap();
        require_iterable(value, NAME, line)?;
        let preserve_keys = args[1]
            .as_ref()
            .map(|value| self.native_bool(value, NAME, 2, line))
            .transpose()?
            .unwrap_or(true);
        let mut array = PhpArray::default();
        if let Value::Array(input) = value {
            if preserve_keys {
                return Ok(value.clone());
            }
            for (_, ref value) in input.iter() {
                array.append(value.clone(), line)?;
            }
            return Ok(Value::Array(Rc::new(array)));
        }
        let Value::Object(object) = value else {
            unreachable!()
        };
        self.visit_iterator(object, line, |runtime, iterator| {
            // PHP keeps current cached until advancing. Preserve the raw zval:
            // a reference-returning current() also yields references in the array.
            let current = runtime.iterator_method(iterator, b"current", line)?;
            let result = (|| {
                if preserve_keys {
                    let key = runtime
                        .iterator_method(iterator, b"key", line)?
                        .dereferenced();
                    let converted = ArrayKey::from_value(&key, line, runtime)?;
                    array.insert(converted, current.clone());
                    // A duplicate-key replacement destroys its previous value
                    // while both the current result and key are still retained.
                    let result = runtime.drain_destructors(Ok(true), line);
                    drop(key);
                    runtime.drain_destructors(result, line)
                } else {
                    array.append(current.clone(), line)?;
                    runtime.drain_destructors(Ok(true), line)
                }
            })();
            drop(current);
            runtime.drain_destructors(result, line)
        })?;
        Ok(Value::Array(Rc::new(array)))
    }

    pub(super) fn iterator_count(&mut self, value: &Value, line: usize) -> Result<Value, Error> {
        require_iterable(value, "iterator_count", line)?;
        if let Value::Array(array) = value {
            return Ok(Value::Int(array.len() as i64));
        }
        let Value::Object(object) = value else {
            unreachable!()
        };
        let mut count = 0;
        self.visit_iterator(object, line, |_, _| {
            if count == i64::MAX {
                return Ok(false);
            }
            count += 1;
            Ok(true)
        })?;
        Ok(Value::Int(count))
    }

    pub(super) fn iterator_apply(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        const NAME: &str = "iterator_apply";
        let value = args[0].as_ref().unwrap();
        let Value::Object(object) = value else {
            return Err(builtins::type_error(value, NAME, 1, "Traversable", line));
        };
        if !object.class.is_a(b"Traversable") {
            return Err(builtins::type_error(value, NAME, 1, "Traversable", line));
        }
        let mut callback = self
            .native_callback(args[1].as_ref().unwrap(), NAME, 2, false, line)?
            .unwrap();
        callback.reference_values = true;
        let arguments = match args[2].as_ref() {
            None | Some(Value::Null) => None,
            Some(Value::Array(array)) => Some(array),
            Some(value) => return Err(builtins::type_error(value, NAME, 3, "?array", line)),
        };
        let mut count = 0_i64;
        self.visit_iterator(object, line, |runtime, _| {
            count = count.wrapping_add(1);
            // Validate the supplied array's named/positional ordering when the
            // callback is actually invoked, including mutable referenced values.
            let arguments = arguments
                .map(|array| callbacks::callback_arguments(array, line))
                .transpose()?
                .unwrap_or_default();
            let strict = std::mem::replace(&mut runtime.strict_types, false);
            let result = runtime.invoke_at(callback.clone(), &arguments, line, true, true);
            runtime.strict_types = strict;
            let result = result?;
            let more = result.to_bool(line, runtime);
            drop(result);
            runtime.drain_destructors(more, line)
        })?;
        Ok(Value::Int(count))
    }
}

fn require_iterable(value: &Value, name: &str, line: usize) -> Result<(), Error> {
    if matches!(value, Value::Array(_))
        || matches!(value, Value::Object(object) if object.class.is_a(b"Traversable"))
    {
        Ok(())
    } else {
        Err(builtins::type_error(
            value,
            name,
            1,
            "Traversable|array",
            line,
        ))
    }
}
