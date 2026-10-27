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
        let outer_parent = std::mem::replace(&mut self.class_parent, parent.clone());
        let outer_class = std::mem::replace(&mut self.class_name, name.clone());
        let mut properties = Vec::new();
        let mut static_properties = Vec::new();
        let mut constants = Vec::new();
        let mut final_constants = Vec::new();
        let mut methods: Vec<Rc<FunctionDecl>> = Vec::new();
        let mut final_methods = Vec::new();
        while !self.eat("}") {
            let mut final_method = false;
            let mut public = false;
            let mut var = false;
            let mut static_method = false;
            loop {
                let flag = if self.eat("final") {
                    &mut final_method
                } else if self.eat("public") {
                    &mut public
                } else if self.eat("var") {
                    var = true;
                    &mut public
                } else if self.eat("static") {
                    &mut static_method
                } else {
                    break;
                };
                if *flag {
                    return Err(Error::parse(
                        "Multiple access type modifiers are not allowed",
                        self.token().line,
                    ));
                }
                *flag = true;
            }
            if var
                && (static_method
                    || final_method
                    || !matches!(self.token().kind, Kind::Variable(_)))
            {
                return Err(Error::parse(
                    "Expected property after var",
                    self.token().line,
                ));
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
                if static_method
                    && [
                        b"__construct".as_slice(),
                        b"__destruct",
                        b"__clone",
                        b"__invoke",
                        b"__get",
                        b"__set",
                        b"__isset",
                        b"__unset",
                        b"__call",
                        b"__tostring",
                        b"__sleep",
                        b"__wakeup",
                        b"__serialize",
                        b"__unserialize",
                        b"__debuginfo",
                    ]
                    .iter()
                    .any(|name| method.name.eq_ignore_ascii_case(name))
                {
                    return Err(Error::parse(
                        "This magic method cannot be static",
                        method.line,
                    ));
                }
                Rc::make_mut(&mut method).is_static = static_method;
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
            } else if self.eat("const") && !static_method {
                loop {
                    let Kind::Word(constant) = self.token().kind.clone() else {
                        return Err(Error::parse(
                            "Expected class constant name",
                            self.token().line,
                        ));
                    };
                    self.pos += 1;
                    if constant.eq_ignore_ascii_case(b"class") || constant.contains(&b'\\') {
                        return Err(Error::parse(
                            "Invalid class constant name",
                            self.token().line,
                        ));
                    }
                    if constants.iter().any(|(n, _)| n == &constant) {
                        return Err(Error::parse(
                            "Cannot redefine class constant",
                            self.token().line,
                        ));
                    }
                    self.expect("=")?;
                    let value = self.expr(0)?;
                    if !value.is_constant_expression() {
                        return Err(Error::parse(
                            "Constant expression contains invalid operations",
                            value.line,
                        ));
                    }
                    if final_method {
                        final_constants.push(constant.clone());
                    }
                    constants.push((constant, value));
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(";")?;
            } else if (public || static_method) && !final_method {
                loop {
                    let Kind::Variable(property) = self.token().kind.clone() else {
                        return Err(Error::parse(
                            "Expected public property or method",
                            self.token().line,
                        ));
                    };
                    self.pos += 1;
                    if properties
                        .iter()
                        .chain(static_properties.iter())
                        .any(|(n, _)| n == &property)
                    {
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
                    if static_method {
                        static_properties.push((property, default));
                    } else {
                        properties.push((property, default));
                    }
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
        self.class_parent = outer_parent;
        Ok(Stmt::Class(Rc::new(ClassDecl {
            parent,
            is_final,
            final_methods,
            name,
            properties,
            static_properties,
            constants,
            final_constants,
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

    pub(super) fn class_name_expression(&self, value: Expr) -> ExprKind {
        if let ExprKind::Constant(name) = &value.kind {
            let name = match name.to_ascii_lowercase().as_slice() {
                b"self" if !self.class_name.is_empty() => Some(self.class_name.clone()),
                b"parent" => self
                    .class_parent
                    .as_ref()
                    .map(|name| name.strip_prefix(b"\\").unwrap_or(name).to_vec()),
                b"self" | b"static" => None,
                _ => Some(name.strip_prefix(b"\\").unwrap_or(name).to_vec()),
            };
            if let Some(name) = name {
                return ExprKind::Literal(Value::String(name));
            }
        }
        ExprKind::ClassName(Box::new(value))
    }

    pub(super) fn scoped_member(&mut self, class: Expr) -> Result<ExprKind, Error> {
        let line = self.token().line;
        let variable = matches!(self.token().kind, Kind::Variable(_)) || self.is("$");
        let mut name = self.property_name()?;
        if variable && !self.is("(") {
            name.kind = match name.kind {
                ExprKind::Variable(name) => ExprKind::Literal(Value::String(name)),
                ExprKind::DynamicVariable(inner) => {
                    return Ok(ExprKind::StaticProperty(Box::new(class), inner));
                }
                kind => kind,
            };
            Ok(ExprKind::StaticProperty(
                Box::new(class),
                Box::new(Expr {
                    kind: name.kind,
                    line,
                }),
            ))
        } else {
            Ok(ExprKind::ScopedMember(Box::new(class), Box::new(name)))
        }
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
