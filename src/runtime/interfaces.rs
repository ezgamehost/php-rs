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

    pub(super) fn validate_interfaces(&self, class: &Class) -> Result<(), Error> {
        for interface in &class.interfaces {
            for required in &interface.methods {
                let name = required.name.rsplit(|b| *b == b':').next().unwrap();
                let implementation = objects::method(class, name).expect("interface method linked");
                if implementation.is_static != required.is_static
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
                        .and_then(|p| p.constant_owner(&name))
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
