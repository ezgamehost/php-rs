use super::*;
use crate::PhpObject;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Magic {
    Get,
    Set,
    Isset,
    Unset,
}

impl Magic {
    fn name(self) -> &'static [u8] {
        match self {
            Self::Get => b"__get",
            Self::Set => b"__set",
            Self::Isset => b"__isset",
            Self::Unset => b"__unset",
        }
    }
}

impl Runtime {
    fn property_backed(&self, object: &PhpObject, name: &[u8]) -> bool {
        if name.first() == Some(&0) {
            return false;
        }
        if !self
            .check_instance_property_access(object, name, true, 0)
            .unwrap_or(false)
        {
            return false;
        }
        if let Some(owner) = self.instance_static_owner(&object.class, name)
            && !self
                .check_static_property_access(&object.class, &owner, name, 0, true)
                .unwrap_or(false)
        {
            return false;
        }
        let key = self.instance_property_key(object, name);
        if object.properties.borrow().get(&key).is_some() {
            return true;
        }
        let ArrayKey::String(key) = key else {
            unreachable!()
        };
        !object.unset_properties.borrow().contains(&key)
            && self
                .instance_property_owner(&object.class, name)
                .is_some_and(|owner| {
                    owner
                        .declaration
                        .properties
                        .iter()
                        .any(|p| p.name == name && p.ty.is_some())
                })
    }

    pub(super) fn magic_get_applicable(&self, object: &PhpObject, name: &[u8]) -> bool {
        !self.property_backed(object, name)
            && objects::method(&object.class, Magic::Get.name()).is_some()
            && !self
                .property_guards
                .contains(&(object.object_id, name.to_vec(), Magic::Get))
    }

    pub(super) fn magic_set_applicable(&self, object: &PhpObject, name: &[u8]) -> bool {
        !self.property_backed(object, name)
            && objects::method(&object.class, Magic::Set.name()).is_some()
            && !self
                .property_guards
                .contains(&(object.object_id, name.to_vec(), Magic::Set))
    }

    fn call_property_magic(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        magic: Magic,
        value: Option<Value>,
        line: usize,
    ) -> Result<Option<Value>, Error> {
        let Some(method) = objects::method(&object.class, magic.name()) else {
            return Ok(None);
        };
        let guard = (object.object_id, name.to_vec(), magic);
        if !self.property_guards.insert(guard.clone()) {
            return Ok(None);
        }
        let mut arguments = vec![Value::String(name.to_vec().into())];
        arguments.extend(value);
        let result = self
            .unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)
            .and_then(|target| {
                let arguments: Vec<_> = arguments
                    .into_iter()
                    .map(|value| {
                        crate::ast::Argument::Positional(Expr {
                            kind: ExprKind::Literal(value),
                            line,
                        })
                    })
                    .collect();
                // Magic calls coerce arguments weakly, but their trace frame
                // points at the property operation in the calling PHP file.
                let strict = std::mem::replace(&mut self.strict_types, false);
                let result = self.invoke(target, &arguments, line, false);
                self.strict_types = strict;
                result
            });
        self.property_guards.remove(&guard);
        result.map(Some)
    }

    fn magic_get(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
        check_type: bool,
    ) -> Result<Option<Value>, Error> {
        let Some(value) = self.call_property_magic(object, name, Magic::Get, None, line)? else {
            return Ok(None);
        };
        if check_type
            && self.check_instance_property_access(object, name, true, line)?
            && let Some(owner) = self.instance_property_owner(&object.class, name)
            && let Some(property) = owner
                .declaration
                .properties
                .iter()
                .find(|p| p.name == name && p.ty.is_some())
        {
            let getter = objects::method(&object.class, Magic::Get.name()).expect("getter");
            let reference = if let Value::Reference(reference) = &value {
                Some(reference.clone())
            } else {
                None
            };
            let value = value.dereferenced();
            let given = builtins::debug_type(&value).to_owned();
            let value = self.check_property_value(&owner, property, value, getter.strict_types, line).map_err(|error| {
                if error.throwable.is_some() || error.kind != ErrorKind::TypeError { return error; }
                Error::new(ErrorKind::TypeError, format!("Value of type {given} returned from {}() must be compatible with unset property {}::${} of type {}", String::from_utf8_lossy(&getter.name), String::from_utf8_lossy(&owner.name), String::from_utf8_lossy(name), typed_properties::type_label(property.ty.as_ref().unwrap(), &owner)), line)
            })?;
            let value = if let Some(reference) = reference {
                *reference.borrow_mut() = value;
                Value::Reference(reference)
            } else {
                value
            };
            return Ok(Some(value));
        }
        Ok(Some(value))
    }

    pub(super) fn read_property_value(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
        access: Access,
    ) -> Result<Value, Error> {
        if self.property_backed(object, name) {
            return self.read_property_storage(object, name, line, access != Access::Read);
        }
        if access != Access::Read {
            let has_isset = objects::method(&object.class, Magic::Isset.name()).is_some();
            if let Some(value) = self.call_property_magic(object, name, Magic::Isset, None, line)? {
                if !value.to_bool(line, &mut self.diagnostics) {
                    return Ok(Value::Null);
                }
                if access == Access::Probe {
                    return Ok(Value::Bool(true));
                }
                if access == Access::Empty {
                    let guard = (object.object_id, name.to_vec(), Magic::Isset);
                    self.property_guards.insert(guard.clone());
                    let result = self.magic_get(object, name, line, false);
                    self.property_guards.remove(&guard);
                    return result.map(|v| v.unwrap_or(Value::Null));
                }
            } else if matches!(access, Access::Probe | Access::Empty)
                || (has_isset && !self.magic_get_applicable(object, name))
            {
                return Ok(Value::Null);
            }
        }
        if let Some(value) = self.magic_get(object, name, line, true)? {
            return Ok(value);
        }
        if self
            .property_guards
            .contains(&(object.object_id, name.to_vec(), Magic::Get))
        {
            // A recursively blocked getter cannot hide an inaccessible slot,
            // even when the outer read is a quiet coalescing operation.
            self.check_instance_property_access(object, name, false, line)?;
            if let Some(owner) = self.instance_static_owner(&object.class, name) {
                self.check_static_property_access(&object.class, &owner, name, line, false)?;
            }
            if name.first() == Some(&0) {
                return self.read_property_storage(object, name, line, false);
            }
        }
        self.read_property_storage(object, name, line, access != Access::Read)
    }

    pub(super) fn magic_property_write(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        value: &Value,
        line: usize,
    ) -> Result<bool, Error> {
        if self.property_backed(object, name) {
            return Ok(false);
        }
        Ok(self
            .call_property_magic(object, name, Magic::Set, Some(value.clone()), line)?
            .is_some())
    }

    pub(super) fn magic_property_unset(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<bool, Error> {
        if self.property_backed(object, name) {
            return Ok(false);
        }
        Ok(self
            .call_property_magic(object, name, Magic::Unset, None, line)?
            .is_some())
    }

    pub(super) fn warn_indirect_magic(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        value: &Value,
        line: usize,
    ) {
        if !matches!(
            value,
            Value::Object(_) | Value::Closure(_) | Value::Reference(_)
        ) {
            self.diagnostics.push(format!("Notice: Indirect modification of overloaded property {}::${} has no effect on line {line}", object.class_name, String::from_utf8_lossy(name)));
        }
    }
}
