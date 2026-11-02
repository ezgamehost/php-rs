use super::*;

impl Runtime {
    pub(super) fn effective_reflected_property(
        &self,
        property: &PropertyReflection,
        object: &PhpObject,
    ) -> PropertyReflection {
        if Rc::ptr_eq(&property.scope, &object.class)
            || property
                .declaration
                .as_ref()
                .is_some_and(|p| p.visibility == Visibility::Private)
        {
            return property.clone();
        }
        let mut effective = property.clone();
        if let Some(decl) = object
            .class
            .properties
            .iter()
            .rev()
            .find(|p| p.name == property.name)
        {
            effective.owner = object.class.property_slot_owner(decl);
            effective.declaration = Some(decl.clone());
            effective.is_static = false;
        } else if let Some(decl) = object
            .class
            .static_properties
            .iter()
            .find(|p| p.name == property.name)
        {
            effective.owner = object
                .class
                .static_owner(&property.name)
                .expect("static property owner");
            effective.declaration = Some(decl.clone());
            effective.is_static = true;
        } else {
            effective.declaration = None;
            effective.is_static = false;
        }
        effective
    }

    pub(super) fn reflected_raw_property_value(
        &mut self,
        property: &PropertyReflection,
        write: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let Value::Object(object) = self.lookup(b"object").unwrap() else {
            unreachable!()
        };
        if !write && !object.class.is_a(&property.scope.name) {
            return Err(self.reflection_error(
                "Given object is not an instance of the class this property was declared in",
                line,
            ));
        }
        let property = self.effective_reflected_property(property, &object);
        if property.is_static {
            return Err(self.reflection_error(
                format!(
                    "May not use {} on static properties",
                    if write { "setRawValue" } else { "getRawValue" }
                ),
                line,
            ));
        }
        let outer = self.class_context.replace(ClassContext {
            scope: property.scope.clone(),
            called: property.scope.clone(),
        });
        let strict = std::mem::replace(&mut self.strict_types, false);
        let value = write.then(|| self.lookup(b"value").unwrap());
        let result =
            self.reflected_raw_property_access(&object, &property.name, value, false, line);
        self.class_context = outer;
        self.strict_types = strict;
        result.map(|value| {
            if write {
                Value::Null
            } else {
                value.dereferenced()
            }
        })
    }

    // Native raw access substitutes an internal accessor only when the requested
    // hook exists. Missing hooks keep the normal read/write behavior, including
    // magic methods, visibility, and virtual read-only/write-only diagnostics.
    pub(super) fn reflected_raw_property_access(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        value: Option<Value>,
        skip_lazy: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let get = value.is_none();
        let property = self
            .instance_property_owner(&object.class, name)
            .and_then(|owner| {
                let mut property = owner
                    .declaration
                    .properties
                    .iter()
                    .find(|p| p.name == name)?
                    .clone();
                property.declaring_class = owner.name.clone();
                Some(property)
            });
        if let Some(property) = property
            && property.hooks.as_ref().is_some_and(|h| {
                if get {
                    h.get.is_some()
                } else {
                    h.set.is_some()
                }
            })
        {
            let function = self.property_hook_trampoline(&property, get, skip_lazy, line);
            let args: Vec<_> = value
                .into_iter()
                .map(|value| Argument::Positional(literal(value, line)))
                .collect();
            return self.invoke_property_hook_arguments(object, function, &args, line, true);
        }
        if let Some(value) = value {
            if skip_lazy {
                self.write_property_value(object, name, value, line)
            } else {
                self.write_location(
                    &mut Location {
                        object: Some(object.clone()),
                        name: name.to_vec(),
                        static_class: None,
                        global: false,
                        keys: Vec::new(),
                        overloaded: None,
                        offset_reference: None,
                    },
                    value,
                    line,
                )
            }
        } else {
            self.read_property_value(object, name, line, Access::Read)
        }
    }
}
