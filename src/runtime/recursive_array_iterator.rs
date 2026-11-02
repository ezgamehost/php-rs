// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_array.c's recursive
// child construction. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::PhpObject;
use std::rc::Weak;

#[derive(Clone)]
pub(super) enum ChildLink {
    Array(Weak<RefCell<Value>>, ArrayKey),
    Object(Weak<PhpObject>, ArrayKey),
}

impl Runtime {
    pub(super) fn register_recursive_child(&mut self, object: &Rc<PhpObject>) {
        let source = object
            .array_iterator
            .borrow()
            .as_ref()
            .unwrap()
            .storage
            .clone();
        self.recursive_child_slots
            .insert(Rc::as_ptr(&source) as usize, Rc::downgrade(object));
    }

    pub(super) fn recursive_child_owner(&self, source: &variables::Cell) -> Option<Rc<PhpObject>> {
        let object = self
            .recursive_child_slots
            .get(&(Rc::as_ptr(source) as usize))?
            .upgrade()?;
        let matches = object
            .array_iterator
            .borrow()
            .as_ref()
            .is_some_and(|data| data.child.is_some() && Rc::ptr_eq(&data.storage, source));
        matches.then_some(object)
    }

    fn replace_child_slot(&self, slot: &mut Value, value: Value) {
        match slot {
            Value::Reference(reference) | Value::Indirect(reference) => {
                self.replace_child_slot(&mut reference.borrow_mut(), value)
            }
            _ => *slot = value,
        }
    }

    pub(super) fn replace_recursive_parent(&self, link: &ChildLink, value: Value) {
        match link {
            ChildLink::Array(source, key) => {
                if let Some(source) = source.upgrade() {
                    let shared = self.recursive_child_owner(&source).is_some();
                    if let Value::Array(array) = &mut *source.borrow_mut() {
                        if !shared {
                            Rc::make_mut(array);
                        }
                        array.mutate_shared(|data| {
                            if let Some(slot) = data.get_mut(key) {
                                self.replace_child_slot(slot, value);
                            }
                        });
                    }
                }
            }
            ChildLink::Object(object, key) => {
                if let Some(object) = object.upgrade()
                    && let Some(slot) = object.properties.borrow_mut().get_mut(key)
                {
                    self.replace_child_slot(slot, value);
                }
            }
        }
    }

    pub(super) fn recursive_array_iterator_child(
        &mut self,
        object: &Rc<PhpObject>,
        create: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let Some(key) = self.array_iterator_key(object) else {
            return Ok(if create {
                Value::Null
            } else {
                Value::Bool(false)
            });
        };
        let backing = array_iterator::storage(object);
        let value = backing.read(&key).unwrap().dereferenced();
        let flags = {
            let state = object.array_iterator.borrow();
            let data = state.as_ref().unwrap();
            let internal = match &*data.storage.borrow() {
                Value::Null => 0x0100_0000,
                Value::Object(inner) if inner.array_iterator.borrow().is_some() => 0x0200_0000,
                _ => 0,
            };
            data.flags | internal
        };
        let is_object = matches!(value, Value::Object(_) | Value::Closure(_));
        if !create {
            return Ok(Value::Bool(
                matches!(value, Value::Array(_)) || (is_object && flags & 4 == 0),
            ));
        }
        if is_object && flags & 4 != 0 {
            return Ok(Value::Null);
        }
        if let Value::Object(inner) = &value
            && inner.class.is_a(&object.class.name)
        {
            return Ok(value);
        }
        let link = match backing {
            array_iterator::Storage::Array(source) => ChildLink::Array(Rc::downgrade(&source), key),
            array_iterator::Storage::Object(object) => {
                ChildLink::Object(Rc::downgrade(&object), key)
            }
        };
        let child = self.instantiate_class(object.class.clone(), &[], false, line)?;
        let Value::Object(child) = child else {
            unreachable!()
        };
        child.array_iterator.borrow_mut().as_mut().unwrap().child = Some(link);
        let constructor = objects::method(&child.class, b"__construct").unwrap();
        let target = self.unchecked_method_target(
            constructor,
            child.class.clone(),
            Some(child.clone()),
            line,
        )?;
        let result = self.call_callback(&target, &[value, Value::Int(flags)], line);
        drop(target);
        match result {
            Ok(_) => Ok(Value::Object(child)),
            Err(error) => {
                // This native factory does not use the `new` failure rule:
                // a child whose constructor throws still runs its destructor.
                drop(child);
                self.drain_destructors(Err(error), line)
            }
        }
    }
}
