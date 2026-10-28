use super::*;
use variables::Cell;

impl Runtime {
    pub(super) fn property_reference(
        &mut self,
        location: &mut Location,
        line: usize,
    ) -> Result<Cell, Error> {
        let root = self.property_reference_root(location, line)?;
        reference_paths::acquire(root, &mut location.keys, line, self)
    }

    fn property_reference_root(
        &mut self,
        location: &mut Location,
        line: usize,
    ) -> Result<Cell, Error> {
        if let Some(object) = &location.object {
            if let Some(value) = location.overloaded.take() {
                return Ok(match value {
                    Value::Reference(reference) => reference,
                    value => cell(value),
                });
            }
            if self.magic_get_applicable(object, &location.name) {
                let value = self.read_property_value(object, &location.name, line, Access::Read)?;
                self.warn_indirect_magic(object, &location.name, &value, line);
                return Ok(match value {
                    Value::Reference(reference) => reference,
                    value => cell(value),
                });
            }
            let key = self.instance_property_key(object, &location.name);
            let existing = object.properties.borrow().get(&key).cloned();
            if !location.keys.is_empty()
                && let Some(value) = &existing
                && matches!(value.dereferenced(), Value::Object(_) | Value::Closure(_))
            {
                self.check_instance_property_access(object, &location.name, false, line)?;
                return Ok(cell(value.dereferenced()));
            }
            // PHP permits fetching a restricted object property for indirect
            // object mutation, but the fetched handle cannot rebind its slot.
            if let Some(value) = &existing
                && matches!(value.dereferenced(), Value::Object(_) | Value::Closure(_))
                && self
                    .check_readonly_access(location, false, true, line)
                    .is_err()
            {
                self.check_instance_property_access(object, &location.name, false, line)?;
                return Ok(cell(value.dereferenced()));
            }
            self.check_readonly_access(location, false, true, line)?;
            let source = self.property_reference_source(location);
            if let Some(Value::Reference(reference)) = existing {
                if let Some(source) = source {
                    self.register_reference_source(&reference, source);
                }
                return Ok(reference);
            }
            let value = match existing {
                Some(value) => value,
                None => {
                    let value = if let Some(source) = &source {
                        self.initialize_reference_property(source, !location.keys.is_empty(), line)?
                    } else {
                        Value::Null
                    };
                    self.write_property_value(object, &location.name, value, line)?
                }
            };
            let reference = cell(value);
            object
                .properties
                .borrow_mut()
                .insert(key, Value::Reference(reference.clone()));
            if let Some(source) = source {
                self.register_reference_source(&reference, source);
            }
            return Ok(reference);
        }
        let class = location.static_class.as_ref().expect("property location");
        let stored = self.static_cell(class, &location.name, line, false)?;
        self.check_static_set_access(location, true, line)?;
        let source = self.property_reference_source(location);
        let reference = if let Some(stored) = stored {
            stored
        } else {
            let value = self.initialize_reference_property(
                source.as_ref().expect("uninitialized typed property"),
                !location.keys.is_empty(),
                line,
            )?;
            let reference = cell(value);
            class
                .static_owner(&location.name)
                .unwrap()
                .static_values
                .borrow_mut()
                .insert(location.name.clone(), reference.clone());
            reference
        };
        if let Some(source) = source {
            self.register_reference_source(&reference, source);
        }
        Ok(reference)
    }

    pub(super) fn bind_property_reference(
        &mut self,
        location: &mut Location,
        reference: Cell,
        line: usize,
    ) -> Result<(), Error> {
        if !location.keys.is_empty() {
            let root = self.property_reference_root(location, line)?;
            return reference_paths::bind(root, &mut location.keys, reference, line, self);
        }
        if let Some(object) = &location.object {
            if self.magic_get_applicable(object, &location.name) {
                let value = self.read_property_value(object, &location.name, line, Access::Read)?;
                self.warn_indirect_magic(object, &location.name, &value, line);
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot assign by reference to overloaded object",
                    line,
                ));
            }
            self.check_readonly_access(location, false, true, line)?;
            let source = self.property_reference_source(location);
            if let Some(source) = &source {
                self.check_reference_binding(&reference, source, line)?;
            }
            let key = self.instance_property_key(object, &location.name);
            if object.properties.borrow().get(&key).is_none() {
                let value = reference.borrow().clone();
                self.write_property_value(object, &location.name, value, line)?;
            }
            object
                .properties
                .borrow_mut()
                .insert(key, Value::Reference(reference.clone()));
            if let Some(source) = source {
                self.register_reference_source(&reference, source);
            }
            return Ok(());
        }
        let class = location.static_class.as_ref().expect("static property");
        self.static_cell(class, &location.name, line, false)?;
        self.check_static_set_access(location, true, line)?;
        let source = self.property_reference_source(location);
        if let Some(source) = &source {
            self.check_reference_binding(&reference, source, line)?;
        }
        let owner = class
            .static_owner(&location.name)
            .expect("declared static property");
        owner
            .static_values
            .borrow_mut()
            .insert(location.name.clone(), reference.clone());
        if let Some(source) = source {
            self.register_reference_source(&reference, source);
        }
        Ok(())
    }
}
