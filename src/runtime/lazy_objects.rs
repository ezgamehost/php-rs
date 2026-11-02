// Altered Rust adaptation of PHP 8.5.0 lazy objects. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, object::Class};
use functions::CallTarget;

#[derive(Clone)]
pub(crate) struct LazyInitializer {
    pub(super) callback: Value,
    target: CallTarget,
    pub(super) proxy: bool,
    pub(super) options: i64,
    pub(super) slots: HashSet<Vec<u8>>,
}

pub(crate) enum LazyState {
    Uninitialized(Box<LazyInitializer>),
    Initializing,
    Proxy(Rc<PhpObject>),
}

impl LazyState {
    pub(super) fn values(&self) -> Vec<Value> {
        match self {
            Self::Proxy(instance) => vec![Value::Object(instance.clone())],
            Self::Uninitialized(init) => {
                let mut values = vec![init.callback.clone()];
                values.extend(init.target.this.iter().cloned().map(Value::Object));
                values.extend(init.target.closure.iter().cloned().map(Value::Closure));
                values
            }
            Self::Initializing => Vec::new(),
        }
    }
}

impl PhpObject {
    pub(crate) fn is_lazy_uninitialized(&self) -> bool {
        matches!(*self.lazy.borrow(), Some(LazyState::Uninitialized(_)))
    }

    pub(crate) fn lazy_instance(&self) -> Option<Rc<Self>> {
        match &*self.lazy.borrow() {
            Some(LazyState::Proxy(instance)) => Some(instance.clone()),
            _ => None,
        }
    }

    fn lazy_is_iterating(&self) -> bool {
        self.property_iterators
            .borrow()
            .iter()
            .any(|i| i.strong_count() > 0)
            || self
                .lazy_instance()
                .is_some_and(|instance| instance.lazy_is_iterating())
    }

    pub(crate) fn suppresses_destructor(&self) -> bool {
        matches!(
            *self.lazy.borrow(),
            Some(LazyState::Uninitialized(_) | LazyState::Proxy(_))
        )
    }

    pub(super) fn lazy_slot(&self, key: &ArrayKey) -> bool {
        matches!((&*self.lazy.borrow(), key), (Some(LazyState::Uninitialized(init)), ArrayKey::String(key)) if init.slots.contains(key))
    }

    pub(super) fn lazy_label(&self) -> &'static [u8] {
        match &*self.lazy.borrow() {
            Some(LazyState::Uninitialized(init)) if !init.proxy => b"lazy ghost ",
            Some(LazyState::Uninitialized(_) | LazyState::Proxy(_)) => b"lazy proxy ",
            _ => b"",
        }
    }

    pub(super) fn realize_lazy_slot(&self, key: &[u8]) {
        let finished = if let Some(LazyState::Uninitialized(init)) = &mut *self.lazy.borrow_mut() {
            init.slots.remove(key);
            init.slots.is_empty()
        } else {
            false
        };
        if finished {
            let old = self.lazy.take();
            drop(old);
        }
    }
}

impl Runtime {
    pub(super) fn lazy_checked_object(
        &mut self,
        class: &Class,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        let value = self.lookup(b"object").unwrap();
        if let Value::Object(object) = value
            && object.class.is_a(&class.name)
        {
            return Ok(object);
        }
        Err(Error::new(
            ErrorKind::TypeError,
            format!(
                "{}(): Argument #1 ($object) must be of type {}, {} given",
                self.current_function_label(),
                class.display_name(),
                builtins::debug_type(&self.lookup(b"object").unwrap())
            ),
            line,
        ))
    }

    fn current_function_label(&self) -> String {
        let name = self.current_function.as_deref().unwrap_or(b"");
        let short = name.rsplit(|b| *b == b':').next().unwrap();
        let canonical = match short {
            b"newlazyghost" => "newLazyGhost",
            b"newlazyproxy" => "newLazyProxy",
            b"resetaslazyghost" => "resetAsLazyGhost",
            b"resetaslazyproxy" => "resetAsLazyProxy",
            b"initializelazyobject" => "initializeLazyObject",
            b"isuninitializedlazyobject" => "isUninitializedLazyObject",
            b"marklazyobjectasinitialized" => "markLazyObjectAsInitialized",
            b"getlazyinitializer" => "getLazyInitializer",
            b"islazy" => "isLazy",
            b"skiplazyinitialization" => "skipLazyInitialization",
            b"setrawvaluewithoutlazyinitialization" => "setRawValueWithoutLazyInitialization",
            _ => "",
        };
        format!(
            "{}::{canonical}",
            if name.starts_with(b"reflectionproperty") {
                "ReflectionProperty"
            } else {
                "ReflectionClass"
            }
        )
    }

