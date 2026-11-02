// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_dllist.c and its stubs.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, ast::NativeMethod, object::Class};
use std::{collections::VecDeque, rc::Weak};

struct Node {
    previous: RefCell<Weak<Node>>,
    next: RefCell<Weak<Node>>,
    value: RefCell<Option<Value>>,
}
#[derive(Default, Clone)]
pub(crate) struct Cursor {
    node: Option<Rc<Node>>,
    position: i64,
}
#[derive(Default)]
pub(crate) struct LinkedListData {
    nodes: VecDeque<Rc<Node>>,
    cursor: Rc<RefCell<Cursor>>,
    flags: i64,
}
impl LinkedListData {
    pub(super) fn new(class: &Class) -> Self {
        Self {
            flags: if class.is_a(b"SplStack") {
                6
            } else if class.is_a(b"SplQueue") {
                4
            } else {
                0
            },
            nodes: VecDeque::new(),
            cursor: Rc::default(),
        }
    }
    pub(super) fn cloned_for(&self, class: &Class) -> Self {
        let mut data = self.clone();
        // Native clone allocation reapplies the subclass's frozen direction,
        // even after __unserialize() replaced the original flags.
        data.flags |= Self::new(class).flags;
        data
    }
    fn insert(&mut self, index: usize, value: Value) {
        let previous = index.checked_sub(1).and_then(|i| self.nodes.get(i));
        let next = self.nodes.get(index);
        let node = Rc::new(Node {
            previous: RefCell::new(previous.map_or_else(Weak::new, Rc::downgrade)),
            next: RefCell::new(next.map_or_else(Weak::new, Rc::downgrade)),
            value: RefCell::new(Some(value)),
        });
        if let Some(previous) = previous {
            *previous.next.borrow_mut() = Rc::downgrade(&node);
        }
        if let Some(next) = next {
            *next.previous.borrow_mut() = Rc::downgrade(&node);
        }
        self.nodes.insert(index, node);
    }
    fn remove(&mut self, index: usize, kind: Removal) -> Option<Value> {
        let node = self.nodes.remove(index)?;
        let previous = node.previous.borrow().upgrade();
        let next = node.next.borrow().upgrade();
        if let Some(previous) = &previous {
            *previous.next.borrow_mut() = next.as_ref().map_or_else(Weak::new, Rc::downgrade);
        }
        if let Some(next) = &next {
            *next.previous.borrow_mut() = previous.as_ref().map_or_else(Weak::new, Rc::downgrade);
        }
        match kind {
            Removal::Pop => *node.previous.borrow_mut() = Weak::new(),
            Removal::Shift => *node.next.borrow_mut() = Weak::new(),
            Removal::Offset => {
                let mut cursor = self.cursor.borrow_mut();
                if cursor
                    .node
                    .as_ref()
                    .is_some_and(|current| Rc::ptr_eq(current, &node))
                {
                    cursor.node = None;
                }
            }
        }
        node.value.borrow_mut().take()
    }
    fn index(&self, index: i64) -> Option<usize> {
        let index = usize::try_from(index)
            .ok()
            .filter(|index| *index < self.nodes.len())?;
        Some(if self.flags & 2 != 0 {
            self.nodes.len() - index - 1
        } else {
            index
        })
    }
    fn storage(&self) -> PhpArray {
        let mut array = PhpArray::default();
        for (index, node) in self.nodes.iter().enumerate() {
            array.insert(
                ArrayKey::Int(index as i64),
                node.value.borrow().as_ref().unwrap().clone(),
            );
        }
        array
    }
    pub(super) fn finish_unserialization(&mut self) {
        for node in &self.nodes {
            let value = node.value.borrow().clone();
            *node.value.borrow_mut() = value;
        }
    }
    pub(super) fn values(&self) -> Vec<Value> {
        self.nodes
            .iter()
            .filter_map(|node| node.value.borrow().clone())
            .collect()
    }
}
impl Clone for LinkedListData {
    fn clone(&self) -> Self {
        let mut data = Self {
            flags: self.flags,
            nodes: VecDeque::new(),
            cursor: Rc::default(),
        };
        for node in &self.nodes {
            data.insert(
                data.nodes.len(),
                node.value.borrow().as_ref().unwrap().clone(),
            );
        }
        data.cursor.borrow_mut().node = data.nodes.front().cloned();
        data
    }
}
impl Drop for LinkedListData {
    fn drop(&mut self) {
        while let Some(node) = self.nodes.pop_back() {
            drop(node.value.borrow_mut().take());
        }
    }
}
enum Removal {
    Pop,
    Shift,
    Offset,
}

