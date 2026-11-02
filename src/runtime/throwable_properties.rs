// Altered Rust adaptation of PHP 8.5.0 Zend/zend_exceptions.c and .stub.php.
// Copyright (c) Zend Technologies Ltd.; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    PhpObject,
    ast::{NativeMethod, Visibility},
    object::{Class, ClassContext, ThrowableData},
};

pub(super) fn property_key(class: &Class, name: &[u8]) -> ArrayKey {
    let private = matches!(name, b"string" | b"trace" | b"previous");
    if !private
        && let Some(property) = class
            .properties
            .iter()
            .find(|p| p.name == name && p.visibility != Visibility::Private)
    {
        return ArrayKey::String(property.storage_name());
    }
    let scope = if private {
        if class.is_a(b"Exception") {
            b"Exception".as_slice()
        } else {
            b"Error"
        }
    } else {
        b"*"
    };
    let mut key = vec![0];
    key.extend(scope);
    key.push(0);
    key.extend(name);
    ArrayKey::String(key)
}

impl PhpObject {
    pub(crate) fn throwable_property(&self, name: &[u8]) -> Value {
        self.properties
            .borrow()
            .get(&property_key(&self.class, name))
            .unwrap_or(Value::Null)
            .dereferenced()
    }
    pub(crate) fn set_throwable_property(&self, name: &[u8], value: Value) {
        let key = property_key(&self.class, name);
        let old = self.properties.borrow().get(&key);
        if let Some(Value::Reference(reference)) = old {
            *reference.borrow_mut() = value;
        } else {
            self.properties.borrow_mut().insert(key, value);
        }
    }
    pub(crate) fn throwable_previous(&self) -> Option<Rc<PhpObject>> {
        match self.throwable_property(b"previous") {
            Value::Object(object) => Some(object),
            _ => None,
        }
    }
    // Reporting paths need a detached view so no property borrow survives a PHP callback.
    pub(crate) fn throwable_snapshot(&self) -> ThrowableData {
        ThrowableData {
            message: self.throwable_property(b"message").to_bytes(),
            file: self.throwable_property(b"file").to_bytes(),
            line: match self.throwable_property(b"line") {
                Value::Int(line) => line,
                _ => 0,
            },
        }
    }
}

