use super::*;
use crate::{ast::ClassDecl, object::Class};

impl Runtime {
    pub(super) fn link_interfaces(&self, declaration: &ClassDecl) -> Result<Vec<Rc<Class>>, Error> {
        let mut interfaces = Vec::<Rc<Class>>::new();
        for name in &declaration.interfaces {
            let interface = self.find_class(name).ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    format!("Interface \"{}\" not found", String::from_utf8_lossy(name)),
                    declaration.line,
                )
            })?;
            if !interface.declaration.is_interface {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Expected an interface",
                    declaration.line,
                ));
            }
            if interfaces.iter().any(|i| Rc::ptr_eq(i, &interface)) {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot implement the same interface twice",
                    declaration.line,
                ));
            }
            interfaces.push(interface);
        }
        Ok(interfaces)
    }

    pub(super) fn validate_interfaces(&mut self, class: &Class) -> Result<(), Error> {
        for interface in &class.interfaces {
            for required in &interface.methods {
                let name = required.name.rsplit(|b| *b == b':').next().unwrap();
                let implementation = objects::method(class, name).expect("interface method linked");
                if implementation.visibility != crate::ast::Visibility::Public
                    || implementation.is_static != required.is_static
                    || !self.compatible_method(&implementation, required, class)
                {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Declaration of {}() must be compatible with {}()",
                            String::from_utf8_lossy(&implementation.name),
                            String::from_utf8_lossy(&required.name)
                        ),
                        class.declaration.line,
                    ));
                }
            }
            for required in &interface.methods {
                let name = required.name.rsplit(|b| *b == b':').next().unwrap();
                let implementation = objects::method(class, name).unwrap();
                if let Some(tentative) = &required.tentative_return_type
                    && implementation.native.is_none()
                    && (implementation.return_type.is_none()
                        || !self.type_subset(
                            implementation.return_type.as_ref(),
                            Some(tentative),
                            class,
                        ))
                    && !implementation
                        .attributes
                        .iter()
                        .any(|a| a.name.eq_ignore_ascii_case(b"ReturnTypeWillChange"))
                {
                    self.diagnostics.push(format!(
                        "Deprecated: Return type of {} should either be compatible with {}: {tentative}, or the #[\\ReturnTypeWillChange] attribute should be used to temporarily suppress the notice on line {}",
                        method_signature(&implementation), method_signature(required), implementation.line
                    ));
                }
            }
            for name in interface.constant_names() {
                let required_owner = interface.constant_owner(&name).expect("interface constant");
                let required = required_owner
                    .declaration
                    .constants
                    .iter()
                    .find(|c| c.name == name)
                    .unwrap();
                let own = class.declaration.constants.iter().find(|c| c.name == name);
                if let Some(own) = own {
                    if own.visibility != crate::ast::Visibility::Public {
                        return Err(Error::new(
                            ErrorKind::Error,
                            "Interface constant implementations must be public",
                            own.value.line,
                        ));
                    }
                    if required.is_final {
                        return Err(Error::new(
                            ErrorKind::Error,
                            "Cannot override final interface constant",
                            own.value.line,
                        ));
                    }
                    if let Some(required_type) = &required.ty {
                        let required_type = inheritance::class_type(required_type, &required_owner);
                        let own_type = own.ty.as_ref().map(|ty| inheritance::class_type(ty, class));
                        if own_type.is_none()
                            || !self.type_subset(own_type.as_ref(), Some(&required_type), class)
                        {
                            return Err(Error::new(
                                ErrorKind::Error,
                                "Incompatible interface constant type",
                                own.value.line,
                            ));
                        }
                    }
                } else {
                    let inherited_owner = class
                        .parent
                        .as_ref()
                        .and_then(|p| p.inheritable_constant_owner(&name))
                        .or_else(|| {
                            class
                                .interfaces
                                .iter()
                                .find_map(|i| i.constant_owner(&name))
                        })
                        .unwrap();
                    if !Rc::ptr_eq(&inherited_owner, &required_owner) {
                        return Err(Error::new(
                            ErrorKind::Error,
                            format!(
                                "Ambiguous inherited constant {}",
                                String::from_utf8_lossy(&name)
                            ),
                            class.declaration.line,
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

fn method_signature(function: &FunctionDecl) -> String {
    let parameters = function
        .parameters
        .iter()
        .map(|p| {
            let mut parameter = format!(
                "{}{}{}${}",
                p.ty.as_ref().map_or(String::new(), |ty| format!("{ty} ")),
                if p.by_reference { "&" } else { "" },
                if p.variadic { "..." } else { "" },
                String::from_utf8_lossy(&p.name)
            );
            if let Some(Expr {
                kind: ExprKind::Literal(value),
                ..
            }) = &p.default
            {
                parameter.push_str(" = ");
                parameter.push_str(&match value {
                    Value::Null => "null".to_owned(),
                    Value::Bool(value) => value.to_string(),
                    Value::String(value) => format!(
                        "'{}'",
                        String::from_utf8_lossy(&value.bytes())
                            .replace('\\', "\\\\")
                            .replace('\'', "\\'")
                    ),
                    _ => String::from_utf8_lossy(&value.to_bytes()).into_owned(),
                });
            }
            parameter
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}{}({parameters}){}",
        if function.returns_reference { "& " } else { "" },
        String::from_utf8_lossy(&function.name),
        function
            .return_type
            .as_ref()
            .map_or(String::new(), |ty| format!(": {ty}"))
    )
}
