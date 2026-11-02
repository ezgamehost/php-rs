// Altered Rust adaptation of PHP 8.5.0's ext/reflection/php_reflection.c.
// See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn reflection_reference_from_array(&mut self, line: usize) -> Result<Value, Error> {
        let Value::Array(array) = self.lookup(b"array").unwrap() else {
            unreachable!()
        };
        let key = match self.lookup(b"key").unwrap() {
            Value::Int(key) => ArrayKey::Int(key),
            // Reflection uses a literal hash lookup, not numeric-string coercion.
            Value::String(key) => ArrayKey::String(key.to_vec()),
            _ => unreachable!(),
        };
        let reference = array
            .with_value(&key, |value| {
                value.map(|value| exposed_reference(value, &array))
            })
            .ok_or_else(|| self.reflection_error("Array key not found", line))?;
        Ok(reference.map_or(Value::Null, |reference| {
            self.reflection_object(b"ReflectionReference", ReflectionData::Reference(reference))
        }))
    }

    pub(super) fn reflection_reference_id(
        &mut self,
        reference: &Rc<RefCell<Value>>,
        line: usize,
    ) -> Result<Value, Error> {
        let key = match self.reflection_reference_key {
            Some(key) => key,
            None => {
                let mut key = [0; 16];
                random::fill(&mut key)
                    .map_err(|message| Error::new(ErrorKind::RandomException, message, line))?;
                self.reflection_reference_key = Some(key);
                key
            }
        };
        let mut digest = crate::digest::Digest::new(crate::digest::Algorithm::Sha1);
        digest.update(&(Rc::as_ptr(reference) as usize).to_ne_bytes());
        digest.update(&key);
        Ok(string(digest.finish()))
    }
}

fn exposed_reference(value: &Value, array: &Rc<PhpArray>) -> Option<Rc<RefCell<Value>>> {
    match value {
        Value::Indirect(slot) => exposed_reference(&slot.borrow(), array),
        Value::Reference(reference)
            if Rc::strong_count(reference) > 1
                || matches!(&*reference.borrow(), Value::Array(value) if Rc::ptr_eq(value, array)) =>
        {
            Some(reference.clone())
        }
        _ => None,
    }
}
