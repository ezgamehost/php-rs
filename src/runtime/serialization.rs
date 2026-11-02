// Altered Rust adaptation of PHP 8.5.0 ext/standard/var.c and
// Zend/zend_interfaces.c. Copyright (c) The PHP Group and Zend Technologies Ltd.;
// see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::PhpObject;

type Identity = (bool, usize);
#[derive(Default)]
pub(super) struct SerializeState {
    next: usize,
    identities: HashMap<Identity, Option<usize>>,
    // Retain tracked zvals in insertion order so object handles cannot be reused.
    retained: Vec<Value>,
}

fn encode_string(bytes: &[u8], output: &mut Vec<u8>) {
    output.extend(format!("s:{}:\"", bytes.len()).bytes());
    output.extend_from_slice(bytes);
    output.extend_from_slice(b"\";");
}
fn encode_key(key: &ArrayKey, output: &mut Vec<u8>) {
    match key {
        ArrayKey::Int(index) => output.extend(format!("i:{index};").bytes()),
        ArrayKey::String(bytes) => encode_string(bytes, output),
    }
}
fn identity(value: &Value) -> Option<Identity> {
    match value {
        Value::Reference(reference) => match &*reference.borrow() {
            Value::Object(object) => Some((true, object.object_id)),
            Value::Closure(closure) => Some((true, closure.object_id)),
            _ => Some((false, Rc::as_ptr(reference) as usize)),
        },
        Value::Object(object) => Some((true, object.object_id)),
        Value::Closure(closure) => Some((true, closure.object_id)),
        _ => None,
    }
}

impl Runtime {
    pub(super) fn serialize_value(&mut self, value: &Value, line: usize) -> Result<Value, Error> {
        let previous = self.serialization.clone();
        let state = previous.clone().unwrap_or_default();
        self.serialization = Some(state.clone());
        let mut output = Vec::new();
        // Function arguments are passed by value, including a reference root.
        let result = self.encode_serialized(&value.dereferenced(), &state, &mut output, 0, line);
        self.serialization = previous;
        drop(state);
        self.drain_destructors(result.map(|()| Value::String(output.into())), line)
    }

    pub(super) fn serialize_native(
        &mut self,
        line: usize,
        action: impl FnOnce(&mut Self, &Rc<RefCell<SerializeState>>, &mut Vec<u8>) -> Result<(), Error>,
    ) -> Result<Value, Error> {
        let previous = self.serialization.clone();
        let state = previous.clone().unwrap_or_default();
        self.serialization = Some(state.clone());
        let mut output = Vec::new();
        let result = action(self, &state, &mut output);
        self.serialization = previous;
        drop(state);
        self.drain_destructors(result.map(|()| Value::String(output.into())), line)
    }

    fn serialization_hook(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        isolated: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let method = objects::method(&object.class, name).unwrap();
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        let previous = if isolated {
            self.serialization.take()
        } else {
            None
        };
        let result = self.invoke_at(target, &[], line, false, true);
        if isolated {
            self.serialization = previous;
        }
        result
    }

    pub(super) fn encode_serialized(
        &mut self,
        value: &Value,
        state: &Rc<RefCell<SerializeState>>,
        output: &mut Vec<u8>,
        depth: usize,
        line: usize,
    ) -> Result<(), Error> {
        if depth > 1024 {
            return Err(Error::new(
                ErrorKind::Error,
                "Serialization nesting level too deep",
                line,
            ));
        }
        {
            let mut state = state.borrow_mut();
            state.next += 1;
            if let Some(identity) = identity(value) {
                if let Some(previous) = state.identities.get(&identity).copied() {
                    if let Some(index) = previous {
                        let is_reference = matches!(value, Value::Reference(_));
                        if is_reference {
                            state.next -= 1;
                        }
                        output.extend(
                            format!("{}:{index};", if is_reference { 'R' } else { 'r' }).bytes(),
                        );
                    } else {
                        output.extend_from_slice(b"N;");
                    }
                    return Ok(());
                }
                let index = state.next;
                state.identities.insert(identity, Some(index));
                state.retained.push(value.clone());
            }
        }
        match value.dereferenced() {
            Value::Null => output.extend_from_slice(b"N;"),
            Value::Bool(value) => output.extend_from_slice(if value { b"b:1;" } else { b"b:0;" }),
            Value::Int(value) => output.extend(format!("i:{value};").bytes()),
            Value::Float(value) => {
                output.extend(format!("d:{};", crate::value::shortest_float(value)).bytes())
            }
            Value::String(value) => encode_string(&value.bytes(), output),
            Value::Resource(_) => output.extend_from_slice(b"i:0;"),
            Value::Closure(_) => {
                return Err(Error::new(
                    ErrorKind::Exception,
                    "Serialization of 'Closure' is not allowed",
                    line,
                ));
            }
            Value::Array(array) => {
                output.extend_from_slice(b"a:");
                self.encode_serialized_table(&array, state, output, depth, true, line)?;
            }
            Value::Object(object) => {
                self.encode_serialized_object(&object, state, output, depth, line)?
            }
            Value::Reference(_) | Value::Indirect(_) => unreachable!(),
        }
        Ok(())
    }

    fn encode_serialized_table(
        &mut self,
        array: &PhpArray,
        state: &Rc<RefCell<SerializeState>>,
        output: &mut Vec<u8>,
        depth: usize,
        is_array: bool,
        line: usize,
    ) -> Result<(), Error> {
        output.extend(format!("{}:{{", array.len()).bytes());
        for (key, value) in array.iter_copied() {
            encode_key(&key, output);
            if is_array
                && matches!(&value, Value::Array(child) if std::ptr::eq(child.as_ref(), array))
            {
                state.borrow_mut().next += 1;
                output.extend_from_slice(b"N;");
            } else {
                self.encode_serialized(&value, state, output, depth + 1, line)?;
            }
            drop(value);
            self.drain_destructors(Ok(()), line)?;
        }
        output.push(b'}');
        Ok(())
    }

