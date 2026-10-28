use super::*;

pub(super) fn assignment_target(expr: Expr) -> Result<Expr, Error> {
    pattern(expr, None)
}

fn pattern(mut expr: Expr, outer_list: Option<bool>) -> Result<Expr, Error> {
    let list = matches!(expr.kind, ExprKind::List(_));
    let elements = match expr.kind {
        ExprKind::Array(elements) | ExprKind::List(elements) => elements,
        _ if expr.is_writable() => return Ok(expr),
        _ => return Err(Error::parse("Cannot assign to expression", expr.line)),
    };
    if outer_list.is_some_and(|outer| outer != list) {
        return Err(Error::parse("Cannot mix [] and list()", expr.line));
    }
    let keyed = elements.iter().any(|element| {
        matches!(
            element,
            ArrayElement::Entry(Some(_), _) | ArrayElement::Reference(Some(_), _)
        )
    });
    let mut targets = Vec::new();
    let mut has_target = false;
    for element in elements {
        match element {
            ArrayElement::Hole if !keyed => targets.push(ArrayElement::Hole),
            ArrayElement::Entry(key, value) if key.is_some() == keyed => {
                targets.push(ArrayElement::Entry(key, pattern(value, Some(list))?));
                has_target = true;
            }
            ArrayElement::Reference(key, value) if key.is_some() == keyed => {
                if !value.is_writable() {
                    return Err(Error::parse("Cannot assign to expression", value.line));
                }
                targets.push(ArrayElement::Reference(key, value));
                has_target = true;
            }
            ArrayElement::Unpack(_) => {
                return Err(Error::parse(
                    "Spread operator is not supported in assignments",
                    expr.line,
                ));
            }
            _ => {
                return Err(Error::parse(
                    "Cannot mix keyed and unkeyed entries in assignments",
                    expr.line,
                ));
            }
        }
    }
    if !has_target {
        return Err(Error::parse("Cannot use empty list", expr.line));
    }
    expr.kind = ExprKind::Destructure(targets);
    Ok(expr)
}

// Array holes and list() are accepted while parsing possible assignment
// targets. Reject any that remain in read contexts after target conversion.
pub(super) fn validate(statements: &[Stmt]) -> Result<(), Error> {
    Validator {
        method_names: false,
    }
    .validate(statements)
}

// Validate resolved method names after folding, so pruned expression branches
// are not diagnosed. Structural assignment validation still precedes folding.
pub(super) fn validate_method_names(statements: &[Stmt]) -> Result<(), Error> {
    Validator { method_names: true }.validate(statements)
}

