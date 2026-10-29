use super::*;
use crate::{PhpObject, ast::PropertyDecl, object::Class};
use std::rc::Weak;
use variables::Cell;

#[derive(Clone)]
enum Holder {
    Instance(Weak<PhpObject>, ArrayKey),
    Static(Weak<Class>),
}

#[derive(Clone)]
pub(super) struct Source {
    holder: Holder,
    owner: Rc<Class>,
    property: PropertyDecl,
}

impl Source {
    fn holds(&self, reference: &Cell) -> bool {
        match &self.holder {
            Holder::Instance(object, key) => object.upgrade().is_some_and(|object| {
                matches!(object.properties.borrow().get(key), Some(Value::Reference(stored)) if Rc::ptr_eq(stored, reference))
            }),
            Holder::Static(class) => class.upgrade().is_some_and(|class| {
                class.static_values.borrow().get(&self.property.name).is_some_and(|stored| Rc::ptr_eq(stored, reference))
            }),
        }
    }

    fn label(&self) -> String {
        format!(
            "{}::${} of type {}",
            self.owner.display_name(),
            String::from_utf8_lossy(&self.property.name),
            typed_properties::type_label(
                self.property.ty.as_ref().expect("typed source"),
                &self.owner
            )
        )
    }
}

#[derive(Default)]
pub(super) struct Constraints {
    // A weak cell keeps the address unique without extending the referent's life.
    // Property holders are weak too: unsetting/rebinding a property or destroying
    // its object must remove its constraint from surviving PHP aliases.
    reference: Weak<RefCell<Value>>,
    sources: Vec<Source>,
}

impl Runtime {
    pub(super) fn iterated_property_reference(
        &mut self,
        object: &Rc<PhpObject>,
        key: &ArrayKey,
        line: usize,
    ) -> Result<Cell, Error> {
        self.property_iteration_reference(object, key, line, false)
    }

    pub(super) fn walked_property_reference(
        &mut self,
        object: &Rc<PhpObject>,
        key: &ArrayKey,
        line: usize,
    ) -> Result<Cell, Error> {
        self.property_iteration_reference(object, key, line, true)
    }

