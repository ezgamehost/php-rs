use super::*;
use crate::{PhpObject, ast::PropertyHooks};

impl Runtime {
    pub(super) fn property_hooks(&self, object: &PhpObject, name: &[u8]) -> Option<PropertyHooks> {
        self.instance_property_owner(&object.class, name)?
            .declaration
            .properties
            .iter()
            .find(|p| p.name == name)?
            .hooks
            .clone()
    }

    pub(super) fn in_property_hook(&self, object: &PhpObject, name: &[u8]) -> bool {
        self.active_property_hooks
            .last()
            .is_some_and(|(active, property, function)| {
                (active.object_id == object.object_id
                    || active
                        .lazy_instance()
                        .is_some_and(|backing| backing.object_id == object.object_id))
                    && property == name
                    && self
                        .current_function
                        .as_ref()
                        .is_some_and(|current| current.eq_ignore_ascii_case(function))
            })
    }

    fn invoke_property_hook(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        function: Rc<FunctionDecl>,
        value: Option<Value>,
        line: usize,
    ) -> Result<Value, Error> {
        let arguments: Vec<_> = value
            .into_iter()
            .map(|v| {
                crate::ast::Argument::Positional(Expr {
                    kind: ExprKind::Literal(v),
                    line,
                })
            })
            .collect();
        self.invoke_property_hook_arguments(object, name, function, &arguments, line, true)
    }

    pub(super) fn read_property_hook(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Option<Value>, Error> {
        let Some(hooks) = self.property_hooks(object, name) else {
            return Ok(None);
        };
        if !self.check_instance_property_access(object, name, true, line)? {
            return Ok(None);
        }
        if let Some(get) = hooks.get {
            if self.in_property_hook(object, name) {
                if hooks.is_virtual {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Must not read from virtual property {}::${}",
                            object.class_name,
                            String::from_utf8_lossy(name)
                        ),
                        line,
                    ));
                }
                return Ok(None);
            }
            return self
                .invoke_property_hook(object, name, get, None, line)
                .map(Some);
        }
        if hooks.is_virtual {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Property {}::${} is write-only",
                    object.class_name,
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        Ok(None)
    }

    pub(super) fn write_property_hook(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        value: &Value,
        line: usize,
    ) -> Result<bool, Error> {
        let Some(hooks) = self.property_hooks(object, name) else {
            return Ok(false);
        };
        if let Some(set) = hooks.set {
            if self.in_property_hook(object, name) {
                if hooks.is_virtual {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Must not write to virtual property {}::${}",
                            object.class_name,
                            String::from_utf8_lossy(name)
                        ),
                        line,
                    ));
                }
                return Ok(false);
            }
            self.invoke_property_hook(object, name, set, Some(value.dereferenced()), line)?;
            return Ok(true);
        }
        if hooks.is_virtual {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Property {}::${} is read-only",
                    object.class_name,
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        Ok(false)
    }

    pub(super) fn indirect_hook_value(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Option<Value>, Error> {
        let Some(hooks) = self.property_hooks(object, name) else {
            return Ok(None);
        };
        if self.in_property_hook(object, name) && !hooks.is_virtual && hooks.get.is_some() {
            return Ok(None);
        }
        let value = self.read_property_value(object, name, line, Access::Read)?;
        if matches!(
            value,
            Value::Reference(_) | Value::Object(_) | Value::Closure(_)
        ) {
            return Ok(Some(value));
        }
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Indirect modification of {}::${} is not allowed",
                object.class_name,
                String::from_utf8_lossy(name)
            ),
            line,
        ))
    }
}

impl Runtime {
    pub(super) fn link_property_hooks(
        &self,
        decl: &mut crate::ast::ClassDecl,
        parent: Option<&Rc<crate::object::Class>>,
    ) -> Result<(), Error> {
        for property in &mut decl.properties {
            let inherited = parent.and_then(|p| {
                p.properties.iter().find(|p| {
                    p.name == property.name && p.visibility != crate::ast::Visibility::Private
                })
            });
            if let Some(inherited) = inherited {
                if let Some(parent_hooks) = &inherited.hooks {
                    if let Some(hooks) = &mut property.hooks {
                        hooks.is_virtual &= parent_hooks.is_virtual;
                        for (hook, is_final, previous) in [
                            (&hooks.get, parent_hooks.get_final, &parent_hooks.get),
                            (&hooks.set, parent_hooks.set_final, &parent_hooks.set),
                        ] {
                            if hook.is_some() && is_final {
                                return Err(Error::new(
                                    ErrorKind::CompileError,
                                    format!(
                                        "Cannot override final property hook {}()",
                                        String::from_utf8_lossy(&previous.as_ref().unwrap().name)
                                    ),
                                    decl.line,
                                ));
                            }
                        }
                        if hooks.get.is_none()
                            && (hooks.is_virtual
                                || parent_hooks.get.as_ref().is_some_and(|f| !f.is_abstract))
                        {
                            hooks.get = parent_hooks.get.clone();
                            hooks.get_final = parent_hooks.get_final;
                        }
                        if hooks.set.is_none()
                            && (hooks.is_virtual
                                || property.is_readonly
                                || parent_hooks.set.as_ref().is_some_and(|f| !f.is_abstract))
                        {
                            hooks.set = parent_hooks.set.clone();
                            hooks.set_final = parent_hooks.set_final;
                        }
                    } else {
                        let mut hooks = parent_hooks.clone();
                        hooks.is_virtual = false;
                        if hooks.get.as_ref().is_some_and(|f| f.is_abstract) {
                            hooks.get = None;
                            hooks.get_final = false;
                        }
                        if !property.is_readonly
                            && hooks.set.as_ref().is_some_and(|f| f.is_abstract)
                        {
                            hooks.set = None;
                            hooks.set_final = false;
                        }
                        if hooks.get.is_some() || hooks.set.is_some() {
                            property.hooks = Some(hooks);
                        }
                    }
                } else if let Some(hooks) = &mut property.hooks {
                    hooks.is_virtual = false;
                }
            }
            if let Some(hooks) = &property.hooks {
                if property.is_abstract
                    && !hooks
                        .get
                        .iter()
                        .chain(hooks.set.iter())
                        .any(|h| h.is_abstract)
                {
                    return Err(Error::new(
                        ErrorKind::CompileError,
                        format!(
                            "Abstract property {}::${} must specify at least one abstract hook",
                            String::from_utf8_lossy(&decl.name),
                            String::from_utf8_lossy(&property.name)
                        ),
                        decl.line,
                    ));
                }
                if hooks.is_virtual && property.default.is_some() {
                    return Err(Error::new(
                        ErrorKind::CompileError,
                        format!(
                            "Cannot specify default value for virtual hooked property {}::${}",
                            String::from_utf8_lossy(&decl.name),
                            String::from_utf8_lossy(&property.name)
                        ),
                        decl.line,
                    ));
                }
                if !hooks.is_virtual
                    && hooks.set.is_some()
                    && hooks.get.as_ref().is_some_and(|g| g.returns_reference)
                {
                    return Err(Error::new(
                        ErrorKind::CompileError,
                        format!(
                            "Get hook of backed property {}::{} with set hook may not return by reference",
                            String::from_utf8_lossy(&decl.name),
                            String::from_utf8_lossy(&property.name)
                        ),
                        decl.line,
                    ));
                }
            }
        }
        Ok(())
    }
}
