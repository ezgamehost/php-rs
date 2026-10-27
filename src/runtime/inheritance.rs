use super::*;
use crate::{ast::TypeDecl, object::Class};

impl Runtime {
    pub(super) fn validate_overrides(&self, class: &Class) -> Result<(), Error> {
        let Some(parent) = &class.parent else {
            return Ok(());
        };
        for function in &class.declaration.methods {
            let name = method_name(function);
            let Some(inherited) = objects::method(parent, name) else {
                continue;
            };
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
                if !self.type_subset(parent_param.ty.as_ref(), child_param.ty.as_ref(), class) {
                    return false;
                }
            }
        }
        parent.return_type.is_none()
            || child.return_type.is_some()
                && self.type_subset(
                    child.return_type.as_ref(),
                    parent.return_type.as_ref(),
                    class,
                )
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
        let narrow = narrow.unwrap_or(&mixed);
        let wide = wide.unwrap_or(&mixed);
        narrow.members.iter().all(|n| {
            wide.members.iter().any(|w| {
                let n = n.trim_start_matches('\\');
                let w = w.trim_start_matches('\\');
                if n.eq_ignore_ascii_case(w) || n == "never" {
                    return true;
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
