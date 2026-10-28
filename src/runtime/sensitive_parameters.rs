use super::*;

impl Runtime {
    pub(super) fn sensitive_parameter_value(&mut self, value: Value) -> Value {
        let class = self
            .find_class(b"SensitiveParameterValue")
            .expect("native sensitive value class");
        let mut properties = PhpArray::default();
        properties.insert(
            ArrayKey::String(class.properties[0].storage_name()),
            value.dereferenced(),
        );
        self.next_object_id += 1;
        Value::Object(Rc::new(crate::PhpObject {
            class_name: "SensitiveParameterValue".into(),
            class,
            properties: RefCell::new(properties),
            unset_properties: RefCell::default(),
            reinitializable: RefCell::default(),
            reflection: RefCell::default(),
            throwable: RefCell::default(),
            object_id: self.next_object_id,
        }))
    }

    pub(super) fn native_sensitive_value(&mut self, line: usize) -> Result<Value, Error> {
        let method = self
            .current_function
            .as_ref()
            .unwrap()
            .rsplit(|byte| *byte == b':')
            .next()
            .unwrap();
        match method {
            b"__construct" => self.native_attribute_constructor(line),
            b"__debuginfo" => Ok(Value::Array(Rc::new(PhpArray::default()))),
            b"getvalue" => {
                let Some(Value::Object(object)) = self.lookup(b"this") else {
                    unreachable!()
                };
                let key = ArrayKey::String(object.class.properties[0].storage_name());
                Ok(object
                    .properties
                    .borrow()
                    .get(&key)
                    .map_or(Value::Null, Value::dereferenced))
            }
            _ => unreachable!(),
        }
    }
}
