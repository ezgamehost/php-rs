use super::*;
use crate::{
    ast::PropertyDecl,
    object::{Class, ClassContext},
};

impl Runtime {
    pub(super) fn property_default(
        &mut self,
        owner: &Rc<Class>,
        property: &PropertyDecl,
    ) -> Result<Option<Value>, Error> {
        if property.default.is_none() && property.ty.is_some() {
            return Ok(None);
        }
        let outer = self.class_context.replace(ClassContext {
            scope: owner.clone(),
            called: owner.clone(),
        });
        let result = match &property.default {
            Some(expr) => self
                .eval_default(expr)
                .and_then(|v| self.check_property_value(owner, property, v, true, expr.line)),
            None => Ok(Value::Null),
        };
        self.class_context = outer;
        result.map(Some)
    }

    pub(super) fn check_property_value(
        &mut self,
        owner: &Class,
        property: &PropertyDecl,
        value: Value,
        strict: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let Some(ty) = &property.ty else {
            return Ok(value);
        };
        let ty = inheritance::class_type(ty, owner);
        let given = builtins::debug_type(&value).to_owned();
        self.coerce_type(value, &ty, strict, line)?.ok_or_else(|| {
            Error::new(
                ErrorKind::TypeError,
                format!(
                    "Cannot assign {given} to property {}::${} of type {ty}",
                    String::from_utf8_lossy(&owner.name),
                    String::from_utf8_lossy(&property.name)
                ),
                line,
            )
        })
    }

    pub(super) fn uninitialized_property(owner: &Class, name: &[u8], line: usize) -> Error {
        Error::new(
            ErrorKind::Error,
            format!(
                "Typed property {}::${} must not be accessed before initialization",
                String::from_utf8_lossy(&owner.name),
                String::from_utf8_lossy(name)
            ),
            line,
        )
    }

    pub(super) fn validate_property_increment(
        &self,
        location: &Location,
        old: &Value,
        delta: i64,
        line: usize,
    ) -> Result<(), Error> {
        if !location.keys.is_empty()
            || !matches!(old, Value::Int(n) if (*n == i64::MAX && delta > 0) || (*n == i64::MIN && delta < 0))
        {
            return Ok(());
        }
        let owner = if let Some(object) = &location.object {
            object.class.property_owner(&location.name)
        } else {
            location
                .static_class
                .as_ref()
                .and_then(|c| c.static_owner(&location.name))
        };
        let Some(owner) = owner else {
            return Ok(());
        };
        let properties = if location.static_class.is_some() {
            &owner.declaration.static_properties
        } else {
            &owner.declaration.properties
        };
        let Some(ty) = properties
            .iter()
            .find(|p| p.name == location.name)
            .and_then(|p| p.ty.as_ref())
        else {
            return Ok(());
        };
        if ty.contains("float") || ty.contains("mixed") {
            return Ok(());
        }
        Err(Error::new(
            ErrorKind::TypeError,
            format!(
                "Cannot {} property {}::${} of type {} past its {} value",
                if delta > 0 { "increment" } else { "decrement" },
                String::from_utf8_lossy(&owner.name),
                String::from_utf8_lossy(&location.name),
                type_label(ty, &owner),
                if delta > 0 { "maximal" } else { "minimal" }
            ),
            line,
        ))
    }

    pub(super) fn write_static_location(
        &mut self,
        location: &mut Location,
        new_value: Value,
        line: usize,
    ) -> Result<Value, Error> {
        let class = location.static_class.as_ref().expect("static location");
        // Resolve the declaration before mutating storage. An absent value in a
        // declared typed slot represents uninitialized storage, not null.
        let stored = self.static_cell(class, &location.name, line, false)?;
        self.check_static_set_access(location, !location.keys.is_empty(), line)?;
        let owner = class
            .static_owner(&location.name)
            .expect("declared static property");
        let property = owner
            .declaration
            .static_properties
            .iter()
            .find(|p| p.name == location.name)
            .expect("property owner");
        let mut value = stored.as_ref().map_or(Value::Null, |c| c.borrow().clone());
        let result = write_path(
            &mut value,
            &mut location.keys,
            new_value,
            line,
            &mut self.warnings,
        )?;
        let value = self.check_property_value(&owner, property, value, self.strict_types, line)?;
        let result = if location.keys.is_empty() {
            value.clone()
        } else {
            result
        };
        if let Some(stored) = stored {
            *stored.borrow_mut() = value;
        } else {
            owner
                .static_values
                .borrow_mut()
                .insert(location.name.clone(), cell(value));
        }
        Ok(result)
    }
}

// PHP displays class names before builtin union members, expanding iterable.
pub(super) fn type_label(ty: &crate::ast::TypeDecl, owner: &Class) -> String {
    let mut ty = inheritance::class_type(ty, owner);
    ty.members = ty
        .members
        .into_iter()
        .flat_map(|m| {
            if m == "iterable" {
                vec!["Traversable".to_owned(), "array".to_owned()]
            } else {
                vec![m]
            }
        })
        .collect();
    ty.members.sort_by_key(|m| match m.as_str() {
        "object" => 1,
        "callable" => 2,
        "array" => 3,
        "string" => 4,
        "int" => 5,
        "float" => 6,
        "bool" | "true" | "false" => 7,
        "null" => 8,
        _ => 0,
    });
    ty.to_string()
}
