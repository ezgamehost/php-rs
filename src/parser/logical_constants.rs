use super::*;

// Zend resolves constant short-circuit operands while compiling. Preserve that
// timing (including NaN warnings), and never visit a pruned right-hand operand.
pub(super) fn fold(statements: &mut [Stmt], warnings: &mut Vec<String>) {
    for statement in statements {
        match statement {
            Stmt::Label(_, _)
            | Stmt::Goto(_, _)
            | Stmt::Text(_)
            | Stmt::Break(_)
            | Stmt::Continue(_, _) => {}
            Stmt::Function(function) => {
                let function = Rc::make_mut(function);
                // Parameter defaults keep named constants unresolved until a call.
                fold(&mut function.body, warnings);
            }
            Stmt::Global(values) | Stmt::Echo(values) | Stmt::Unset(values) => {
                for value in values {
                    expression(value, warnings);
                }
            }
            Stmt::Static(values) => {
                for (_, value) in values {
                    if let Some(value) = value {
                        initializer(value, warnings);
                        expression(value, warnings);
                    }
                }
            }
            Stmt::Const(values) => {
                for (_, value) in values {
                    initializer(value, warnings);
                }
            }
            Stmt::Expr(value) => expression(value, warnings),
            Stmt::Block(body) => fold(body, warnings),
            Stmt::If(condition, yes, no) => {
                expression(condition, warnings);
                fold(std::slice::from_mut(yes), warnings);
                if let Some(no) = no {
                    fold(std::slice::from_mut(no), warnings);
                }
            }
            Stmt::While(condition, body) | Stmt::DoWhile(body, condition) => {
                fold(std::slice::from_mut(body), warnings);
                expression(condition, warnings);
            }
            Stmt::For(init, condition, step, body) => {
                for value in init {
                    expression(value, warnings);
                }
                fold(std::slice::from_mut(body), warnings);
                for value in step.iter_mut().chain(condition) {
                    expression(value, warnings);
                }
            }
            Stmt::Foreach(iter, key, value, body) => {
                expression(iter, warnings);
                expression(value, warnings);
                if let Some(key) = key {
                    expression(key, warnings);
                }
                fold(std::slice::from_mut(body), warnings);
            }
            Stmt::Switch(subject, cases) => {
                expression(subject, warnings);
                for case in cases.iter_mut() {
                    if let Some(condition) = &mut case.condition {
                        initializer(condition, warnings);
                        if !matches!(constant(condition), Some(Value::Int(_) | Value::String(_))) {
                            break;
                        }
                    }
                }
                for case in cases.iter_mut() {
                    if let Some(condition) = &mut case.condition {
                        expression(condition, warnings);
                    }
                }
                for case in cases {
                    fold(&mut case.body, warnings);
                }
            }
            Stmt::Return(value) => {
                if let Some(value) = value {
                    expression(value, warnings);
                }
            }
        }
    }
}

fn expression(expr: &mut Expr, warnings: &mut Vec<String>) {
    if matches!(expr.kind, ExprKind::Array(_) | ExprKind::LongArray(_)) {
        initializer(expr, warnings);
    }
    let line = expr.line;
    match &mut expr.kind {
        ExprKind::Binary(op, left, right) if matches!(op.as_str(), "&&" | "and" | "||" | "or") => {
            expression(left, warnings);
            if let Some(value) = constant(left) {
                let truth = value.to_bool(line, warnings);
                if truth == matches!(op.as_str(), "||" | "or") {
                    expr.kind = ExprKind::Literal(Value::Bool(value.to_bool(line, warnings)));
                } else {
                    expression(right, warnings);
                    expr.kind = if let Some(value) = constant(right) {
                        ExprKind::Literal(Value::Bool(value.to_bool(line, warnings)))
                    } else {
                        ExprKind::Cast(CastKind::Bool, right.clone())
                    };
                }
                return;
            }
            expression(right, warnings);
        }
        ExprKind::Binary(op, left, right) => {
            expression(left, warnings);
            expression(right, warnings);
            if op == "xor"
                && let (Some(left), Some(right)) = (constant(left), constant(right))
            {
                expr.kind = ExprKind::Literal(Value::Bool(
                    left.to_bool(line, warnings) ^ right.to_bool(line, warnings),
                ));
            }
        }
        ExprKind::Literal(_) | ExprKind::Variable(_) | ExprKind::Constant(_) => {}
        ExprKind::Group(value)
        | ExprKind::DynamicVariable(value)
        | ExprKind::Unary(_, value)
        | ExprKind::Cast(_, value)
        | ExprKind::Eval(value)
        | ExprKind::Include(_, value)
        | ExprKind::Empty(value)
        | ExprKind::Print(value)
        | ExprKind::Increment(value, _, _) => expression(value, warnings),
        ExprKind::Assign(left, _, right) => {
            expression(left, warnings);
            expression(right, warnings);
        }
        ExprKind::Array(elements)
        | ExprKind::LongArray(elements)
        | ExprKind::List(elements)
        | ExprKind::Destructure(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Entry(key, value) => {
                        if let Some(key) = key {
                            expression(key, warnings);
                        }
                        expression(value, warnings);
                    }
                    ArrayElement::Unpack(value) => expression(value, warnings),
                    ArrayElement::Hole => {}
                }
            }
        }
        ExprKind::Index(base, key) => {
            expression(base, warnings);
            if let Some(key) = key {
                expression(key, warnings);
            }
        }
        ExprKind::Call(callee, args) => {
            expression(callee, warnings);
            for arg in args {
                let (Argument::Positional(value)
                | Argument::Named(_, value)
                | Argument::Unpack(value)) = arg;
                expression(value, warnings);
            }
        }
        ExprKind::Interpolated(values) | ExprKind::Isset(values) => {
            for value in values {
                expression(value, warnings);
            }
        }
        ExprKind::Conditional(condition, yes, no) => {
            expression(condition, warnings);
            if let Some(yes) = yes {
                expression(yes, warnings);
            }
            expression(no, warnings);
        }
        ExprKind::Match(subject, arms) => {
            expression(subject, warnings);
            'conditions: for arm in arms.iter_mut() {
                for condition in &mut arm.conditions {
                    initializer(condition, warnings);
                    if !matches!(constant(condition), Some(Value::Int(_) | Value::String(_))) {
                        break 'conditions;
                    }
                }
            }
            for arm in arms.iter_mut() {
                for condition in &mut arm.conditions {
                    expression(condition, warnings);
                }
            }
            for arm in arms {
                expression(&mut arm.result, warnings);
            }
        }
    }
}

