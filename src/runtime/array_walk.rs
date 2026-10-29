use super::*;
use crate::object::PropertyCursor;

impl Runtime {
    pub(super) fn array_walk(
        &mut self,
        recursive: bool,
        name: &str,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        line: usize,
    ) -> Result<Value, Error> {
        let mut args = args.into_iter();
        let first = args.next().unwrap().unwrap();
        if !matches!(
            first,
            Value::Array(_) | Value::Object(_) | Value::Closure(_)
        ) {
            // PHP's native parser accepts objects here but retains the legacy
            // "array" wording for invalid argument types.
            return Err(builtins::type_error(&first, name, 1, "array", line));
        }
        let source = if let Some(reference) = &references[0] {
            drop(first);
            reference.clone()
        } else {
            cell(first)
        };
        separate_walk_array(&source);
        let callback = self
            .native_callback(&args.next().unwrap().unwrap(), name, 2, false, line)?
            .unwrap();
        let userdata = args.next().flatten();
        self.walk_container(
            &source,
            &callback,
            userdata.as_ref(),
            recursive,
            &mut HashSet::new(),
            line,
        )?;
        Ok(Value::Bool(true))
    }

    fn walk_container(
        &mut self,
        source: &variables::Cell,
        callback: &functions::CallTarget,
        userdata: Option<&Value>,
        recursive: bool,
        active: &mut HashSet<usize>,
        line: usize,
    ) -> Result<(), Error> {
        let token = Rc::new(());
        let mut first = true;
        let mut object_iterator: Option<(
            std::rc::Weak<crate::PhpObject>,
            Rc<RefCell<PropertyCursor>>,
        )> = None;
        loop {
            let next = {
                let mut value = source.borrow_mut();
                match &mut *value {
                    Value::Array(array) => {
                        if !array.has_iterator(&token) {
                            let array = Rc::make_mut(array);
                            let position = if first { 0 } else { array.cursor_position() };
                            array.start_reference_iteration_at(&token, position);
                        }
                        array.next_reference(&token)
                    }
                    Value::Object(object) => {
                        let identity = Rc::downgrade(object);
                        if object_iterator
                            .as_ref()
                            .is_none_or(|(id, _)| !id.ptr_eq(&identity))
                        {
                            let mut cursor = PropertyCursor::default();
                            object.sync_property_cursor(&mut cursor);
                            if !first {
                                cursor.position = object.property_cursor.borrow().position;
                            }
                            let cursor = Rc::new(RefCell::new(cursor));
                            let mut iterators = object.property_iterators.borrow_mut();
                            iterators.retain(|iterator| iterator.strong_count() != 0);
                            iterators.push(Rc::downgrade(&cursor));
                            object_iterator = Some((identity, cursor));
                        }
                        let mut cursor = object_iterator.as_ref().unwrap().1.borrow_mut();
                        object.sync_property_cursor(&mut cursor);
                        let mut next = None;
                        while let Some(key) = cursor.keys.get(cursor.position).cloned() {
                            cursor.position += 1;
                            if object.properties.borrow().get(&key).is_some() {
                                let reference =
                                    self.walked_property_reference(object, &key, line)?;
                                next = Some((key, reference));
                                break;
                            }
                        }
                        next
                    }
                    Value::Closure(_) => None,
                    _ => {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            "Iterated value is no longer an array or object",
                            line,
                        ));
                    }
                }
            };
            first = false;
            let Some((key, reference)) = next else {
                return Ok(());
            };
            let identity = if recursive {
                separate_walk_array(&reference)
            } else {
                None
            };
            if let Some(identity) = identity {
                if !active.insert(identity) {
                    return Err(Error::new(ErrorKind::Error, "Recursion detected", line));
                }
                self.walk_container(&reference, callback, userdata, true, active, line)?;
                active.remove(&identity);
            } else {
                let mut values = vec![Value::Reference(reference), key.to_value()];
                if let Some(userdata) = userdata {
                    values.push(userdata.clone());
                }
                self.call_callback(callback, &values, line)?;
            }
        }
    }
}

fn separate_walk_array(reference: &variables::Cell) -> Option<usize> {
    match &mut *reference.borrow_mut() {
        Value::Array(array) => {
            Rc::make_mut(array);
            Some(Rc::as_ptr(array) as usize)
        }
        _ => None,
    }
}
