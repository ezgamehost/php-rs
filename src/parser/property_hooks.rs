use super::*;
use crate::ast::{PropertyDecl, PropertyHooks};

impl Parser {
    pub(super) fn property_hooks(
        &mut self,
        property: &PropertyDecl,
        is_interface: bool,
    ) -> Result<PropertyHooks, Error> {
        self.expect("{")?;
        let mut hooks = PropertyHooks {
            get_final: false,
            set_final: false,
            get: None,
            set: None,
            is_virtual: true,
        };
        let label = format!(
            "{}::${}",
            String::from_utf8_lossy(&self.class_name),
            String::from_utf8_lossy(&property.name)
        );
        while !self.eat("}") {
            let line = self.token().line;
            let doc_comment = self.token().doc_comment.clone();
            let final_hook = self.eat("final");
            if final_hook && property.visibility == Visibility::Private {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    "Property hook cannot be both final and private",
                    line,
                ));
            }
            for modifier in [
                "public",
                "protected",
                "private",
                "static",
                "abstract",
                "readonly",
            ] {
                if self.is(modifier) {
                    return Err(Error::new(
                        crate::ErrorKind::CompileError,
                        format!("Cannot use the {modifier} modifier on a property hook"),
                        line,
                    ));
                }
            }
            let returns_reference = self.eat("&");
            let Kind::Word(kind) = self.token().kind.clone() else {
                return Err(Error::parse("Expected property hook", line));
            };
            self.pos += 1;
            let kind = kind.to_ascii_lowercase();
            let get = kind == b"get";
            if !get && kind != b"set" {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    format!(
                        "Unknown hook \"{}\" for property {label}, expected \"get\" or \"set\"",
                        String::from_utf8_lossy(&kind)
                    ),
                    line,
                ));
            }
            if (get && hooks.get.is_some()) || (!get && hooks.set.is_some()) {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    format!(
                        "Cannot redeclare property hook \"{}\"",
                        String::from_utf8_lossy(&kind)
                    ),
                    line,
                ));
            }
            if !get && returns_reference {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    "Set hook cannot return by reference",
                    line,
                ));
            }
            let parameters = if self.is("(") {
                if get {
                    return Err(Error::new(
                        crate::ErrorKind::CompileError,
                        format!("get hook of property {label} must not have a parameter list"),
                        line,
                    ));
                }
                let parameters = self.function_parameters(false)?;
                if parameters.len() != 1 {
                    return Err(Error::new(
                        crate::ErrorKind::CompileError,
                        format!("set hook of property {label} must accept exactly one parameters"),
                        line,
                    ));
                }
                let parameter = &parameters[0];
                let invalid = if parameter.by_reference {
                    Some("must not be pass-by-reference")
                } else if parameter.variadic {
                    Some("must not be variadic")
                } else if parameter.default.is_some() {
                    Some("must not have a default value")
                } else {
                    None
                };
                if let Some(reason) = invalid {
                    return Err(Error::new(
                        crate::ErrorKind::CompileError,
                        format!(
                            "Parameter ${} of set hook {label} {reason}",
                            String::from_utf8_lossy(&parameter.name)
                        ),
                        line,
                    ));
                }
                if parameter.ty.is_some() != property.ty.is_some() {
                    return Err(Error::new(
                        crate::ErrorKind::CompileError,
                        format!(
                            "Type of parameter ${} of hook {label}::set must be compatible with property type",
                            String::from_utf8_lossy(&parameter.name)
                        ),
                        line,
                    ));
                }
                parameters
            } else if get {
                Vec::new()
            } else {
                vec![Parameter {
                    attributes: Vec::new(),
                    by_reference: false,
                    promotion: None,
                    name: b"value".to_vec(),
                    ty: property.ty.clone(),
                    implicit_nullable: false,
                    default: None,
                    variadic: false,
                }]
            };
            let name = format!(
                "${}::{}",
                String::from_utf8_lossy(&property.name),
                String::from_utf8_lossy(&kind)
            )
            .into_bytes();
            let outer_name = std::mem::replace(&mut self.function_name, name.clone());
            let outer_class = std::mem::replace(&mut self.function_class, self.class_name.clone());
            let return_type = if get {
                property.ty.clone()
            } else {
                Some(TypeDecl {
                    members: vec!["void".into()],
                })
            };
            let outer_return = std::mem::replace(&mut self.return_type, return_type.clone());
            let outer_static = std::mem::take(&mut self.static_names);
            let outer_depth = std::mem::replace(&mut self.control_depth, 0);
            let outer_finally = std::mem::take(&mut self.finally_depths);
            let mut body = Vec::new();
            let is_abstract = property.is_abstract && self.is(";");
            if is_interface && !is_abstract {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    "Abstract property hook cannot have body",
                    line,
                ));
            }
            if is_abstract && property.visibility == Visibility::Private {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    "Property hook cannot be both abstract and private",
                    line,
                ));
            }
            if is_abstract && final_hook {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    "Property hook cannot be both abstract and final",
                    line,
                ));
            }
            if self.eat("=>") {
                let value = self.expr(0)?;
                body.push(if get {
                    Stmt::Return(Some(value))
                } else {
                    Stmt::Expr(Expr {
                        line,
                        kind: ExprKind::Assign(
                            Box::new(Expr {
                                line,
                                kind: ExprKind::Property(
                                    Box::new(Expr {
                                        line,
                                        kind: ExprKind::Variable(b"this".to_vec()),
                                    }),
                                    Box::new(Expr {
                                        line,
                                        kind: ExprKind::Literal(Value::String(
                                            property.name.clone().into(),
                                        )),
                                    }),
                                ),
                            }),
                            "=".into(),
                            Box::new(value),
                        ),
                    })
                });
                self.expect(";")?;
            } else if self.eat(";") {
                if !is_abstract {
                    return Err(Error::new(
                        crate::ErrorKind::CompileError,
                        "Non-abstract property hook must have a body",
                        line,
                    ));
                }
            } else {
                self.expect("{")?;
                while !self.eat("}") {
                    if self.token().kind == Kind::End {
                        return Err(Error::parse("Unclosed property hook", line));
                    }
                    body.push(self.statement()?);
                }
            }
            self.function_name = outer_name;
            self.function_class = outer_class;
            self.return_type = outer_return;
            self.static_names = outer_static;
            self.control_depth = outer_depth;
            self.finally_depths = outer_finally;
            hooks.is_virtual &= !body.iter().any(|s| uses_property(s, &property.name));
            let mut qualified = self.class_name.clone();
            qualified.extend(b"::");
            qualified.extend(name);
            let function = Rc::new(FunctionDecl {
                doc_comment,
                tentative_return_type: None,
                end_line: self.tokens[self.pos - 1].line,
                attributes: Vec::new(),
                returns_reference,
                trait_origin: None,
                native: None,
                visibility: if property.visibility == Visibility::Private {
                    Visibility::Private
                } else {
                    Visibility::Public
                },
                is_abstract,
                is_static: false,
                name: qualified,
                parameters,
                body,
                line,
                return_type,
                strict_types: self.strict_types,
                source: self.source.clone(),
            });
            if get {
                hooks.get = Some(function);
                hooks.get_final = final_hook;
            } else {
                hooks.set = Some(function);
                hooks.set_final = final_hook;
            }
        }
        if hooks.get.is_none() && hooks.set.is_none() {
            return Err(Error::new(
                crate::ErrorKind::CompileError,
                "Property hook list must not be empty",
                self.tokens[self.pos - 1].line,
            ));
        }
        Ok(hooks)
    }
}

