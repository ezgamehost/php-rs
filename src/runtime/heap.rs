// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_heap.c and .stub.php.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, ast::NativeMethod, object::Class};

#[derive(Clone)]
struct Entry {
    value: Value,
    priority: Option<Value>,
}
impl Entry {
    fn extract(&self, flags: i64) -> Value {
        let Some(priority) = &self.priority else {
            return self.value.dereferenced();
        };
        match flags & 3 {
            1 => self.value.clone(),
            2 => priority.clone(),
            _ => {
                let mut array = PhpArray::default();
                array.insert(ArrayKey::String(b"data".to_vec()), self.value.clone());
                array.insert(ArrayKey::String(b"priority".to_vec()), priority.clone());
                Value::Array(Rc::new(array))
            }
        }
    }
}
#[derive(Clone)]
pub(crate) struct HeapData {
    entries: Vec<Option<Entry>>,
    count: usize,
    corrupted: bool,
    locked: bool,
    flags: i64,
    priority: bool,
    minimum: bool,
}
impl HeapData {
    pub(super) fn new(class: &Class) -> Self {
        let priority = class.is_a(b"SplPriorityQueue");
        Self {
            entries: Vec::new(),
            count: 0,
            corrupted: false,
            locked: false,
            flags: i64::from(priority),
            priority,
            minimum: class.is_a(b"SplMinHeap"),
        }
    }
    pub(super) fn values(&self) -> Vec<Value> {
        self.entries
            .iter()
            .take(self.count)
            .flatten()
            .flat_map(|e| std::iter::once(e.value.clone()).chain(e.priority.iter().cloned()))
            .collect()
    }
    fn entry(&self, index: usize) -> Entry {
        self.entries
            .get(index)
            .and_then(Option::as_ref)
            .cloned()
            .unwrap_or(Entry {
                value: Value::Null,
                priority: self.priority.then_some(Value::Null),
            })
    }
    fn put(&mut self, index: usize, entry: Entry) {
        if self.entries.len() <= index {
            self.entries.resize_with(index + 1, || None);
        }
        self.entries[index] = Some(entry);
    }
    fn storage(&self) -> PhpArray {
        let mut array = PhpArray::default();
        for i in 0..self.count {
            let entry = self.entry(i);
            array.insert(
                ArrayKey::Int(i as i64),
                if self.priority {
                    entry.extract(3)
                } else {
                    entry.value
                },
            );
        }
        array
    }
}
pub(super) fn native_iteration(object: &PhpObject) -> bool {
    object.heap.borrow().is_some()
        && [b"rewind".as_slice(), b"valid", b"current", b"key", b"next"]
            .iter()
            .all(|name| {
                objects::method(&object.class, name)
                    .is_some_and(|m| matches!(m.native, Some(NativeMethod::Heap)))
            })
}

