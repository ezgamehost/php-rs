use super::*;

impl Runtime {
    pub(super) fn check_readonly_write(
        &self,
        location: &Location,
        unset: bool,
        line: usize,
    ) -> Result<(), Error> {
        self.check_readonly_access(location, unset, !location.keys.is_empty(), line)
    }

    fn check_readonly_access(
        &self,
        location: &Location,
        unset: bool,
        indirect: bool,
        line: usize,
    ) -> Result<(), Error> {
        if location.overloaded.is_some() {
            return Ok(());
        }
        if !unset
            && !indirect
            && location
                .object
                .as_ref()
                .is_some_and(|object| self.magic_set_applicable(object, &location.name))
        {
            return Ok(());
        }
        self.check_instance_set_access(location, unset, indirect, line)?;
        let Some(object) = &location.object else {
            return Ok(());
        };
        let Some(owner) = self.instance_property_owner(&object.class, &location.name) else {
            return Ok(());
        };
        let property = owner
            .declaration
            .properties
            .iter()
            .find(|p| p.name == location.name)
            .expect("property owner");
        if !property.is_readonly {
            return Ok(());
        }
        let key = self.instance_property_key(object, &location.name);
        let ArrayKey::String(storage_name) = &key else {
            unreachable!()
        };
        let initialized = object.properties.borrow().get(&key).is_some();
        let qualified = format!(
            "{}::${}",
            String::from_utf8_lossy(&owner.name),
            String::from_utf8_lossy(&location.name)
        );
        // Unsetting an offset in absent storage is a no-op, including when the
        // absent root is readonly. Direct unsetting still checks set visibility.
        if unset && indirect && !initialized {
            return Ok(());
        }
        if indirect {
            return Err(Error::new(
                ErrorKind::Error,
                format!("Cannot indirectly modify readonly property {qualified}"),
                line,
            ));
        }
        let operation = if unset { "unset" } else { "modify" };
        if initialized && !object.reinitializable.borrow().contains(storage_name) {
            return Err(Error::new(
                ErrorKind::Error,
                format!("Cannot {operation} readonly property {qualified}"),
                line,
            ));
        }
        if self.can_access_instance_visibility(&owner, &location.name, property.set_visibility) {
            return Ok(());
        }
        let scope = self.class_context.as_ref().map_or_else(
            || "global scope".to_owned(),
            |c| format!("scope {}", String::from_utf8_lossy(&c.scope.name)),
        );
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "Cannot {operation} {}(set) readonly property {qualified} from {scope}",
                if property.set_visibility == crate::ast::Visibility::Private {
                    "private"
                } else {
                    "protected"
                }
            ),
            line,
        ))
    }
    // A dimension on the path to an object write is fetched for modification.
    // Preserve its property origin so readonly arrays cannot be used as lvalues
    // indirectly. Copying the array first intentionally loses that origin.
    pub(super) fn property_write_base(
        &mut self,
        expr: &Expr,
        silent: bool,
    ) -> Result<Value, Error> {
        self.write_base_value(expr, silent).map(|(value, _)| value)
    }

    fn write_base_value(
        &mut self,
        expr: &Expr,
        silent: bool,
    ) -> Result<(Value, Option<Location>), Error> {
        match &expr.kind {
            ExprKind::Group(inner) => self.write_base_value(inner, silent),
            ExprKind::StaticProperty(class, name) => {
                let mut location = self.static_location(class, name, expr.line)?;
                if silent
                    && self
                        .static_cell(
                            location.static_class.as_ref().expect("static property"),
                            &location.name,
                            expr.line,
                            false,
                        )?
                        .is_none()
                {
                    return Ok((Value::Null, None));
                }
                let value = self.read_location(&mut location, expr.line, silent, None)?;
                Ok((value, Some(location)))
            }
            ExprKind::Property(base, name) => {
                let (value, _) = self.write_base_value(base, silent)?;
                let name = self.variable_name(name)?;
                if let Value::Object(object) = value {
                    let overloaded = self.magic_get_applicable(&object, &name);
                    let value = self.read_property_value(
                        &object,
                        &name,
                        expr.line,
                        if silent && !overloaded {
                            Access::Coalesce
                        } else {
                            Access::Read
                        },
                    )?;
                    if overloaded {
                        self.warn_indirect_magic(&object, &name, &value, expr.line);
                    }
                    let cached = overloaded.then(|| value.clone());
                    Ok((
                        value,
                        Some(Location {
                            overloaded: cached,
                            object: Some(object),
                            static_class: None,
                            name,
                            global: false,
                            keys: Vec::new(),
                        }),
                    ))
                } else {
                    if !silent {
                        self.diagnostics.push(format!(
                            "Attempt to read property \"{}\" on {} on line {}",
                            String::from_utf8_lossy(&name),
                            builtins::debug_type(&value),
                            expr.line
                        ));
                    }
                    Ok((Value::Null, None))
                }
            }
            ExprKind::Index(base, Some(key)) => {
                let (value, origin) = self.write_base_value(base, silent)?;
                let key = self.eval(key)?;
                if let Some(origin) = origin {
                    self.check_readonly_access(&origin, silent, true, expr.line)?;
                    self.check_static_set_access(&origin, true, expr.line)?;
                }
                if silent && matches!(value, Value::Int(_) | Value::Float(_) | Value::Bool(true)) {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Cannot unset offset in a non-array variable",
                        expr.line,
                    ));
                }
                let value = read_offset(
                    value,
                    &mut Offset::new(Some(key)),
                    expr.line,
                    if silent {
                        Access::Coalesce
                    } else {
                        Access::Read
                    },
                    &mut self.diagnostics,
                )?;
                Ok((value, None))
            }
            _ => Ok((
                if silent {
                    self.coalesce_read(expr, Access::Coalesce)?
                } else {
                    self.eval(expr)?
                },
                None,
            )),
        }
    }
}
