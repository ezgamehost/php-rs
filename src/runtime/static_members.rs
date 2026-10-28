use super::*;
use crate::ast::Visibility;
use crate::object::{Class, ClassContext};

impl Runtime {
    fn member_class_name(&mut self, class: &Expr, line: usize) -> Result<Vec<u8>, Error> {
        if let Some(name) = class.static_class_name() {
            return self.contextual_class_name(name, line);
        }
        match self.eval(class)? {
            Value::String(name) => Ok(name.to_vec()),
            Value::Object(object) => Ok(object.class.name.clone()),
            Value::Closure(_) => Ok(b"Closure".to_vec()),
            _ => Err(Error::new(
                ErrorKind::Error,
                "Class name must be a valid object or a string",
                line,
            )),
        }
    }

    fn require_member_class(&mut self, name: &[u8], line: usize) -> Result<Rc<Class>, Error> {
        self.load_class(name, line)?.ok_or_else(|| {
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
        if !self.in_constant_expression && name.bytes().eq_ignore_ascii_case(b"class") {
            return Ok(Value::String(class.name.clone().into()));
        }
        self.read_class_constant(&class, &name.bytes(), line)
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
        let Some(class) = self.load_class(&class_name, line)? else {
            return if defined {
                Ok(Value::Bool(false))
            } else {
                Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Class \"{}\" not found",
                        String::from_utf8_lossy(&class_name)
                    ),
                    line,
                ))
            };
        };
        if defined && class.declaration.is_trait {
            return Ok(Value::Bool(false));
        }
        if defined
            && !class
                .constant_owner(name)
                .is_some_and(|owner| self.can_access_constant(&owner, name))
        {
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
        if class.declaration.is_trait && !self.checking_trait_values {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot access trait constant {}::{} directly",
                    String::from_utf8_lossy(&class.name),
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
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
        if !self.can_access_constant(&owner, name) {
            let constant = owner
                .declaration
                .constants
                .iter()
                .find(|c| c.name == name)
                .unwrap();
            let visibility = if constant.visibility == Visibility::Private {
                "private"
            } else {
                "protected"
            };
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot access {visibility} constant {}::{}",
                    String::from_utf8_lossy(&class.name),
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        self.evaluate_class_constant(&owner, name, line)
    }

    fn can_access_constant(&self, owner: &Class, name: &[u8]) -> bool {
        let constant = owner
            .declaration
            .constants
            .iter()
            .find(|c| c.name == name)
            .expect("constant owner");
        constant.visibility == Visibility::Public
            || self.class_context.as_ref().is_some_and(|context| {
                context.scope.name.eq_ignore_ascii_case(&owner.name)
                    || (constant.visibility == Visibility::Protected
                        && !context.scope.declaration.is_interface
                        && (context.scope.is_a(&owner.name) || owner.is_a(&context.scope.name)))
            })
    }

    pub(super) fn evaluate_class_constant(
        &mut self,
        owner: &Rc<Class>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
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
            self.coerce_type(value, ty, true, line)?.ok_or_else(|| {
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
            let owner = class.constant_owner(&name).expect("class constant");
            self.evaluate_class_constant(&owner, &name, class.declaration.line)?;
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
            overloaded: None,
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
            if !self.check_static_property_access(class, &owner, name, line, silent)? {
                return Ok(None);
            }
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

    pub(super) fn check_static_property_access(
        &self,
        class: &Class,
        owner: &Class,
        name: &[u8],
        line: usize,
        silent: bool,
    ) -> Result<bool, Error> {
        let property = owner
            .declaration
            .static_properties
            .iter()
            .find(|p| p.name == name)
            .expect("static property owner");
        if self.can_access_static_visibility(owner, property.visibility) {
            return Ok(true);
        }
        if silent {
            return Ok(false);
        }
        let visibility = if property.visibility == Visibility::Private {
            "private"
        } else {
            "protected"
        };
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Cannot access {visibility} property {}::${}",
                String::from_utf8_lossy(&class.name),
                String::from_utf8_lossy(name)
            ),
            line,
        ))
    }

    pub(super) fn can_access_static_visibility(
        &self,
        owner: &Class,
        visibility: Visibility,
    ) -> bool {
        visibility == Visibility::Public
            || self.class_context.as_ref().is_some_and(|context| {
                context.scope.name.eq_ignore_ascii_case(&owner.name)
                    || (visibility == Visibility::Protected
                        && !context.scope.declaration.is_interface
                        && (context.scope.is_a(&owner.name) || owner.is_a(&context.scope.name)))
            })
    }

    pub(super) fn check_static_set_access(
        &self,
        location: &Location,
        indirect: bool,
        line: usize,
    ) -> Result<(), Error> {
        let Some(class) = &location.static_class else {
            return Ok(());
        };
        let Some(owner) = class.static_owner(&location.name) else {
            return Ok(());
        };
        let property = owner
            .declaration
            .static_properties
            .iter()
            .find(|p| p.name == location.name)
            .expect("static property owner");
        if self.can_access_static_visibility(&owner, property.set_visibility) {
            return Ok(());
        }
        self.check_static_property_access(class, &owner, &location.name, line, false)?;
        let visibility = if property.set_visibility == Visibility::Private {
            "private"
        } else {
            "protected"
        };
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Cannot {}modify {visibility}(set) property {}::${} from {}",
                if indirect { "indirectly " } else { "" },
                String::from_utf8_lossy(&owner.name),
                String::from_utf8_lossy(&location.name),
                self.class_context.as_ref().map_or_else(
                    || "global scope".to_owned(),
                    |c| format!("scope {}", String::from_utf8_lossy(&c.scope.name))
                )
            ),
            line,
        ))
    }

    pub(super) fn instance_static_owner(
        &self,
        class: &Rc<Class>,
        name: &[u8],
    ) -> Option<Rc<Class>> {
        if self.instance_property_owner(class, name).is_some() {
            return None;
        }
        if let Some(context) = &self.class_context
            && class.is_a(&context.scope.name)
            && context
                .scope
                .declaration
                .static_properties
                .iter()
                .any(|p| p.name == name && p.visibility == Visibility::Private)
        {
            return Some(context.scope.clone());
        }
        class.static_owner(name).filter(|owner| {
            owner.name == class.name
                || owner
                    .declaration
                    .static_properties
                    .iter()
                    .any(|p| p.name == name && p.visibility != Visibility::Private)
        })
    }
}
