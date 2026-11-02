// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_array.c and
// ext/spl/spl_array.stub.php. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    PhpObject,
    ast::{Argument, NativeMethod},
    object::{Class, PropertyCursor},
};

pub(crate) struct ArrayIteratorData {
    pub storage: variables::Cell,
    pub flags: i64,
    cursor: PropertyCursor,
}

impl Default for ArrayIteratorData {
    fn default() -> Self {
        Self {
            storage: cell(Value::Array(Rc::new(PhpArray::default()))),
            flags: 0,
            cursor: PropertyCursor::default(),
        }
    }
}

impl ArrayIteratorData {
    pub(super) fn cloned_from(object: &Rc<PhpObject>, data: &Self) -> Self {
        Self {
            // PHP ArrayIterator clones retain the original as backing storage;
            // the new object's cursor is independent and starts at the beginning.
            storage: cell(if matches!(*data.storage.borrow(), Value::Null) {
                Value::Null
            } else {
                Value::Object(object.clone())
            }),
            flags: data.flags,
            cursor: PropertyCursor::default(),
        }
    }
}

pub(super) enum Storage {
    Array(variables::Cell),
    Object(Rc<PhpObject>),
}

impl Storage {
    pub(super) fn snapshot(&self) -> PhpArray {
        match self {
            Self::Array(cell) => match &*cell.borrow() {
                Value::Array(array) => (**array).clone(),
                _ => unreachable!(),
            },
            Self::Object(object) => object.snapshot(),
        }
    }
    fn keys(&self) -> Vec<ArrayKey> {
        self.snapshot()
            .entries()
            .iter()
            .filter(|(key, _)| {
                !matches!(self, Self::Object(_))
                    || !matches!(key, ArrayKey::String(key) if key.first() == Some(&0))
            })
            .map(|(key, _)| key.clone())
            .collect()
    }
    pub(super) fn read(&self, key: &ArrayKey) -> Option<Value> {
        match self {
            Self::Array(cell) => match &*cell.borrow() {
                Value::Array(array) => array.get(key).cloned(),
                _ => unreachable!(),
            },
            Self::Object(object) => object.properties.borrow().get(key).cloned(),
        }
    }
}

pub(super) fn storage(object: &Rc<PhpObject>) -> Storage {
    let mut object = object.clone();
    loop {
        let source = object
            .array_iterator
            .borrow()
            .as_ref()
            .unwrap()
            .storage
            .clone();
        let value = source.borrow().clone();
        match value {
            Value::Array(_) => return Storage::Array(source),
            Value::Null => return Storage::Object(object),
            Value::Object(inner) if inner.array_iterator.borrow().is_some() => object = inner,
            Value::Object(inner) => return Storage::Object(inner),
            _ => unreachable!(),
        }
    }
}

pub(crate) fn export_storage(object: &PhpObject) -> PhpArray {
    let value = object
        .array_iterator
        .borrow()
        .as_ref()
        .unwrap()
        .storage
        .borrow()
        .clone();
    match value {
        Value::Array(array) => (*array).clone(),
        Value::Object(inner) if inner.array_iterator.borrow().is_some() => export_storage(&inner),
        Value::Object(inner) => inner.snapshot(),
        Value::Null => object.snapshot(),
        _ => unreachable!(),
    }
}

pub(super) fn native_offset_get(object: &PhpObject) -> bool {
    object.array_iterator.borrow().is_some()
        && objects::method(&object.class, b"offsetGet")
            .is_some_and(|method| matches!(method.native, Some(NativeMethod::ArrayIterator)))
}

pub(super) fn native_iteration(object: &PhpObject) -> bool {
    object.array_iterator.borrow().is_some()
        && [b"rewind".as_slice(), b"valid", b"current", b"key", b"next"]
            .iter()
            .all(|name| {
                objects::method(&object.class, name).is_some_and(|method| {
                    matches!(method.native, Some(NativeMethod::ArrayIterator))
                })
            })
}