impl Runtime {
    fn throwable_scope(&self, object: &PhpObject) -> ClassContext {
        ClassContext {
            scope: self
                .find_class(if object.class.is_a(b"Exception") {
                    b"Exception"
                } else {
                    b"Error"
                })
                .unwrap(),
            called: object.class.clone(),
        }
    }
    fn read_throwable_property(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        silent: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let scope = self.throwable_scope(object);
        let previous = self.class_context.replace(scope);
        let result = self.read_property_value(
            object,
            name,
            line,
            if silent {
                Access::Coalesce
            } else {
                Access::Read
            },
        );
        self.class_context = previous;
        result
    }
    fn write_throwable_property(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        value: Value,
        line: usize,
    ) -> Result<(), Error> {
        let scope = self.throwable_scope(object);
        let previous = self.class_context.replace(scope);
        let result = self
            .write_property_value(object, name, value, line)
            .map(drop);
        self.class_context = previous;
        result
    }
    pub(super) fn throwable_method(
        &mut self,
        object: &Rc<PhpObject>,
        method: NativeMethod,
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        match method {
            NativeMethod::ThrowableClone => Err(Error::new(
                ErrorKind::Exception,
                "Cannot clone object using __clone()",
                line,
            )),
            NativeMethod::ThrowableWakeup => {
                for (name, is_message) in [(b"message".as_slice(), true), (b"code", false)] {
                    let value = self.read_throwable_property(object, name, true, line)?;
                    if !matches!(value, Value::Null)
                        && !matches!(
                            (&value, is_message),
                            (Value::String(_), true) | (Value::Int(_), false)
                        )
                    {
                        let key = property_key(&object.class, name);
                        object.remove_property(&key);
                    }
                }
                self.drain_destructors(Ok(Value::Null), line)
            }
            NativeMethod::ThrowableConstruct | NativeMethod::ErrorExceptionConstruct => {
                let message = self.lookup(b"message").unwrap();
                let code = self.lookup(b"code").unwrap();
                let previous = self.lookup(b"previous").unwrap();
                if supplied > 0 {
                    self.write_throwable_property(object, b"message", message, line)?;
                }
                if code != Value::Int(0) {
                    self.write_throwable_property(object, b"code", code, line)?;
                }
                if !matches!(previous, Value::Null) {
                    self.write_throwable_property(object, b"previous", previous, line)?;
                }
                if matches!(method, NativeMethod::ErrorExceptionConstruct) {
                    let severity = self.lookup(b"severity").unwrap();
                    let filename = self.lookup(b"filename").unwrap();
                    let lineno = self.lookup(b"line").unwrap();
                    self.write_throwable_property(object, b"severity", severity, line)?;
                    let has_filename = !matches!(filename, Value::Null);
                    if has_filename {
                        self.write_throwable_property(object, b"file", filename, line)?;
                    }
                    if !matches!(lineno, Value::Null) {
                        self.write_throwable_property(object, b"line", lineno, line)?;
                    } else if has_filename {
                        self.write_throwable_property(object, b"line", Value::Int(0), line)?;
                    }
                }
                Ok(Value::Null)
            }
            NativeMethod::GetMessage | NativeMethod::GetFile => {
                let name = if matches!(method, NativeMethod::GetMessage) {
                    b"message".as_slice()
                } else {
                    b"file"
                };
                let value = self.read_throwable_property(object, name, false, line)?;
                Ok(Value::String(self.string_bytes(&value, line)?.into()))
            }
            NativeMethod::GetLine => {
                let value = self.read_throwable_property(object, b"line", false, line)?;
                Ok(Value::Int(value.integer(line, self)?))
            }
            NativeMethod::GetCode
            | NativeMethod::GetTrace
            | NativeMethod::GetPrevious
            | NativeMethod::GetSeverity => {
                let name = match method {
                    NativeMethod::GetCode => b"code".as_slice(),
                    NativeMethod::GetTrace => b"trace",
                    NativeMethod::GetPrevious => b"previous",
                    _ => b"severity",
                };
                self.read_throwable_property(
                    object,
                    name,
                    matches!(method, NativeMethod::GetPrevious),
                    line,
                )
            }
            NativeMethod::GetTraceAsString => {
                let value = self.read_throwable_property(object, b"trace", true, line)?;
                let Value::Array(trace) = value else {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        "Exception trace must be an array",
                        line,
                    ));
                };
                Ok(Value::String(
                    self.throwable_trace_string(&trace, line)?.into(),
                ))
            }
            NativeMethod::ThrowableToString => self.native_throwable_string(object, line),
            _ => unreachable!("throwable method"),
        }
    }

    fn native_throwable_string(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let mut chain = Vec::new();
        let mut current = Some(object.clone());
        let mut seen = HashSet::new();
        while let Some(current_object) = current {
            if !seen.insert(current_object.object_id) {
                break;
            }
            let message = self.read_throwable_property(&current_object, b"message", false, line)?;
            let message = self.string_bytes(&message, line)?;
            let file = self.read_throwable_property(&current_object, b"file", false, line)?;
            let file = self.string_bytes(&file, line)?;
            let lineno = self
                .read_throwable_property(&current_object, b"line", false, line)?
                .integer(line, self)?;
            let method = objects::method(&current_object.class, b"getTraceAsString").unwrap();
            let target = self.unchecked_method_target(
                method,
                current_object.class.clone(),
                Some(current_object.clone()),
                line,
            )?;
            let trace = self.call_callback_at(&target, &[], line, true)?.to_bytes();
            let mut text = current_object.class.name.clone();
            if !message.is_empty() {
                text.extend(b": ");
                text.extend(&message);
                if matches!(
                    current_object.class.name.as_slice(),
                    b"TypeError" | b"ArgumentCountError"
                ) && message
                    .windows(b", called in ".len())
                    .any(|s| s == b", called in ")
                {
                    text.extend(b" and defined");
                }
            }
            text.extend(b" in ");
            text.extend(file);
            text.extend(format!(":{lineno}\nStack trace:\n").bytes());
            text.extend(trace);
            chain.push(text);
            current = current_object.throwable_previous();
        }
        let mut result = Vec::new();
        for (index, text) in chain.iter().rev().enumerate() {
            if index != 0 {
                result.extend(b"\n\nNext ");
            }
            result.extend(text);
        }
        let value = Value::String(result.into());
        self.write_throwable_property(object, b"string", value.clone(), line)?;
        Ok(value)
    }

    fn throwable_trace_string(&mut self, trace: &PhpArray, line: usize) -> Result<Vec<u8>, Error> {
        let mut output = Vec::new();
        let mut number = 0;
        for (key, value) in trace.iter() {
            let Value::Array(frame) = value else {
                let index = match key {
                    ArrayKey::Int(index) => index as u64,
                    _ => 0,
                };
                self.warn(format!(
                    "Warning: Expected array for frame {index} on line {line}"
                ))?;
                continue;
            };
            output.extend(format!("#{number} ").bytes());
            number += 1;
            let get = |key: &[u8]| frame.get(&ArrayKey::String(key.to_vec()));
            if let Some(file) = get(b"file") {
                if let Value::String(file) = file {
                    let lineno = match get(b"line") {
                        Some(Value::Int(line)) => line,
                        None => 0,
                        _ => {
                            self.warn(format!("Warning: Line is not an int on line {line}"))?;
                            0
                        }
                    };
                    output.extend(file.bytes().iter());
                    output.extend(format!("({lineno}): ").bytes());
                } else {
                    self.warn(format!("Warning: File name is not a string on line {line}"))?;
                    output.extend(b"[unknown file]: ");
                }
            } else {
                output.extend(b"[internal function]: ");
            }
            for key in [b"class".as_slice(), b"type", b"function"] {
                if let Some(value) = get(key) {
                    if let Value::String(value) = value {
                        output.extend(value.bytes().iter());
                    } else {
                        self.warn(format!(
                            "Warning: Value for {} is not a string on line {line}",
                            String::from_utf8_lossy(key)
                        ))?;
                        output.extend(b"[unknown]");
                    }
                }
            }
            output.push(b'(');
            if let Some(args) = get(b"args") {
                if let Value::Array(args) = args {
                    for (index, (key, value)) in args.iter().enumerate() {
                        if index != 0 {
                            output.extend(b", ");
                        }
                        if let ArrayKey::String(name) = key {
                            output.extend(name);
                            output.extend(b": ");
                        }
                        traces::format_argument(&mut output, &value);
                    }
                } else {
                    self.warn(format!(
                        "Warning: args element is not an array on line {line}"
                    ))?;
                }
            }
            output.extend(b")\n");
        }
        output.extend(format!("#{number} {{main}}").bytes());
        Ok(output)
    }
}
