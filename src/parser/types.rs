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
            if matches!(bare, "self" | "parent" | "static") && lower.starts_with('\\') {
                return Err(Error::parse(
                    "Contextual class types must be unqualified",
                    line,
                ));
            }
            if bare == "parent" && self.class_parent.is_none() {
                return Err(Error::parse(
                    "Cannot use parent when current class scope has no parent",
                    line,
                ));
            }
            if matches!(bare, "self" | "parent" | "static")
                && (self.class_name.is_empty() || bare == "static" && !returns)
            {
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
            let member = if builtin || matches!(bare, "self" | "parent" | "static") {
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
                if param.promotion.is_some() {
                    return Err(Error::parse(
                        "Promoted properties cannot have implicitly nullable types",
                        default.line,
                    ));
                }
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

impl Parser {
    pub(super) fn validate_constant_value(
        &self,
        name: &[u8],
        ty: &TypeDecl,
        value: &Expr,
    ) -> Result<(), Error> {
        let Some(value) = compile_time_value(value) else {
            return Ok(());
        };
        if ty.accepts(&value) || ty.contains("float") && matches!(value, Value::Int(_)) {
            return Ok(());
        }
        Err(Error::parse(
            format!(
                "Cannot use value for class constant {}::{} of type {ty}",
                String::from_utf8_lossy(&self.class_name),
                String::from_utf8_lossy(name)
            ),
            self.token().line,
        ))
    }
}

impl Parser {
    pub(super) fn validate_property_default(
        &self,
        name: &[u8],
        ty: &TypeDecl,
        value: &Expr,
    ) -> Result<(), Error> {
        let Some(value) = compile_time_value(value) else {
            return Ok(());
        };
        if ty.accepts(&value) || ty.contains("float") && matches!(value, Value::Int(_)) {
            return Ok(());
        }
        Err(Error::parse(
            format!(
                "Cannot use default value for property {}::${} of type {ty}",
                String::from_utf8_lossy(&self.class_name),
                String::from_utf8_lossy(name)
            ),
            self.token().line,
        ))
    }
}

// Callable and class-constant values remain lazy, even when their types appear
// obvious. Only expressions the compiler can fully resolve are checked here.
fn compile_time_value(expr: &Expr) -> Option<Value> {
    let eligible = match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Constant(_) => true,
        ExprKind::Group(e) | ExprKind::Unary(_, e) | ExprKind::Cast(_, e) => {
            compile_time_value(e).is_some()
        }
        ExprKind::Binary(_, a, b) | ExprKind::Index(a, Some(b)) => {
            compile_time_value(a).is_some() && compile_time_value(b).is_some()
        }
        ExprKind::Conditional(a, b, c) => {
            let value = compile_time_value(a)?;
            if value.to_bool(expr.line, &mut Vec::new()) {
                b.as_ref().is_none_or(|b| compile_time_value(b).is_some())
            } else {
                compile_time_value(c).is_some()
            }
        }
        ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
            elements.iter().all(|element| match element {
                ArrayElement::Entry(k, v) => {
                    k.as_ref().is_none_or(|k| compile_time_value(k).is_some())
                        && compile_time_value(v).is_some()
                }
                ArrayElement::Unpack(v) => compile_time_value(v).is_some(),
                ArrayElement::Hole => false,
            })
        }
        _ => false,
    };
    eligible
        .then(|| crate::Runtime::constant_without_diagnostics(expr))
        .flatten()
}