pub(super) fn native_iteration(object: &PhpObject) -> bool {
    object.linked_list.borrow().is_some()
        && [b"rewind".as_slice(), b"valid", b"current", b"key", b"next"]
            .iter()
            .all(|name| {
                objects::method(&object.class, name)
                    .is_some_and(|method| matches!(method.native, Some(NativeMethod::LinkedList)))
            })
}
pub(super) fn has_current(cursor: &Cursor) -> bool {
    cursor
        .node
        .as_ref()
        .is_some_and(|node| node.value.borrow().is_some())
}

impl Runtime {
    fn linked_list_serialize(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        self.serialize_native(line, |runtime, state, output| {
            let (flags, mut current) = {
                let data = object.linked_list.borrow();
                let data = data.as_ref().unwrap();
                (data.flags, data.nodes.front().cloned())
            };
            runtime.encode_serialized(&Value::Int(flags), state, output, 0, line)?;
            while let Some(node) = current {
                let next = node.next.borrow().upgrade();
                let Some(value) = node.value.borrow().clone() else {
                    break;
                };
                output.push(b':');
                runtime.encode_serialized(&value, state, output, 0, line)?;
                // Serialization hooks can remove the next node. Retain its handle
                // across the callback, then stop if it is no longer in the list.
                current = next.filter(|node| node.value.borrow().is_some());
            }
            Ok(())
        })
    }

    fn linked_list_unserialize(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        use unserialization::Failure;
        let bytes = self.lookup(b"data").unwrap().to_bytes();
        if bytes.is_empty() {
            return Ok(Value::Null);
        }
        loop {
            let old = {
                let mut data = object.linked_list.borrow_mut();
                let data = data.as_mut().unwrap();
                let Some(index) = data.nodes.len().checked_sub(1) else {
                    break;
                };
                data.remove(index, Removal::Pop)
            };
            drop(old);
            self.drain_destructors(Ok(()), line)?;
        }
        self.unserialize_native(
            &bytes,
            "SplDoublyLinkedList::unserialize",
            line,
            |runtime, decoder| {
                decoder.track_native_object(object);
                let flags = decoder.value(runtime)?;
                let Value::Int(flags) = *flags.borrow() else {
                    return Err(Failure::Syntax);
                };
                object.linked_list.borrow_mut().as_mut().unwrap().flags = flags as i32 as i64;
                while decoder.peek() == Some(b':') {
                    decoder.pos += 1;
                    let slot = decoder.value(runtime)?;
                    let mut data = object.linked_list.borrow_mut();
                    let data = data.as_mut().unwrap();
                    data.insert(data.nodes.len(), Value::Indirect(slot));
                }
                if !matches!(decoder.peek(), None | Some(0)) {
                    return Err(Failure::Syntax);
                }
                Ok(())
            },
        )
    }