    fn property_iteration_reference(
        &mut self,
        object: &Rc<PhpObject>,
        key: &ArrayKey,
        line: usize,
        native_walk: bool,
    ) -> Result<Cell, Error> {
        let property = object
            .class
            .properties
            .iter()
            .find(|property| ArrayKey::String(property.storage_name()) == *key);
        // Native array_walk acquires storage references directly; foreach has
        // the additional readonly restriction. Both retain property type sources.
        if let Some(property) = property
            && property.is_readonly
            && !native_walk
        {
            let owner = object.class.property_slot_owner(property);
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot acquire reference to readonly property {}::${}",
                    owner.display_name(),
                    String::from_utf8_lossy(&property.name)
                ),
                line,
            ));
        }
        let reference = {
            let mut properties = object.properties.borrow_mut();
            let value = properties.get_mut(key).expect("iterated property exists");
            match value {
                Value::Reference(reference) => reference.clone(),
                _ => {
                    let reference = cell(std::mem::replace(value, Value::Null));
                    *value = Value::Reference(reference.clone());
                    reference
                }
            }
        };
        if let Some(property) = property
            && property.ty.is_some()
        {
            self.register_reference_source(
                &reference,
                Source {
                    holder: Holder::Instance(Rc::downgrade(object), key.clone()),
                    owner: object.class.property_slot_owner(property),
                    property: property.clone(),
                },
            );
        }
        Ok(reference)
    }

    pub(super) fn check_property_array_init(
        &mut self,
        location: &Location,
        value: &Value,
        line: usize,
    ) -> Result<(), Error> {
        if !location.keys.is_empty()
            && matches!(value, Value::Null | Value::Bool(false))
            && let Some(source) = self.property_reference_source(location)
        {
            self.initialize_reference_property(&source, true, line)?;
        }
        Ok(())
    }

    fn reference_sources(&mut self, reference: &Cell) -> Vec<Source> {
        let identity = Rc::as_ptr(reference) as usize;
        let Some(constraints) = self.reference_constraints.get_mut(&identity) else {
            return Vec::new();
        };
        if !constraints.reference.ptr_eq(&Rc::downgrade(reference)) {
            return Vec::new();
        }
        constraints.sources.retain(|source| source.holds(reference));
        constraints.sources.clone()
    }

    pub(super) fn property_reference_source(&self, location: &Location) -> Option<Source> {
        let (owner, holder) = if let Some(object) = &location.object {
            (
                self.instance_property_owner(&object.class, &location.name)?,
                Holder::Instance(
                    Rc::downgrade(object),
                    self.instance_property_key(object, &location.name),
                ),
            )
        } else {
            let owner = location
                .static_class
                .as_ref()?
                .static_owner(&location.name)?;
            let holder = Holder::Static(Rc::downgrade(&owner));
            (owner, holder)
        };
        let properties = if location.object.is_some() {
            &owner.declaration.properties
        } else {
            &owner.declaration.static_properties
        };
        let property = properties
            .iter()
            .find(|p| p.name == location.name && p.ty.is_some())?
            .clone();
        Some(Source {
            holder,
            owner,
            property,
        })
    }

    pub(super) fn register_reference_source(&mut self, reference: &Cell, source: Source) {
        let identity = Rc::as_ptr(reference) as usize;
        let constraints = self
            .reference_constraints
            .entry(identity)
            .or_insert_with(|| Constraints {
                reference: Rc::downgrade(reference),
                sources: Vec::new(),
            });
        constraints.sources.retain(|s| s.holds(reference));
        let duplicate = constraints
            .sources
            .iter()
            .any(|s| match (&s.holder, &source.holder) {
                (Holder::Instance(a, ak), Holder::Instance(b, bk)) => a.ptr_eq(b) && ak == bk,
                (Holder::Static(a), Holder::Static(b)) => {
                    a.ptr_eq(b) && s.property.name == source.property.name
                }
                _ => false,
            });
        if !duplicate {
            constraints.sources.push(source);
        }
    }

    pub(super) fn initialize_reference_property(
        &mut self,
        source: &Source,
        array: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let ty = inheritance::class_type(source.property.ty.as_ref().unwrap(), &source.owner);
        if array {
            let value = Value::Array(Rc::new(PhpArray::default()));
            if ty.accepts(&value) {
                return Ok(value);
            }
            return Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "Cannot auto-initialize an array inside property {}",
                    source.label()
                ),
                line,
            ));
        }
        if ty.accepts(&Value::Null) {
            return Ok(Value::Null);
        }
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Cannot access uninitialized non-nullable property {}::${} by reference",
                source.owner.display_name(),
                String::from_utf8_lossy(&source.property.name)
            ),
            line,
        ))
    }

    pub(super) fn check_reference_binding(
        &mut self,
        reference: &Cell,
        source: &Source,
        line: usize,
    ) -> Result<(), Error> {
        let value = reference.borrow().clone();
        let ty = inheritance::class_type(source.property.ty.as_ref().unwrap(), &source.owner);
        let existing = self.reference_sources(reference);
        let converted = self.check_property_value(
            &source.owner,
            &source.property,
            value.clone(),
            self.strict_types,
            line,
        )?;
        if !ty.accepts(&value)
            && let Some(held) = existing.first()
        {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "Reference with value of type {} held by property {} is not compatible with property {}",
                    builtins::debug_type(&value),
                    held.label(),
                    source.label()
                ),
                line,
            ));
        }
        *reference.borrow_mut() = converted;
        Ok(())
    }

    pub(super) fn check_reference_value(
        &mut self,
        reference: &Cell,
        value: Value,
        strict: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let sources = self.reference_sources(reference);
        let mut result: Option<(Value, &Source)> = None;
        for source in &sources {
            let ty = inheritance::class_type(source.property.ty.as_ref().unwrap(), &source.owner);
            let converted = self
                .coerce_type(value.clone(), &ty, strict, line)?
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "Cannot assign {} to reference held by property {}",
                            builtins::debug_type(&value),
                            source.label()
                        ),
                        line,
                    )
                })?;
            if let Some((previous, held)) = &result {
                let same_nan = matches!((&converted, previous), (Value::Float(a), Value::Float(b)) if a.is_nan() && b.is_nan());
                if !same_nan && !converted.identical(previous, line)? {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "Cannot assign {} to reference held by property {} and property {}, as this would result in an inconsistent type conversion",
                            builtins::debug_type(&value),
                            held.label(),
                            source.label()
                        ),
                        line,
                    ));
                }
            } else {
                result = Some((converted, source));
            }
        }
        Ok(result.map_or(value, |(value, _)| value))
    }

    pub(super) fn write_reference_value(
        &mut self,
        reference: &Cell,
        value: Value,
        strict: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let value = self.check_reference_value(reference, value, strict, line)?;
        *reference.borrow_mut() = value.clone();
        Ok(value)
    }

    pub(super) fn check_reference_array_init(
        &mut self,
        reference: &Cell,
        line: usize,
    ) -> Result<(), Error> {
        for source in self.reference_sources(reference) {
            let ty = inheritance::class_type(source.property.ty.as_ref().unwrap(), &source.owner);
            if !ty.accepts(&Value::Array(Rc::new(PhpArray::default()))) {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!(
                        "Cannot auto-initialize an array inside a reference held by property {}",
                        source.label()
                    ),
                    line,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn inherit_reference_sources(&mut self, object: &Rc<PhpObject>) {
        for property in &object.class.properties {
            let key = ArrayKey::String(property.storage_name());
            let reference = match object.properties.borrow().get(&key) {
                Some(Value::Reference(reference)) if property.ty.is_some() => reference.clone(),
                _ => continue,
            };
            let source = Source {
                holder: Holder::Instance(Rc::downgrade(object), key),
                owner: object.class.property_slot_owner(property),
                property: property.clone(),
            };
            self.register_reference_source(&reference, source);
        }
    }

    pub(super) fn validate_reference_increment(
        &mut self,
        location: &mut Location,
        old: &Value,
        delta: i64,
        line: usize,
    ) -> Result<(), Error> {
        if !matches!(old, Value::Int(n) if (*n == i64::MAX && delta > 0) || (*n == i64::MIN && delta < 0))
        {
            return Ok(());
        }
        let reference = if let Some(reference) = &location.offset_reference {
            reference.clone()
        } else {
            let mut value = if let Some(object) = &location.object {
                object
                    .properties
                    .borrow()
                    .get(&self.instance_property_key(object, &location.name))
                    .cloned()
            } else if let Some(class) = &location.static_class {
                self.static_cell(class, &location.name, line, false)?
                    .map(Value::Reference)
            } else {
                self.location_scope(location)
                    .get(&location.name)
                    .cloned()
                    .map(Value::Reference)
            };
            for key in &mut location.keys {
                let Some(Value::Array(array)) = value.map(|v| v.dereferenced()) else {
                    return Ok(());
                };
                let key = key.array_key(line, false, &mut self.diagnostics)?;
                value = array.get(&key).cloned();
            }
            let Some(Value::Reference(reference)) = value else {
                return Ok(());
            };
            reference
        };
        for source in self.reference_sources(&reference) {
            let ty = source.property.ty.as_ref().unwrap();
            if ty.contains("int") && !ty.contains("float") {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!(
                        "Cannot {} a reference held by property {} past its {} value",
                        if delta > 0 { "increment" } else { "decrement" },
                        source.label(),
                        if delta > 0 { "maximal" } else { "minimal" }
                    ),
                    line,
                ));
            }
        }
        Ok(())
    }
}