fn constant(expr: &Expr) -> Option<Value> {
    fn eligible(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Literal(_) | ExprKind::Constant(_) => true,
            ExprKind::Group(value) => eligible(value),
            ExprKind::Unary(op, value) => {
                eligible(value) && !(op == "!" && matches!(constant(value), Some(Value::Float(_))))
            }
            ExprKind::Binary(op, left, right) => op != "??" && eligible(left) && eligible(right),
            ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
                elements.iter().all(|element| match element {
                    ArrayElement::Entry(key, value) => {
                        key.as_ref().is_none_or(eligible) && eligible(value)
                    }
                    _ => false,
                })
            }
            _ => false,
        }
    }
    eligible(expr)
        .then(|| crate::Runtime::constant_without_diagnostics(expr))
        .flatten()
}

// Constant initializers use Zend's AST evaluator, which tests logical operands
// once and visits both children before short-circuiting. Ordinary expressions
// instead use the compiler's short-circuit path above.
fn initializer(expr: &mut Expr, warnings: &mut Vec<String>) {
    let line = expr.line;
    match &mut expr.kind {
        ExprKind::Binary(op, left, right) if op == "??" => {
            initializer(left, warnings);
            if let Some(value) = constant(left) {
                if value == Value::Null {
                    initializer(right, warnings);
                    *expr = *right.clone();
                } else {
                    *expr = *left.clone();
                }
                return;
            }
            initializer(right, warnings);
        }
        ExprKind::Binary(op, left, right) => {
            initializer(left, warnings);
            initializer(right, warnings);
            if matches!(op.as_str(), "&&" | "and" | "||" | "or") {
                if let Some(value) = constant(left) {
                    let truth = value.to_bool(line, warnings);
                    if truth == matches!(op.as_str(), "||" | "or") {
                        expr.kind = ExprKind::Literal(Value::Bool(truth));
                    } else if let Some(value) = constant(right) {
                        expr.kind = ExprKind::Literal(Value::Bool(value.to_bool(line, warnings)));
                    }
                }
                return;
            }
            if op == "xor"
                && let (Some(left), Some(right)) = (constant(left), constant(right))
            {
                expr.kind = ExprKind::Literal(Value::Bool(
                    left.to_bool(line, warnings) ^ right.to_bool(line, warnings),
                ));
                return;
            }
        }
        ExprKind::Conditional(condition, yes, no) => {
            initializer(condition, warnings);
            if let Some(value) = constant(condition) {
                let selected = if value.to_bool(line, warnings) {
                    yes.as_mut().unwrap_or(condition)
                } else {
                    no
                };
                initializer(selected, warnings);
                *expr = *selected.clone();
                return;
            }
            if let Some(yes) = yes {
                initializer(yes, warnings);
            }
            initializer(no, warnings);
        }
        ExprKind::Group(value) | ExprKind::Unary(_, value) | ExprKind::Cast(_, value) => {
            initializer(value, warnings);
        }
        ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
            for element in elements {
                match element {
                    ArrayElement::Entry(key, value) => {
                        initializer(value, warnings);
                        if let Some(key) = key {
                            initializer(key, warnings);
                        }
                    }
                    ArrayElement::Unpack(value) => initializer(value, warnings),
                    ArrayElement::Hole => {}
                }
            }
        }
        _ => return,
    }
    if let Some(value) = constant(expr) {
        expr.kind = ExprKind::Literal(value);
    }
}