// Backing storage depends on a syntactic access in this hook's scope, even in
// dead code. Nested functions/classes have their own scope and do not count.
fn uses_property(statement: &Stmt, name: &[u8]) -> bool {
    let expr = |e: &Expr| expression_uses_property(e, name);
    let stmts = |body: &[Stmt]| body.iter().any(|s| uses_property(s, name));
    match statement {
        Stmt::Function(_)
        | Stmt::Class(_)
        | Stmt::Label(..)
        | Stmt::Goto(..)
        | Stmt::Text(_)
        | Stmt::Break(_)
        | Stmt::Continue(..) => false,
        Stmt::Global(values) | Stmt::Echo(values) | Stmt::Unset(values) => values.iter().any(expr),
        Stmt::Static(values) => values.iter().any(|(_, e)| e.as_ref().is_some_and(expr)),
        Stmt::Const(values, _) => values.iter().any(|(_, e)| expr(e)),
        Stmt::Expr(e) => expr(e),
        Stmt::Return(e) => e.as_ref().is_some_and(expr),
        Stmt::Block(body) => stmts(body),
        Stmt::Try(body, catches, finally) => {
            stmts(body)
                || catches.iter().any(|c| stmts(&c.body))
                || finally.as_ref().is_some_and(|b| stmts(b))
        }
        Stmt::If(e, yes, no) => {
            expr(e)
                || uses_property(yes, name)
                || no.as_ref().is_some_and(|s| uses_property(s, name))
        }
        Stmt::While(e, body) | Stmt::DoWhile(body, e) => expr(e) || uses_property(body, name),
        Stmt::For(a, b, c, body) => {
            a.iter().chain(b).chain(c).any(expr) || uses_property(body, name)
        }
        Stmt::Switch(e, cases) => {
            expr(e)
                || cases
                    .iter()
                    .any(|c| c.condition.as_ref().is_some_and(expr) || stmts(&c.body))
        }
        Stmt::Foreach(e, key, value, _, body) => {
            expr(e) || key.as_ref().is_some_and(expr) || expr(value) || uses_property(body, name)
        }
    }
}
fn expression_uses_property(value: &Expr, name: &[u8]) -> bool {
    let expr = |e: &Expr| expression_uses_property(e, name);
    match &value.kind {
        ExprKind::Property(base, key) | ExprKind::NullsafeProperty(base, key) => {
            (matches!(&base.kind, ExprKind::Variable(v) if v == b"this")
                && matches!(&key.kind, ExprKind::Literal(Value::String(v)) if v.bytes() == name))
                || expr(base)
                || expr(key)
        }
        ExprKind::Lambda(_)
        | ExprKind::AnonymousClass(_)
        | ExprKind::Literal(_)
        | ExprKind::Variable(_)
        | ExprKind::Constant(_)
        | ExprKind::NamespacedName(_) => false,
        ExprKind::Interpolated(parts) => parts.iter().any(|p| expr(&p.expression)),
        ExprKind::Group(e)
        | ExprKind::DynamicVariable(e)
        | ExprKind::Unary(_, e)
        | ExprKind::Cast(_, e)
        | ExprKind::Callable(e)
        | ExprKind::ClassName(e)
        | ExprKind::Eval(e)
        | ExprKind::Include(_, e)
        | ExprKind::Empty(e)
        | ExprKind::Print(e)
        | ExprKind::Throw(e)
        | ExprKind::Increment(e, _, _) => expr(e),
        ExprKind::Binary(_, a, b)
        | ExprKind::ScopedMember(a, b)
        | ExprKind::StaticProperty(a, b)
        | ExprKind::Assign(a, _, b) => expr(a) || expr(b),
        ExprKind::Index(a, b) | ExprKind::Conditional(a, b, _) => {
            expr(a)
                || b.as_ref().is_some_and(|e| expr(e))
                || match &value.kind {
                    ExprKind::Conditional(_, _, c) => expr(c),
                    _ => false,
                }
        }
        ExprKind::Array(values)
        | ExprKind::LongArray(values)
        | ExprKind::List(values)
        | ExprKind::Destructure(values) => values.iter().any(|v| match v {
            ArrayElement::Entry(k, v) | ArrayElement::Reference(k, v) => {
                k.as_ref().is_some_and(expr) || expr(v)
            }
            ArrayElement::Unpack(v) => expr(v),
            ArrayElement::Hole => false,
        }),
        ExprKind::Call(e, args) | ExprKind::New(e, args) => {
            expr(e)
                || args.iter().any(|a| match a {
                    Argument::Positional(e) | Argument::Unpack(e) | Argument::Named(_, e) => {
                        expr(e)
                    }
                })
        }
        ExprKind::Isset(values) => values.iter().any(expr),
        ExprKind::Match(e, arms) => {
            expr(e)
                || arms
                    .iter()
                    .any(|a| a.conditions.iter().any(expr) || expr(&a.result))
        }
    }
}