    pub(super) fn builtin_linked_list_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        if !matches!(name, b"spldoublylinkedlist" | b"splqueue" | b"splstack") {
            return None;
        }
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let base = name == b"spldoublylinkedlist";
        let code: &[u8] = if base {
            b"<?php class SplDoublyLinkedList implements Iterator, Countable, ArrayAccess, Serializable {
            public const int IT_MODE_LIFO = 2, IT_MODE_FIFO = 0, IT_MODE_DELETE = 1, IT_MODE_KEEP = 0;
            public function add(int $index, mixed $value): void {}
            public function pop(): mixed {}
            public function shift(): mixed {}
            public function push(mixed $value): void {}
            public function unshift(mixed $value): void {}
            public function top(): mixed {}
            public function bottom(): mixed {}
            public function __debugInfo(): array {}
            public function count(): int {}
            public function isEmpty(): bool {}
            public function setIteratorMode(int $mode): int {}
            public function getIteratorMode(): int {}
            public function offsetExists($index): bool {}
            public function offsetGet($index): mixed {}
            public function offsetSet($index, mixed $value): void {}
            public function offsetUnset($index): void {}
            public function rewind(): void {}
            public function current(): mixed {}
            public function key(): int {}
            public function prev(): void {}
            public function next(): void {}
            public function valid(): bool {}
            public function serialize(): string {}
            public function unserialize(string $data): void {}
            public function __serialize(): array {}
            public function __unserialize(array $data): void {}
        }"
        } else if name == b"splqueue" {
            b"<?php class SplQueue extends SplDoublyLinkedList {
            public function enqueue(mixed $value): void {}
            public function dequeue(): mixed {}
        }"
        } else {
            b"<?php class SplStack extends SplDoublyLinkedList {}"
        };
        let (mut statements, _, _) = parser::parse(lexer::lex(code).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::LinkedList);
            method.tentative_return_type = method.return_type.take();
        }
        let parent = (!base).then(|| self.find_class(b"SplDoublyLinkedList").unwrap());
        let mut methods = declaration.methods.clone();
        if let Some(parent) = &parent {
            methods.extend(parent.methods.iter().cloned());
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods,
            declaration,
            parent,
            interfaces: if base {
                [
                    b"Iterator".as_slice(),
                    b"Countable",
                    b"ArrayAccess",
                    b"Serializable",
                ]
                .iter()
                .map(|name| self.find_class(name).unwrap())
                .collect()
            } else {
                Vec::new()
            },
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
    pub(super) fn linked_list_iterator(
        &mut self,
        object: Rc<PhpObject>,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        let state = {
            let data = object.linked_list.borrow();
            let data = data.as_ref().unwrap();
            native_iterator::IteratorState::LinkedList {
                cursor: Rc::new(RefCell::new(data.cursor.borrow().clone())),
                flags: data.flags & 3,
            }
        };
        self.native_iterator(object, state, true, line)
    }
    pub(super) fn linked_list_cursor_step(
        &mut self,
        object: &PhpObject,
        cursor: &Rc<RefCell<Cursor>>,
        flags: i64,
        name: &[u8],
        raw: bool,
        line: usize,
    ) -> Result<Value, Error> {
        match name {
            b"rewind" => {
                let data = object.linked_list.borrow();
                let data = data.as_ref().unwrap();
                let mut cursor = cursor.borrow_mut();
                cursor.node = if flags & 2 != 0 {
                    data.nodes.back()
                } else {
                    data.nodes.front()
                }
                .cloned();
                cursor.position = if flags & 2 != 0 {
                    data.nodes.len() as i64 - 1
                } else {
                    0
                };
                Ok(Value::Null)
            }
            b"valid" => Ok(Value::Bool(cursor.borrow().node.is_some())),
            b"key" => Ok(Value::Int(cursor.borrow().position)),
            b"current" => {
                let value = cursor
                    .borrow()
                    .node
                    .as_ref()
                    .and_then(|node| node.value.borrow().clone())
                    .unwrap_or(Value::Null);
                Ok(if raw { value } else { value.dereferenced() })
            }
            b"next" | b"prev" => {
                let flags = if name == b"prev" { flags ^ 2 } else { flags };
                let old = {
                    let mut cursor = cursor.borrow_mut();
                    let Some(old) = cursor.node.take() else {
                        return Ok(Value::Null);
                    };
                    cursor.node = if flags & 2 != 0 {
                        old.previous.borrow().upgrade()
                    } else {
                        old.next.borrow().upgrade()
                    };
                    if flags & 2 != 0 {
                        cursor.position -= 1;
                    } else if flags & 1 == 0 {
                        cursor.position += 1;
                    }
                    old
                };
                if flags & 1 != 0 {
                    let removed = {
                        let mut data = object.linked_list.borrow_mut();
                        let data = data.as_mut().unwrap();
                        if flags & 2 != 0 {
                            data.nodes
                                .len()
                                .checked_sub(1)
                                .and_then(|index| data.remove(index, Removal::Pop))
                        } else {
                            data.remove(0, Removal::Shift)
                        }
                    };
                    drop(removed);
                    self.drain_destructors(Ok(()), line)?;
                }
                drop(old);
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }
    fn linked_list_range_error(&mut self, method: &[u8], line: usize) -> Error {
        self.array_iterator_exception(
            b"OutOfRangeException",
            format!(
                "SplDoublyLinkedList::{}(): Argument #1 ($index) is out of range",
                String::from_utf8_lossy(method)
            ),
            line,
        )
    }
    pub(super) fn native_linked_list_method(&mut self, line: usize) -> Result<Value, Error> {
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
            b"rewind" | b"current" | b"key" | b"prev" | b"next" | b"valid" => {
                let (cursor, flags) = {
                    let data = object.linked_list.borrow();
                    let data = data.as_ref().unwrap();
                    (data.cursor.clone(), data.flags)
                };
                self.linked_list_cursor_step(&object, &cursor, flags, &name, false, line)
            }
            b"push" | b"enqueue" | b"unshift" | b"add" => {
                let value = self.lookup(b"value").unwrap();
                let index = if name == b"add" {
                    let Value::Int(index) = self.lookup(b"index").unwrap() else {
                        unreachable!()
                    };
                    Some(index)
                } else {
                    None
                };
                let mut data = object.linked_list.borrow_mut();
                let data = data.as_mut().unwrap();
                let index = if let Some(index) = index {
                    if index < 0 || index as usize > data.nodes.len() {
                        return Err(self.linked_list_range_error(b"add", line));
                    }
                    if index as usize == data.nodes.len() {
                        data.nodes.len()
                    } else {
                        data.index(index).unwrap()
                    }
                } else if name == b"unshift" {
                    0
                } else {
                    data.nodes.len()
                };
                data.insert(index, value);
                Ok(Value::Null)
            }
            b"pop" | b"shift" | b"dequeue" => {
                let value = {
                    let mut data = object.linked_list.borrow_mut();
                    let data = data.as_mut().unwrap();
                    if name == b"pop" {
                        data.nodes
                            .len()
                            .checked_sub(1)
                            .and_then(|index| data.remove(index, Removal::Pop))
                    } else {
                        data.remove(0, Removal::Shift)
                    }
                };
                value.ok_or_else(|| {
                    self.array_iterator_exception(
                        b"RuntimeException",
                        format!(
                            "Can't {} from an empty datastructure",
                            if name == b"pop" { "pop" } else { "shift" }
                        ),
                        line,
                    )
                })
            }
            b"top" | b"bottom" => {
                let value = {
                    let data = object.linked_list.borrow();
                    let data = data.as_ref().unwrap();
                    (if name == b"top" {
                        data.nodes.back()
                    } else {
                        data.nodes.front()
                    })
                    .and_then(|node| node.value.borrow().as_ref().map(Value::dereferenced))
                };
                value.ok_or_else(|| {
                    self.array_iterator_exception(
                        b"RuntimeException",
                        "Can't peek at an empty datastructure".into(),
                        line,
                    )
                })
            }
            b"count" => Ok(Value::Int(
                object.linked_list.borrow().as_ref().unwrap().nodes.len() as i64,
            )),
            b"isempty" => {
                let value = self.count_value(&Value::Object(object), None, "count", line)?;
                Ok(Value::Bool(value == Value::Int(0)))
            }
            b"getiteratormode" => Ok(Value::Int(
                object.linked_list.borrow().as_ref().unwrap().flags,
            )),
            b"setiteratormode" => {
                let Value::Int(mode) = self.lookup(b"mode").unwrap() else {
                    unreachable!()
                };
                let flags = object.linked_list.borrow().as_ref().unwrap().flags;
                if flags & 4 != 0 && flags & 2 != mode & 2 {
                    return Err(self.array_iterator_exception(
                        b"RuntimeException",
                        "Iterators' LIFO/FIFO modes for SplStack/SplQueue objects are frozen"
                            .into(),
                        line,
                    ));
                }
                let flags = (mode & 3) | (flags & 4);
                object.linked_list.borrow_mut().as_mut().unwrap().flags = flags;
                Ok(Value::Int(flags))
            }
            b"offsetexists" | b"offsetget" | b"offsetset" | b"offsetunset" => {
                let index = self.lookup(b"index").unwrap();
                if name == b"offsetset" && index == Value::Null {
                    let value = self.lookup(b"value").unwrap();
                    let mut data = object.linked_list.borrow_mut();
                    let data = data.as_mut().unwrap();
                    data.insert(data.nodes.len(), value);
                    return Ok(Value::Null);
                }
                let Value::Int(index) = index else {
                    unreachable!()
                };
                let index = object.linked_list.borrow().as_ref().unwrap().index(index);
                if name == b"offsetexists" {
                    return Ok(Value::Bool(index.is_some()));
                }
                let Some(index) = index else {
                    return Err(self.linked_list_range_error(
                        match name.as_slice() {
                            b"offsetget" => b"offsetGet",
                            b"offsetset" => b"offsetSet",
                            _ => b"offsetUnset",
                        },
                        line,
                    ));
                };
                if name == b"offsetget" {
                    let node = object.linked_list.borrow().as_ref().unwrap().nodes[index].clone();
                    return Ok(node.value.borrow().as_ref().unwrap().dereferenced());
                }
                let old = if name == b"offsetunset" {
                    object
                        .linked_list
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .remove(index, Removal::Offset)
                } else {
                    let value = self.lookup(b"value").unwrap();
                    let node = object.linked_list.borrow().as_ref().unwrap().nodes[index].clone();
                    node.value.borrow_mut().replace(value)
                };
                drop(old);
                self.drain_destructors(Ok(Value::Null), line)
            }
            b"__debuginfo" => {
                let mut array = object.snapshot();
                let data = object.linked_list.borrow();
                let data = data.as_ref().unwrap();
                array.insert(
                    ArrayKey::String(b"\0SplDoublyLinkedList\0flags".to_vec()),
                    Value::Int(data.flags),
                );
                array.insert(
                    ArrayKey::String(b"\0SplDoublyLinkedList\0dllist".to_vec()),
                    Value::Array(Rc::new(data.storage())),
                );
                Ok(Value::Array(Rc::new(array)))
            }
            b"serialize" => self.linked_list_serialize(&object, line),
            b"unserialize" => self.linked_list_unserialize(&object, line),
            b"__serialize" => {
                let data = object.linked_list.borrow();
                let data = data.as_ref().unwrap();
                let mut array = PhpArray::default();
                array.insert(ArrayKey::Int(0), Value::Int(data.flags));
                array.insert(ArrayKey::Int(1), Value::Array(Rc::new(data.storage())));
                array.insert(
                    ArrayKey::Int(2),
                    Value::Array(Rc::new(self.mangled_object_variables(&object))),
                );
                Ok(Value::Array(Rc::new(array)))
            }
            b"__unserialize" => {
                let Value::Array(data) = self.lookup(b"data").unwrap() else {
                    unreachable!()
                };
                let (
                    Some(Value::Int(flags)),
                    Some(Value::Array(storage)),
                    Some(Value::Array(members)),
                ) = (
                    data.get(&ArrayKey::Int(0)),
                    data.get(&ArrayKey::Int(1)),
                    data.get(&ArrayKey::Int(2)),
                )
                else {
                    return Err(self.array_iterator_exception(
                        b"UnexpectedValueException",
                        "Incomplete or ill-typed serialization data".into(),
                        line,
                    ));
                };
                {
                    let mut data = object.linked_list.borrow_mut();
                    let data = data.as_mut().unwrap();
                    data.flags = flags as i32 as i64;
                    for (_, value) in storage.iter() {
                        data.insert(data.nodes.len(), value);
                    }
                }
                for (key, value) in members.iter() {
                    self.load_native_object_member(&object, key, value, line)?;
                }
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }
}