    pub(super) fn native_lazy_factory(
        &mut self,
        target: CallTarget,
        line: usize,
    ) -> Result<Value, Error> {
        let Some(Value::Object(reflector)) = self.lookup(b"this") else {
            unreachable!()
        };
        let class = match reflector.reflection.borrow().as_ref().unwrap() {
            reflection::ReflectionData::Class(class) => class.clone(),
            reflection::ReflectionData::Object(object) => object.class.clone(),
            _ => unreachable!(),
        };
        let name = self.current_function.as_ref().unwrap();
        let reset = name.windows(7).any(|n| n == b"resetas");
        let proxy = name.ends_with(b"proxy");
        let callback = self
            .lookup(if proxy { b"factory" } else { b"initializer" })
            .unwrap();
        let Value::Int(options) = self.lookup(b"options").unwrap() else {
            unreachable!()
        };
        let object = if reset {
            Some(self.lazy_checked_object(&class, line)?)
        } else {
            None
        };
        if options & !24 != 0 || (!reset && options & 16 != 0) {
            let reason = if options & !24 != 0 {
                "contains invalid flags"
            } else {
                "does not accept ReflectionClass::SKIP_DESTRUCTOR"
            };
            return Err(self.reflection_error(
                format!(
                    "{}(): Argument #{} ($options) {reason}",
                    self.current_function_label(),
                    if reset { 3 } else { 2 }
                ),
                line,
            ));
        }
        if object.as_ref().is_some_and(|o| o.is_lazy_uninitialized()) {
            return Err(self.reflection_error("Object is already lazy", line));
        }
        let mut ancestor = Some(class.clone());
        while let Some(current) = ancestor {
            if current.name != b"stdClass"
                && self
                    .builtin_classes
                    .borrow()
                    .values()
                    .any(|c| Rc::ptr_eq(c, &current))
            {
                let reason = if Rc::ptr_eq(&class, &current) {
                    format!("{} is internal", class.display_name())
                } else {
                    format!(
                        "{} inherits internal class {}",
                        class.display_name(),
                        current.display_name()
                    )
                };
                return Err(Error::new(
                    ErrorKind::Error,
                    format!("Cannot make instance of internal class lazy: {reason}"),
                    line,
                ));
            }
            ancestor = current.parent.clone();
        }
        let object = if let Some(object) = object {
            if matches!(*object.lazy.borrow(), Some(LazyState::Initializing)) {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Can not reset an object while it is being initialized",
                    line,
                ));
            }
            if object.lazy_is_iterating() {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Can not reset an object during property iteration",
                    line,
                ));
            }
            if object.lazy_instance().is_none()
                && options & 16 == 0
                && !object._handle.destructor_called.get()
            {
                self.invoke_destructor(&object, line)?;
            }
            let old = object.lazy.take();
            drop(old);
            self.drain_destructors(Ok(()), line)?;
            object._handle.destructor_called.set(false);
            object
        } else {
            let Value::Object(object) = self.instantiate_class(class.clone(), &[], false, line)?
            else {
                unreachable!()
            };
            object
        };
        let mut slots = HashSet::new();
        for property in &class.properties {
            let key = property.storage_name();
            if reset
                && property.is_readonly
                && (object.class.declaration.is_final || property.is_final)
                && object
                    .properties
                    .borrow()
                    .get(&ArrayKey::String(key.clone()))
                    .is_some()
                && !object.reinitializable.borrow().contains(&key)
            {
                continue;
            }
            object
                .properties
                .borrow_mut()
                .remove(&ArrayKey::String(key.clone()));
            object.unset_properties.borrow_mut().remove(&key);
            object.reinitializable.borrow_mut().remove(&key);
            slots.insert(key);
        }
        let declared: HashSet<_> = object
            .class
            .properties
            .iter()
            .map(|p| ArrayKey::String(p.storage_name()))
            .collect();
        let dynamic: Vec<_> = object
            .properties
            .borrow()
            .entries()
            .iter()
            .filter(|(k, _)| !declared.contains(k))
            .map(|(k, _)| k.clone())
            .collect();
        for key in dynamic {
            object.remove_property(&key);
        }
        if !slots.is_empty() {
            *object.lazy.borrow_mut() = Some(LazyState::Uninitialized(Box::new(LazyInitializer {
                callback,
                target,
                proxy,
                options,
                slots,
            })));
        }
        Ok(if reset {
            Value::Null
        } else {
            Value::Object(object)
        })
    }

    fn populate_lazy_defaults(
        &mut self,
        object: &PhpObject,
        slots: &HashSet<Vec<u8>>,
    ) -> Result<(), Error> {
        for property in &object.class.properties {
            let key = property.storage_name();
            if slots.contains(&key) {
                let owner = object.class.property_slot_owner(property);
                if let Some(value) = self.property_default(&owner, property)? {
                    object
                        .properties
                        .borrow_mut()
                        .insert(ArrayKey::String(key), value);
                }
            }
        }
        Ok(())
    }

    pub(super) fn mark_lazy_initialized(
        &mut self,
        object: &Rc<PhpObject>,
    ) -> Result<Rc<PhpObject>, Error> {
        let slots = match &*object.lazy.borrow() {
            Some(LazyState::Uninitialized(init)) => Some(init.slots.clone()),
            _ => None,
        };
        if let Some(slots) = slots {
            self.populate_lazy_defaults(object, &slots)?;
            let old = object.lazy.take();
            drop(old);
        }
        Ok(object.lazy_instance().unwrap_or_else(|| object.clone()))
    }

    pub(super) fn initialize_lazy_object(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        if let Some(instance) = object.lazy_instance() {
            return self.initialize_lazy_object(&instance, line);
        }
        let init = match &*object.lazy.borrow() {
            Some(LazyState::Uninitialized(init)) => init.clone(),
            _ => return Ok(object.clone()),
        };
        let snapshot = object.properties.borrow().clone();
        let unset = object.unset_properties.borrow().clone();
        let reinitializable = object.reinitializable.borrow().clone();
        let reordered = object.properties_reordered.get();
        *object.lazy.borrow_mut() = Some(LazyState::Initializing);
        let result = (|| {
            if !init.proxy {
                self.populate_lazy_defaults(object, &init.slots)?;
            }
            let result = self
                .call_callback_at(&init.target, &[Value::Object(object.clone())], line, false)?
                .dereferenced();
            if !init.proxy {
                if !matches!(result, Value::Null) {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        "Lazy object initializer must return NULL or no value",
                        line,
                    ));
                }
                return Ok(object.clone());
            }
            let Value::Object(instance) = result else {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!(
                        "Lazy proxy factory must return an instance of a class compatible with {}, {} returned",
                        object.class_name,
                        builtins::debug_type(&result)
                    ),
                    line,
                ));
            };
            let same_method = |name: &[u8]| match (
                objects::method(&object.class, name),
                objects::method(&instance.class, name),
            ) {
                (None, None) => true,
                (Some(a), Some(b)) => Rc::ptr_eq(&a, &b),
                _ => false,
            };
            if !Rc::ptr_eq(&object.class, &instance.class)
                && !(object.class.is_a(&instance.class.name)
                    && object.class.properties.len() + object.class.static_properties.len()
                        == instance.class.properties.len() + instance.class.static_properties.len()
                    && same_method(b"__destruct")
                    && same_method(b"__clone"))
            {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!(
                        "The real instance class {} is not compatible with the proxy class {}. The proxy must be a instance of the same class as the real instance, or a sub-class with no additional properties, and no overrides of the __destructor or __clone methods.",
                        instance.class_name, object.class_name
                    ),
                    line,
                ));
            }
            if Rc::ptr_eq(object, &instance) || instance.lazy.borrow().is_some() {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Lazy proxy factory must return a non-lazy object",
                    line,
                ));
            }
            Ok(instance)
        })();
        match result {
            Ok(instance) => {
                if init.proxy {
                    *object.lazy.borrow_mut() = Some(LazyState::Proxy(instance.clone()));
                    let old = object.properties.replace(PhpArray::default());
                    drop(old);
                } else {
                    object.lazy.take();
                }
                Ok(instance)
            }
            Err(error) => {
                if !init.proxy {
                    let old = object.properties.replace(snapshot);
                    *object.unset_properties.borrow_mut() = unset;
                    *object.reinitializable.borrow_mut() = reinitializable;
                    object.properties_reordered.set(reordered);
                    drop(old);
                    self.inherit_reference_sources(object);
                }
                *object.lazy.borrow_mut() = Some(LazyState::Uninitialized(init));
                Err(error)
            }
        }
    }

    pub(super) fn lazy_property_target(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        if let Some(instance) = object.lazy_instance() {
            return self.lazy_property_target(&instance, name, line);
        }
        if object.is_lazy_uninitialized()
            && (object.lazy_slot(&self.instance_property_key(object, name))
                || self.instance_property_owner(&object.class, name).is_none())
        {
            self.initialize_lazy_object(object, line)
        } else {
            Ok(object.clone())
        }
    }

    pub(super) fn initialize_lazy_location(
        &mut self,
        location: &mut Location,
        unset: bool,
        indirect: bool,
        line: usize,
    ) -> Result<(), Error> {
        if let Some(object) = &location.object {
            // A denied write must not execute user initialization code. Readonly
            // value checks still happen afterwards, once defaults/constructor
            // assignments are visible.
            if object.lazy.borrow().is_some() {
                self.check_instance_set_access(location, unset, indirect, line)?;
                if let Some(owner) = self.instance_property_owner(&object.class, &location.name)
                    && let Some(property) = owner
                        .declaration
                        .properties
                        .iter()
                        .find(|p| p.name == location.name)
                    && property.is_readonly
                    && !self.can_access_instance_visibility(
                        &owner,
                        &location.name,
                        property.set_visibility,
                    )
                {
                    self.check_readonly_access(location, unset, false, line)?;
                }
            }
            location.object = Some(self.lazy_property_target(object, &location.name, line)?);
        }
        Ok(())
    }
}
