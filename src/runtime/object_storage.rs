// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_observer.c and .stub.php.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, ast::NativeMethod, object::Class};

struct Entry {
    key: ArrayKey,
    object: Value,
    info: Value,
}
#[derive(Default)]
pub(crate) struct ObjectStorageData {
    entries: Vec<Option<Entry>>,
    indices: HashMap<ArrayKey, usize>,
    position: usize,
    index: i64,
    // MultipleIterator reuses the native storage flags for its public modes.
    // SplObjectStorage's override checks are derived from methods below.
    flags: i64,
}
impl ObjectStorageData {
    fn advance(&mut self) {
        while self.position < self.entries.len() && self.entries[self.position].is_none() {
            self.position += 1;
        }
    }
    fn rewind(&mut self) {
        self.position = 0;
        self.index = 0;
        self.advance();
    }
    fn current(&self) -> Option<&Entry> {
        self.entries.get(self.position).and_then(Option::as_ref)
    }
    fn read(&self, key: &ArrayKey) -> Option<Value> {
        self.indices
            .get(key)
            .and_then(|i| self.entries[*i].as_ref())
            .map(|e| e.info.clone())
    }
    fn next_entry(&self, index: &mut usize) -> Option<(Value, Value)> {
        while let Some(entry) = self.entries.get(*index) {
            *index += 1;
            if let Some(entry) = entry {
                return Some((entry.object.clone(), entry.info.clone()));
            }
        }
        None
    }
    pub(super) fn values(&self) -> Vec<Value> {
        self.entries
            .iter()
            .flatten()
            .flat_map(|e| [e.object.clone(), e.info.clone()])
            .collect()
    }
    pub(super) fn finish_unserialization(&mut self) {
        for entry in self.entries.iter_mut().flatten() {
            entry.info = entry.info.clone();
        }
    }
    pub(super) fn comparison_table(&self) -> PhpArray {
        let mut table = PhpArray::default();
        for entry in self.entries.iter().flatten() {
            table.insert(entry.key.clone(), entry.info.clone());
        }
        table
    }
}

fn object_id(value: &Value) -> Option<usize> {
    match value {
        Value::Object(o) => Some(o.object_id),
        Value::Closure(o) => Some(o.object_id),
        _ => None,
    }
}
pub(super) fn native_dimensions(object: &PhpObject, methods: &[&[u8]]) -> bool {
    if object.class.is_a(b"MultipleIterator") {
        // PHP shares these dimension handlers with SplObjectStorage. Its public
        // mode bits therefore also select native access versus ArrayAccess.
        let mask = if methods.contains(&b"offsetSet".as_slice()) {
            2
        } else if methods.contains(&b"offsetUnset".as_slice()) {
            4
        } else {
            1
        };
        return object
            .object_storage
            .borrow()
            .as_ref()
            .is_some_and(|data| data.flags & mask == 0);
    }
    object.object_storage.borrow().is_some()
        && std::iter::once(b"getHash".as_slice())
            .chain(methods.iter().copied())
            .all(|name| {
                objects::method(&object.class, name)
                    .is_some_and(|m| matches!(m.native, Some(NativeMethod::ObjectStorage)))
            })
}

