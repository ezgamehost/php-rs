// Altered Rust adaptation of PHP 8.5.0 Zend/zend_interfaces.c and native
// SPL iterator wrappers. Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, object::ObjectHandle};

pub(super) enum IteratorState {
    FixedArray(usize),
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
            _ => true,
        }
    }
}
