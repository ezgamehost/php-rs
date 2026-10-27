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
        let Some(object) = &location.object else {
            return Ok(());
        };
        let Some(owner) = object.class.property_owner(&location.name) else {
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
        let initialized = object
            .properties
            .borrow()
            .get(&ArrayKey::String(location.name.clone()))
            .is_some();
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
        if initialized {
            return Err(Error::new(
                ErrorKind::Error,
                format!("Cannot {operation} readonly property {qualified}"),
                line,
            ));
        }
        if self
            .class_context
            .as_ref()
            .is_some_and(|c| c.scope.is_a(&owner.name) || owner.is_a(&c.scope.name))
        {
            return Ok(());
        }
        let scope = self.class_context.as_ref().map_or_else(
            || "global scope".to_owned(),
            |c| format!("scope {}", String::from_utf8_lossy(&c.scope.name)),
        );
        Err(Error::new(
            ErrorKind::Error,
            format!("Cannot {operation} protected(set) readonly property {qualified} from {scope}"),
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
            ExprKind::Property(base, name) => {
                let (value, _) = self.write_base_value(base, silent)?;
                let name = self.variable_name(name)?;
                if let Value::Object(object) = value {
                    let value = self.read_property_value(&object, &name, expr.line, silent)?;
                    Ok((
                        value,
                        Some(Location {
                            object: Some(object),
                            static_class: None,
                            name,
                            global: false,
                            keys: Vec::new(),
                        }),
                    ))
                } else {
                    if !silent {
                        self.warnings.push(format!(
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
                    &mut self.warnings,
                )?;
                Ok((value, None))
            }
            _ => Ok((
                if silent {
                    self.coalesce_read(expr, true)?
                } else {
                    self.eval(expr)?
                },
                None,
            )),
        }
    }
}
