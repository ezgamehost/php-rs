// Altered Rust adaptation of PHP 8.5.0's count()/php_count_recursive() in
// ext/standard/array.c and is_countable() in ext/standard/type.c.
// See THIRD_PARTY_NOTICES.md.
use super::*;

pub(super) fn is_countable(value: &Value) -> bool {
    matches!(value, Value::Array(_))
        || matches!(value, Value::Object(object) if object.class.is_a(b"Countable"))
}

impl Runtime {
    pub(super) fn count_value(
        &mut self,
        value: &Value,
        mode: Option<&Value>,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        // The first parameter is parsed as an arbitrary zval. Validate the
        // optional mode before checking whether that value can be counted.
        let mode = mode
            .map(|mode| self.native_int(mode, name, 2, line))
            .transpose()?
            .unwrap_or(0);
        if !matches!(mode, 0 | 1) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!(
                    "{name}(): Argument #2 ($mode) must be either COUNT_NORMAL or COUNT_RECURSIVE"
                ),
                line,
            ));
        }
        match value {
            Value::Array(array) if mode == 0 => Ok(Value::Int(array.len() as i64)),
            Value::Array(array) => self.count_recursive(array, name, line).map(Value::Int),
            Value::Object(object) if object.class.is_a(b"Countable") => {
                let method = objects::method(&object.class, b"count").expect("Countable method");
                let target = self.unchecked_method_target(
                    method,
                    object.class.clone(),
                    Some(object.clone()),
                    line,
                )?;
                let result = self
                    .invoke_at(target, &[], line, false, true)?
                    .dereferenced();
                // Native count consumes the callback result using zval_get_long
                // with non-strict conversion, regardless of the caller's mode.
                // Doubles wrap without cast diagnostics; numeric strings use
                // the saturating, warning-producing string conversion.
                let converted = match result {
                    Value::Float(number) => Ok(Value::Int(crate::value::float_to_int(number))),
                    value => casting::cast(crate::ast::CastKind::Int, value, line, self),
                };
                self.drain_destructors(converted, line)
            }
            value => Err(builtins::type_error(
                value,
                name,
                1,
                "Countable|array",
                line,
            )),
        }
    }

    fn count_recursive(&mut self, array: &PhpArray, name: &str, line: usize) -> Result<i64, Error> {
        let identity = array as *const PhpArray as usize;
        if !self.counting_arrays.insert(identity) {
            self.warn(format!("{name}(): Recursion detected on line {line}"))?;
            return Ok(0);
        }
        // Keep recursion protection across calls made by a warning handler,
        // but release it even when that handler throws or exits.
        let result = (|| {
            let mut count = array.len() as i64;
            for (_, value) in array.entries() {
                if let Value::Array(child) = value.dereferenced() {
                    count += self.count_recursive(&child, name, line)?;
                }
            }
            Ok(count)
        })();
        self.counting_arrays.remove(&identity);
        result
    }
}
