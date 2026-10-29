use super::*;
use crate::{
    PhpObject,
    closure::{CapturedValue, Environment},
};

impl Runtime {
    pub(super) fn clone_value(
        &mut self,
        original: &Value,
        updates: Option<&Value>,
        line: usize,
    ) -> Result<Value, Error> {
        if !matches!(original, Value::Object(_) | Value::Closure(_)) {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "clone(): Argument #1 ($object) must be of type object, {} given",
                    builtins::debug_type(original)
                ),
                line,
            ));
        }
        let updates = match updates {
            None => None,
            Some(Value::Array(array)) => Some(array),
            Some(value) => {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!(
                        "clone(): Argument #2 ($withProperties) must be of type array, {} given",
                        builtins::debug_type(value)
                    ),
                    line,
                ));
            }
        };
        if let Value::Object(object) = original
            && (object.class.declaration.is_enum
                || object.class.is_a(b"ReflectionClass")
                || object.class.is_a(b"ReflectionAttribute")
                || object.class.is_a(b"ReflectionFunctionAbstract")
                || object.class.is_a(b"ReflectionType")
                || object.class.is_a(b"ReflectionParameter")
                || object.class.is_a(b"ReflectionProperty")
                || object.class.is_a(b"ReflectionClassConstant")
                || object.class.is_a(b"ReflectionConstant")
                || object.class.is_a(b"ReflectionExtension"))
        {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Trying to clone an uncloneable object of class {}",
                    object.class_name
                ),
                line,
            ));
        }
        if let Value::Object(object) = original
            && let Some(method) = objects::method(&object.class, b"__clone")
        {
            self.check_method_access(&method, line)?;
        }
        let handle = self.allocate_object_handle();
        if let Value::Closure(closure) = original {
            if let Some((key, _)) = updates.and_then(|a| a.entries().first()) {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Cannot create dynamic property Closure::${}",
                        String::from_utf8_lossy(&property_name(key))
                    ),
                    line,
                ));
            }
            let environment = closure.environment.as_ref().map(|env| Environment {
                captures: env
                    .captures
                    .iter()
                    .map(|(name, value)| {
                        (
                            name.clone(),
                            match value {
                                CapturedValue::Value(value) => CapturedValue::Value(value.clone()),
                                CapturedValue::Reference(value) => {
                                    CapturedValue::Reference(value.clone())
                                }
                            },
                        )
                    })
                    .collect(),
                statics: RefCell::new(
                    env.statics
                        .borrow()
                        .iter()
                        .map(|(name, value)| (name.clone(), cell(value.borrow().clone())))
                        .collect(),
                ),
            });
            return Ok(Value::Closure(Rc::new(crate::Closure {
                magic_name: closure.magic_name.clone(),
                name: closure.name.clone(),
                this: closure.this.clone(),
                context: closure.context.clone(),
                function: closure.function.clone(),
                environment,
                object_id: handle.id,
                _handle: handle,
            })));
        }
        let Value::Object(original) = original else {
            unreachable!()
        };
        if original.class.is_a(b"Throwable") {
            return Err(Error::new(
                ErrorKind::Error,
                "Trying to clone an uncloneable throwable object",
                line,
            ));
        }
        let object = Rc::new(PhpObject {
            reflection: RefCell::default(),
            throwable: RefCell::new(None),
            class: original.class.clone(),
            class_name: original.class_name.clone(),
            properties: RefCell::new(original.properties.borrow().clone()),
            property_cursor: RefCell::new(original.property_cursor.borrow().clone()),
            unset_properties: RefCell::new(original.unset_properties.borrow().clone()),
            reinitializable: RefCell::new(original.reinitializable.borrow().clone()),
            object_id: handle.id,
            _handle: handle,
        });
        self.inherit_reference_sources(&object);
        if let Some(method) = objects::method(&object.class, b"__clone") {
            unlock_readonly(&object);
            let result = self
                .method_target(method, object.class.clone(), Some(object.clone()), line)
                .and_then(|target| self.invoke(target, &[], line, false));
            // Escaped copies must be locked even if the method fails.
            object.reinitializable.borrow_mut().clear();
            result?;
        }
        if let Some(updates) = updates.filter(|a| !a.entries().is_empty()) {
            // PHP 8.5.0 clears each flag on a successful write, but retains
            // flags for untouched slots after clone-with (also after errors).
            unlock_readonly(&object);
            for (key, value) in updates.entries() {
                if value.is_shared_reference() {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Cannot assign by reference when cloning with updated properties",
                        line,
                    ));
                }
                let value = value.dereferenced();
                let mut location = Location {
                    offset_reference: None,
                    overloaded: None,
                    object: Some(object.clone()),
                    static_class: None,
                    name: property_name(key),
                    global: false,
                    keys: Vec::new(),
                };
                // Clone-with writes through an internal function and therefore
                // uses weak property coercion even in a strict caller.
                let outer_strict = std::mem::replace(&mut self.strict_types, false);
                let result = self.write_location(&mut location, value.clone(), line);
                self.strict_types = outer_strict;
                result?;
            }
        }
        Ok(Value::Object(object))
    }
}

fn unlock_readonly(object: &PhpObject) {
    object.reinitializable.borrow_mut().extend(
        object
            .class
            .properties
            .iter()
            .filter(|p| p.is_readonly)
            .map(|p| p.storage_name()),
    );
}

fn property_name(key: &ArrayKey) -> Vec<u8> {
    match key {
        ArrayKey::String(name) => name.clone(),
        ArrayKey::Int(index) => index.to_string().into_bytes(),
    }
}
