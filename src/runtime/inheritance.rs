use super::*;
use crate::{ast::TypeDecl, object::Class};

impl Runtime {
    pub(super) fn validate_final_properties(
        &self,
        decl: &crate::ast::ClassDecl,
        parent: Option<&Rc<Class>>,
    ) -> Result<(), Error> {
        let Some(parent) = parent else {
            return Ok(());
        };
        for property in decl.properties.iter().chain(&decl.static_properties) {
            if let Some(inherited) = parent
                .properties
                .iter()
                .chain(&parent.static_properties)
                .find(|p| p.name == property.name && p.is_final)
            {
                let owner = if inherited.declaring_class.is_empty() {
                    parent
                        .static_owner(&property.name)
                        .unwrap_or_else(|| parent.clone())
                        .name
                        .clone()
                } else {
                    inherited.declaring_class.clone()
                };
                return Err(Error::new(
                    ErrorKind::CompileError,
                    format!(
                        "Cannot override final property {}::${}",
                        String::from_utf8_lossy(&owner),
                        String::from_utf8_lossy(&property.name)
                    ),
                    decl.line,
                ));
            }
        }
        Ok(())
    }

    // A comparison involving unresolved class names cannot be early-bound.
    // The candidate's own type is known in its declarations, but not yet in
    // unresolved declarations inherited from its parent.
    pub(super) fn property_variance_unresolved(&self, class: &Class) -> bool {
        if self.hook_variance_unresolved(class) {
            return true;
        }
        let Some(parent) = &class.parent else {
            return false;
        };
        for (properties, inherited, is_static) in [
            (&class.declaration.properties, &parent.properties, false),
            (
                &class.declaration.static_properties,
                &parent.static_properties,
                true,
            ),
        ] {
            for property in properties {
                let Some(parent_property) = inherited.iter().find(|p| {
                    p.name == property.name && p.visibility != crate::ast::Visibility::Private
                }) else {
                    continue;
                };
                if parent_property.visibility == crate::ast::Visibility::Private {
                    continue;
                }
                let (Some(child_ty), Some(parent_ty)) = (&property.ty, &parent_property.ty) else {
                    continue;
                };
                let owner = if is_static {
                    parent.static_owner(&property.name)
                } else {
                    parent.property_owner(&property.name)
                }
                .expect("property owner");
                let child_ty = class_type(child_ty, class);
                let parent_ty = class_type(parent_ty, &owner);
                let is_class = |name: &str| {
                    !matches!(
                        name,
                        "int"
                            | "float"
                            | "string"
                            | "bool"
                            | "true"
                            | "false"
                            | "null"
                            | "array"
                            | "iterable"
                            | "object"
                            | "mixed"
                            | "callable"
                    )
                };
                let builtin_members = |ty: &TypeDecl| -> HashSet<String> {
                    ty.members
                        .iter()
                        .filter(|m| !is_class(m))
                        .map(|m| {
                            if m == "iterable" {
                                "array".to_owned()
                            } else {
                                m.clone()
                            }
                        })
                        .collect()
                };
                if builtin_members(&child_ty) != builtin_members(&parent_ty) {
                    continue;
                }
                for child in child_ty.members.iter().filter(|n| is_class(n)) {
                    for inherited in parent_ty.members.iter().filter(|n| is_class(n)) {
                        if !child.eq_ignore_ascii_case(inherited)
                            && ((self.find_class(inherited.as_bytes()).is_none()
                                && !inherited.eq_ignore_ascii_case("Closure")
                                && !inherited.eq_ignore_ascii_case("Traversable"))
                                || (self.find_class(child.as_bytes()).is_none()
                                    && !child.eq_ignore_ascii_case("Closure")
                                    && !child.eq_ignore_ascii_case("Traversable")
                                    && !class.name.eq_ignore_ascii_case(child.as_bytes())))
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub(super) fn validate_overrides(&mut self, class: &Class) -> Result<(), Error> {
        for method in &class.declaration.methods {
            if method
                .name
                .rsplit(|byte| *byte == b':')
                .next()
                .is_some_and(|name| name.eq_ignore_ascii_case(b"__destruct"))
            {
                let invalid = if method.is_static {
                    Some("be static")
                } else if !method.parameters.is_empty() {
                    Some("take arguments")
                } else if method.return_type.is_some() {
                    Some("declare a return type")
                } else {
                    None
                };
                if let Some(invalid) = invalid {
                    return Err(self.compile_fatal_at(
                        &format!(
                            "Method {}() cannot {invalid}",
                            String::from_utf8_lossy(&method.name)
                        ),
                        &method.source,
                        method.line,
                    ));
                }
            }
        }
        if class.declaration.is_trait {
            return Ok(());
        }
        self.validate_iterator_interfaces(class)?;
        self.validate_inheritance(class)?;
        if !class.declaration.is_interface
            && !class.declaration.is_abstract
            && class.is_a(b"Serializable")
            && (objects::method(class, b"__serialize").is_none()
                || objects::method(class, b"__unserialize").is_none())
        {
            self.warn(format!("Deprecated: {} implements the Serializable interface, which is deprecated. Implement __serialize() and __unserialize() instead (or in addition, if support for old PHP versions is necessary) on line {}", class.display_name(), class.declaration.line))?;
        }
        self.validate_override_attributes(class)
    }

    // Adapted from PHP 8.5.0 Zend/zend_inheritance.c override checks.
    // Check only declarations introduced by this class: inherited declarations
    // were already checked in their original class, before any child interfaces.
    fn validate_override_attributes(&mut self, class: &Class) -> Result<(), Error> {
        self.validate_hook_overrides(class)?;
        let has_override = |attributes: &[crate::ast::Attribute]| {
            attributes
                .iter()
                .any(|attribute| attribute.name.eq_ignore_ascii_case(b"Override"))
        };
        for function in &class.declaration.methods {
            if !has_override(&function.attributes) {
                continue;
            }
            let name = method_name(function);
            let parent = class.parent.as_ref().and_then(|parent| {
                if name.eq_ignore_ascii_case(b"__construct") {
                    abstract_constructor(parent)
                } else {
                    objects::method(parent, name)
                        .filter(|method| method.visibility != crate::ast::Visibility::Private)
                }
            });
            let interface = class
                .interfaces
                .iter()
                .any(|interface| objects::method(interface, name).is_some());
            let requirement = class.declaration.trait_requirements.iter().any(|required| {
                method_name(required).eq_ignore_ascii_case(name)
                    && required.trait_origin != function.trait_origin
            });
            if parent.is_none() && !interface && !requirement {
                return Err(self.compile_fatal_at(
                    &format!(
                        "{}::{}() has #[\\Override] attribute, but no matching parent method exists",
                        class.display_name(),
                        String::from_utf8_lossy(name)
                    ),
                    &function.source,
                    function.line,
                ));
            }
        }
        let mut properties: Vec<_> = class
            .declaration
            .properties
            .iter()
            .chain(&class.declaration.static_properties)
            .collect();
        properties.sort_by_key(|property| property.order);
        for property in properties {
            if !has_override(&property.attributes) {
                continue;
            }
            let matches = |parent: &crate::ast::PropertyDecl| {
                parent.name == property.name && parent.visibility != crate::ast::Visibility::Private
            };
            let parent = class.parent.as_ref().is_some_and(|parent| {
                parent.properties.iter().any(matches)
                    || parent.static_properties.iter().any(matches)
            });
            let interface = class
                .interfaces
                .iter()
                .any(|interface| interface.properties.iter().any(matches));
            if !parent && !interface {
                return Err(self.compile_fatal_at(
                    &format!(
                        "{}::${} has #[\\Override] attribute, but no matching parent property exists",
                        class.display_name(),
                        String::from_utf8_lossy(&property.name)
                    ),
                    &class.declaration.source,
                    class.declaration.end_line,
                ));
            }
        }
        Ok(())
    }

    fn inherited_property_error(&mut self, class: &Class, message: String) -> Error {
        self.deferred_compile_fatal_at(message, &class.declaration.source, class.declaration.line)
    }

    fn validate_inheritance(&mut self, class: &Class) -> Result<(), Error> {
        self.validate_trait_requirements(class)?;
        self.validate_property_contracts(class)?;
        self.validate_interfaces(class)?;
        if !class.declaration.is_abstract
            && let Some(method) = class.methods.iter().find(|method| method.is_abstract)
        {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Class {} must implement abstract method {}()",
                    String::from_utf8_lossy(&class.name),
                    String::from_utf8_lossy(&method.name)
                ),
                class.declaration.line,
            ));
        }
        let Some(parent) = &class.parent else {
            return Ok(());
        };
        for (properties, inherited, is_static) in [
            (&class.declaration.properties, &parent.properties, false),
            (
                &class.declaration.static_properties,
                &parent.static_properties,
                true,
            ),
        ] {
            for property in properties {
                let Some(parent_property) = inherited.iter().find(|p| {
                    p.name == property.name && p.visibility != crate::ast::Visibility::Private
                }) else {
                    continue;
                };
                let owner = if is_static {
                    parent.static_owner(&property.name)
                } else {
                    parent.property_owner(&property.name)
                }
                .expect("property owner");
                let parent_get_only = parent_property
                    .hooks
                    .as_ref()
                    .is_some_and(|h| h.is_virtual && h.set.is_none());
                let property_name = String::from_utf8_lossy(&property.name);
                let child_label = format!("{}::${property_name}", class.display_name());
                if property.is_readonly != parent_property.is_readonly && !parent_get_only {
                    let status = |readonly| if readonly { "readonly" } else { "non-readonly" };
                    return Err(self.inherited_property_error(
                        class,
                        format!(
                            "Cannot redeclare {} property {}::${property_name} as {} {child_label}",
                            status(parent_property.is_readonly),
                            owner.display_name(),
                            status(property.is_readonly),
                        ),
                    ));
                }
                if property.set_visibility != property.visibility
                    && property.set_visibility > parent_property.set_visibility
                    && !parent_get_only
                {
                    let (visibility, suffix) =
                        if parent_property.set_visibility != parent_property.visibility {
                            (
                                match parent_property.set_visibility {
                                    crate::ast::Visibility::Protected => "protected(set)",
                                    crate::ast::Visibility::Private => "private(set)",
                                    crate::ast::Visibility::Public => unreachable!(),
                                },
                                " or weaker",
                            )
                        } else {
                            ("omitted", "")
                        };
                    return Err(self.inherited_property_error(class, format!(
                        "Set access level of {child_label} must be {visibility} (as in class {}){suffix}", owner.display_name(),
                    )));
                }
                if property.visibility > parent_property.visibility {
                    let (visibility, suffix) = match parent_property.visibility {
                        crate::ast::Visibility::Public => ("public", ""),
                        crate::ast::Visibility::Protected => ("protected", " or weaker"),
                        crate::ast::Visibility::Private => unreachable!(),
                    };
                    return Err(self.inherited_property_error(class, format!(
                        "Access level to {child_label} must be {visibility} (as in class {}){suffix}", owner.display_name(),
                    )));
                }
                if property.hooks.is_some() || parent_property.hooks.is_some() {
                    self.validate_hook_property_type(property, class, parent_property, &owner)?;
                    continue;
                }
                let child_type = property.ty.as_ref().map(|ty| class_type(ty, class));
                let parent_type = parent_property.ty.as_ref().map(|ty| class_type(ty, &owner));
                if child_type.is_some() != parent_type.is_some()
                    || !self.type_subset(child_type.as_ref(), parent_type.as_ref(), class)
                    || !self.type_subset(parent_type.as_ref(), child_type.as_ref(), class)
                {
                    let message = if let Some(ty) = &parent_property.ty {
                        format!(
                            "Type of {child_label} must be {} (as in class {})",
                            typed_properties::type_label(ty, &owner),
                            owner.display_name()
                        )
                    } else {
                        format!(
                            "Type of {child_label} must be omitted to match the parent definition in class {}",
                            owner.display_name()
                        )
                    };
                    return Err(self.inherited_property_error(class, message));
                }
            }
        }
        for constant in &class.declaration.constants {
            let Some(owner) = parent.inheritable_constant_owner(&constant.name) else {
                continue;
            };
            let inherited = owner
                .declaration
                .constants
                .iter()
                .find(|c| c.name == constant.name)
                .expect("constant owner");
            if constant.visibility > inherited.visibility {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot narrow inherited class constant visibility",
                    constant.value.line,
                ));
            }
            if let Some(parent_type) = &inherited.ty {
                let child_type = constant.ty.as_ref().map(|ty| class_type(ty, class));
                let parent_type = class_type(parent_type, &owner);
                if child_type.is_none()
                    || !self.type_subset(child_type.as_ref(), Some(&parent_type), class)
                {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Type of {}::{} must be compatible with {}::{} of type {}",
                            String::from_utf8_lossy(&class.name),
                            String::from_utf8_lossy(&constant.name),
                            String::from_utf8_lossy(&owner.name),
                            String::from_utf8_lossy(&constant.name),
                            inherited.ty.as_ref().unwrap()
                        ),
                        constant.value.line,
                    ));
                }
            }
        }
        for function in &class.declaration.methods {
            let name = method_name(function);
            let Some(inherited) = objects::method(parent, name) else {
                continue;
            };
            if inherited.visibility == crate::ast::Visibility::Private {
                if name.eq_ignore_ascii_case(b"__construct") && parent.method_is_final(name) {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Cannot override final constructor",
                        function.line,
                    ));
                }
                continue;
            }
            if function.is_abstract && !inherited.is_abstract {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Cannot make non abstract method {}() abstract",
                        String::from_utf8_lossy(&inherited.name)
                    ),
                    function.line,
                ));
            }
            if function.is_static != inherited.is_static {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot change staticness of inherited method",
                    function.line,
                ));
            }
            if parent.method_is_final(name) {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Cannot override final method {}()",
                        String::from_utf8_lossy(&inherited.name)
                    ),
                    function.line,
                ));
            }
            // Constructors retain their original abstract prototype, including
            // after an intermediate subclass supplies a concrete implementation.
            let inherited = if name.eq_ignore_ascii_case(b"__construct") {
                let Some(prototype) = abstract_constructor(parent) else {
                    continue;
                };
                prototype
            } else {
                inherited
            };
            if function.visibility > inherited.visibility {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot restrict inherited method visibility",
                    function.line,
                ));
            }
            if !self.compatible_method(function, &inherited, class) {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Declaration of {}() must be compatible with {}()",
                        String::from_utf8_lossy(&function.name),
                        String::from_utf8_lossy(&inherited.name)
                    ),
                    function.line,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn compatible_method(
        &self,
        child: &FunctionDecl,
        parent: &FunctionDecl,
        class: &Class,
    ) -> bool {
        if parent.returns_reference && !child.returns_reference {
            return false;
        }
        let required = |f: &FunctionDecl| {
            f.parameters
                .iter()
                .rposition(|p| p.default.is_none() && !p.variadic)
                .map_or(0, |i| i + 1)
        };
        if required(child) > required(parent) {
            return false;
        }
        let child_variadic = child.parameters.last().is_some_and(|p| p.variadic);
        let parent_variadic = parent.parameters.last().is_some_and(|p| p.variadic);
        if parent_variadic && !child_variadic {
            return false;
        }
        if !child_variadic && child.parameters.len() < parent.parameters.len() {
            return false;
        }
        for index in 0..child.parameters.len().max(parent.parameters.len()) {
            let parent_param = parent
                .parameters
                .get(index)
                .or_else(|| parent.parameters.last().filter(|p| p.variadic));
            let child_param = child
                .parameters
                .get(index)
                .or_else(|| child.parameters.last().filter(|p| p.variadic));
            if let Some(parent_param) = parent_param {
                let Some(child_param) = child_param else {
                    return false;
                };
                if parent_param.by_reference != child_param.by_reference {
                    return false;
                }
                let parent_type = self.signature_type(parent_param.ty.as_ref(), parent, class);
                let child_type = self.signature_type(child_param.ty.as_ref(), child, class);
                if !self.type_subset(parent_type.as_ref(), child_type.as_ref(), class) {
                    return false;
                }
            }
        }
        parent.return_type.is_none()
            || child.return_type.is_some()
                && self.type_subset(
                    self.signature_type(child.return_type.as_ref(), child, class)
                        .as_ref(),
                    self.signature_type(parent.return_type.as_ref(), parent, class)
                        .as_ref(),
                    class,
                )
    }

    fn signature_type(
        &self,
        ty: Option<&TypeDecl>,
        function: &FunctionDecl,
        candidate: &Class,
    ) -> Option<TypeDecl> {
        let mut ty = ty?.clone();
        let declaring = function.name.windows(2).position(|part| part == b"::")?;
        let name = &function.name[..declaring];
        let stored = self.find_class(name);
        let scope = if candidate.name.eq_ignore_ascii_case(name) {
            candidate
        } else {
            stored.as_deref()?
        };
        ty = class_type(&ty, scope);
        Some(ty)
    }

    pub(super) fn type_subset(
        &self,
        narrow: Option<&TypeDecl>,
        wide: Option<&TypeDecl>,
        class: &Class,
    ) -> bool {
        let mixed = TypeDecl {
            members: vec!["mixed".to_owned()],
        };
        let expand = |ty: &TypeDecl| TypeDecl {
            members: ty
                .members
                .iter()
                .flat_map(|m| {
                    if m == "iterable" {
                        vec!["array".to_owned(), "Traversable".to_owned()]
                    } else {
                        vec![m.clone()]
                    }
                })
                .collect(),
        };
        let narrow = expand(narrow.unwrap_or(&mixed));
        let wide = expand(wide.unwrap_or(&mixed));
        narrow.members.iter().all(|n| {
            wide.members.iter().any(|w| {
                let n = n.trim_start_matches('\\');
                let w = w.trim_start_matches('\\');
                if n.eq_ignore_ascii_case(w) || n == "never" {
                    return true;
                }
                if w == "static" {
                    return false;
                }
                if n == "static" {
                    return w == "mixed" || w == "object" || class.is_a(w.as_bytes());
                }
                if w == "mixed" && n != "void" {
                    return true;
                }
                if w == "bool" && matches!(n, "true" | "false") {
                    return true;
                }
                if w == "callable" && n.eq_ignore_ascii_case("Closure") {
                    return true;
                }
                if w == "iterable" && n == "array" {
                    return true;
                }
                let builtin = matches!(
                    n,
                    "int"
                        | "float"
                        | "string"
                        | "bool"
                        | "true"
                        | "false"
                        | "null"
                        | "array"
                        | "iterable"
                        | "callable"
                        | "object"
                        | "mixed"
                        | "void"
                );
                if !builtin && w == "object" {
                    return true;
                }
                if n.as_bytes().eq_ignore_ascii_case(&class.name) {
                    return class.is_a(w.as_bytes());
                }
                self.find_class(n.as_bytes())
                    .is_some_and(|c| c.is_a(w.as_bytes()))
            })
        })
    }
}

fn method_name(function: &FunctionDecl) -> &[u8] {
    function.name.rsplit(|b| *b == b':').next().unwrap()
}

fn abstract_constructor(class: &Class) -> Option<Rc<FunctionDecl>> {
    class
        .parent
        .as_ref()
        .and_then(|parent| abstract_constructor(parent))
        .or_else(|| {
            class
                .interfaces
                .iter()
                .find_map(|i| objects::method(i, b"__construct"))
        })
        .or_else(|| objects::method(class, b"__construct").filter(|method| method.is_abstract))
}

pub(super) fn class_type(ty: &TypeDecl, scope: &Class) -> TypeDecl {
    let mut ty = ty.clone();
    for member in &mut ty.members {
        let class = match member.as_str() {
            "self" => Some(scope),
            "parent" => scope.parent.as_deref(),
            _ => None,
        };
        if let Some(class) = class {
            *member = String::from_utf8_lossy(&class.name).into_owned();
        }
    }
    ty
}