impl Runtime {
    pub(super) fn builtin_array_iterator_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        if name != b"arrayiterator" {
            return None;
        }
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let code =
            b"<?php class ArrayIterator implements SeekableIterator, ArrayAccess, Countable {
            public const int STD_PROP_LIST = 1, ARRAY_AS_PROPS = 2;
            public function __construct(array|object $array = [], int $flags = 0) {}
            public function offsetExists(mixed $key): bool {}
            public function offsetGet(mixed $key): mixed {}
            public function offsetSet(mixed $key, mixed $value): void {}
            public function offsetUnset(mixed $key): void {}
            public function append(mixed $value): void {}
            public function getArrayCopy(): array {}
            public function count(): int {}
            public function getFlags(): int {}
            public function setFlags(int $flags): void {}
            public function rewind(): void {}
            public function current(): mixed {}
            public function key(): string|int|null {}
            public function next(): void {}
            public function valid(): bool {}
            public function seek(int $offset): void {}
            public function __debugInfo(): array {}
        }";
        let (mut statements, _, _) =
            parser::parse(lexer::lex(code).expect("native ArrayIterator source"))
                .expect("native ArrayIterator declaration");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::ArrayIterator);
            method.tentative_return_type = method.return_type.take();
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            declaration,
            parent: None,
            interfaces: [b"SeekableIterator".as_slice(), b"ArrayAccess", b"Countable"]
                .iter()
                .map(|name| self.find_class(name).unwrap())
                .collect(),
            properties: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic: false,
            defaults: RefCell::default(),
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        });
        self.builtin_classes
            .borrow_mut()
            .insert(name.to_vec(), class.clone());
        Some(class)
    }

    pub(super) fn native_array_iterator_method(
        &mut self,
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let name = self
            .current_function
            .as_ref()
            .unwrap()
            .rsplit(|b| *b == b':')
            .next()
            .unwrap()
            .to_vec();
        match name.as_slice() {
            b"__construct" => {
                if supplied == 0 {
                    return Ok(Value::Null);
                }
                let value = self.lookup(b"array").unwrap().dereferenced();
                let mut flags = self.lookup(b"flags").unwrap().integer(line, self)? & 0xffff;
                let value = match value {
                    Value::Array(array) => Value::Array(Rc::new((*array).clone())),
                    Value::Object(backing) => {
                        self.warn(format!("Deprecated: ArrayIterator::__construct(): Using an object as a backing array for ArrayIterator is deprecated, as it allows violating class constraints and invariants on line {line}"))?;
                        if backing.class.declaration.is_enum {
                            return Err(self.array_iterator_exception(
                                b"InvalidArgumentException",
                                "Enums are not compatible with ArrayIterator".into(),
                                line,
                            ));
                        }
                        if supplied == 1
                            && let Some(data) = backing.array_iterator.borrow().as_ref()
                        {
                            flags = data.flags;
                        }
                        if Rc::ptr_eq(&object, &backing) {
                            Value::Null
                        } else {
                            Value::Object(backing)
                        }
                    }
                    value => {
                        return Err(builtins::type_error(
                            &value,
                            "ArrayIterator::__construct",
                            1,
                            "array|object",
                            line,
                        ));
                    }
                };
                // Reinitialization accumulates public flags; setFlags() is the
                // API that replaces them. Storage/cursor are replaced below.
                flags |= object.array_iterator.borrow().as_ref().unwrap().flags;
                let old = object
                    .array_iterator
                    .borrow_mut()
                    .replace(ArrayIteratorData {
                        storage: cell(value),
                        flags,
                        cursor: PropertyCursor::default(),
                    });
                drop(old);
                self.drain_destructors(Ok(Value::Null), line)
            }
            b"getflags" => Ok(Value::Int(
                object.array_iterator.borrow().as_ref().unwrap().flags,
            )),
            b"setflags" => {
                let flags = self.lookup(b"flags").unwrap().integer(line, self)?;
                object.array_iterator.borrow_mut().as_mut().unwrap().flags = flags & 0xffff;
                Ok(Value::Null)
            }
            b"getarraycopy" => Ok(Value::Array(Rc::new(storage(&object).snapshot()))),
            b"count" => Ok(Value::Int(storage(&object).keys().len() as i64)),
            b"__debuginfo" => {
                let mut properties = object.snapshot();
                let backing = object
                    .array_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .storage
                    .borrow()
                    .clone();
                if !matches!(backing, Value::Null) {
                    properties.insert(
                        ArrayKey::String(b"\0ArrayIterator\0storage".to_vec()),
                        backing,
                    );
                }
                Ok(Value::Array(Rc::new(properties)))
            }
            b"offsetget" | b"offsetexists" | b"offsetset" | b"offsetunset" | b"append" => {
                let key = if name == b"append" {
                    None
                } else {
                    self.lookup(b"key")
                };
                let value = self.lookup(b"value");
                self.array_iterator_offset(&object, &name, key.as_ref(), value, line)
            }
            b"seek" => {
                let position = self.lookup(b"offset").unwrap().integer(line, self)?;
                let keys = storage(&object).keys();
                if position >= 0 {
                    let mut state = object.array_iterator.borrow_mut();
                    let cursor = &mut state.as_mut().unwrap().cursor;
                    cursor.position = (position as usize).min(keys.len());
                    cursor.keys = keys;
                    if cursor.position < cursor.keys.len() {
                        return Ok(Value::Null);
                    }
                }
                Err(self.array_iterator_exception(
                    b"OutOfBoundsException",
                    format!("Seek position {position} is out of range"),
                    line,
                ))
            }
            _ => self.array_iterator_step(&object, &name, false, line),
        }
    }

    fn array_iterator_exception(&mut self, class: &[u8], message: String, line: usize) -> Error {
        let class = self.find_class(class).unwrap();
        let args = [Argument::Positional(Expr {
            kind: ExprKind::Literal(Value::String(message.into_bytes().into())),
            line,
        })];
        match self.instantiate_class(class, &args, true, line) {
            Ok(value) => self.throw_value(value, line),
            Err(error) => error,
        }
    }

    pub(super) fn array_iterator_key(&self, object: &Rc<PhpObject>) -> Option<ArrayKey> {
        let keys = storage(object).keys();
        let mut state = object.array_iterator.borrow_mut();
        let cursor = &mut state.as_mut().unwrap().cursor;
        if cursor.keys != keys {
            cursor.position = cursor
                .keys
                .iter()
                .take(cursor.position)
                .filter(|key| keys.contains(key))
                .count();
            cursor.keys = keys;
        }
        cursor.keys.get(cursor.position).cloned()
    }

    pub(super) fn array_iterator_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        raw: bool,
        _line: usize,
    ) -> Result<Value, Error> {
        let key = self.array_iterator_key(object);
        match name {
            b"valid" => Ok(Value::Bool(key.is_some())),
            b"key" => Ok(key.map_or(Value::Null, |key| key.to_value())),
            b"current" => {
                let value = key
                    .and_then(|key| storage(object).read(&key))
                    .unwrap_or(Value::Null);
                Ok(if raw { value } else { value.dereferenced() })
            }
            b"rewind" | b"next" => {
                let mut state = object.array_iterator.borrow_mut();
                let cursor = &mut state.as_mut().unwrap().cursor;
                if name == b"rewind" {
                    cursor.position = 0;
                } else if cursor.position < cursor.keys.len() {
                    cursor.position += 1;
                }
                Ok(Value::Null)
            }
            _ => unreachable!("native iterator method"),
        }
    }

    pub(super) fn array_iterator_reference(
        &mut self,
        object: &Rc<PhpObject>,
        key: &ArrayKey,
        line: usize,
    ) -> Result<variables::Cell, Error> {
        match storage(object) {
            Storage::Array(source) => reference_paths::acquire(
                source,
                &mut [Offset::new(Some(key.to_value()))],
                line,
                self,
            ),
            Storage::Object(backing) => self.iterated_property_reference(&backing, key, line),
        }
    }

    pub(super) fn array_iterator_offset_key(
        &mut self,
        object: &Rc<PhpObject>,
        value: &Value,
        method: &[u8],
        line: usize,
    ) -> Result<ArrayKey, Error> {
        let value = value.dereferenced();
        if matches!(
            value,
            Value::Array(_) | Value::Object(_) | Value::Closure(_)
        ) {
            let kind = builtins::debug_type(&value);
            let message = if method == b"offsetexists" {
                format!("Cannot access offset of type {kind} in isset or empty")
            } else if method == b"offsetunset" {
                format!(
                    "Cannot unset offset of type {kind} on {}",
                    object.class_name
                )
            } else {
                format!(
                    "Cannot access offset of type {kind} on {}",
                    object.class_name
                )
            };
            return Err(Error::new(ErrorKind::TypeError, message, line));
        }
        let key = ArrayKey::from_value(&value, line, self)?;
        Ok(if matches!(storage(object), Storage::Object(_)) {
            match key {
                ArrayKey::Int(n) => ArrayKey::String(n.to_string().into_bytes()),
                key => key,
            }
        } else {
            key
        })
    }

    fn array_iterator_offset(
        &mut self,
        object: &Rc<PhpObject>,
        method: &[u8],
        offset: Option<&Value>,
        value: Option<Value>,
        line: usize,
    ) -> Result<Value, Error> {
        let backing = storage(object);
        let append =
            method == b"append" || (method == b"offsetset" && offset == Some(&Value::Null));
        if append && matches!(backing, Storage::Object(_)) {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot append properties to objects, use {}::offsetSet() instead",
                    object.class_name
                ),
                line,
            ));
        }
        let key = if append {
            None
        } else {
            Some(self.array_iterator_offset_key(object, offset.unwrap(), method, line)?)
        };
        match method {
            b"offsetexists" => Ok(Value::Bool(backing.read(key.as_ref().unwrap()).is_some())),
            b"offsetget" => {
                let key = key.unwrap();
                if let Some(value) = backing.read(&key) {
                    return Ok(value.dereferenced());
                }
                let display = match key {
                    ArrayKey::Int(n) => n.to_string(),
                    ArrayKey::String(name) => format!("\"{}\"", String::from_utf8_lossy(&name)),
                };
                self.warn(format!("Undefined array key {display} on line {line}"))?;
                Ok(Value::Null)
            }
            b"offsetset" | b"append" | b"offsetunset" => {
                // Release the storage borrow before PHP destructors run.
                let result = match backing {
                    Storage::Array(source) => {
                        let mut source = source.borrow_mut();
                        let Value::Array(array) = &mut *source else {
                            unreachable!()
                        };
                        let array = Rc::make_mut(array);
                        if method == b"offsetunset" {
                            array.remove(key.as_ref().unwrap());
                            Ok(())
                        } else if let Some(key) = key {
                            array.insert(key, value.unwrap());
                            Ok(())
                        } else {
                            array.append(value.unwrap(), line)
                        }
                    }
                    Storage::Object(backing) => {
                        let key = key.unwrap();
                        if method == b"offsetunset" {
                            backing.remove_property(&key);
                        } else {
                            backing.properties.borrow_mut().insert(key, value.unwrap());
                        }
                        Ok(())
                    }
                };
                self.drain_destructors(result.map(|_| Value::Null), line)
            }
            _ => unreachable!(),
        }
    }
}
