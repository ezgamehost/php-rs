use super::*;
use crate::object::{Class, ClassContext};

impl Runtime {
    fn member_class_name(&mut self, class: &Expr, line: usize) -> Result<Vec<u8>, Error> {
        if let ExprKind::Constant(name) = &class.kind {
            return self.contextual_class_name(name, line);
        }
        match self.eval(class)? {
            Value::String(name) => Ok(name),
            Value::Object(object) => Ok(object.class.name.clone()),
            Value::Closure(_) => Ok(b"Closure".to_vec()),
            _ => Err(Error::new(
                ErrorKind::Error,
                "Class name must be a valid object or a string",
                line,
            )),
        }
    }

    fn require_member_class(&self, name: &[u8], line: usize) -> Result<Rc<Class>, Error> {
        self.find_class(name).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!("Class \"{}\" not found", String::from_utf8_lossy(name)),
                line,
            )
        })
    }

    pub(super) fn class_constant(
        &mut self,
        class: &Expr,
        name: &Expr,
        line: usize,
    ) -> Result<Value, Error> {
        let class_name = self.member_class_name(class, line)?;
        let name = self.eval(name)?;
        let Value::String(name) = name else {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "Cannot use value of type {} as class constant name",
                    builtins::debug_type(&name)
                ),
                line,
            ));
        };
        let class = self.require_member_class(&class_name, line)?;
        if !self.in_constant_expression && name.eq_ignore_ascii_case(b"class") {
            return Ok(Value::String(class.name.clone()));
        }
        self.read_class_constant(&class, &name, line)
    }

    pub(super) fn named_class_constant(
        &mut self,
        qualified: &[u8],
        defined: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let split = qualified
            .windows(2)
            .position(|p| p == b"::")
            .expect("qualified constant");
        let name = &qualified[split + 2..];
        let class_name = self.contextual_class_name(&qualified[..split], line)?;
        let Some(class) = self.find_class(&class_name) else {
            return if defined {
                Ok(Value::Bool(false))
            } else {
                self.require_member_class(&class_name, line)
                    .map(|_| Value::Null)
            };
        };
        if defined && class.constant_owner(name).is_none() {
            return Ok(Value::Bool(false));
        }
        let value = self.read_class_constant(&class, name, line)?;
        Ok(if defined { Value::Bool(true) } else { value })
    }

    fn read_class_constant(
        &mut self,
        class: &Rc<Class>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let owner = class.constant_owner(name).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!(
                    "Undefined constant {}::{}",
                    String::from_utf8_lossy(&class.name),
                    String::from_utf8_lossy(name)
                ),
                line,
            )
        })?;
        if let Some(value) = owner.constant_values.borrow().get(name) {
            return Ok(value.clone());
        }
        if !owner
            .evaluating_constants
            .borrow_mut()
            .insert(name.to_vec())
        {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot declare self-referencing constant {}::{}",
                    String::from_utf8_lossy(&owner.name),
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        let constant = owner
            .declaration
            .constants
            .iter()
            .find(|c| c.name == name)
            .expect("constant owner");
        let outer = self.class_context.replace(ClassContext {
            scope: owner.clone(),
            called: owner.clone(),
        });
        let result = self.eval_default(&constant.value).and_then(|value| {
            let Some(ty) = &constant.ty else {
                return Ok(value);
            };
            let given = builtins::debug_type(&value).to_owned();
            self.coerce_type(value, ty, true, line).ok_or_else(|| {
                Error::new(
                    ErrorKind::TypeError,
                    format!(
                        "Cannot assign {given} to class constant {}::{} of type {ty}",
                        String::from_utf8_lossy(&owner.name),
                        String::from_utf8_lossy(name)
                    ),
                    line,
                )
            })
        });
        self.class_context = outer;
        owner.evaluating_constants.borrow_mut().remove(name);
        let value = result?;
        owner
            .constant_values
            .borrow_mut()
            .insert(name.to_vec(), value.clone());
        Ok(value)
    }

    /// Object construction and static storage access resolve class defaults.
    /// Reading an individual constant only resolves that constant's dependencies.
    pub(super) fn initialize_class(&mut self, class: &Rc<Class>) -> Result<(), Error> {
        if class.initialized.get() || class.initializing.get() {
            return Ok(());
        }
        class.initializing.set(true);
        let result = self.initialize_class_values(class);
        class.initializing.set(false);
        if result.is_ok() {
            class.initialized.set(true);
        }
        result
    }

    fn initialize_class_values(&mut self, class: &Rc<Class>) -> Result<(), Error> {
        for name in class.constant_names() {
            self.read_class_constant(class, &name, class.declaration.line)?;
        }
        self.initialize_property_values(class)
    }

    fn initialize_property_values(&mut self, class: &Rc<Class>) -> Result<(), Error> {
        if let Some(parent) = &class.parent {
            self.initialize_property_values(parent)?;
        }
        self.property_defaults(class)?;
        for property in &class.static_properties {
            let owner = class
                .static_owner(&property.name)
                .expect("static property owner");
            if owner.static_values.borrow().contains_key(&property.name) {
                continue;
            }
            if let Some(value) = self.property_default(&owner, property)? {
                owner
                    .static_values
                    .borrow_mut()
                    .insert(property.name.clone(), cell(value));
            }
        }
        Ok(())
    }

    pub(super) fn static_location(
        &mut self,
        class: &Expr,
        name: &Expr,
        line: usize,
    ) -> Result<Location, Error> {
        let class_name = self.member_class_name(class, line)?;
        let name = self.variable_name(name)?;
        let class = self.require_member_class(&class_name, line)?;
        Ok(Location {
            object: None,
            static_class: Some(class),
            name,
            global: false,
            keys: Vec::new(),
        })
    }

    pub(super) fn static_cell(
        &mut self,
        class: &Rc<Class>,
        name: &[u8],
        line: usize,
        silent: bool,
    ) -> Result<Option<variables::Cell>, Error> {
        if let Some(owner) = class.static_owner(name) {
            self.initialize_class(class)?;
            return Ok(owner.static_values.borrow().get(name).cloned());
        }
        if silent {
            Ok(None)
        } else {
            Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Access to undeclared static property {}::${}",
                    String::from_utf8_lossy(&class.name),
                    String::from_utf8_lossy(name)
                ),
                line,
            ))
        }
    }
}
