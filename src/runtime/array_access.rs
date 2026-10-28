use super::*;
use crate::{PhpObject, ast::Argument};

impl Runtime {
    pub(super) fn object_offset_root(&self, location: &Location) -> bool {
        let value = if let Some(object) = &location.object {
            object
                .properties
                .borrow()
                .get(&self.instance_property_key(object, &location.name))
                .cloned()
        } else if let Some(class) = &location.static_class {
            class.static_owner(&location.name).and_then(|owner| {
                owner
                    .static_values
                    .borrow()
                    .get(&location.name)
                    .map(|v| v.borrow().clone())
            })
        } else {
            None
        };
        value.is_some_and(|v| matches!(v.dereferenced(), Value::Object(_)))
    }

    pub(super) fn read_location_offsets(
        &mut self,
        mut value: Value,
        location: &mut Location,
        line: usize,
        silent: bool,
        modify: Option<ReadModify>,
    ) -> Result<Value, Error> {
        for index in 0..location.keys.len() {
            if matches!(value.dereferenced(), Value::String(_))
                && let Some(modify) = modify
            {
                return Err(Error::new(ErrorKind::Error, modify.string_error(), line));
            }
            if modify == Some(ReadModify::Increment)
                && matches!(value.dereferenced(), Value::Object(_))
            {
                let reference = reference_paths::acquire(
                    variables::cell(value),
                    &mut location.keys[index..],
                    line,
                    self,
                )?;
                let old = reference.borrow().dereferenced();
                location.offset_reference = Some(reference);
                return Ok(old);
            }
            value = self.read_offset(
                value,
                &mut location.keys[index],
                line,
                if silent {
                    Access::Coalesce
                } else {
                    Access::Read
                },
            )?;
        }
        Ok(value)
    }

    pub(super) fn check_array_access(&self, object: &PhpObject, line: usize) -> Result<(), Error> {
        if object.class.is_a(b"ArrayAccess") {
            Ok(())
        } else {
            Err(Error::new(
                ErrorKind::Error,
                format!("Cannot use object of type {} as array", object.class_name),
                line,
            ))
        }
    }

    pub(super) fn array_access_call(
        &mut self,
        object: &Rc<PhpObject>,
        method: &[u8],
        values: Vec<Value>,
        line: usize,
    ) -> Result<Value, Error> {
        self.check_array_access(object, line)?;
        let function = objects::method(&object.class, method).expect("ArrayAccess method");
        let target = self.unchecked_method_target(
            function,
            object.class.clone(),
            Some(object.clone()),
            line,
        )?;
        let args: Vec<_> = values
            .into_iter()
            .map(|value| {
                Argument::Positional(Expr {
                    kind: ExprKind::Literal(value),
                    line,
                })
            })
            .collect();
        self.invoke(target, &args, line, false)
    }

    pub(super) fn warn_indirect_array_access(
        &mut self,
        object: &PhpObject,
        value: &Value,
        line: usize,
    ) {
        if !matches!(
            value,
            Value::Reference(_) | Value::Indirect(_) | Value::Object(_) | Value::Closure(_)
        ) {
            self.diagnostics.push(format!("Notice: Indirect modification of overloaded element of {} has no effect on line {line}", object.class_name));
        }
    }

    pub(super) fn read_offset(
        &mut self,
        value: Value,
        offset: &mut Offset,
        line: usize,
        access: Access,
    ) -> Result<Value, Error> {
        let Value::Object(object) = value.dereferenced() else {
            return offsets::read_offset(value, offset, line, access, &mut self.diagnostics);
        };
        if self.in_constant_expression {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot use [] on objects in constant expression",
                line,
            ));
        }
        self.check_array_access(&object, line)?;
        let key = offset.argument();
        if access != Access::Read {
            let exists = self
                .array_access_call(&object, b"offsetExists", vec![key.clone()], line)?
                .is_truthy();
            if !exists {
                return Ok(Value::Null);
            }
            if access == Access::Probe {
                return Ok(Value::Bool(true));
            }
        }
        self.array_access_call(&object, b"offsetGet", vec![key], line)
            .map(|v| v.dereferenced())
    }
}
