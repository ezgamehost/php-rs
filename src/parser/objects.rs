use super::*;
use crate::ast::ClassDecl;

impl Parser {
    pub(super) fn class_declaration(&mut self, is_final: bool) -> Result<Stmt, Error> {
        let line = self.token().line;
        let Kind::Word(name) = self.token().kind.clone() else {
            return Err(Error::parse("Expected class name", line));
        };
        if name.contains(&b'\\')
            || [
                "self", "parent", "static", "int", "float", "string", "bool", "null", "true",
                "false", "object", "mixed", "never", "void", "array", "callable", "iterable",
            ]
            .iter()
            .any(|word| name.eq_ignore_ascii_case(word.as_bytes()))
        {
            return Err(Error::parse("Invalid class name", line));
        }
        self.label_name()?;
        if name == b"_" {
            self.warnings.push(format!(
                r#"Deprecated: Using "_" as a class name is deprecated on line {line}"#
            ));
        }
        let parent = if self.eat("extends") {
            let Kind::Word(name) = self.token().kind.clone() else {
                return Err(Error::parse(
                    "Expected parent class name",
                    self.token().line,
                ));
            };
            self.pos += 1;
            Some(name)
        } else {
            None
        };
        self.expect("{")?;
        let outer_class = std::mem::replace(&mut self.class_name, name.clone());
        let mut properties = Vec::new();
        let mut methods: Vec<Rc<FunctionDecl>> = Vec::new();
        let mut final_methods = Vec::new();
        while !self.eat("}") {
            let mut final_method = self.eat("final");
            let public = self.eat("public") || self.eat("var");
            if !final_method {
                final_method = self.eat("final");
            }
            if self.eat("function") {
                let Stmt::Function(mut method) = self.function(true)? else {
                    unreachable!()
                };
                if methods.iter().any(|m| {
                    m.name
                        .rsplit(|b| *b == b':')
                        .next()
                        .is_some_and(|n| n.eq_ignore_ascii_case(&method.name))
                }) {
                    return Err(Error::parse("Cannot redeclare method", method.line));
                }
                if method.name.eq_ignore_ascii_case(b"__construct") && method.return_type.is_some()
                {
                    return Err(Error::parse(
                        "Method __construct() cannot declare a return type",
                        method.line,
                    ));
                }
                if final_method {
                    final_methods.push(method.name.to_ascii_lowercase());
                }
                // Validate each method as an independent function/label scope.
                targets::validate(&[Stmt::Function(method.clone())])?;
                jumps::validate(&method.body)?;
                let function = Rc::make_mut(&mut method);
                let mut qualified = name.clone();
                qualified.extend(b"::");
                qualified.extend(&function.name);
                function.name = qualified;
                methods.push(method);
            } else if public && !final_method {
                loop {
                    let Kind::Variable(property) = self.token().kind.clone() else {
                        return Err(Error::parse(
                            "Expected public property or method",
                            self.token().line,
                        ));
                    };
                    self.pos += 1;
                    if properties.iter().any(|(n, _)| n == &property) {
                        return Err(Error::parse("Cannot redeclare property", self.token().line));
                    }
                    let default = if self.eat("=") {
                        Some(self.expr(0)?)
                    } else {
                        None
                    };
                    if default
                        .as_ref()
                        .is_some_and(|e| !e.is_constant_expression())
                    {
                        return Err(Error::parse(
                            "Constant expression contains invalid operations",
                            self.token().line,
                        ));
                    }
                    properties.push((property, default));
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(";")?;
            } else {
                return Err(Error::parse(
                    "Expected public property or method",
                    self.token().line,
                ));
            }
        }
        self.class_name = outer_class;
        Ok(Stmt::Class(Rc::new(ClassDecl {
            parent,
            is_final,
            final_methods,
            name,
            properties,
            methods,
            line,
        })))
    }

    pub(super) fn new_expression(&mut self) -> Result<ExprKind, Error> {
        let line = self.token().line;
        let class = if let Kind::Word(name) = self.token().kind.clone() {
            self.pos += 1;
            Expr {
                kind: ExprKind::Constant(name),
                line,
            }
        } else {
            let mut value = self.expr(24)?;
            loop {
                if self.eat("[") {
                    let key = self.expr(0)?;
                    self.expect("]")?;
                    value = Expr {
                        kind: ExprKind::Index(Box::new(value), Some(Box::new(key))),
                        line,
                    };
                } else if self.eat("->") {
                    let name = self.property_name()?;
                    value = Expr {
                        kind: ExprKind::Property(Box::new(value), Box::new(name)),
                        line,
                    };
                } else {
                    break;
                }
            }
            value
        };
        let args = if self.eat("(") {
            self.arguments()?
        } else {
            Vec::new()
        };
        Ok(ExprKind::New(Box::new(class), args))
    }

    pub(super) fn instanceof_target(&mut self) -> Result<Expr, Error> {
        let line = self.token().line;
        let mut target = match self.token().kind.clone() {
            Kind::Word(name)
                if ![b"new".as_slice(), b"clone", b"function", b"fn"]
                    .iter()
                    .any(|n| name.eq_ignore_ascii_case(n)) =>
            {
                self.pos += 1;
                Expr {
                    kind: ExprKind::Constant(name),
                    line,
                }
            }
            Kind::Variable(_) => self.expr(24)?,
            Kind::Symbol(s) if s == "$" || s == "(" => self.expr(24)?,
            _ => return Err(Error::parse("Expected class name", line)),
        };
        if !matches!(target.kind, ExprKind::Constant(_)) {
            loop {
                if self.eat("[") {
                    let key = self.expr(0)?;
                    self.expect("]")?;
                    target = Expr {
                        kind: ExprKind::Index(Box::new(target), Some(Box::new(key))),
                        line,
                    };
                } else if self.eat("->") {
                    let name = self.property_name()?;
                    target = Expr {
                        kind: ExprKind::Property(Box::new(target), Box::new(name)),
                        line,
                    };
                } else {
                    break;
                }
            }
        }
        if self.is("(") {
            return Err(Error::parse(
                "Unexpected '(' after class name",
                self.token().line,
            ));
        }
        let mut inner = &target;
        while let ExprKind::Group(value) = &inner.kind {
            inner = value;
        }
        if matches!(
            inner.kind,
            ExprKind::Literal(Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_))
                | ExprKind::Array(_)
                | ExprKind::LongArray(_)
        ) {
            return Err(Error::parse("Illegal class name", line));
        }
        Ok(target)
    }

    pub(super) fn property_name(&mut self) -> Result<Expr, Error> {
        let line = self.token().line;
        let token = self.token().clone();
        self.pos += 1;
        let kind = match token.kind {
            Kind::Word(name) => ExprKind::Literal(Value::String(name)),
            Kind::Variable(name) => ExprKind::Variable(name),
            Kind::Symbol(s) if s == "{" => {
                let value = self.expr(0)?;
                self.expect("}")?;
                return Ok(value);
            }
            Kind::Symbol(s) if s == "$" => self.dynamic_variable()?,
            _ => return Err(Error::parse("Expected property name", line)),
        };
        Ok(Expr { kind, line })
    }
}
