// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_fixedarray.c and
// Zend/zend_interfaces.c. Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, ast::NativeMethod, object::Class};

#[derive(Default)]
pub(crate) struct FixedArrayData {
    pub elements: Vec<Value>,
    resizing: Option<usize>,
}
impl Clone for FixedArrayData {
    fn clone(&self) -> Self {
        Self {
            elements: self.elements.clone(),
            resizing: None,
        }
    }
}
impl Drop for FixedArrayData {
    fn drop(&mut self) {
        while self.elements.pop().is_some() {}
    }
}

pub(super) fn native_method(object: &PhpObject, name: &[u8]) -> bool {
    object.fixed_array.borrow().is_some()
        && objects::method(&object.class, name)
            .is_some_and(|method| matches!(method.native, Some(NativeMethod::FixedArray)))
}
pub(crate) fn export_storage(object: &PhpObject) -> PhpArray {
    let mut result = to_array(object);
    for (key, value) in object.snapshot().iter_copied() {
        result.insert(key, value);
    }
    result
}
fn to_array(object: &PhpObject) -> PhpArray {
    let data = object.fixed_array.borrow();
    let mut result = PhpArray::default();
    for (index, value) in data.as_ref().unwrap().elements.iter().enumerate() {
        result.insert(ArrayKey::Int(index as i64), value.clone());
    }
    result
}

