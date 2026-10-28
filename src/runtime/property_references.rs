use super::*;
use variables::Cell;

impl Runtime {
    pub(super) fn property_reference(
        &mut self,
        location: &mut Location,
        line: usize,
    ) -> Result<Cell, Error> {
        let root = self.property_reference_root(location, line)?;
        reference_paths::acquire(root, &mut location.keys, line, &mut self.diagnostics)
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
            self.check_property_reference_type(location, line)?;
            if let Some(Value::Reference(reference)) = existing {
                return Ok(reference);
            }
            let value = match existing {
                Some(value) => value,
                None => self.write_property_value(object, &location.name, Value::Null, line)?,
            };
            let reference = cell(value);
            object
                .properties
                .borrow_mut()
                .insert(key, Value::Reference(reference.clone()));
            return Ok(reference);
        }
        let class = location.static_class.as_ref().expect("property location");
        let stored = self.static_cell(class, &location.name, line, false)?;
        self.check_static_set_access(location, true, line)?;
        self.check_property_reference_type(location, line)?;
        Ok(stored.expect("untyped static property is initialized"))
    }

    pub(super) fn bind_property_reference(
        &mut self,
        location: &mut Location,
        reference: Cell,
        line: usize,
    ) -> Result<(), Error> {
        if !location.keys.is_empty() {
            let root = self.property_reference_root(location, line)?;
            return reference_paths::bind(
                root,
                &mut location.keys,
                reference,
                line,
                &mut self.diagnostics,
            );
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
            self.check_property_reference_type(location, line)?;
            let key = self.instance_property_key(object, &location.name);
            if object.properties.borrow().get(&key).is_none() {
                let value = reference.borrow().clone();
                self.write_property_value(object, &location.name, value, line)?;
            }
            object
                .properties
                .borrow_mut()
                .insert(key, Value::Reference(reference));
            return Ok(());
        }
        let class = location.static_class.as_ref().expect("static property");
        self.static_cell(class, &location.name, line, false)?;
        self.check_static_set_access(location, true, line)?;
        self.check_property_reference_type(location, line)?;
        let owner = class
            .static_owner(&location.name)
            .expect("declared static property");
        owner
            .static_values
            .borrow_mut()
            .insert(location.name.clone(), reference);
        Ok(())
    }

    fn check_property_reference_type(&self, location: &Location, line: usize) -> Result<(), Error> {
        let owner = if let Some(object) = &location.object {
            self.instance_property_owner(&object.class, &location.name)
        } else {
            location
                .static_class
                .as_ref()
                .and_then(|c| c.static_owner(&location.name))
        };
        if let Some(owner) = owner {
            let properties = if location.object.is_some() {
                &owner.declaration.properties
            } else {
                &owner.declaration.static_properties
            };
            if properties
                .iter()
                .any(|p| p.name == location.name && p.ty.is_some())
            {
                return Err(Error::new(
                    ErrorKind::Error,
                    "References to typed properties are not supported yet",
                    line,
                ));
            }
        }
        Ok(())
    }
}
