use super::*;

impl Parser {
    pub(super) fn strict_declaration(&mut self, first: bool) -> Result<(), Error> {
        let line = self.token().line;
        if !first {
            return Err(Error::parse(
                "strict_types declaration must be the very first statement in the script",
                line,
            ));
        }
        self.expect("(")?;
        loop {
            self.expect("strict_types")?;
            self.expect("=")?;
            let expr = self.expr(0)?;
            let mut literal = &expr;
            while let ExprKind::Group(inner) = &literal.kind {
                literal = inner;
            }
            match literal.kind {
                ExprKind::Literal(Value::Int(0)) => {}
                ExprKind::Literal(Value::Int(1)) => self.strict_types = true,
                _ => {
                    return Err(Error::parse(
                        "strict_types declaration must have literal 0 or 1 as its value",
                        line,
                    ));
                }
            }
            if !self.eat(",") {
                break;
            }
        }
        self.expect(")")?;
        self.expect(";")?;
        Ok(())
    }

    pub(super) fn type_declaration(&mut self, returns: bool) -> Result<TypeDecl, Error> {
        let line = self.token().line;
        let nullable = self.eat("?");
        let mut members = Vec::<String>::new();
        loop {
            let Kind::Word(name) = self.token().kind.clone() else {
                return Err(Error::parse("Expected type name", self.token().line));
            };
            self.pos += 1;
            let original = String::from_utf8_lossy(&name).into_owned();
            let lower = original.to_ascii_lowercase();
            let bare = lower.trim_start_matches('\\');
            let builtin = matches!(
                bare,
                "int"
                    | "float"
                    | "bool"
                    | "string"
                    | "array"
                    | "iterable"
                    | "callable"
                    | "object"
                    | "mixed"
                    | "null"
                    | "true"
                    | "false"
                    | "void"
                    | "never"
            );
            if builtin && lower.starts_with('\\') {
                return Err(Error::parse(
                    format!("Type declaration '{bare}' must be unqualified"),
                    line,
                ));
            }
            if matches!(bare, "self" | "parent" | "static") {
                return Err(Error::parse(
                    format!("Cannot use {bare} outside a class"),
                    line,
                ));
            }
            if !returns && matches!(bare, "void" | "never") {
                return Err(Error::parse(
                    format!("{bare} cannot be used as a parameter type"),
                    line,
                ));
            }
            if matches!(
                lower.as_str(),
                "integer" | "boolean" | "double" | "real" | "resource"
            ) {
                self.warnings.push(format!(
                    "\"{original}\" will be interpreted as a class name on line {line}"
                ));
            }
            let member = if builtin {
                lower
            } else {
                original.trim_start_matches('\\').to_owned()
            };
            if members
                .iter()
                .any(|existing| existing.eq_ignore_ascii_case(&member))
            {
                return Err(Error::parse(
                    format!("Duplicate type {member} is redundant"),
                    line,
                ));
            }
            members.push(member);
            if !self.eat("|") {
                break;
            }
            if nullable {
                return Err(Error::parse(
                    "Nullable shorthand cannot be combined with union types",
                    line,
                ));
            }
        }
        let mut ty = TypeDecl { members };
        if nullable {
            if ["mixed", "null", "void", "never"]
                .iter()
                .any(|name| ty.contains(name))
            {
                return Err(Error::parse(
                    format!("{ty} cannot be marked as nullable"),
                    line,
                ));
            }
            ty.members.push("null".into());
        }
        if ty.members.len() > 1 {
            if ["mixed", "void", "never"]
                .iter()
                .any(|name| ty.contains(name))
            {
                return Err(Error::parse(
                    "mixed, void and never can only be used as standalone types",
                    line,
                ));
            }
            if (ty.contains("bool") && (ty.contains("true") || ty.contains("false")))
                || (ty.contains("true") && ty.contains("false"))
                || (ty.contains("iterable")
                    && (ty.contains("array")
                        || ty
                            .members
                            .iter()
                            .any(|m| m.eq_ignore_ascii_case("Traversable"))))
                || (ty.contains("object")
                    && ty.members.iter().any(|m| {
                        !matches!(
                            m.as_str(),
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
                        )
                    }))
            {
                return Err(Error::parse("Union contains redundant types", line));
            }
        }
        Ok(ty)
    }

    pub(super) fn validate_defaults(
        &mut self,
        function: &[u8],
        parameters: &mut [Parameter],
        line: usize,
    ) -> Result<(), Error> {
        for param in parameters {
            let (Some(ty), Some(default)) = (&mut param.ty, &param.default) else {
                continue;
            };
            // Unresolved constants are checked when the default is evaluated at a call.
            let Some(value) = crate::Runtime::constant_default(default) else {
                continue;
            };
            if matches!(value, Value::Null) && !ty.accepts(&value) {
                param.implicit_nullable = true;
                ty.members.push("null".into());
                self.warnings.push(format!("Deprecated: {}(): Implicitly marking parameter ${} as nullable is deprecated, the explicit nullable type must be used instead on line {line}", String::from_utf8_lossy(function), String::from_utf8_lossy(&param.name)));
            } else if !ty.accepts(&value)
                && !(ty.contains("float") && matches!(value, Value::Int(_)))
            {
                return Err(Error::parse(
                    format!(
                        "Cannot use default value for parameter ${} of type {ty}",
                        String::from_utf8_lossy(&param.name)
                    ),
                    default.line,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn validate_return(&self, value: Option<&Expr>) -> Result<(), Error> {
        let Some(ty) = &self.return_type else {
            return Ok(());
        };
        let line = value.map_or(self.token().line, |v| v.line);
        if ty.contains("never") {
            return Err(Error::parse(
                "A never-returning function must not return",
                line,
            ));
        }
        if ty.contains("void") {
            if value.is_some() {
                return Err(Error::parse(
                    "A void function must not return a value",
                    line,
                ));
            }
        } else if value.is_none() {
            return Err(Error::parse(
                "A function with return type must return a value",
                line,
            ));
        }
        Ok(())
    }
}