struct Validator {
    method_names: bool,
}
impl Validator {
    fn validate(&self, statements: &[Stmt]) -> Result<(), Error> {
        for statement in statements {
            match statement {
                Stmt::Global(variables) => {
                    for variable in variables {
                        self.expression(variable, false)?;
                    }
                }
                Stmt::Static(declarations) => {
                    for (_, initializer) in declarations {
                        if let Some(expr) = initializer {
                            self.expression(expr, false)?;
                        }
                    }
                }
                Stmt::Const(constants) => {
                    for (_, value) in constants {
                        self.expression(value, false)?;
                    }
                }
                Stmt::Class(class) => {
                    for property in class
                        .properties
                        .iter()
                        .chain(class.static_properties.iter())
                    {
                        if let Some(value) = &property.default {
                            self.expression(value, false)?;
                        }
                    }
                    for constant in &class.constants {
                        self.expression(&constant.value, false)?;
                    }
                    for method in &class.methods {
                        self.validate(&[Stmt::Function(method.clone())])?;
                    }
                }
                Stmt::Function(function) => {
                    for parameter in &function.parameters {
                        if parameter.name == b"this" {
                            return Err(Error::parse(
                                "Cannot use $this as parameter",
                                function.line,
                            ));
                        }
                        if parameter.name == b"GLOBALS" {
                            return Err(Error::parse(
                                "Cannot re-assign auto-global variable GLOBALS",
                                function.line,
                            ));
                        }
                        if let Some(default) = &parameter.default {
                            self.expression(default, false)?;
                        }
                    }
                    self.validate(&function.body)?;
                }
                Stmt::Label(_, _)
                | Stmt::Goto(_, _)
                | Stmt::Text(_)
                | Stmt::Break(_)
                | Stmt::Continue(_, _) => {}
                Stmt::Unset(expressions) => {
                    for expr in expressions {
                        self.expression(expr, true)?;
                    }
                }
                Stmt::Echo(expressions) => {
                    for expr in expressions {
                        self.expression(expr, false)?;
                    }
                }
                Stmt::Expr(expr) => self.expression(expr, false)?,
                Stmt::Block(body) => self.validate(body)?,
                Stmt::Try(body, catches, finally) => {
                    self.validate(body)?;
                    for catch in catches {
                        self.validate(&catch.body)?;
                    }
                    if let Some(finally) = finally {
                        self.validate(finally)?;
                    }
                }
                Stmt::If(condition, yes, no) => {
                    self.expression(condition, false)?;
                    self.validate(std::slice::from_ref(yes))?;
                    if let Some(no) = no {
                        self.validate(std::slice::from_ref(no))?;
                    }
                }
                Stmt::While(condition, body) | Stmt::DoWhile(body, condition) => {
                    self.expression(condition, false)?;
                    self.validate(std::slice::from_ref(body))?;
                }
                Stmt::For(init, condition, step, body) => {
                    for expr in init.iter().chain(condition).chain(step) {
                        self.expression(expr, false)?;
                    }
                    self.validate(std::slice::from_ref(body))?;
                }
                Stmt::Switch(subject, cases) => {
                    self.expression(subject, false)?;
                    for case in cases {
                        if let Some(condition) = &case.condition {
                            self.expression(condition, false)?;
                        }
                        self.validate(&case.body)?;
                    }
                }
                Stmt::Foreach(iter, key, value, _, body) => {
                    self.expression(iter, false)?;
                    if let Some(key) = key {
                        self.expression(key, true)?;
                    }
                    self.expression(value, true)?;
                    self.validate(std::slice::from_ref(body))?;
                }
                Stmt::Return(value) => {
                    if let Some(value) = value {
                        self.expression(value, false)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn expression(&self, expr: &Expr, target: bool) -> Result<(), Error> {
        if self.method_names
            && let ExprKind::Call(callee, _) | ExprKind::Callable(callee) = &expr.kind
            && let ExprKind::Property(_, name)
            | ExprKind::NullsafeProperty(_, name)
            | ExprKind::ScopedMember(_, name) = &callee.kind
            && logical_constants::constant(name)
                .is_some_and(|value| !matches!(value, Value::String(_)))
        {
            return Err(Error::parse("Method name must be a string", name.line));
        }

        if let ExprKind::Lambda(lambda) = &expr.kind {
            return self.validate(&[Stmt::Function(lambda.function.clone())]);
        }
        if target && matches!(&expr.kind, ExprKind::Variable(name) if name == b"this") {
            return Err(Error::parse("Cannot re-assign $this", expr.line));
        }
        if target && expr.is_globals() {
            return Err(Error::parse(
                "$GLOBALS can only be modified using the $GLOBALS[$name] = $value syntax",
                expr.line,
            ));
        }
        match &expr.kind {
            ExprKind::AnonymousClass(class) => self.validate(&[Stmt::Class(class.clone())])?,
            ExprKind::Literal(_)
            | ExprKind::Variable(_)
            | ExprKind::Constant(_)
            | ExprKind::NamespacedName(_)
            | ExprKind::Lambda(_) => {}
            ExprKind::List(_) => {
                return Err(Error::parse(
                    "Cannot use list() in a read context",
                    expr.line,
                ));
            }
            ExprKind::Destructure(_) if !target => {
                return Err(Error::parse(
                    "Cannot read a destructuring pattern",
                    expr.line,
                ));
            }
            ExprKind::Array(elements)
            | ExprKind::LongArray(elements)
            | ExprKind::Destructure(elements) => {
                let pattern = matches!(expr.kind, ExprKind::Destructure(_));
                for element in elements {
                    match element {
                        ArrayElement::Entry(key, value) => {
                            if let Some(key) = key {
                                self.expression(key, false)?;
                            }
                            self.expression(value, pattern)?;
                        }
                        ArrayElement::Reference(key, value) => {
                            if let Some(key) = key {
                                self.expression(key, false)?;
                            }
                            if (!value.is_writable() && !matches!(value.kind, ExprKind::Call(_, _)))
                                || matches!(value.kind, ExprKind::Group(_))
                            {
                                return Err(Error::parse(
                                    "Cannot create reference to a non-variable",
                                    value.line,
                                ));
                            }
                            self.expression(value, true)?;
                        }
                        ArrayElement::Unpack(value) => self.expression(value, false)?,
                        ArrayElement::Hole if pattern => {}
                        ArrayElement::Hole => {
                            return Err(Error::parse(
                                "Cannot use empty array elements in arrays",
                                expr.line,
                            ));
                        }
                    }
                }
            }
            ExprKind::Interpolated(values) | ExprKind::Isset(values) => {
                for value in values {
                    self.expression(value, false)?;
                }
            }
            ExprKind::ClassName(value)
            | ExprKind::Group(value)
            | ExprKind::Callable(value)
            | ExprKind::DynamicVariable(value)
            | ExprKind::Eval(value)
            | ExprKind::Include(_, value)
            | ExprKind::Cast(_, value)
            | ExprKind::Unary(_, value)
            | ExprKind::Empty(value)
            | ExprKind::Throw(value)
            | ExprKind::Print(value) => self.expression(value, false)?,
            ExprKind::Increment(value, _, _) => self.expression(value, true)?,
            ExprKind::StaticProperty(left, right)
            | ExprKind::ScopedMember(left, right)
            | ExprKind::Property(left, right)
            | ExprKind::NullsafeProperty(left, right)
            | ExprKind::Binary(_, left, right) => {
                self.expression(left, false)?;
                self.expression(right, false)?;
            }
            ExprKind::Assign(left, op, right) => {
                if left.has_destructure_references() {
                    let mut source = right.as_ref();
                    while let ExprKind::Group(inner) = &source.kind {
                        source = inner;
                    }
                    let call = matches!(source.kind, ExprKind::Call(_, _))
                        || matches!(&source.kind, ExprKind::Binary(op, _, _) if op == "|>");
                    if !source.is_writable() && !call {
                        return Err(Error::parse(
                            "Cannot assign reference to non referenceable value",
                            right.line,
                        ));
                    }
                    if source.has_nullsafe_chain() {
                        return Err(Error::parse(
                            "Cannot take reference of a nullsafe chain",
                            right.line,
                        ));
                    }
                }
                self.expression(left, true)?;
                self.expression(right, op == "=&")?;
            }
            ExprKind::Index(base, key) => {
                if key.is_none() && base.is_globals() {
                    return Err(Error::parse("Cannot append to $GLOBALS", expr.line));
                }
                self.expression(base, false)?;
                if let Some(key) = key {
                    self.expression(key, false)?;
                }
            }
            ExprKind::Conditional(condition, yes, no) => {
                self.expression(condition, false)?;
                if let Some(yes) = yes {
                    self.expression(yes, false)?;
                }
                self.expression(no, false)?;
            }
            ExprKind::Match(subject, arms) => {
                self.expression(subject, false)?;
                for arm in arms {
                    for condition in &arm.conditions {
                        self.expression(condition, false)?;
                    }
                    self.expression(&arm.result, false)?;
                }
            }
            ExprKind::New(callee, args) | ExprKind::Call(callee, args) => {
                self.expression(callee, false)?;
                for arg in args {
                    let (Argument::Positional(value)
                    | Argument::Named(_, value)
                    | Argument::Unpack(value)) = arg;
                    self.expression(value, false)?;
                }
            }
        }
        Ok(())
    }
}
