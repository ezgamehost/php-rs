use super::*;
use crate::ast::Argument;

// PHP reserves slots for statically named variables at compilation time. They
// appear in the symbol table in this order once defined; dynamic names follow
// them. A reserved but undefined slot must not appear in a $GLOBALS snapshot.
impl Scope {
    pub(super) fn reserve_variables(&mut self, statements: &[Stmt]) {
        for statement in statements {
            match statement {
                Stmt::Label(_, _)
                | Stmt::Goto(_, _)
                | Stmt::Class(_)
                | Stmt::Function(_)
                | Stmt::Text(_)
                | Stmt::Break(_)
                | Stmt::Continue(_, _) => {}
                Stmt::Global(values) => {
                    for value in values {
                        if let ExprKind::Variable(name) = &value.kind {
                            self.reserve(name);
                        } else {
                            self.reserve_expression(value);
                        }
                    }
                }
                Stmt::Static(values) => {
                    for (name, value) in values {
                        self.reserve(name);
                        if let Some(value) = value {
                            self.reserve_expression(value);
                        }
                    }
                }
                Stmt::Const(values) => {
                    for (_, value) in values {
                        self.reserve_expression(value);
                    }
                }
                Stmt::Echo(values) | Stmt::Unset(values) => {
                    for value in values {
                        self.reserve_expression(value);
                    }
                }
                Stmt::Expr(value) => self.reserve_expression(value),
                Stmt::Block(body) => self.reserve_variables(body),
                Stmt::Try(body, catches, finally) => {
                    self.reserve_variables(body);
                    for catch in catches {
                        if let Some(name) = &catch.variable {
                            self.reserve(name);
                        }
                        self.reserve_variables(&catch.body);
                    }
                    if let Some(finally) = finally {
                        self.reserve_variables(finally);
                    }
                }
                Stmt::If(condition, yes, no) => {
                    self.reserve_expression(condition);
                    self.reserve_variables(std::slice::from_ref(yes));
                    if let Some(no) = no {
                        self.reserve_variables(std::slice::from_ref(no));
                    }
                }
                Stmt::While(condition, body) | Stmt::DoWhile(body, condition) => {
                    self.reserve_variables(std::slice::from_ref(body));
                    self.reserve_expression(condition);
                }
                Stmt::For(init, condition, step, body) => {
                    for value in init {
                        self.reserve_expression(value);
                    }
                    self.reserve_variables(std::slice::from_ref(body));
                    for value in step.iter().chain(condition) {
                        self.reserve_expression(value);
                    }
                }
                Stmt::Switch(subject, cases) => {
                    self.reserve_expression(subject);
                    for case in cases {
                        if let Some(condition) = &case.condition {
                            self.reserve_expression(condition);
                        }
                    }
                    for case in cases {
                        self.reserve_variables(&case.body);
                    }
                }
                Stmt::Foreach(iter, key, value, _, body) => {
                    self.reserve_expression(iter);
                    self.reserve_expression(value);
                    if let Some(key) = key {
                        self.reserve_expression(key);
                    }
                    self.reserve_variables(std::slice::from_ref(body));
                }
                Stmt::Return(value) => {
                    if let Some(value) = value {
                        self.reserve_expression(value);
                    }
                }
            }
        }
    }

    fn reserve_expression(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Lambda(lambda) => {
                for capture in &lambda.captures {
                    self.reserve(&capture.name);
                }
            }
            ExprKind::Variable(name) if !expr.is_globals() => self.reserve(name),
            ExprKind::Literal(_)
            | ExprKind::AnonymousClass(_)
            | ExprKind::Variable(_)
            | ExprKind::Constant(_)
            | ExprKind::NamespacedName(_) => {}
            ExprKind::ClassName(inner)
            | ExprKind::Group(inner)
            | ExprKind::Callable(inner)
            | ExprKind::DynamicVariable(inner)
            | ExprKind::Unary(_, inner)
            | ExprKind::Cast(_, inner)
            | ExprKind::Eval(inner)
            | ExprKind::Include(_, inner)
            | ExprKind::Empty(inner)
            | ExprKind::Throw(inner)
            | ExprKind::Print(inner)
            | ExprKind::Increment(inner, _, _) => self.reserve_expression(inner),
            ExprKind::Assign(left, _, right) if matches!(left.kind, ExprKind::Destructure(_)) => {
                self.reserve_expression(right);
                self.reserve_expression(left);
            }
            ExprKind::StaticProperty(left, right)
            | ExprKind::ScopedMember(left, right)
            | ExprKind::Property(left, right)
            | ExprKind::NullsafeProperty(left, right)
            | ExprKind::Binary(_, left, right)
            | ExprKind::Assign(left, _, right) => {
                self.reserve_expression(left);
                self.reserve_expression(right);
            }
            ExprKind::Array(elements)
            | ExprKind::LongArray(elements)
            | ExprKind::List(elements)
            | ExprKind::Destructure(elements) => {
                for element in elements {
                    match element {
                        ArrayElement::Entry(key, value) | ArrayElement::Reference(key, value) => {
                            if let Some(key) = key {
                                self.reserve_expression(key);
                            }
                            self.reserve_expression(value);
                        }
                        ArrayElement::Unpack(value) => self.reserve_expression(value),
                        ArrayElement::Hole => {}
                    }
                }
            }
            ExprKind::Index(base, key) => {
                self.reserve_expression(base);
                if let Some(key) = key {
                    self.reserve_expression(key);
                }
            }
            ExprKind::Interpolated(values) | ExprKind::Isset(values) => {
                for value in values {
                    self.reserve_expression(value);
                }
            }
            ExprKind::New(callee, args) | ExprKind::Call(callee, args) => {
                self.reserve_expression(callee);
                for arg in args {
                    let (Argument::Positional(value)
                    | Argument::Named(_, value)
                    | Argument::Unpack(value)) = arg;
                    self.reserve_expression(value);
                }
            }
            ExprKind::Conditional(condition, yes, no) => {
                self.reserve_expression(condition);
                if let Some(yes) = yes {
                    self.reserve_expression(yes);
                }
                self.reserve_expression(no);
            }
            ExprKind::Match(subject, arms) => {
                self.reserve_expression(subject);
                for arm in arms {
                    for condition in &arm.conditions {
                        self.reserve_expression(condition);
                    }
                }
                for arm in arms {
                    self.reserve_expression(&arm.result);
                }
            }
        }
    }
}
