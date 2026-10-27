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
    let keyed = elements
        .iter()
        .any(|element| matches!(element, ArrayElement::Entry(Some(_), _)));
    let mut targets = Vec::new();
    let mut has_target = false;
    for element in elements {
        match element {
            ArrayElement::Hole if !keyed => targets.push(ArrayElement::Hole),
            ArrayElement::Entry(key, value) if key.is_some() == keyed => {
                targets.push(ArrayElement::Entry(key, pattern(value, Some(list))?));
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
    for statement in statements {
        match statement {
            Stmt::Global(variables) => {
                for variable in variables {
                    expression(variable, false)?;
                }
            }
            Stmt::Static(declarations) => {
                for (_, initializer) in declarations {
                    if let Some(expr) = initializer {
                        expression(expr, false)?;
                    }
                }
            }
            Stmt::Const(constants) => {
                for (_, value) in constants {
                    expression(value, false)?;
                }
            }
            Stmt::Class(class) => {
                for (_, value) in &class.properties {
                    if let Some(value) = value {
                        expression(value, false)?;
                    }
                }
                for method in &class.methods {
                    validate(&[Stmt::Function(method.clone())])?;
                }
            }
            Stmt::Function(function) => {
                for parameter in &function.parameters {
                    if parameter.name == b"this" {
                        return Err(Error::parse("Cannot use $this as parameter", function.line));
                    }
                    if parameter.name == b"GLOBALS" {
                        return Err(Error::parse(
                            "Cannot re-assign auto-global variable GLOBALS",
                            function.line,
                        ));
                    }
                    if let Some(default) = &parameter.default {
                        expression(default, false)?;
                    }
                }
                validate(&function.body)?;
            }
            Stmt::Label(_, _)
            | Stmt::Goto(_, _)
            | Stmt::Text(_)
            | Stmt::Break(_)
            | Stmt::Continue(_, _) => {}
            Stmt::Unset(expressions) => {
                for expr in expressions {
                    expression(expr, true)?;
                }
            }
            Stmt::Echo(expressions) => {
                for expr in expressions {
                    expression(expr, false)?;
                }
            }
            Stmt::Expr(expr) => expression(expr, false)?,
            Stmt::Block(body) => validate(body)?,
            Stmt::If(condition, yes, no) => {
                expression(condition, false)?;
                validate(std::slice::from_ref(yes))?;
                if let Some(no) = no {
                    validate(std::slice::from_ref(no))?;
                }
            }
            Stmt::While(condition, body) | Stmt::DoWhile(body, condition) => {
                expression(condition, false)?;
                validate(std::slice::from_ref(body))?;
            }
            Stmt::For(init, condition, step, body) => {
                for expr in init.iter().chain(condition).chain(step) {
                    expression(expr, false)?;
                }
                validate(std::slice::from_ref(body))?;
            }
            Stmt::Switch(subject, cases) => {
                expression(subject, false)?;
                for case in cases {
                    if let Some(condition) = &case.condition {
                        expression(condition, false)?;
                    }
                    validate(&case.body)?;
                }
            }
            Stmt::Foreach(iter, key, value, body) => {
                expression(iter, false)?;
                if let Some(key) = key {
                    expression(key, true)?;
                }
                expression(value, true)?;
                validate(std::slice::from_ref(body))?;
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    expression(value, false)?;
                }
            }
        }
    }
    Ok(())
}

fn expression(expr: &Expr, target: bool) -> Result<(), Error> {
    if let ExprKind::Lambda(lambda) = &expr.kind {
        return validate(&[Stmt::Function(lambda.function.clone())]);
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
        ExprKind::Literal(_)
        | ExprKind::Variable(_)
        | ExprKind::Constant(_)
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
                            expression(key, false)?;
                        }
                        expression(value, pattern)?;
                    }
                    ArrayElement::Unpack(value) => expression(value, false)?,
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
                expression(value, false)?;
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
        | ExprKind::Print(value) => expression(value, false)?,
        ExprKind::Increment(value, _, _) => expression(value, true)?,
        ExprKind::ScopedMember(left, right)
        | ExprKind::Property(left, right)
        | ExprKind::Binary(_, left, right) => {
            expression(left, false)?;
            expression(right, false)?;
        }
        ExprKind::Assign(left, _, right) => {
            expression(left, true)?;
            expression(right, false)?;
        }
        ExprKind::Index(base, key) => {
            if key.is_none() && base.is_globals() {
                return Err(Error::parse("Cannot append to $GLOBALS", expr.line));
            }
            expression(base, false)?;
            if let Some(key) = key {
                expression(key, false)?;
            }
        }
        ExprKind::Conditional(condition, yes, no) => {
            expression(condition, false)?;
            if let Some(yes) = yes {
                expression(yes, false)?;
            }
            expression(no, false)?;
        }
        ExprKind::Match(subject, arms) => {
            expression(subject, false)?;
            for arm in arms {
                for condition in &arm.conditions {
                    expression(condition, false)?;
                }
                expression(&arm.result, false)?;
            }
        }
        ExprKind::New(callee, args) | ExprKind::Call(callee, args) => {
            expression(callee, false)?;
            for arg in args {
                let (Argument::Positional(value)
                | Argument::Named(_, value)
                | Argument::Unpack(value)) = arg;
                expression(value, false)?;
            }
        }
    }
    Ok(())
}