impl Runtime {
    pub(super) fn builtin_fixed_array_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        if !matches!(name, b"splfixedarray" | b"internaliterator") {
            return None;
        }
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let fixed = name == b"splfixedarray";
        let code: &[u8] = if fixed {
            b"<?php class SplFixedArray implements IteratorAggregate, ArrayAccess, Countable, JsonSerializable {
            public function __construct(int $size = 0) {}
            #[\\Deprecated(since: '8.4', message: 'this method is obsolete, as serialization hooks are provided by __unserialize() and __serialize()')]
            public function __wakeup(): void {}
            public function __serialize(): array {}
            public function __unserialize(array $data): void {}
            public function count(): int {}
            public function toArray(): array {}
            public static function fromArray(array $array, bool $preserveKeys = true): SplFixedArray {}
            public function getSize(): int {}
            public function setSize(int $size): true {}
            public function offsetExists($index): bool {}
            public function offsetGet($index): mixed {}
            public function offsetSet($index, mixed $value): void {}
            public function offsetUnset($index): void {}
            public function getIterator(): Iterator {}
            public function jsonSerialize(): array {}
        }"
        } else {
            b"<?php final class InternalIterator implements Iterator {
            private function __construct() {}
            public function current(): mixed {}
            public function key(): mixed {}
            public function next(): void {}
            public function valid(): bool {}
            public function rewind(): void {}
        }"
        };
        let (mut statements, _, _) = parser::parse(lexer::lex(code).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::FixedArray);
            let short = method.name.rsplit(|b| *b == b':').next().unwrap();
            if fixed
                && !matches!(
                    short,
                    b"__construct"
                        | b"__serialize"
                        | b"__unserialize"
                        | b"getIterator"
                        | b"jsonSerialize"
                )
            {
                method.tentative_return_type = method.return_type.take();
            }
        }
        let interfaces: &[&[u8]] = if fixed {
            &[
                b"IteratorAggregate",
                b"ArrayAccess",
                b"Countable",
                b"JsonSerializable",
            ]
        } else {
            &[b"Iterator"]
        };
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            declaration,
            parent: None,
            interfaces: interfaces
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

    pub(super) fn fixed_array_index(&mut self, value: &Value, line: usize) -> Result<i64, Error> {
        let value = value.dereferenced();
        if matches!(
            value,
            Value::Int(_)
                | Value::Bool(_)
                | Value::Float(_)
                | Value::Resource(_)
                | Value::String(_)
        ) && let ArrayKey::Int(index) = ArrayKey::from_value(&value, line, self)?
        {
            return Ok(index);
        }
        Err(Error::new(
            ErrorKind::TypeError,
            format!(
                "Cannot access offset of type {} on SplFixedArray",
                builtins::debug_type(&value)
            ),
            line,
        ))
    }
    fn fixed_array_bounds(&mut self, line: usize) -> Error {
        self.array_iterator_exception(
            b"OutOfBoundsException",
            "Index invalid or out of range".into(),
            line,
        )
    }
    pub(super) fn fixed_array_read(
        &mut self,
        object: &PhpObject,
        index: i64,
        line: usize,
    ) -> Result<Value, Error> {
        let value = usize::try_from(index).ok().and_then(|index| {
            object
                .fixed_array
                .borrow()
                .as_ref()
                .unwrap()
                .elements
                .get(index)
                .cloned()
        });
        value.ok_or_else(|| self.fixed_array_bounds(line))
    }
    fn fixed_array_resize(
        &mut self,
        object: &PhpObject,
        size: usize,
        line: usize,
    ) -> Result<(), Error> {
        let removed = {
            let mut data = object.fixed_array.borrow_mut();
            let data = data.as_mut().unwrap();
            if size == data.elements.len() {
                return Ok(());
            }
            if data.elements.is_empty() {
                data.elements
                    .try_reserve_exact(size - data.elements.len())
                    .map_err(|_| {
                        Error::new(
                            ErrorKind::Error,
                            "Unable to allocate SplFixedArray storage",
                            line,
                        )
                    })?;
                data.elements.resize(size, Value::Null);
                data.resizing = None;
                return Ok(());
            }
            if data.resizing.is_some() {
                data.resizing = Some(size);
                return Ok(());
            }
            if size > data.elements.len() {
                data.elements
                    .try_reserve_exact(size - data.elements.len())
                    .map_err(|_| {
                        Error::new(
                            ErrorKind::Error,
                            "Unable to allocate SplFixedArray storage",
                            line,
                        )
                    })?;
            }
            data.resizing = Some(size);
            if size < data.elements.len() {
                let mut removed = data.elements.split_off(size);
                if size == 0 {
                    removed.reverse();
                }
                removed
            } else {
                data.elements.resize(size, Value::Null);
                Vec::new()
            }
        };
        let mut result = Ok(());
        for value in removed {
            drop(value);
            result = self.drain_destructors(result, line);
        }
        let next = object
            .fixed_array
            .borrow_mut()
            .as_mut()
            .unwrap()
            .resizing
            .take();
        if let Some(next) = next.filter(|next| *next != size) {
            let resize = self.fixed_array_resize(object, next, line);
            result = result.and(resize);
        }
        result
    }
    pub(super) fn fixed_array_iterator(
        &mut self,
        source: Rc<PhpObject>,
        hidden: bool,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        self.native_iterator(
            source,
            native_iterator::IteratorState::FixedArray(0),
            hidden,
            line,
        )
    }
    pub(super) fn native_fixed_array_method(&mut self, line: usize) -> Result<Value, Error> {
        let name = self
            .current_function
            .as_ref()
            .unwrap()
            .rsplit(|b| *b == b':')
            .next()
            .unwrap()
            .to_vec();
        if name == b"fromarray" {
            let Value::Array(array) = self.lookup(b"array").unwrap() else {
                unreachable!()
            };
            let preserve = self.lookup(b"preserveKeys").unwrap().is_truthy();
            let mut size = 0usize;
            if preserve {
                for (key, _) in array.iter() {
                    let ArrayKey::Int(index) = key else {
                        return Err(self.array_iterator_exception(
                            b"InvalidArgumentException",
                            "array must contain only positive integer keys".into(),
                            line,
                        ));
                    };
                    if index < 0 {
                        return Err(self.array_iterator_exception(
                            b"InvalidArgumentException",
                            "array must contain only positive integer keys".into(),
                            line,
                        ));
                    }
                    if index == i64::MAX {
                        return Err(self.array_iterator_exception(
                            b"InvalidArgumentException",
                            "integer overflow detected".into(),
                            line,
                        ));
                    }
                    size = size.max(index as usize + 1);
                }
            } else {
                size = array.len();
            }
            let class = self.find_class(b"SplFixedArray").unwrap();
            let object = self.allocate_class_object(class, line)?;
            self.fixed_array_resize(&object, size, line)?;
            for (position, (key, value)) in array.iter().enumerate() {
                let index = if preserve {
                    let ArrayKey::Int(index) = key else {
                        unreachable!()
                    };
                    index as usize
                } else {
                    position
                };
                object.fixed_array.borrow_mut().as_mut().unwrap().elements[index] =
                    value.dereferenced();
            }
            return Ok(Value::Object(object));
        }
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        if object.class.is_a(b"InternalIterator") {
            if name == b"__construct" {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot manually construct InternalIterator",
                    line,
                ));
            }
            return self.internal_iterator_step(&object, &name, false, line);
        }
        match name.as_slice() {
            b"__construct" | b"setsize" => {
                let Value::Int(size) = self.lookup(b"size").unwrap() else {
                    unreachable!()
                };
                if size < 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        format!(
                            "SplFixedArray::{}(): Argument #1 ($size) must be greater than or equal to 0",
                            if name == b"setsize" {
                                "setSize"
                            } else {
                                "__construct"
                            }
                        ),
                        line,
                    ));
                }
                if name != b"__construct"
                    || object
                        .fixed_array
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .elements
                        .is_empty()
                {
                    self.fixed_array_resize(&object, size as usize, line)?;
                }
                Ok(if name == b"setsize" {
                    Value::Bool(true)
                } else {
                    Value::Null
                })
            }
            b"count" | b"getsize" => Ok(Value::Int(
                object.fixed_array.borrow().as_ref().unwrap().elements.len() as i64,
            )),
            b"toarray" | b"jsonserialize" => Ok(Value::Array(Rc::new(to_array(&object)))),
            b"__serialize" => {
                let mut array = to_array(&object);
                for (key, value) in object.snapshot().iter_copied() {
                    if matches!(key, ArrayKey::String(_)) {
                        array.insert(key, value);
                    }
                }
                Ok(Value::Array(Rc::new(array)))
            }
            b"__unserialize" => {
                if !object
                    .fixed_array
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .elements
                    .is_empty()
                {
                    return Ok(Value::Null);
                }
                let Value::Array(array) = self.lookup(b"data").unwrap() else {
                    unreachable!()
                };
                let mut members = Vec::new();
                for (key, value) in array.iter() {
                    if matches!(key, ArrayKey::Int(_)) {
                        object
                            .fixed_array
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .elements
                            .push(value);
                    } else {
                        members.push((key, value));
                    }
                }
                for (key, value) in members {
                    self.load_native_object_member(&object, key, value, line)?;
                }
                Ok(Value::Null)
            }
            b"__wakeup" => {
                if object
                    .fixed_array
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .elements
                    .is_empty()
                {
                    let properties = std::mem::take(&mut *object.properties.borrow_mut());
                    object.fixed_array.borrow_mut().as_mut().unwrap().elements =
                        properties.iter().map(|(_, value)| value).collect();
                }
                Ok(Value::Null)
            }
            b"getiterator" => self
                .fixed_array_iterator(object, false, line)
                .map(Value::Object),
            b"offsetexists" | b"offsetget" | b"offsetset" | b"offsetunset" => {
                let index = self.lookup(b"index").unwrap();
                let index = self.fixed_array_index(&index, line)?;
                if name == b"offsetexists" {
                    let exists = usize::try_from(index)
                        .ok()
                        .and_then(|index| {
                            object
                                .fixed_array
                                .borrow()
                                .as_ref()
                                .unwrap()
                                .elements
                                .get(index)
                                .map(|value| !matches!(value, Value::Null))
                        })
                        .unwrap_or(false);
                    return Ok(Value::Bool(exists));
                }
                if name == b"offsetget" {
                    return self
                        .fixed_array_read(&object, index, line)
                        .map(|value| value.dereferenced());
                }
                let value = if name == b"offsetset" {
                    self.lookup(b"value").unwrap()
                } else {
                    Value::Null
                };
                let previous = {
                    let mut data = object.fixed_array.borrow_mut();
                    let slot = usize::try_from(index)
                        .ok()
                        .and_then(|index| data.as_mut().unwrap().elements.get_mut(index));
                    let Some(slot) = slot else {
                        return Err(self.fixed_array_bounds(line));
                    };
                    std::mem::replace(slot, value)
                };
                drop(previous);
                self.drain_destructors(Ok(Value::Null), line)
            }
            _ => unreachable!(),
        }
    }
}
