// Altered Rust adaptation of PHP 8.5.0 Zend/zend_interfaces.c and native
// SPL iterator wrappers. Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, object::ObjectHandle};

pub(super) enum IteratorState {
    FixedArray(usize),
    Directory {
        current: Option<Value>,
    },
    Heap {
        current: Option<Value>,
    },
    LinkedList {
        cursor: Rc<RefCell<linked_list::Cursor>>,
        flags: i64,
    },
}

pub(crate) struct InternalIteratorData {
    pub source: Rc<PhpObject>,
    state: IteratorState,
    rewound: bool,
    hidden: bool,
    _wrapper: Option<ObjectHandle>,
}

pub(super) fn hidden_iterator(object: &PhpObject) -> bool {
    object
        .internal_iterator
        .borrow()
        .as_ref()
        .is_some_and(|data| data.hidden)
}
pub(super) fn source_object(object: &Rc<PhpObject>) -> Rc<PhpObject> {
    object
        .internal_iterator
        .borrow()
        .as_ref()
        .filter(|data| data.hidden)
        .map_or_else(|| object.clone(), |data| data.source.clone())
}
impl Runtime {
    pub(super) fn native_iterator(
        &mut self,
        source: Rc<PhpObject>,
        state: IteratorState,
        hidden: bool,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        let wrapper = (!hidden).then(|| self.allocate_object_handle());
        let class = self.find_class(b"InternalIterator").unwrap();
        let object = self.allocate_class_object(class, line)?;
        *object.internal_iterator.borrow_mut() = Some(InternalIteratorData {
            source,
            state,
            rewound: false,
            hidden,
            _wrapper: wrapper,
        });
        Ok(object)
    }
    pub(super) fn internal_iterator_step(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        raw: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let directory = {
            let payload = object.internal_iterator.borrow();
            payload.as_ref().and_then(|data| match &data.state {
                IteratorState::Directory { current } => Some((
                    data.source.clone(),
                    (name == b"current").then(|| current.clone()).flatten(),
                )),
                _ => None,
            })
        };
        if let Some((source, cached)) = directory {
            let cache = name == b"current" && directory_iterator::caches_current(&source);
            if cache && let Some(value) = cached {
                return Ok(value);
            }
            let result = self.directory_iterator_step(
                &source,
                name,
                source.class.is_a(b"FilesystemIterator"),
                true,
                line,
            );
            if matches!(name, b"next" | b"rewind") {
                let old = {
                    let mut payload = object.internal_iterator.borrow_mut();
                    let IteratorState::Directory { current } = &mut payload.as_mut().unwrap().state
                    else {
                        unreachable!()
                    };
                    current.take()
                };
                drop(old);
                return self.drain_destructors(result, line);
            }
            let value = result?;
            if cache {
                let mut payload = object.internal_iterator.borrow_mut();
                let IteratorState::Directory { current } = &mut payload.as_mut().unwrap().state
                else {
                    unreachable!()
                };
                *current = Some(value.clone());
            }
            return Ok(value);
        }
        let heap = {
            let payload = object.internal_iterator.borrow();
            payload.as_ref().and_then(|data| {
                if let IteratorState::Heap { current } = &data.state {
                    Some((
                        data.source.clone(),
                        (name == b"current").then(|| current.clone()).flatten(),
                    ))
                } else {
                    None
                }
            })
        };
        if let Some((source, cached)) = heap {
            let result = self.heap_iterator_step(&source, name, line);
            if name == b"next" {
                let old = {
                    let mut payload = object.internal_iterator.borrow_mut();
                    let IteratorState::Heap { current } = &mut payload.as_mut().unwrap().state
                    else {
                        unreachable!()
                    };
                    current.take()
                };
                drop(old);
                return self.drain_destructors(result, line);
            }
            let value = result?;
            if name == b"current"
                && source.class.is_a(b"SplPriorityQueue")
                && self.heap_has_current(&source)
            {
                if let Some(cached) = cached {
                    return Ok(cached);
                }
                let mut payload = object.internal_iterator.borrow_mut();
                let IteratorState::Heap { current } = &mut payload.as_mut().unwrap().state else {
                    unreachable!()
                };
                *current = Some(value.clone());
            }
            return Ok(value);
        }
        let linked = {
            let mut payload = object.internal_iterator.borrow_mut();
            let data = payload.as_mut().ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    "The InternalIterator object has not been properly initialized",
                    line,
                )
            })?;
            if let IteratorState::LinkedList { cursor, flags } = &data.state {
                let linked = Some((data.source.clone(), cursor.clone(), *flags, !data.rewound));
                data.rewound = true;
                linked
            } else {
                None
            }
        };
        if let Some((source, cursor, flags, initial)) = linked {
            if initial && name != b"rewind" {
                self.linked_list_cursor_step(&source, &cursor, flags, b"rewind", raw, line)?;
            }
            return self.linked_list_cursor_step(&source, &cursor, flags, name, raw, line);
        }
        let (source, position) = {
            let mut payload = object.internal_iterator.borrow_mut();
            let data = payload.as_mut().unwrap();
            let IteratorState::FixedArray(position) = &mut data.state else {
                unreachable!()
            };
            if !data.rewound || name == b"rewind" {
                *position = 0;
                data.rewound = true;
            }
            if name == b"next" {
                *position = position.saturating_add(1);
            }
            (data.source.clone(), *position)
        };
        match name {
            b"rewind" | b"next" => Ok(Value::Null),
            b"key" => Ok(Value::Int(position as i64)),
            b"valid" => Ok(Value::Bool(
                position < source.fixed_array.borrow().as_ref().unwrap().elements.len(),
            )),
            b"current" => self
                .fixed_array_read(&source, position as i64, line)
                .map(|value| if raw { value } else { value.dereferenced() }),
            _ => unreachable!(),
        }
    }
    pub(super) fn native_iterator_has_current(&self, object: &PhpObject) -> bool {
        let data = object.internal_iterator.borrow();
        match data.as_ref().map(|data| &data.state) {
            Some(IteratorState::LinkedList { cursor, .. }) => {
                linked_list::has_current(&cursor.borrow())
            }
            Some(IteratorState::Directory { .. }) => {
                directory_iterator::has_current(&data.as_ref().unwrap().source)
            }
            Some(IteratorState::Heap { .. }) => {
                self.heap_has_current(&data.as_ref().unwrap().source)
            }
            _ => true,
        }
    }
}
