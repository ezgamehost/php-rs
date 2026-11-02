use super::*;
use crate::{PhpObject, ast::PropertyDecl, object::ClassContext};

pub(super) struct ExportProperty {
    pub key: ArrayKey,
    pub declaration: Option<PropertyDecl>,
    pub value: Value,
}

impl ExportProperty {
    pub(super) fn snapshot(key: ArrayKey, value: Value) -> Self {
        Self {
            key,
            declaration: None,
            value,
        }
    }
}

impl Runtime {
    pub(super) fn has_property_hooks(&self, object: &PhpObject) -> bool {
        object.class.properties.iter().any(|p| p.hooks.is_some())
    }

    // Declared entries remain live until consumed; dynamic values are captured
    // before callbacks. Keep uninitialized declarations so an earlier getter can
    // initialize a later slot without changing the enumeration order.
    pub(super) fn property_export_entries(&self, object: &PhpObject) -> Vec<ExportProperty> {
        let mut entries: Vec<_> = object
            .class
            .properties
            .iter()
            .map(|p| ExportProperty {
                key: ArrayKey::String(p.storage_name()),
                declaration: Some(p.clone()),
                value: Value::Null,
            })
            .collect();
        let declared: HashSet<_> = entries.iter().map(|p| p.key.clone()).collect();
        for (key, value) in object.snapshot().iter_copied() {
            if !declared.contains(&key) {
                entries.push(ExportProperty::snapshot(key, value));
            }
        }
        entries
    }

    pub(super) fn read_export_property(
        &mut self,
        object: &Rc<PhpObject>,
        property: &PropertyDecl,
        line: usize,
    ) -> Result<Option<Value>, Error> {
        if let Some(hooks) = &property.hooks {
            if hooks.is_virtual && hooks.get.is_none() {
                return Ok(None);
            }
            let scope = object.class.property_slot_owner(property);
            let outer = self.class_context.replace(ClassContext {
                scope,
                called: object.class.clone(),
            });
            let result = self.read_property_value(object, &property.name, line, Access::Probe);
            self.class_context = outer;
            result.map(Some)
        } else {
            Ok(object
                .properties
                .borrow()
                .get(&ArrayKey::String(property.storage_name())))
        }
    }

    pub(super) fn object_variables(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<PhpArray, Error> {
        if !self.has_property_hooks(object) {
            return Ok(self.raw_object_variables(object));
        }
        let entries = self.property_export_entries(object);
        let mut result = PhpArray::default();
        for entry in entries {
            if !self.property_key_visible(object, &entry.key) {
                continue;
            }
            let (key, value) = if let Some(property) = entry.declaration {
                let Some(value) = self.read_export_property(object, &property, line)? else {
                    continue;
                };
                (ArrayKey::String(property.name), value)
            } else {
                (
                    ArrayKey::from_value(&entry.key.to_value(), line, self)?,
                    entry.value,
                )
            };
            if result.get(&key).is_none() {
                result.insert(key, value);
            }
        }
        Ok(result)
    }
}

impl Runtime {
    pub(super) fn foreach_hooked_object(
        &mut self,
        object: &Rc<PhpObject>,
        key_target: Option<&Expr>,
        value_target: &Expr,
        by_reference: bool,
        body: &Stmt,
    ) -> Result<Flow, Error> {
        let mut declared = object
            .class
            .properties
            .iter()
            .filter(|p| self.property_key_visible(object, &ArrayKey::String(p.storage_name())))
            .cloned()
            .collect::<Vec<_>>()
            .into_iter();
        let cursor = Rc::new(RefCell::new(crate::object::PropertyCursor::default()));
        object.sync_property_cursor(&mut cursor.borrow_mut());
        cursor.borrow_mut().position = object.class.properties.len();
        {
            let mut iterators = object.property_iterators.borrow_mut();
            iterators.retain(|iterator| iterator.strong_count() > 0);
            iterators.push(Rc::downgrade(&cursor));
        }
        loop {
            let (key, key_value, mut value, hooked) = if let Some(property) = declared.next() {
                if let Some(hooks) = &property.hooks {
                    if hooks.get.is_none() && hooks.is_virtual {
                        continue;
                    }
                    if by_reference && hooks.get.as_ref().is_none_or(|get| !get.returns_reference) {
                        return Err(Error::new(
                            ErrorKind::Error,
                            format!(
                                "Cannot create reference to property {}::${}",
                                object.class_name,
                                String::from_utf8_lossy(&property.name)
                            ),
                            value_target.line,
                        ));
                    }
                    if hooks.get.is_none()
                        && object
                            .properties
                            .borrow()
                            .get(&ArrayKey::String(property.storage_name()))
                            .is_none()
                    {
                        continue;
                    }
                }
                let Some(value) =
                    self.read_export_property(object, &property, value_target.line)?
                else {
                    continue;
                };
                (
                    ArrayKey::String(property.storage_name()),
                    Value::String(property.name.clone().into()),
                    value,
                    property.hooks.is_some(),
                )
            } else {
                let key = {
                    let mut cursor = cursor.borrow_mut();
                    object.sync_property_cursor(&mut cursor);
                    let Some(key) = cursor.keys.get(cursor.position).cloned() else {
                        break;
                    };
                    // Advancing before the body lets removal adjust the position
                    // correctly, including removal and reinsertion of this key.
                    cursor.position += 1;
                    key
                };
                if !self.property_key_visible(object, &key) {
                    continue;
                }
                let Some(value) = object.properties.borrow().get(&key) else {
                    continue;
                };
                let key_value = key.to_value();
                (key, key_value, value, false)
            };
            if by_reference {
                let reference = if hooked {
                    match &value {
                        Value::Reference(reference) => reference.clone(),
                        _ => cell(value.clone()),
                    }
                } else {
                    self.iterated_property_reference(object, &key, value_target.line)?
                };
                value = Value::Reference(reference.clone());
                if matches!(value_target.kind, ExprKind::Destructure(_)) {
                    self.destructure_reference(value_target, reference)?;
                } else {
                    let mut target = self.location(value_target)?;
                    self.bind_reference_location(&mut target, reference, value_target.line)?;
                }
            } else {
                self.assign_target(value_target, value.dereferenced())?;
            }
            if let Some(target) = key_target {
                self.assign_target(target, key_value)?;
            }
            let result = self.block(std::slice::from_ref(body));
            // The iterator owns its current value through the body, even if the
            // loop variable and backing slot are unset by user code.
            drop(value);
            match self.drain_destructors(result, value_target.line)? {
                Flow::Next | Flow::Continue(1) => {}
                Flow::Break(1) => break,
                flow => return Ok(flow.leave_level()),
            }
        }
        Ok(Flow::Next)
    }
}
