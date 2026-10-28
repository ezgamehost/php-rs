use super::*;
use crate::ast::{ClassConstant, ClassDecl, PropertyDecl};

impl Parser {
    pub(super) fn class_declaration(
        &mut self,
        is_final: bool,
        is_readonly: bool,
        is_abstract: bool,
        is_interface: bool,
    ) -> Result<Stmt, Error> {
        let line = self.token().line;
        if is_abstract && is_final {
            return Err(Error::parse(
                "A class cannot be both abstract and final",
                line,
            ));
        }
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
        let parent = if !is_interface && self.eat("extends") {
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
        let mut interfaces = Vec::new();
        if self.eat(if is_interface {
            "extends"
        } else {
            "implements"
        }) {
            loop {
                let Kind::Word(name) = self.token().kind.clone() else {
                    return Err(Error::parse("Expected interface name", self.token().line));
                };
                interfaces.push(name);
                self.pos += 1;
                if !self.eat(",") {
                    break;
                }
            }
        }
        self.expect("{")?;
        let outer_parent = std::mem::replace(&mut self.class_parent, parent.clone());
        let outer_class = std::mem::replace(&mut self.class_name, name.clone());
        let mut properties: Vec<PropertyDecl> = Vec::new();
        let mut static_properties: Vec<PropertyDecl> = Vec::new();
        let mut constants: Vec<ClassConstant> = Vec::new();
        let mut methods: Vec<Rc<FunctionDecl>> = Vec::new();
        let mut final_methods = Vec::new();
        while !self.eat("}") {
            let mut final_method = false;
            let mut abstract_method = false;
            let mut public = false;
            let mut visibility = Visibility::Public;
            let mut set_visibility = None;
            let mut var = false;
            let mut static_method = false;
            let mut readonly_property = false;
            loop {
                if self.is("public") || self.is("protected") || self.is("private") {
                    let access = if self.eat("public") {
                        Visibility::Public
                    } else if self.eat("protected") {
                        Visibility::Protected
                    } else {
                        self.expect("private")?;
                        Visibility::Private
                    };
                    if self.eat("(") {
                        self.expect("set")?;
                        self.expect(")")?;
                        if set_visibility.replace(access).is_some() {
                            return Err(Error::parse(
                                "Multiple set visibility modifiers are not allowed",
                                self.token().line,
                            ));
                        }
                    } else {
                        if public {
                            return Err(Error::parse(
                                "Multiple access type modifiers are not allowed",
                                self.token().line,
                            ));
                        }
                        visibility = access;
                        public = true;
                    }
                    continue;
                }
                let flag = if self.eat("final") {
                    &mut final_method
                } else if self.eat("abstract") {
                    &mut abstract_method
                } else if self.eat("var") {
                    var = true;
                    &mut public
                } else if self.eat("static") {
                    &mut static_method
                } else if self.eat("readonly") {
                    &mut readonly_property
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
            if set_visibility.is_some()
                && (!static_method || self.is("function") || self.is("const") || is_interface)
            {
                return Err(Error::parse(
                    "Set visibility requires a static property",
                    self.token().line,
                ));
            }
            if visibility != Visibility::Public
                && ((!self.is("function") && !self.is("const") && !static_method) || is_interface)
            {
                return Err(Error::parse(
                    "Invalid non-public member declaration",
                    self.token().line,
                ));
            }
            if abstract_method && visibility == Visibility::Private {
                return Err(Error::parse(
                    "Abstract methods cannot be private",
                    self.token().line,
                ));
            }
            if is_interface
                && (abstract_method
                    || (final_method && self.is("function"))
                    || (!self.is("function") && !self.is("const")))
            {
                return Err(Error::parse(
                    "Invalid interface member declaration",
                    self.token().line,
                ));
            }
            if abstract_method && (final_method || var || !self.is("function")) {
                return Err(Error::parse(
                    "Invalid abstract method declaration",
                    self.token().line,
                ));
            }
            if var
                && (static_method
                    || final_method
                    || readonly_property
                    || self.is("function")
                    || self.is("const"))
            {
                return Err(Error::parse(
                    "Expected property after var",
                    self.token().line,
                ));
            }
            if readonly_property && (self.is("function") || self.is("const")) {
                return Err(Error::parse(
                    "Readonly modifier is only allowed on properties",
                    self.token().line,
                ));
            }
            if self.eat("function") {
                let abstract_method = abstract_method || is_interface;
                if abstract_method && !is_abstract {
                    return Err(Error::parse(
                        "A class with abstract methods must be abstract",
                        self.token().line,
                    ));
                }
                let Stmt::Function(mut method) = self.function(true, abstract_method)? else {
                    unreachable!()
                };
                if abstract_method && method.parameters.iter().any(|p| p.promotion.is_some()) {
                    return Err(Error::parse(
                        "Cannot declare promoted property in an abstract constructor",
                        method.line,
                    ));
                }
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
                if method.name.eq_ignore_ascii_case(b"__clone")
                    && (method.parameters.iter().any(|param| !param.variadic)
                        || method
                            .return_type
                            .as_ref()
                            .is_some_and(|ty| !ty.contains("void") && !ty.contains("never")))
                {
                    return Err(Error::parse(
                        "Method __clone() has an invalid signature",
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
                Rc::make_mut(&mut method).visibility = visibility;
                if final_method
                    && visibility == Visibility::Private
                    && !method.name.eq_ignore_ascii_case(b"__construct")
                {
                    self.warnings.push(format!("Private methods cannot be final as they are never overridden by other classes on line {}", method.line));
                    final_method = false;
                }
                if visibility != Visibility::Public
                    && [
                        b"__invoke".as_slice(),
                        b"__get",
                        b"__set",
                        b"__isset",
                        b"__unset",
                        b"__call",
                        b"__callstatic",
                        b"__tostring",
                        b"__sleep",
                        b"__wakeup",
                        b"__serialize",
                        b"__unserialize",
                        b"__set_state",
                        b"__debuginfo",
                    ]
                    .iter()
                    .any(|name| method.name.eq_ignore_ascii_case(name))
                {
                    self.warnings.push(format!(
                        "The magic method {}::{}() must have public visibility on line {}",
                        String::from_utf8_lossy(&name),
                        String::from_utf8_lossy(&method.name),
                        method.line
                    ));
                }
                if final_method {
                    final_methods.push(method.name.to_ascii_lowercase());
                }
                self.promote_properties(
                    &mut method,
                    &mut properties,
                    &static_properties,
                    is_readonly,
                )?;
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
                if final_method && visibility == Visibility::Private {
                    return Err(Error::parse(
                        "Private constants cannot be final",
                        self.token().line,
                    ));
                }
                // An untyped constant has a name immediately followed by '='.
                let ty = if self
                    .tokens
                    .get(self.pos + 1)
                    .is_some_and(|t| matches!(&t.kind, Kind::Symbol(s) if s == "="))
                {
                    None
                } else {
                    let ty = self.type_declaration(true)?;
                    if ["callable", "void", "never"]
                        .iter()
                        .any(|name| ty.contains(name))
                    {
                        return Err(Error::parse(
                            format!("Class constants cannot have type {ty}"),
                            self.token().line,
                        ));
                    }
                    Some(ty)
                };
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
                    if constants.iter().any(|c| c.name == constant) {
                        return Err(Error::parse(
                            "Cannot redefine class constant",
                            self.token().line,
                        ));
                    }
                    self.expect("=")?;
                    let mut value = self.expr(0)?;
                    if !value.is_class_initializer() {
                        return Err(Error::parse(
                            "Constant expression contains invalid operations",
                            value.line,
                        ));
                    }
                    if let Some(ty) = &ty {
                        logical_constants::initializer(&mut value, &mut self.warnings);
                        self.validate_constant_value(&constant, ty, &value)?;
                    }
                    constants.push(ClassConstant {
                        visibility,
                        name: constant,
                        value,
                        ty: ty.clone(),
                        is_final: final_method,
                    });
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect(";")?;
            } else if (public || static_method || readonly_property)
                && (!final_method || static_method)
            {
                if final_method && visibility == Visibility::Private {
                    return Err(Error::parse(
                        "Property cannot be both final and private",
                        self.token().line,
                    ));
                }
                let ty = if matches!(self.token().kind, Kind::Variable(_)) {
                    None
                } else {
                    let ty = self.type_declaration(false)?;
                    if ty.contains("callable") {
                        return Err(Error::parse(
                            "Property cannot have type callable",
                            self.token().line,
                        ));
                    }
                    Some(ty)
                };
                if set_visibility.is_some() && ty.is_none() {
                    return Err(Error::parse(
                        "Properties with set visibility must have a type",
                        self.token().line,
                    ));
                }
                let set_visibility = set_visibility.unwrap_or(visibility);
                if set_visibility < visibility {
                    return Err(Error::parse(
                        "Set visibility must not be wider than read visibility",
                        self.token().line,
                    ));
                }
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
                        .any(|p| p.name == property)
                    {
                        return Err(Error::parse("Cannot redeclare property", self.token().line));
                    }
                    let mut default = if self.eat("=") {
                        Some(self.expr(0)?)
                    } else {
                        None
                    };
                    if default.as_ref().is_some_and(|e| !e.is_class_initializer()) {
                        return Err(Error::parse(
                            "Constant expression contains invalid operations",
                            self.token().line,
                        ));
                    }
                    if let (Some(ty), Some(default)) = (&ty, &mut default) {
                        logical_constants::initializer(default, &mut self.warnings);
                        self.validate_property_default(&property, ty, default)?;
                    }
                    let readonly = is_readonly || readonly_property;
                    if readonly && (ty.is_none() || default.is_some() || static_method) {
                        return Err(Error::parse(
                            "Readonly properties must be typed, non-static, and have no default",
                            self.token().line,
                        ));
                    }
                    let property = PropertyDecl {
                        visibility,
                        set_visibility,
                        is_final: final_method
                            || (set_visibility == Visibility::Private
                                && visibility != Visibility::Private),
                        is_readonly: readonly,
                        name: property,
                        default,
                        ty: ty.clone(),
                    };
                    if static_method {
                        static_properties.push(property);
                    } else {
                        properties.push(property);
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
            is_interface,
            interfaces,
            parent,
            is_abstract,
            is_final,
            is_readonly,
            final_methods,
            name,
            properties,
            static_properties,
            constants,
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
                } else if self.is("->") || self.is("?->") {
                    let nullsafe = self.eat("?->");
                    if !nullsafe {
                        self.expect("->")?;
                    }
                    let name = self.property_name()?;
                    value = Expr {
                        kind: if nullsafe {
                            ExprKind::NullsafeProperty(Box::new(value), Box::new(name))
                        } else {
                            ExprKind::Property(Box::new(value), Box::new(name))
                        },
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
                } else if self.is("->") || self.is("?->") {
                    let nullsafe = self.eat("?->");
                    if !nullsafe {
                        self.expect("->")?;
                    }
                    let name = self.property_name()?;
                    target = Expr {
                        kind: if nullsafe {
                            ExprKind::NullsafeProperty(Box::new(target), Box::new(name))
                        } else {
                            ExprKind::Property(Box::new(target), Box::new(name))
                        },
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
                return ExprKind::Literal(Value::String(name.into()));
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
                ExprKind::Variable(name) => ExprKind::Literal(Value::String(name.into())),
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
            Kind::Word(name) => ExprKind::Literal(Value::String(name.into())),
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
