use super::*;
use crate::{PhpObject, ast::Visibility, object::Class};

impl Runtime {
    pub(super) fn instance_property_owner(
        &self,
        class: &Rc<Class>,
        name: &[u8],
    ) -> Option<Rc<Class>> {
        if let Some(context) = &self.class_context
            && class.is_a(&context.scope.name)
            && context
                .scope
                .declaration
                .properties
                .iter()
                .any(|p| p.name == name && p.visibility == Visibility::Private)
        {
            return Some(context.scope.clone());
        }
        class.property_owner(name)
    }

    pub(super) fn can_access_instance_visibility(
        &self,
        owner: &Rc<Class>,
        name: &[u8],
        visibility: Visibility,
    ) -> bool {
        let mut prototype = owner.clone();
        if visibility == Visibility::Protected {
            while let Some(parent) = &prototype.parent {
                let Some(ancestor) = parent.property_owner(name) else {
                    break;
                };
                if ancestor
                    .declaration
                    .properties
                    .iter()
                    .any(|p| p.name == name && p.visibility == Visibility::Private)
                {
                    break;
                }
                prototype = ancestor;
            }
        }
        self.can_access_static_visibility(&prototype, visibility)
    }

    pub(super) fn instance_property_key(&self, object: &PhpObject, name: &[u8]) -> ArrayKey {
        let key = self
            .instance_property_owner(&object.class, name)
            .map_or_else(
                || name.to_vec(),
                |owner| {
                    owner
                        .declaration
                        .properties
                        .iter()
                        .find(|p| p.name == name)
                        .expect("property owner")
                        .storage_name_for(&owner.name)
                },
            );
        ArrayKey::String(key)
    }

    pub(super) fn check_instance_property_access(
        &self,
        object: &PhpObject,
        name: &[u8],
        silent: bool,
        line: usize,
    ) -> Result<bool, Error> {
        let Some(owner) = self.instance_property_owner(&object.class, name) else {
            return Ok(true);
        };
        let property = owner
            .declaration
            .properties
            .iter()
            .find(|p| p.name == name)
            .expect("property owner");
        if self.can_access_instance_visibility(&owner, name, property.visibility) {
            return Ok(true);
        }
        if silent {
            return Ok(false);
        }
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Cannot access {} property {}::${}",
                if property.visibility == Visibility::Private {
                    "private"
                } else {
                    "protected"
                },
                object.class_name,
                String::from_utf8_lossy(name)
            ),
            line,
        ))
    }

    pub(super) fn check_instance_set_access(
        &self,
        location: &Location,
        unset: bool,
        indirect: bool,
        line: usize,
    ) -> Result<(), Error> {
        let Some(object) = &location.object else {
            return Ok(());
        };
        if location.name.first() == Some(&0) {
            return Err(Error::new(
                ErrorKind::Error,
                r#"Cannot access property starting with "\0""#,
                line,
            ));
        }
        self.check_instance_property_access(object, &location.name, false, line)?;
        let Some(owner) = self.instance_property_owner(&object.class, &location.name) else {
            return Ok(());
        };
        let property = owner
            .declaration
            .properties
            .iter()
            .find(|p| p.name == location.name)
            .expect("property owner");
        if property.is_readonly
            || self.can_access_instance_visibility(&owner, &location.name, property.set_visibility)
        {
            return Ok(());
        }
        let scope = self.class_context.as_ref().map_or_else(
            || "global scope".to_owned(),
            |c| format!("scope {}", String::from_utf8_lossy(&c.scope.name)),
        );
        let operation = if unset && !indirect {
            "unset"
        } else if indirect {
            "indirectly modify"
        } else {
            "modify"
        };
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Cannot {operation} {}(set) property {}::${} from {scope}",
                if property.set_visibility == Visibility::Private {
                    "private"
                } else {
                    "protected"
                },
                String::from_utf8_lossy(&owner.name),
                String::from_utf8_lossy(&location.name)
            ),
            line,
        ))
    }

    pub(super) fn property_key_visible(&self, object: &PhpObject, key: &ArrayKey) -> bool {
        let ArrayKey::String(raw) = key else {
            return true;
        };
        let declared = object
            .class
            .properties
            .iter()
            .find(|p| p.storage_name() == *raw);
        // Mangled dynamic properties from array casts do not carry declarations.
        if declared.is_none() && raw.first() == Some(&0) {
            return true;
        }
        let name = declared.map_or(raw.as_slice(), |p| p.name.as_slice());
        self.check_instance_property_access(object, name, true, 0)
            .unwrap_or(false)
            && self.instance_property_key(object, name) == *key
    }

    pub(super) fn object_variables(&self, object: &PhpObject) -> PhpArray {
        let mut result = PhpArray::default();
        for (key, value) in object.export_properties().entries() {
            if !self.property_key_visible(object, key) {
                continue;
            }
            let key = match key {
                ArrayKey::String(raw) => object
                    .class
                    .properties
                    .iter()
                    .find(|p| p.storage_name() == *raw)
                    .map_or_else(
                        || {
                            ArrayKey::from_value(
                                &Value::String(raw.clone().into()),
                                0,
                                &mut Vec::new(),
                            )
                            .expect("string key")
                        },
                        |p| ArrayKey::String(p.name.clone()),
                    ),
                _ => key.clone(),
            };
            result.insert(key, value.clone());
        }
        result
    }
}