    fn encode_serialized_object(
        &mut self,
        object: &Rc<PhpObject>,
        state: &Rc<RefCell<SerializeState>>,
        output: &mut Vec<u8>,
        depth: usize,
        line: usize,
    ) -> Result<(), Error> {
        let class = &object.class;
        let forbidden = class.name.contains(&0)
            || class.is_a(b"InternalIterator")
            || class.is_a(b"SensitiveParameterValue")
            || [
                b"ReflectionClass".as_slice(),
                b"ReflectionFunctionAbstract",
                b"ReflectionType",
                b"ReflectionParameter",
                b"ReflectionProperty",
                b"ReflectionClassConstant",
                b"ReflectionConstant",
                b"ReflectionAttribute",
                b"ReflectionExtension",
            ]
            .iter()
            .any(|name| class.is_a(name));
        if forbidden {
            return Err(Error::new(
                ErrorKind::Exception,
                format!(
                    "Serialization of '{}' is not allowed",
                    object.class_name.split('\0').next().unwrap()
                ),
                line,
            ));
        }
        if class.declaration.is_enum {
            let name = object
                .properties
                .borrow()
                .get(&ArrayKey::String(b"name".to_vec()))
                .unwrap()
                .to_bytes();
            output.extend(format!("E:{}:\"", class.name.len() + 1 + name.len()).bytes());
            output.extend(&class.name);
            output.push(b':');
            output.extend(name);
            output.extend_from_slice(b"\";");
            return Ok(());
        }
        let mut properties = if objects::method(class, b"__serialize").is_some() {
            let value = self.serialization_hook(object, b"__serialize", true, line)?;
            let Value::Array(array) = value else {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!("{}::__serialize() must return an array", object.class_name),
                    line,
                ));
            };
            array
        } else if class.is_a(b"Serializable") {
            let value = self.serialization_hook(object, b"serialize", false, line)?;
            return match value {
                Value::Null => {
                    state
                        .borrow_mut()
                        .identities
                        .insert((true, object.object_id), None);
                    output.extend_from_slice(b"N;");
                    Ok(())
                }
                Value::String(bytes) => {
                    output.extend(format!("C:{}:\"", class.name.len()).bytes());
                    output.extend(&class.name);
                    output.extend(format!("\":{}:{{", bytes.len()).bytes());
                    output.extend(bytes.bytes().iter());
                    output.push(b'}');
                    Ok(())
                }
                _ => Err(Error::new(
                    ErrorKind::Exception,
                    format!(
                        "{}::serialize() must return a string or NULL",
                        object.class_name
                    ),
                    line,
                )),
            };
        } else if objects::method(class, b"__sleep").is_some() {
            let names = self.serialization_hook(object, b"__sleep", true, line)?;
            let Value::Array(names) = names else {
                self.warn(format!("Warning: serialize(): {}::__sleep() should return an array only containing the names of instance-variables to serialize on line {line}", object.class_name))?;
                output.extend_from_slice(b"N;");
                return Ok(());
            };
            Rc::new(self.serialize_sleep_properties(object, &names, line)?)
        } else {
            Rc::new(object.snapshot())
        };
        let mut name = class.name.clone();
        if class.name == b"__PHP_Incomplete_Class" {
            let key = ArrayKey::String(b"__PHP_Incomplete_Class_Name".to_vec());
            if let Some(Value::String(original)) = properties.get(&key) {
                name = original.to_vec();
                Rc::make_mut(&mut properties).remove(&key);
            }
        }
        output.extend(format!("O:{}:\"", name.len()).bytes());
        output.extend(&name);
        output.extend_from_slice(b"\":");
        let result = self.encode_serialized_table(&properties, state, output, depth, false, line);
        drop(properties);
        self.drain_destructors(result, line)
    }

    fn serialize_sleep_properties(
        &mut self,
        object: &PhpObject,
        names: &PhpArray,
        line: usize,
    ) -> Result<PhpArray, Error> {
        let properties = object.snapshot();
        let mut result = PhpArray::default();
        for (_, value) in names.iter() {
            let value = value.dereferenced();
            if !matches!(value, Value::String(_)) {
                self.warn(format!("Warning: serialize(): {}::__sleep() should return an array only containing the names of instance-variables to serialize on line {line}", object.class_name))?;
            }
            let name = self.string_bytes(&value, line)?;
            let mut private = vec![0];
            private.extend(&object.class.name);
            private.push(0);
            private.extend(&name);
            let mut protected = b"\0*\0".to_vec();
            protected.extend(&name);
            let mut found = false;
            for key in [name.clone(), private, protected]
                .into_iter()
                .map(ArrayKey::String)
            {
                if let Some(value) =
                    properties.with_value(&key, |value| value.map(Value::copy_array_entry))
                {
                    if result.get(&key).is_some() {
                        self.warn(format!("Warning: serialize(): \"{}\" is returned from __sleep() multiple times on line {line}", String::from_utf8_lossy(&name)))?;
                    } else {
                        result.insert(key, value);
                    }
                    found = true;
                    break;
                }
                if object.class.properties.iter().any(|property| {
                    property.ty.is_some() && key == ArrayKey::String(property.storage_name())
                }) {
                    found = true;
                    break;
                }
            }
            if !found {
                self.warn(format!("Warning: serialize(): \"{}\" returned as member variable from __sleep() but does not exist on line {line}", String::from_utf8_lossy(&name)))?;
            }
        }
        Ok(result)
    }
}
