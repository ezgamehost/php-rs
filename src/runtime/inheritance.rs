use super::*;
use crate::{ast::TypeDecl, object::Class};

impl Runtime {
    // A comparison involving unresolved class names cannot be early-bound.
    // The candidate's own type is known in its declarations, but not yet in
    // unresolved declarations inherited from its parent.
    pub(super) fn property_variance_unresolved(&self, class: &Class) -> bool {
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
                let Some(parent_property) = inherited.iter().find(|p| p.name == property.name)
                else {
                    continue;
                };
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

    pub(super) fn validate_overrides(&self, class: &Class) -> Result<(), Error> {
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
                let Some(parent_property) = inherited.iter().find(|p| p.name == property.name)
                else {
                    continue;
                };
                let owner = if is_static {
                    parent.static_owner(&property.name)
                } else {
                    parent.property_owner(&property.name)
                }
                .expect("property owner");
                if property.is_readonly != parent_property.is_readonly {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Cannot change readonly status of an inherited property",
                        class.declaration.line,
                    ));
                }
                let child_type = property.ty.as_ref().map(|ty| class_type(ty, class));
                let parent_type = parent_property.ty.as_ref().map(|ty| class_type(ty, &owner));
                if child_type.is_some() != parent_type.is_some()
                    || !self.type_subset(child_type.as_ref(), parent_type.as_ref(), class)
                    || !self.type_subset(parent_type.as_ref(), child_type.as_ref(), class)
                {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Type of {}::${} must match its inherited property type",
                            String::from_utf8_lossy(&class.name),
                            String::from_utf8_lossy(&property.name)
                        ),
                        class.declaration.line,
                    ));
                }
            }
        }
        for constant in &class.declaration.constants {
            let Some(owner) = parent.constant_owner(&constant.name) else {
                continue;
            };
            let inherited = owner
                .declaration
                .constants
                .iter()
                .find(|c| c.name == constant.name)
                .expect("constant owner");
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
            // Constructors need not have compatible signatures unless an
            // interface or abstract declaration requires one (not yet parsed).
            if name.eq_ignore_ascii_case(b"__construct") {
                continue;
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

    fn compatible_method(
        &self,
        child: &FunctionDecl,
        parent: &FunctionDecl,
        class: &Class,
    ) -> bool {
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

    fn type_subset(
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
