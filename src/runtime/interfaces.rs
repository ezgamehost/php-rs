use super::*;
use crate::{ast::ClassDecl, object::Class};

impl Runtime {
    pub(super) fn inheritance_method_signature(
        &self,
        function: &FunctionDecl,
        class: &Class,
    ) -> String {
        let declaring = method_visibility::declaring_name(function);
        let owner = self.find_class(declaring);
        let owner = if declaring.eq_ignore_ascii_case(&class.name) {
            class
        } else {
            owner.as_deref().unwrap_or(class)
        };
        method_signature_with_types(function, |ty| typed_properties::type_label(ty, owner), true)
    }

    pub(super) fn trait_requirement_signature(
        &self,
        function: &FunctionDecl,
        class: &Class,
    ) -> String {
        let mut declaration = function.clone();
        if let Some(origin) = &function.trait_origin {
            declaration.name = origin.clone();
        }
        method_signature_with_types(
            &declaration,
            |ty| typed_properties::type_label(ty, class),
            true,
        )
    }

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
                let message = if implementation.is_static != required.is_static {
                    let status = |is_static| if is_static { "static" } else { "non static" };
                    Some(format!(
                        "Cannot make {} method {}() {} in class {}",
                        status(required.is_static),
                        String::from_utf8_lossy(&required.name),
                        status(implementation.is_static),
                        class.display_name()
                    ))
                } else if implementation.visibility != crate::ast::Visibility::Public {
                    Some(format!(
                        "Access level to {}() must be public (as in class {})",
                        String::from_utf8_lossy(&implementation.name),
                        String::from_utf8_lossy(method_visibility::declaring_name(required))
                    ))
                } else if !self.compatible_method_loaded(&implementation, required, class)? {
                    Some(format!(
                        "Declaration of {} must be compatible with {}",
                        self.inheritance_method_signature(&implementation, class),
                        self.inheritance_method_signature(required, class)
                    ))
                } else {
                    None
                };
                if let Some(message) = message {
                    return Err(self.deferred_compile_fatal_at(
                        message,
                        &implementation.source,
                        implementation.line,
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
                    self.warn(format!(
                        "Deprecated: Return type of {} should either be compatible with {}: {tentative}, or the #[\\ReturnTypeWillChange] attribute should be used to temporarily suppress the notice on line {}",
                        method_signature(&implementation), method_signature(required), implementation.line
                    ))?;
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

pub(super) fn method_signature(function: &FunctionDecl) -> String {
    method_signature_with_types(function, ToString::to_string, false)
}

fn method_signature_with_types(
    function: &FunctionDecl,
    type_label: impl Fn(&crate::ast::TypeDecl) -> String,
    include_tentative: bool,
) -> String {
    let required = function
        .parameters
        .iter()
        .rposition(|p| p.default.is_none() && !p.variadic)
        .map_or(0, |i| i + 1);
    let parameters = function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let mut parameter = format!(
                "{}{}{}${}",
                p.ty.as_ref()
                    .map_or(String::new(), |ty| format!("{} ", type_label(ty))),
                if p.by_reference { "&" } else { "" },
                if p.variadic { "..." } else { "" },
                String::from_utf8_lossy(&p.name)
            );
            if index >= required
                && !p.variadic
                && let Some(default) = &p.default
            {
                parameter.push_str(" = ");
                parameter.push_str(&signature_default(default));
            }
            parameter
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}{}({parameters}){}",
        if function.returns_reference { "& " } else { "" },
        signature_name(function),
        function
            .return_type
            .as_ref()
            .or_else(|| include_tentative
                .then_some(function.tentative_return_type.as_ref())
                .flatten())
            .map_or(String::new(), |ty| format!(": {}", type_label(ty)))
    )
}

fn signature_name(function: &FunctionDecl) -> String {
    let scope = method_visibility::declaring_name(function);
    if let Some(end) = scope.iter().position(|b| *b == 0) {
        format!(
            "{}{}",
            String::from_utf8_lossy(&scope[..end]),
            String::from_utf8_lossy(&function.name[scope.len()..])
        )
    } else {
        String::from_utf8_lossy(&function.name).into_owned()
    }
}

fn signature_default(expression: &Expr) -> String {
    match &expression.kind {
        ExprKind::Literal(Value::Null) => "null".to_owned(),
        ExprKind::Literal(Value::Bool(value)) => value.to_string(),
        ExprKind::Literal(Value::String(value)) => {
            let bytes = value.bytes();
            format!(
                "'{}{}'",
                String::from_utf8_lossy(&bytes[..bytes.len().min(10)]),
                if bytes.len() > 10 { "..." } else { "" }
            )
        }
        ExprKind::Literal(Value::Array(value)) => {
            if value.is_empty() { "[]" } else { "[...]" }.to_owned()
        }
        ExprKind::Literal(value) => String::from_utf8_lossy(&value.to_bytes()).into_owned(),
        ExprKind::Array(items) | ExprKind::LongArray(items) => {
            if items.is_empty() { "[]" } else { "[...]" }.to_owned()
        }
        ExprKind::Constant(name) => String::from_utf8_lossy(name).into_owned(),
        ExprKind::NamespacedName(name) => String::from_utf8_lossy(&name.constant).into_owned(),
        ExprKind::ScopedMember(class, member) => {
            let class = match &class.kind {
                ExprKind::NamespacedName(name) => String::from_utf8_lossy(&name.class).into_owned(),
                _ => signature_default(class),
            };
            if let ExprKind::Literal(Value::String(name)) = &member.kind {
                format!("{class}::{}", String::from_utf8_lossy(&name.bytes()))
            } else {
                "<expression>".to_owned()
            }
        }
        ExprKind::Group(inner) => signature_default(inner),
        _ => "<expression>".to_owned(),
    }
}
