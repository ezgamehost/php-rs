use super::*;
use crate::ast::{Capture, LambdaDecl};

impl Parser {
    pub(super) fn lambda(
        &mut self,
        arrow: bool,
        is_static: bool,
        line: usize,
    ) -> Result<ExprKind, Error> {
        let name = format!(
            "{{closure:{}:{line}}}",
            String::from_utf8_lossy(&self.source.name)
        )
        .into_bytes();
        let outer_name = std::mem::replace(&mut self.function_name, name.clone());
        let mut parameters = self.function_parameters()?;
        let mut captures = Vec::<Capture>::new();
        if !arrow && self.eat("use") {
            self.expect("(")?;
            loop {
                let by_reference = self.eat("&");
                let Kind::Variable(name) = self.token().kind.clone() else {
                    return Err(Error::parse("Expected lexical variable", self.token().line));
                };
                self.pos += 1;
                if is_auto_global(&name) || name == b"this" {
                    return Err(Error::parse(
                        "Cannot use auto-global or $this as lexical variable",
                        line,
                    ));
                }
                if captures.iter().any(|c| c.name == name) {
                    return Err(Error::parse(
                        "Cannot use variable twice in lexical use list",
                        line,
                    ));
                }
                if parameters.iter().any(|p| p.name == name) {
                    return Err(Error::parse(
                        "Cannot use lexical variable as a parameter name",
                        line,
                    ));
                }
                captures.push(Capture { name, by_reference });
                if !self.eat(",") || self.is(")") {
                    break;
                }
            }
            self.expect(")")?;
        }
        let return_type = if self.eat(":") {
            Some(self.type_declaration(true)?)
        } else {
            None
        };
        self.validate_defaults(&name, &mut parameters, line)?;
        if let Some(last) = parameters
            .iter()
            .rposition(|p| p.default.is_none() && !p.variadic)
        {
            for param in &parameters[..last] {
                if param.default.is_some() && !param.implicit_nullable {
                    self.warnings.push(format!("Deprecated: Optional parameter ${} declared before required parameter is implicitly treated as required on line {line}", String::from_utf8_lossy(&param.name)));
                }
            }
        }
        let outer_return = std::mem::replace(&mut self.return_type, return_type.clone());
        let outer_statics = std::mem::replace(
            &mut self.static_names,
            captures.iter().map(|c| c.name.clone()).collect(),
        );
        let outer_depth = std::mem::replace(&mut self.control_depth, 0);
        let body = if arrow {
            self.expect("=>")?;
            let result = self.expr(0)?;
            if return_type.as_ref().is_some_and(|t| t.contains("void")) {
                return Err(Error::parse(
                    "A void function must not return a value",
                    line,
                ));
            }
            collect_captures(&result, &mut captures);
            captures.retain(|c| !parameters.iter().any(|p| p.name == c.name));
            vec![Stmt::Return(Some(result))]
        } else {
            self.expect("{")?;
            let mut body = Vec::new();
            while !self.eat("}") {
                if self.token().kind == Kind::End {
                    return Err(Error::parse("Unclosed function body", line));
                }
                body.push(self.statement()?);
            }
            body
        };
        self.function_name = outer_name;
        self.return_type = outer_return;
        self.static_names = outer_statics;
        self.control_depth = outer_depth;
        let function = Rc::new(FunctionDecl {
            name,
            parameters,
            body,
            line,
            return_type,
            strict_types: self.strict_types,
            source: self.source.clone(),
        });
        targets::validate(&[Stmt::Function(function.clone())])?;
        jumps::validate(&function.body)?;
        Ok(ExprKind::Lambda(Rc::new(LambdaDecl {
            function,
            captures,
            arrow,
            is_static,
        })))
    }
}

fn is_auto_global(name: &[u8]) -> bool {
    matches!(
        name,
        b"GLOBALS"
            | b"_SERVER"
            | b"_GET"
            | b"_POST"
            | b"_FILES"
            | b"_COOKIE"
            | b"_SESSION"
            | b"_REQUEST"
            | b"_ENV"
    )
}

fn capture(name: &[u8], captures: &mut Vec<Capture>) {
    if !is_auto_global(name) && !captures.iter().any(|c| c.name == name) {
        captures.push(Capture {
            name: name.to_vec(),
            by_reference: false,
        });
    }
}

fn collect_captures(expr: &Expr, captures: &mut Vec<Capture>) {
    match &expr.kind {
        ExprKind::Variable(name) => capture(name, captures),
        ExprKind::Lambda(lambda) => {
            for c in &lambda.captures {
                capture(&c.name, captures);
            }
        }
        ExprKind::Literal(_) | ExprKind::Constant(_) => {}
        ExprKind::Group(e)
        | ExprKind::DynamicVariable(e)
        | ExprKind::Unary(_, e)
        | ExprKind::Cast(_, e)
        | ExprKind::Callable(e)
        | ExprKind::Eval(e)
        | ExprKind::Include(_, e)
        | ExprKind::Empty(e)
        | ExprKind::Print(e)
        | ExprKind::Increment(e, _, _) => collect_captures(e, captures),
        ExprKind::Binary(_, a, b) | ExprKind::Assign(a, _, b) => {
            collect_captures(a, captures);
            collect_captures(b, captures);
        }
        ExprKind::Array(elements)
        | ExprKind::LongArray(elements)
        | ExprKind::List(elements)
        | ExprKind::Destructure(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Entry(key, value) => {
                        if let Some(key) = key {
                            collect_captures(key, captures);
                        }
                        collect_captures(value, captures);
                    }
                    ArrayElement::Unpack(e) => collect_captures(e, captures),
                    ArrayElement::Hole => {}
                }
            }
        }
        ExprKind::Index(base, key) => {
            collect_captures(base, captures);
            if let Some(key) = key {
                collect_captures(key, captures);
            }
        }
        ExprKind::Call(callee, args) => {
            collect_captures(callee, captures);
            for arg in args {
                let (Argument::Positional(e) | Argument::Named(_, e) | Argument::Unpack(e)) = arg;
                collect_captures(e, captures);
            }
        }
        ExprKind::Interpolated(values) | ExprKind::Isset(values) => {
            for value in values {
                collect_captures(value, captures);
            }
        }
        ExprKind::Conditional(condition, yes, no) => {
            collect_captures(condition, captures);
            if let Some(yes) = yes {
                collect_captures(yes, captures);
            }
            collect_captures(no, captures);
        }
        ExprKind::Match(subject, arms) => {
            collect_captures(subject, captures);
            for arm in arms {
                for condition in &arm.conditions {
                    collect_captures(condition, captures);
                }
                collect_captures(&arm.result, captures);
            }
        }
    }
}