impl Runtime {
    pub(super) fn builtin_heap_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, parent, priority) = match name {
            b"splheap" => ("SplHeap", None, false),
            b"splminheap" => ("SplMinHeap", Some("SplHeap"), false),
            b"splmaxheap" => ("SplMaxHeap", Some("SplHeap"), false),
            b"splpriorityqueue" => ("SplPriorityQueue", None, true),
            _ => return None,
        };
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let methods = if parent.is_some() {
            "protected function compare(mixed $value1, mixed $value2): int {}".to_owned()
        } else {
            let compare = if priority {
                "public const int EXTR_BOTH = 3, EXTR_PRIORITY = 2, EXTR_DATA = 1; public function compare(mixed $priority1, mixed $priority2): int {} public function insert(mixed $value, mixed $priority): true {} public function setExtractFlags(int $flags): int {} public function getExtractFlags(): int {}"
            } else {
                "abstract protected function compare(mixed $value1, mixed $value2): int; public function insert(mixed $value): true {}"
            };
            format!(
                "{compare} public function extract(): mixed {{}} public function top(): mixed {{}} public function count(): int {{}} public function isEmpty(): bool {{}} public function rewind(): void {{}} public function current(): mixed {{}} public function key(): int {{}} public function next(): void {{}} public function valid(): bool {{}} public function recoverFromCorruption(): true {{}} public function isCorrupted(): bool {{}} public function __debugInfo(): array {{}} public function __serialize(): array {{}} public function __unserialize(array $data): void {{}}"
            )
        };
        let code = format!(
            "<?php {}class {canonical} {} {{ {methods} }}",
            if name == b"splheap" { "abstract " } else { "" },
            parent.map_or("implements Iterator, Countable".to_owned(), |p| format!(
                "extends {p}"
            ))
        );
        let (mut statements, _, _) = parser::parse(lexer::lex(code.as_bytes()).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::Heap);
            method.tentative_return_type = method.return_type.take();
        }
        let parent = parent.map(|p| self.find_class(p.as_bytes()).unwrap());
        let mut methods = declaration.methods.clone();
        if let Some(parent) = &parent {
            methods.extend(
                parent
                    .methods
                    .iter()
                    .filter(|m| !m.name.ends_with(b"::compare"))
                    .cloned(),
            );
        }
        let interfaces = if parent.is_none() {
            [b"Iterator".as_slice(), b"Countable"]
                .iter()
                .map(|n| self.find_class(n).unwrap())
                .collect()
        } else {
            Vec::new()
        };
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods,
            declaration,
            parent,
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
    fn heap_check(&mut self, object: &PhpObject, write: bool, line: usize) -> Result<(), Error> {
        let message = {
            let data = object.heap.borrow();
            let data = data.as_ref().unwrap();
            if data.corrupted {
                Some("Heap is corrupted, heap properties are no longer ensured.")
            } else if write && data.locked {
                Some("Heap cannot be changed when it is already being modified.")
            } else {
                None
            }
        };
        match message {
            Some(message) => {
                Err(self.array_iterator_exception(b"RuntimeException", message.into(), line))
            }
            None => Ok(()),
        }
    }
    fn heap_compare(
        &mut self,
        object: &Rc<PhpObject>,
        a: &Entry,
        b: &Entry,
        pending: &mut Option<Error>,
        line: usize,
    ) -> i64 {
        if pending.is_some() {
            return 0;
        }
        let result = (|| {
            let method = objects::method(&object.class, b"compare").unwrap();
            let a = a.priority.as_ref().unwrap_or(&a.value);
            let b = b.priority.as_ref().unwrap_or(&b.value);
            if matches!(method.native, Some(NativeMethod::Heap)) {
                let minimum = object.heap.borrow().as_ref().unwrap().minimum;
                let order = if minimum {
                    self.compare(b, a, line)?
                } else {
                    self.compare(a, b, line)?
                };
                Ok(order.unwrap_or(Ordering::Greater) as i64)
            } else {
                let target = self.unchecked_method_target(
                    method,
                    object.class.clone(),
                    Some(object.clone()),
                    line,
                )?;
                let result = self
                    .call_callback_at(&target, &[a.clone(), b.clone()], line, true)?
                    .dereferenced();
                Ok(result.integer(line, self)?.signum())
            }
        })();
        match result {
            Ok(value) => value,
            Err(error) => {
                *pending = Some(error);
                0
            }
        }
    }
    fn heap_insert(
        &mut self,
        object: &Rc<PhpObject>,
        entry: Entry,
        line: usize,
    ) -> Result<(), Error> {
        let mut position = {
            let mut data = object.heap.borrow_mut();
            let data = data.as_mut().unwrap();
            data.locked = true;
            data.count
        };
        let mut pending = None;
        while position > 0 {
            let parent = (position - 1) / 2;
            let other = object.heap.borrow().as_ref().unwrap().entry(parent);
            if self.heap_compare(object, &other, &entry, &mut pending, line) >= 0 {
                break;
            }
            object
                .heap
                .borrow_mut()
                .as_mut()
                .unwrap()
                .put(position, other);
            position = parent;
        }
        {
            let mut data = object.heap.borrow_mut();
            let data = data.as_mut().unwrap();
            data.count += 1;
            data.locked = false;
            data.corrupted |= pending.is_some();
            data.put(position, entry);
        }
        self.drain_destructors(pending.map_or(Ok(()), Err), line)
    }
    fn heap_delete(
        &mut self,
        object: &Rc<PhpObject>,
        discard: bool,
        line: usize,
    ) -> Result<Option<Entry>, Error> {
        let (count, top) = {
            let mut data = object.heap.borrow_mut();
            let data = data.as_mut().unwrap();
            if data.count == 0 {
                return Ok(None);
            }
            data.locked = true;
            // Extraction transfers the top to the caller, but PHP keeps its
            // slot readable until the sift overwrites it. Comparison callbacks
            // can inspect that slot through top(), current(), and debug output.
            let top = if discard {
                data.entries[0].take()
            } else {
                data.entries[0].clone()
            };
            (data.count, top)
        };
        let mut pending = None;
        let top = if discard {
            drop(top);
            if let Err(error) = self.drain_destructors(Ok(()), line) {
                pending = Some(error);
            }
            None
        } else {
            top
        };
        let bottom = {
            let mut data = object.heap.borrow_mut();
            let data = data.as_mut().unwrap();
            data.count = data.count.saturating_sub(1);
            data.entry(data.count)
        };
        let limit = (count - 1) / 2;
        let mut parent = 0;
        while parent < limit {
            let mut child = parent * 2 + 1;
            let (left, right, last) = {
                let data = object.heap.borrow();
                let data = data.as_ref().unwrap();
                (data.entry(child), data.entry(child + 1), data.count)
            };
            if child != last && self.heap_compare(object, &right, &left, &mut pending, line) > 0 {
                child += 1;
            }
            let value = object.heap.borrow().as_ref().unwrap().entry(child);
            if self.heap_compare(object, &bottom, &value, &mut pending, line) >= 0 {
                break;
            }
            object
                .heap
                .borrow_mut()
                .as_mut()
                .unwrap()
                .put(parent, value);
            parent = child;
        }
        {
            let mut data = object.heap.borrow_mut();
            let data = data.as_mut().unwrap();
            data.locked = false;
            data.corrupted |= pending.is_some();
            if data.count != 0 {
                data.put(parent, bottom);
            }
            data.entries.truncate(data.count);
        }
        self.drain_destructors(pending.map_or(Ok(top), Err), line)
    }
    pub(super) fn heap_iterator_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(name, b"current" | b"next") {
            self.heap_check(object, false, line)?;
        }
        self.heap_step(object, name, line)
    }
    fn heap_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        if name == b"next" {
            self.heap_delete(object, true, line)?;
            return Ok(Value::Null);
        }
        let data = object.heap.borrow();
        let data = data.as_ref().unwrap();
        Ok(match name {
            b"rewind" => Value::Null,
            b"key" => Value::Int(data.count as i64 - 1),
            b"valid" => Value::Bool(data.count != 0),
            b"current" if data.count != 0 => data.entry(0).extract(data.flags),
            b"current" => Value::Null,
            _ => unreachable!(),
        })
    }
    pub(super) fn heap_has_current(&self, object: &PhpObject) -> bool {
        object.heap.borrow().as_ref().unwrap().count != 0
    }
    pub(super) fn native_heap_method(&mut self, line: usize) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let function = self.current_function.clone().unwrap();
        let name = function.rsplit(|b| *b == b':').next().unwrap();
        match name {
            b"insert" => {
                self.heap_check(&object, true, line)?;
                let entry = Entry {
                    value: self.lookup(b"value").unwrap(),
                    priority: self.lookup(b"priority"),
                };
                self.heap_insert(&object, entry, line)?;
                Ok(Value::Bool(true))
            }
            b"extract" => {
                self.heap_check(&object, true, line)?;
                let Some(entry) = self.heap_delete(&object, false, line)? else {
                    return Err(self.array_iterator_exception(
                        b"RuntimeException",
                        "Can't extract from an empty heap".into(),
                        line,
                    ));
                };
                let flags = object.heap.borrow().as_ref().unwrap().flags;
                let result = entry.extract(flags);
                drop(entry);
                self.drain_destructors(Ok(result), line)
            }
            b"top" => {
                self.heap_check(&object, false, line)?;
                if !self.heap_has_current(&object) {
                    return Err(self.array_iterator_exception(
                        b"RuntimeException",
                        "Can't peek at an empty heap".into(),
                        line,
                    ));
                }
                self.heap_step(&object, b"current", line)
            }
            b"count" => Ok(Value::Int(
                object.heap.borrow().as_ref().unwrap().count as i64,
            )),
            b"isempty" => Ok(Value::Bool(!self.heap_has_current(&object))),
            b"iscorrupted" => Ok(Value::Bool(
                object.heap.borrow().as_ref().unwrap().corrupted,
            )),
            b"recoverfromcorruption" => {
                object.heap.borrow_mut().as_mut().unwrap().corrupted = false;
                Ok(Value::Bool(true))
            }
            b"setextractflags" => {
                let Value::Int(flags) = self.lookup(b"flags").unwrap() else {
                    unreachable!()
                };
                let flags = flags & 3;
                if flags == 0 {
                    return Err(self.array_iterator_exception(
                        b"RuntimeException",
                        "Must specify at least one extract flag".into(),
                        line,
                    ));
                }
                object.heap.borrow_mut().as_mut().unwrap().flags = flags;
                Ok(Value::Int(flags))
            }
            b"getextractflags" => Ok(Value::Int(object.heap.borrow().as_ref().unwrap().flags)),
            b"compare" => {
                let a = self
                    .lookup(b"value1")
                    .or_else(|| self.lookup(b"priority1"))
                    .unwrap();
                let b = self
                    .lookup(b"value2")
                    .or_else(|| self.lookup(b"priority2"))
                    .unwrap();
                let order = if function.starts_with(b"splminheap::") {
                    self.compare(&b, &a, line)?
                } else {
                    self.compare(&a, &b, line)?
                };
                Ok(Value::Int(order.unwrap_or(Ordering::Greater) as i64))
            }
            b"__debuginfo" => {
                let mut properties = object.snapshot();
                let data = object.heap.borrow();
                let data = data.as_ref().unwrap();
                let base = if data.priority {
                    "SplPriorityQueue"
                } else {
                    "SplHeap"
                };
                for (name, value) in [
                    ("flags", Value::Int(data.flags)),
                    ("isCorrupted", Value::Bool(data.corrupted)),
                    ("heap", Value::Array(Rc::new(data.storage()))),
                ] {
                    properties.insert(
                        ArrayKey::String(format!("\0{base}\0{name}").into_bytes()),
                        value,
                    );
                }
                Ok(Value::Array(Rc::new(properties)))
            }
            b"__serialize" => self.heap_serialize(&object, line),
            b"__unserialize" => self.heap_unserialize(&object, line),
            _ => self.heap_step(&object, name, line),
        }
    }
    fn heap_serialize(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<Value, Error> {
        self.heap_check(object, false, line)?;
        if object.heap.borrow().as_ref().unwrap().locked {
            return Err(self.array_iterator_exception(
                b"RuntimeException",
                "Cannot serialize heap while it is being modified.".into(),
                line,
            ));
        }
        let mut state = PhpArray::default();
        let data = object.heap.borrow();
        let data = data.as_ref().unwrap();
        state.insert(ArrayKey::String(b"flags".to_vec()), Value::Int(data.flags));
        state.insert(
            ArrayKey::String(b"heap_elements".to_vec()),
            Value::Array(Rc::new(data.storage())),
        );
        let mut result = PhpArray::default();
        result.insert(
            ArrayKey::Int(0),
            Value::Array(Rc::new(self.mangled_object_variables(object))),
        );
        result.insert(ArrayKey::Int(1), Value::Array(Rc::new(state)));
        Ok(Value::Array(Rc::new(result)))
    }
    fn heap_invalid_data(
        &mut self,
        object: &PhpObject,
        previous: Option<Error>,
        line: usize,
    ) -> Error {
        if let Some(error) = previous.as_ref()
            && error.kind == ErrorKind::UnwindExit
        {
            return previous.unwrap();
        }
        let error = self.array_iterator_exception(
            b"Exception",
            format!(
                "Invalid serialization data for {} object",
                object.class_name
            ),
            line,
        );
        if let Some(previous) = previous {
            let previous = self.materialize_error(previous);
            if let (Some(current), Some(previous)) = (&error.throwable, previous.throwable) {
                throwables::append_previous(current, &previous);
            }
        }
        error
    }
    fn heap_unserialize(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<Value, Error> {
        let priority = object.heap.borrow().as_ref().unwrap().priority;
        if !priority {
            self.heap_check(object, true, line)?;
        }
        let Value::Array(data) = self.lookup(b"data").unwrap() else {
            unreachable!()
        };
        if data.len() != 2 {
            return Err(self.heap_invalid_data(object, None, line));
        }
        let Some(Value::Array(props)) = data.get(&ArrayKey::Int(0)) else {
            return Err(self.heap_invalid_data(object, None, line));
        };
        for (key, value) in props.iter() {
            if let Err(error) = self.load_native_object_member(object, key, value, line) {
                return Err(self.heap_invalid_data(object, Some(error), line));
            }
        }
        let Some(Value::Array(state)) = data.get(&ArrayKey::Int(1)) else {
            return Err(self.heap_invalid_data(object, None, line));
        };
        let Some(Value::Int(mut flags)) = state.get(&ArrayKey::String(b"flags".to_vec())) else {
            return Err(self.heap_invalid_data(object, None, line));
        };
        if priority {
            flags &= 3;
            if flags == 0 {
                return Err(self.heap_invalid_data(object, None, line));
            }
        } else if flags != 0 {
            return Err(self.heap_invalid_data(object, None, line));
        }
        object.heap.borrow_mut().as_mut().unwrap().flags = flags;
        let Some(Value::Array(entries)) = state.get(&ArrayKey::String(b"heap_elements".to_vec()))
        else {
            return Err(self.heap_invalid_data(object, None, line));
        };
        for (_, entry) in entries.iter() {
            let entry = if priority {
                let Value::Array(entry) = entry else {
                    return Err(self.heap_invalid_data(object, None, line));
                };
                if entry.len() != 2 {
                    return Err(self.heap_invalid_data(object, None, line));
                }
                let (Some(value), Some(priority)) = (
                    entry.get(&ArrayKey::String(b"data".to_vec())),
                    entry.get(&ArrayKey::String(b"priority".to_vec())),
                ) else {
                    return Err(self.heap_invalid_data(object, None, line));
                };
                Entry {
                    value,
                    priority: Some(priority),
                }
            } else {
                Entry {
                    value: entry,
                    priority: None,
                }
            };
            if let Err(error) = self.heap_insert(object, entry, line) {
                return Err(self.heap_invalid_data(object, Some(error), line));
            }
        }
        self.heap_check(object, false, line)?;
        Ok(Value::Null)
    }
}