impl Runtime {
    pub(super) fn builtin_object_storage_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let code: &[u8] = match name {
            b"multipleiterator" => br#"<?php class MultipleIterator implements Iterator {
                public const int MIT_NEED_ANY = 0, MIT_NEED_ALL = 1, MIT_KEYS_NUMERIC = 0, MIT_KEYS_ASSOC = 2;
                public function __construct(int $flags = 1) {}
                public function getFlags(): int {}
                public function setFlags(int $flags): void {}
                public function attachIterator(Iterator $iterator, string|int|null $info = null): void {}
                public function detachIterator(Iterator $iterator): void {}
                public function containsIterator(Iterator $iterator): bool {}
                public function countIterators(): int {}
                public function rewind(): void {}
                public function valid(): bool {}
                public function key(): array {}
                public function current(): array {}
                public function next(): void {}
                public function __debugInfo(): array {}
            }"#,
            b"splobserver" => b"<?php interface SplObserver { public function update(SplSubject $subject): void; }",
            b"splsubject" => b"<?php interface SplSubject { public function attach(SplObserver $observer): void; public function detach(SplObserver $observer): void; public function notify(): void; }",
            b"splobjectstorage" => br#"<?php class SplObjectStorage implements Countable, SeekableIterator, Serializable, ArrayAccess {
                #[Deprecated(since: '8.5', message: 'use method SplObjectStorage::offsetSet() instead')]
                public function attach(object $object, mixed $info = null): void {}
                #[Deprecated(since: '8.5', message: 'use method SplObjectStorage::offsetUnset() instead')]
                public function detach(object $object): void {}
                #[Deprecated(since: '8.5', message: 'use method SplObjectStorage::offsetExists() instead')]
                public function contains(object $object): bool {}
                public function addAll(SplObjectStorage $storage): int {}
                public function removeAll(SplObjectStorage $storage): int {}
                public function removeAllExcept(SplObjectStorage $storage): int {}
                public function getInfo(): mixed {}
                public function setInfo(mixed $info): void {}
                public function count(int $mode = COUNT_NORMAL): int {}
                public function rewind(): void {}
                public function valid(): bool {}
                public function key(): int {}
                public function current(): object {}
                public function next(): void {}
                public function seek(int $offset): void {}
                public function unserialize(string $data): void {}
                public function serialize(): string {}
                public function offsetExists($object): bool {}
                public function offsetGet($object): mixed {}
                public function offsetSet($object, mixed $info = null): void {}
                public function offsetUnset($object): void {}
                public function getHash(object $object): string {}
                public function __serialize(): array {}
                public function __unserialize(array $data): void {}
                public function __debugInfo(): array {}
            }"#,
            _ => return None,
        };
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let (mut statements, _, _) = parser::parse(lexer::lex(code).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            if name == b"multipleiterator" && method.name.ends_with(b"::__construct") {
                let constant = |name: &[u8]| Expr {
                    line: 0,
                    kind: ExprKind::ScopedMember(
                        Box::new(Expr {
                            line: 0,
                            kind: ExprKind::Constant(b"MultipleIterator".to_vec()),
                        }),
                        Box::new(Expr {
                            line: 0,
                            kind: ExprKind::Literal(Value::String(name.to_vec().into())),
                        }),
                    ),
                };
                method.parameters[0].default = Some(Expr {
                    line: 0,
                    kind: ExprKind::Binary(
                        "|".into(),
                        Box::new(constant(b"MIT_NEED_ALL")),
                        Box::new(constant(b"MIT_KEYS_NUMERIC")),
                    ),
                });
            }
            method.native = Some(NativeMethod::ObjectStorage);
            if method.name.rsplit(|b| *b == b':').next().unwrap() != b"seek" {
                method.tentative_return_type = method.return_type.take();
            }
        }
        let interfaces = if name == b"multipleiterator" {
            vec![self.find_class(b"Iterator").unwrap()]
        } else if name == b"splobjectstorage" {
            [
                b"Countable".as_slice(),
                b"SeekableIterator",
                b"Serializable",
                b"ArrayAccess",
            ]
            .iter()
            .map(|n| self.find_class(n).unwrap())
            .collect()
        } else {
            Vec::new()
        };
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            declaration,
            parent: None,
            interfaces,
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

    fn multiple_iterator_call(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        // These are explicit method calls, not calls through a native foreach
        // iterator: wrapper current() methods must not use iterator caches.
        let method = objects::method(&object.class, name).unwrap();
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        self.invoke_at(target, &[], line, false, true)
    }

    fn multiple_iterator_advance(object: &PhpObject) {
        let mut data = object.object_storage.borrow_mut();
        let data = data.as_mut().unwrap();
        if data.position < data.entries.len() {
            data.position += 1;
        }
        data.advance();
    }

    fn multiple_iterator_method(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        match name {
            b"__construct" | b"setflags" => {
                let Value::Int(flags) = self.lookup(b"flags").unwrap() else {
                    unreachable!()
                };
                object.object_storage.borrow_mut().as_mut().unwrap().flags = flags;
                return Ok(Value::Null);
            }
            b"getflags" => {
                return Ok(Value::Int(
                    object.object_storage.borrow().as_ref().unwrap().flags,
                ));
            }
            b"countiterators" => {
                return Ok(Value::Int(
                    object
                        .object_storage
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .indices
                        .len() as i64,
                ));
            }
            b"attachiterator" | b"detachiterator" | b"containsiterator" => {
                let value = self.lookup(b"iterator").unwrap();
                if name == b"containsiterator" {
                    return Ok(Value::Bool(
                        self.object_storage_native_read(object, &value).is_some(),
                    ));
                }
                if name == b"detachiterator" {
                    self.object_storage_detach(object, &value, line)?;
                    object
                        .object_storage
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .rewind();
                    return Ok(Value::Null);
                }
                let info = self.lookup(b"info").unwrap();
                if info != Value::Null {
                    object
                        .object_storage
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .rewind();
                    loop {
                        let old = object
                            .object_storage
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .current()
                            .map(|e| e.info.clone());
                        let Some(old) = old else {
                            break;
                        };
                        if info.identical(&old, line)? {
                            return Err(self.array_iterator_exception(
                                b"InvalidArgumentException",
                                "Key duplication error".into(),
                                line,
                            ));
                        }
                        Self::multiple_iterator_advance(object);
                    }
                }
                self.object_storage_attach(object, value, info, line)?;
                return Ok(Value::Null);
            }
            _ => (),
        }
        let (empty, expect) = {
            let data = object.object_storage.borrow();
            let data = data.as_ref().unwrap();
            (data.indices.is_empty(), data.flags & 1 != 0)
        };
        let collect = matches!(name, b"current" | b"key");
        if empty {
            if collect {
                return Err(self.array_iterator_exception(
                    b"RuntimeException",
                    format!(
                        "Called {}() on an invalid iterator",
                        String::from_utf8_lossy(name)
                    ),
                    line,
                ));
            }
            return Ok(if name == b"valid" {
                Value::Bool(false)
            } else {
                Value::Null
            });
        }
        object
            .object_storage
            .borrow_mut()
            .as_mut()
            .unwrap()
            .rewind();
        let mut result = PhpArray::default();
        loop {
            let entry = {
                let data = object.object_storage.borrow();
                let data = data.as_ref().unwrap();
                data.current()
                    .map(|entry| (data.position, entry.object.clone(), entry.info.clone()))
            };
            let Some((position, Value::Object(iterator), original_info)) = entry else {
                break;
            };
            if matches!(name, b"rewind" | b"next") {
                let value = self.multiple_iterator_call(&iterator, name, line)?;
                drop(value);
                self.drain_destructors(Ok(()), line)?;
            } else {
                let valid = self.multiple_iterator_call(&iterator, b"valid", line)?;
                let is_valid = matches!(valid, Value::Bool(true));
                drop(valid);
                self.drain_destructors(Ok(()), line)?;
                if name == b"valid" {
                    if expect != is_valid {
                        return Ok(Value::Bool(!expect));
                    }
                } else {
                    let value = if is_valid {
                        self.multiple_iterator_call(&iterator, name, line)?
                    } else if object.object_storage.borrow().as_ref().unwrap().flags & 1 != 0 {
                        return Err(self.array_iterator_exception(
                            b"RuntimeException",
                            format!(
                                "Called {}() with non valid sub iterator",
                                String::from_utf8_lossy(name)
                            ),
                            line,
                        ));
                    } else {
                        Value::Null
                    };
                    let (flags, info) = {
                        let data = object.object_storage.borrow();
                        let data = data.as_ref().unwrap();
                        (
                            data.flags,
                            data.entries
                                .get(position)
                                .and_then(Option::as_ref)
                                .map(|e| e.info.clone())
                                .unwrap_or(original_info),
                        )
                    };
                    let key = if flags & 2 != 0 {
                        match info {
                            Value::Int(key) => ArrayKey::Int(key),
                            Value::String(key) => {
                                ArrayKey::from_value(&Value::String(key), line, self)?
                            }
                            _ => {
                                return Err(self.array_iterator_exception(
                                    b"InvalidArgumentException",
                                    "Sub-Iterator is associated with NULL".into(),
                                    line,
                                ));
                            }
                        }
                    } else {
                        result.append_key(line)?
                    };
                    result.insert(key, value);
                }
            }
            Self::multiple_iterator_advance(object);
        }
        Ok(if collect {
            Value::Array(Rc::new(result))
        } else if name == b"valid" {
            Value::Bool(expect)
        } else {
            Value::Null
        })
    }

    fn object_storage_hash(
        &mut self,
        storage: &Rc<PhpObject>,
        value: &Value,
        line: usize,
    ) -> Result<ArrayKey, Error> {
        if storage.class.is_a(b"MultipleIterator") {
            return Ok(ArrayKey::Int(object_id(value).unwrap() as i64));
        }
        let method = objects::method(&storage.class, b"getHash").unwrap();
        if matches!(method.native, Some(NativeMethod::ObjectStorage)) {
            return Ok(ArrayKey::Int(object_id(value).unwrap() as i64));
        }
        let target = self.unchecked_method_target(
            method,
            storage.class.clone(),
            Some(storage.clone()),
            line,
        )?;
        let result = self.call_callback_at(&target, std::slice::from_ref(value), line, true)?;
        let Value::String(hash) = result else {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "{}::getHash(): Return value must be of type string, {} returned",
                    storage.class_name,
                    builtins::debug_type(&result)
                ),
                line,
            ));
        };
        Ok(ArrayKey::String(hash.bytes().to_vec()))
    }
    pub(super) fn object_storage_attach(
        &mut self,
        storage: &Rc<PhpObject>,
        value: Value,
        info: Value,
        line: usize,
    ) -> Result<(), Error> {
        let key = self.object_storage_hash(storage, &value, line)?;
        let old = {
            let mut data = storage.object_storage.borrow_mut();
            let data = data.as_mut().unwrap();
            if let Some(index) = data.indices.get(&key).copied() {
                Some(std::mem::replace(
                    &mut data.entries[index].as_mut().unwrap().info,
                    info,
                ))
            } else {
                let index = data.entries.len();
                data.indices.insert(key.clone(), index);
                data.entries.push(Some(Entry {
                    key,
                    object: value,
                    info,
                }));
                None
            }
        };
        drop(old);
        self.drain_destructors(Ok(()), line)
    }
    fn object_storage_remove_key(
        &mut self,
        storage: &Rc<PhpObject>,
        key: &ArrayKey,
        line: usize,
    ) -> Result<bool, Error> {
        let old = {
            let mut data = storage.object_storage.borrow_mut();
            let data = data.as_mut().unwrap();
            let old = data
                .indices
                .remove(key)
                .and_then(|index| data.entries[index].take());
            data.advance();
            old
        };
        let removed = old.is_some();
        drop(old);
        self.drain_destructors(Ok(removed), line)
    }
    fn object_storage_detach(
        &mut self,
        storage: &Rc<PhpObject>,
        value: &Value,
        line: usize,
    ) -> Result<bool, Error> {
        let key = self.object_storage_hash(storage, value, line)?;
        self.object_storage_remove_key(storage, &key, line)
    }
    pub(super) fn object_storage_native_read(
        &self,
        storage: &PhpObject,
        key: &Value,
    ) -> Option<Value> {
        object_id(key).and_then(|id| {
            storage
                .object_storage
                .borrow()
                .as_ref()
                .unwrap()
                .read(&ArrayKey::Int(id as i64))
        })
    }
    pub(super) fn object_storage_native_unset(
        &mut self,
        storage: &Rc<PhpObject>,
        key: &Value,
        line: usize,
    ) -> Result<(), Error> {
        self.object_storage_remove_key(
            storage,
            &ArrayKey::Int(object_id(key).unwrap() as i64),
            line,
        )
        .map(drop)
    }
    pub(super) fn clone_object_storage(
        &mut self,
        original: &PhpObject,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<(), Error> {
        let mut index = 0;
        loop {
            let entry = original
                .object_storage
                .borrow()
                .as_ref()
                .unwrap()
                .next_entry(&mut index);
            let Some((value, info)) = entry else {
                break;
            };
            self.object_storage_attach(object, value, info, line)?;
        }
        Ok(())
    }

    pub(super) fn native_object_storage_method(&mut self, line: usize) -> Result<Value, Error> {
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
        if object.class.is_a(b"MultipleIterator") && name != b"__debuginfo" {
            return self.multiple_iterator_method(&object, &name, line);
        }
        match name.as_slice() {
            b"attach" | b"detach" | b"contains" | b"offsetset" | b"offsetunset"
            | b"offsetexists" | b"offsetget" | b"gethash" => {
                let value = self.lookup(b"object").unwrap().dereferenced();
                if object_id(&value).is_none() {
                    let method = match name.as_slice() {
                        b"offsetset" => "offsetSet",
                        b"offsetunset" => "offsetUnset",
                        b"offsetexists" => "offsetExists",
                        _ => "offsetGet",
                    };
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "SplObjectStorage::{method}(): Argument #1 ($object) must be of type object, {} given",
                            builtins::debug_type(&value)
                        ),
                        line,
                    ));
                }
                match name.as_slice() {
                    b"gethash" => Ok(Value::String(
                        format!("{:016x}0000000000000000", object_id(&value).unwrap())
                            .into_bytes()
                            .into(),
                    )),
                    b"attach" | b"offsetset" => {
                        let info = self.lookup(b"info").unwrap();
                        self.object_storage_attach(&object, value, info, line)?;
                        Ok(Value::Null)
                    }
                    b"detach" | b"offsetunset" => {
                        self.object_storage_detach(&object, &value, line)?;
                        object
                            .object_storage
                            .borrow_mut()
                            .as_mut()
                            .unwrap()
                            .rewind();
                        Ok(Value::Null)
                    }
                    _ => {
                        let key = self.object_storage_hash(&object, &value, line)?;
                        let info = object.object_storage.borrow().as_ref().unwrap().read(&key);
                        if name == b"offsetget" {
                            info.map(|v| v.dereferenced()).ok_or_else(|| {
                                self.array_iterator_exception(
                                    b"UnexpectedValueException",
                                    "Object not found".into(),
                                    line,
                                )
                            })
                        } else {
                            Ok(Value::Bool(info.is_some()))
                        }
                    }
                }
            }
            b"count" => Ok(Value::Int(
                object
                    .object_storage
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .indices
                    .len() as i64,
            )),
            b"rewind" | b"next" | b"valid" | b"key" | b"current" | b"getinfo" | b"setinfo"
            | b"seek" => {
                let mut payload = object.object_storage.borrow_mut();
                let data = payload.as_mut().unwrap();
                match name.as_slice() {
                    b"rewind" => {
                        data.rewind();
                        Ok(Value::Null)
                    }
                    b"next" => {
                        if data.position < data.entries.len() {
                            data.position += 1;
                        }
                        data.advance();
                        data.index = data.index.wrapping_add(1);
                        Ok(Value::Null)
                    }
                    b"valid" => Ok(Value::Bool(data.current().is_some())),
                    b"key" => Ok(Value::Int(data.index)),
                    b"current" => {
                        let value = data.current().map(|e| e.object.clone());
                        drop(payload);
                        value.ok_or_else(|| {
                            self.array_iterator_exception(
                                b"RuntimeException",
                                "Called current() on invalid iterator".into(),
                                line,
                            )
                        })
                    }
                    b"getinfo" => Ok(data.current().map_or(Value::Null, |e| e.info.clone())),
                    b"setinfo" => {
                        let value = self.lookup(b"info").unwrap();
                        let old = data
                            .entries
                            .get_mut(data.position)
                            .and_then(Option::as_mut)
                            .map(|e| std::mem::replace(&mut e.info, value));
                        drop(payload);
                        drop(old);
                        self.drain_destructors(Ok(Value::Null), line)
                    }
                    b"seek" => {
                        let Value::Int(offset) = self.lookup(b"offset").unwrap() else {
                            unreachable!()
                        };
                        if offset < 0 || offset as usize >= data.indices.len() {
                            drop(payload);
                            return Err(self.array_iterator_exception(
                                b"OutOfBoundsException",
                                format!("Seek position {offset} is out of range"),
                                line,
                            ));
                        }
                        if offset == 0 || (offset < data.index && data.index - offset > offset) {
                            data.rewind();
                        }
                        while data.index < offset {
                            if data.position < data.entries.len() {
                                data.position += 1;
                            }
                            data.advance();
                            data.index += 1;
                        }
                        while data.index > offset {
                            data.position = data.position.saturating_sub(1);
                            while data.position > 0
                                && data.entries.get(data.position).is_none_or(Option::is_none)
                            {
                                data.position -= 1;
                            }
                            data.index -= 1;
                        }
                        Ok(Value::Null)
                    }
                    _ => unreachable!(),
                }
            }
            b"addall" | b"removeall" | b"removeallexcept" => {
                let Value::Object(other) = self.lookup(b"storage").unwrap() else {
                    unreachable!()
                };
                let source = if name == b"removeallexcept" {
                    &object
                } else {
                    &other
                };
                let mut index = 0;
                loop {
                    let entry = source
                        .object_storage
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .next_entry(&mut index);
                    let Some((value, info)) = entry else {
                        break;
                    };
                    if name == b"addall" {
                        self.object_storage_attach(&object, value, info, line)?;
                    } else if name == b"removeall" {
                        self.object_storage_detach(&object, &value, line)?;
                    } else {
                        let key = self.object_storage_hash(&other, &value, line)?;
                        let exists = other
                            .object_storage
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .indices
                            .contains_key(&key);
                        if !exists {
                            self.object_storage_detach(&object, &value, line)?;
                        }
                    }
                }
                let mut data = object.object_storage.borrow_mut();
                let data = data.as_mut().unwrap();
                if name == b"addall" {
                    data.index = 0;
                } else {
                    data.rewind();
                }
                Ok(Value::Int(data.indices.len() as i64))
            }
            b"__debuginfo" | b"__serialize" => {
                let data = object.object_storage.borrow();
                let data = data.as_ref().unwrap();
                let mut storage = PhpArray::default();
                for entry in data.entries.iter().flatten() {
                    if name == b"__serialize" {
                        storage.insert(ArrayKey::Int(storage.len() as i64), entry.object.clone());
                        storage.insert(ArrayKey::Int(storage.len() as i64), entry.info.clone());
                    } else {
                        let mut pair = PhpArray::default();
                        pair.insert(ArrayKey::String(b"obj".to_vec()), entry.object.clone());
                        pair.insert(ArrayKey::String(b"inf".to_vec()), entry.info.clone());
                        storage.insert(
                            ArrayKey::Int(storage.len() as i64),
                            Value::Array(Rc::new(pair)),
                        );
                    }
                }
                let mut result = if name == b"__serialize" {
                    PhpArray::default()
                } else {
                    object.snapshot()
                };
                let key = if name == b"__serialize" {
                    ArrayKey::Int(0)
                } else {
                    ArrayKey::String(b"\0SplObjectStorage\0storage".to_vec())
                };
                result.insert(key, Value::Array(Rc::new(storage)));
                if name == b"__serialize" {
                    result.insert(
                        ArrayKey::Int(1),
                        Value::Array(Rc::new(self.mangled_object_variables(&object))),
                    );
                }
                Ok(Value::Array(Rc::new(result)))
            }
            b"__unserialize" => self.object_storage_restore(&object, line),
            b"serialize" => self.object_storage_serialize(&object, line),
            b"unserialize" => self.object_storage_unserialize(&object, line),
            _ => unreachable!(),
        }
    }

    fn object_storage_restore(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let Value::Array(data) = self.lookup(b"data").unwrap() else {
            unreachable!()
        };
        let (Some(Value::Array(storage)), Some(Value::Array(members))) =
            (data.get(&ArrayKey::Int(0)), data.get(&ArrayKey::Int(1)))
        else {
            return Err(self.array_iterator_exception(
                b"UnexpectedValueException",
                "Incomplete or ill-typed serialization data".into(),
                line,
            ));
        };
        if storage.len() % 2 != 0 {
            return Err(self.array_iterator_exception(
                b"UnexpectedValueException",
                "Odd number of elements".into(),
                line,
            ));
        }
        let mut values = storage.iter();
        while let Some((_, key)) = values.next() {
            if object_id(&key).is_none() {
                return Err(self.array_iterator_exception(
                    b"UnexpectedValueException",
                    "Non-object key".into(),
                    line,
                ));
            }
            let (_, info) = values.next().unwrap();
            self.object_storage_attach(object, key, info.dereferenced(), line)?;
        }
        for (key, value) in members.iter() {
            self.load_native_object_member(object, key, value, line)?;
        }
        Ok(Value::Null)
    }
    fn object_storage_serialize(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        self.serialize_native(line, |runtime, state, output| {
            let count = object
                .object_storage
                .borrow()
                .as_ref()
                .unwrap()
                .indices
                .len();
            output.extend(b"x:");
            runtime.encode_serialized(&Value::Int(count as i64), state, output, 0, line)?;
            let mut index = 0;
            loop {
                let entry = object
                    .object_storage
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .next_entry(&mut index);
                let Some((value, info)) = entry else {
                    break;
                };
                runtime.encode_serialized(&value, state, output, 0, line)?;
                output.push(b',');
                runtime.encode_serialized(&info, state, output, 0, line)?;
                output.push(b';');
            }
            output.extend(b"m:");
            let members = Value::Array(Rc::new(runtime.mangled_object_variables(object)));
            runtime.encode_serialized(&members, state, output, 0, line)
        })
    }
    fn object_storage_unserialize(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        use unserialization::Failure;
        let bytes = self.lookup(b"data").unwrap().to_bytes();
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        self.unserialize_native(
            &bytes,
            "SplObjectStorage::unserialize",
            line,
            |runtime, decoder| {
                decoder.track_native_object(object);
                decoder.expect(b'x')?;
                decoder.expect(b':')?;
                let count = decoder.value(runtime)?;
                let Value::Int(count) = *count.borrow() else {
                    return Err(Failure::Syntax);
                };
                decoder.pos -= 1;
                if count < 0 {
                    return Err(Failure::Syntax);
                }
                for _ in 0..count {
                    decoder.expect(b';')?;
                    if !matches!(decoder.peek(), Some(b'O' | b'C' | b'r')) {
                        return Err(Failure::Syntax);
                    }
                    let slot = decoder.value(runtime)?;
                    let info = if decoder.peek() == Some(b',') {
                        decoder.pos += 1;
                        Value::Indirect(decoder.value(runtime)?)
                    } else {
                        Value::Null
                    };
                    let value = slot.borrow().clone();
                    if object_id(&value).is_none() {
                        return Err(Failure::Syntax);
                    }
                    runtime.object_storage_attach(object, value, info, line)?;
                }
                decoder.expect(b';')?;
                decoder.expect(b'm')?;
                decoder.expect(b':')?;
                let members = decoder.value(runtime)?;
                let Value::Array(members) = members.borrow().clone() else {
                    return Err(Failure::Syntax);
                };
                decoder.load_native_members(runtime, object, &members)
            },
        )
    }
}
