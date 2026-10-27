use crate::{
    ArrayKey, Error, ErrorKind, PhpArray, Value,
    ast::{ArrayElement, Expr, ExprKind, FunctionDecl, Stmt},
    lexer, parser,
};
use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::{HashMap, HashSet},
    rc::Rc,
};

mod builtins;
mod casting;
mod constants;
mod destructuring;
mod function_arguments;
mod functions;
mod jumps;
mod loading;
mod matching;
mod offsets;
mod scope_slots;
mod termination;
mod types;
mod variables;
use offsets::{Access, Offset, read_offset, unset_path, write_path};
use variables::{Scope, cell};

/// One request's variable scope, output, and diagnostics. Output survives errors.
#[derive(Default)]
pub struct Runtime {
    pub output: Vec<u8>,
    pub warnings: Vec<String>,
    /// Global inputs before execute, and the global values after it returns.
    pub variables: HashMap<Vec<u8>, Value>,
    scope: Scope,
    callers: Vec<Scope>,
    static_variables: HashMap<Option<Vec<u8>>, Scope>,
    current_function: Option<Vec<u8>>,
    current_arguments: Option<function_arguments::FunctionArguments>,
    functions: HashMap<Vec<u8>, Rc<FunctionDecl>>,
    constants: HashMap<Vec<u8>, Value>,
    strict_types: bool,
    source: Rc<crate::source::Source>,
    included_files: Vec<std::path::PathBuf>,
    included_set: HashSet<std::path::PathBuf>,
    working_directory: Option<std::path::PathBuf>,
    exit_status: Option<i32>,
}
struct Location {
    name: Vec<u8>,
    global: bool,
    keys: Vec<Offset>,
}
enum Flow {
    Next,
    Goto(Vec<u8>),
    Break(usize),
    Continue(usize),
    Return(Value),
}
impl Flow {
    fn leave_level(self) -> Self {
        match self {
            Self::Break(level) => Self::Break(level - 1),
            Self::Continue(level) => Self::Continue(level - 1),
            other => other,
        }
    }
}
impl Runtime {
    /// Status requested by exit/die. A terminated request cannot execute more PHP.
    pub fn exit_status(&self) -> Option<i32> {
        self.exit_status
    }

    fn block(&mut self, stmts: &[Stmt]) -> Result<Flow, Error> {
        self.block_from(stmts, 0, None)
    }
    fn statement(&mut self, stmt: &Stmt) -> Result<Flow, Error> {
        match stmt {
            Stmt::Label(_, _) => {}
            Stmt::Goto(name, _) => return Ok(Flow::Goto(name.clone())),
            Stmt::Global(variables) => {
                for variable in variables {
                    let location = self.location(variable)?;
                    self.bind_global(location.name);
                }
            }
            Stmt::Static(declarations) => {
                for (name, initializer) in declarations {
                    self.bind_static(name, initializer.as_ref())?;
                }
            }
            Stmt::Const(constants) => {
                for (name, expr) in constants {
                    let value = self.eval(expr)?;
                    self.define_constant(name.clone(), value, expr.line);
                }
            }
            Stmt::Function(function) => {
                self.register_function(function.clone())?;
            }
            Stmt::Text(text) => self.output.extend(text),
            Stmt::Echo(values) => {
                for value in values {
                    let v = self.eval(value)?;
                    self.output
                        .extend(string_bytes(&v, value.line, &mut self.warnings));
                }
            }
            Stmt::Expr(expr) => {
                self.eval(expr)?;
            }
            Stmt::Block(stmts) => return self.block(stmts),
            Stmt::If(cond, yes, no) => {
                if self.eval(cond)?.to_bool(cond.line, &mut self.warnings) {
                    return self.statement(yes);
                }
                if let Some(no) = no {
                    return self.statement(no);
                }
            }
            Stmt::While(cond, body) => {
                while self.eval(cond)?.to_bool(cond.line, &mut self.warnings) {
                    match self.block(std::slice::from_ref(body))? {
                        Flow::Next | Flow::Continue(1) => {}
                        Flow::Break(1) => break,
                        flow => return Ok(flow.leave_level()),
                    }
                }
            }
            Stmt::DoWhile(body, cond) => loop {
                match self.block(std::slice::from_ref(body))? {
                    Flow::Next | Flow::Continue(1) => {}
                    Flow::Break(1) => break,
                    flow => return Ok(flow.leave_level()),
                }
                if !self.eval(cond)?.to_bool(cond.line, &mut self.warnings) {
                    break;
                }
            },
            Stmt::For(init, cond, step, body) => {
                self.eval_sequence(init)?;
                while self
                    .eval_sequence(cond)?
                    .to_bool(cond.last().map_or(0, |expr| expr.line), &mut self.warnings)
                {
                    match self.block(std::slice::from_ref(body))? {
                        Flow::Next | Flow::Continue(1) => {}
                        Flow::Break(1) => break,
                        flow => return Ok(flow.leave_level()),
                    }
                    self.eval_sequence(step)?;
                }
            }
            Stmt::Switch(subject, cases) => {
                // Simple variables remain live until each comparison. Other
                // expressions are evaluated once before examining the cases.
                let mut subject = subject;
                while let ExprKind::Group(inner) = &subject.kind {
                    subject = inner;
                }
                let value =
                    if matches!(subject.kind, ExprKind::Variable(_)) && !subject.is_globals() {
                        None
                    } else {
                        Some(self.eval(subject)?)
                    };
                let mut selected = None;
                let mut default = None;
                for (index, case) in cases.iter().enumerate() {
                    if let Some(condition) = &case.condition {
                        let candidate = self.eval(condition)?;
                        let value = match &value {
                            Some(value) => value.clone(),
                            None => self.eval(subject)?,
                        };
                        if value.compare(&candidate) == Some(Ordering::Equal) {
                            selected = Some(index);
                            break;
                        }
                    } else {
                        default = Some(index);
                    }
                }
                if let Some(start) = selected.or(default) {
                    return self.switch_cases(cases, start);
                }
            }
            Stmt::Foreach(iter, key_target, value_target, body) => {
                let value = self.eval(iter)?;
                if let Value::Array(array) = value {
                    for (key, value) in array.entries() {
                        self.assign_target(value_target, value.clone())?;
                        if let Some(target) = key_target {
                            let mut location = self.location(target)?;
                            self.write_location(&mut location, key.to_value(), target.line)?;
                        }
                        match self.block(std::slice::from_ref(body))? {
                            Flow::Next | Flow::Continue(1) => {}
                            Flow::Break(1) => break,
                            flow => return Ok(flow.leave_level()),
                        }
                    }
                } else {
                    self.warnings.push(format!(
                        "foreach() argument must be of type array|object on line {}",
                        iter.line
                    ));
                }
            }
            Stmt::Unset(targets) => {
                for target in targets {
                    let mut location = self.location(target)?;
                    if location.keys.is_empty() {
                        self.location_scope(&location).remove(&location.name);
                    } else if let Some(value) =
                        self.location_scope(&location).get(&location.name).cloned()
                    {
                        unset_path(
                            &mut value.borrow_mut(),
                            &mut location.keys,
                            target.line,
                            &mut self.warnings,
                        )?;
                    }
                }
            }
            Stmt::Break(level) => return Ok(Flow::Break(*level)),
            Stmt::Continue(level, _) => return Ok(Flow::Continue(*level)),
            Stmt::Return(value) => {
                return Ok(Flow::Return(match value {
                    Some(expr) => self.eval(expr)?,
                    None => Value::Null,
                }));
            }
        }
        Ok(Flow::Next)
    }
    fn eval_sequence(&mut self, expressions: &[Expr]) -> Result<Value, Error> {
        let mut value = Value::Bool(true);
        for expression in expressions {
            value = self.eval(expression)?;
        }
        Ok(value)
    }
    fn read(&mut self, name: &[u8], line: usize) -> Value {
        if let Some(value) = self.lookup(name) {
            value
        } else {
            self.warnings.push(format!(
                "Undefined variable ${} on line {line}",
                String::from_utf8_lossy(name)
            ));
            Value::Null
        }
    }
    fn location(&mut self, expr: &Expr) -> Result<Location, Error> {
        match &expr.kind {
            ExprKind::Variable(name) => Ok(Location {
                name: name.clone(),
                global: false,
                keys: Vec::new(),
            }),
            ExprKind::DynamicVariable(name) => Ok(Location {
                name: self.variable_name(name)?,
                global: false,
                keys: Vec::new(),
            }),
            ExprKind::Group(inner) => self.location(inner),
            ExprKind::Index(base, key) if base.is_globals() => {
                let key = key.as_ref().ok_or_else(|| {
                    Error::new(ErrorKind::Error, "Cannot append to $GLOBALS", expr.line)
                })?;
                Ok(Location {
                    name: self.variable_name(key)?,
                    global: true,
                    keys: Vec::new(),
                })
            }
            ExprKind::Index(base, key) => {
                let mut location = self.location(base)?;
                let value = match key {
                    Some(key) => Some(self.eval(key)?),
                    None => None,
                };
                location.keys.push(Offset::new(value));
                Ok(location)
            }
            _ => Err(Error::new(
                ErrorKind::Error,
                "Invalid assignment target",
                expr.line,
            )),
        }
    }
    fn read_location(
        &mut self,
        location: &mut Location,
        line: usize,
        silent: bool,
        string_error: Option<&str>,
    ) -> Result<Value, Error> {
        let stored = self.location_scope(location).get(&location.name).cloned();
        let mut value = match stored {
            Some(value) => value.borrow().clone(),
            None => {
                if !silent {
                    self.warnings.push(format!(
                        "Undefined {}variable ${} on line {line}",
                        if location.global { "global " } else { "" },
                        String::from_utf8_lossy(&location.name)
                    ));
                }
                Value::Null
            }
        };
        for key in &mut location.keys {
            if matches!(value, Value::String(_))
                && let Some(message) = string_error
            {
                return Err(Error::new(ErrorKind::Error, message, line));
            }
            value = read_offset(
                value,
                key,
                line,
                if silent {
                    Access::Coalesce
                } else {
                    Access::Read
                },
                &mut self.warnings,
            )?;
        }
        Ok(value)
    }
    fn write_location(
        &mut self,
        location: &mut Location,
        new_value: Value,
        line: usize,
    ) -> Result<Value, Error> {
        let value = self
            .location_scope(location)
            .get_or_insert(&location.name, Value::Null);
        write_path(
            &mut value.borrow_mut(),
            &mut location.keys,
            new_value,
            line,
            &mut self.warnings,
        )
    }
    fn coalesce_read(&mut self, expr: &Expr, probe: bool) -> Result<Value, Error> {
        match &expr.kind {
            ExprKind::Variable(_) if expr.is_globals() => Ok(self.globals_snapshot()),
            ExprKind::Variable(name) => Ok(self.lookup(name).unwrap_or(Value::Null)),
            ExprKind::DynamicVariable(name) => {
                let name = self.variable_name(name)?;
                Ok(self.lookup(&name).unwrap_or(Value::Null))
            }
            ExprKind::Group(inner) => self.coalesce_read(inner, probe),
            ExprKind::Index(base, Some(_)) if base.is_globals() => {
                let mut location = self.location(expr)?;
                self.read_location(&mut location, expr.line, true, None)
            }
            ExprKind::Index(base, Some(key)) => {
                let value = self.coalesce_read(base, probe)?;
                let key = self.eval(key)?;
                read_offset(
                    value,
                    &mut Offset::new(Some(key)),
                    expr.line,
                    if probe {
                        Access::Probe
                    } else {
                        Access::Coalesce
                    },
                    &mut self.warnings,
                )
            }
            _ => self.eval(expr),
        }
    }
    fn eval(&mut self, expr: &Expr) -> Result<Value, Error> {
        let line = expr.line;
        match &expr.kind {
            ExprKind::Eval(code) => {
                let value = self.eval(code)?;
                let code = string_bytes(&value, line, &mut self.warnings);
                self.eval_code(&code, line)
            }
            ExprKind::Include(kind, path) => {
                let value = self.eval(path)?;
                let path = string_bytes(&value, line, &mut self.warnings);
                self.include_file(*kind, &path, line)
            }
            ExprKind::Cast(kind, operand) => {
                let value = self.eval(operand)?;
                casting::cast(*kind, value, line, &mut self.warnings)
            }
            ExprKind::Match(subject, arms) => self.match_expression(subject, arms, line),
            ExprKind::Print(operand) => {
                let value = self.eval(operand)?;
                self.output
                    .extend(string_bytes(&value, line, &mut self.warnings));
                Ok(Value::Int(1))
            }
            ExprKind::Call(callee, args) => self.call(callee, args, line),
            ExprKind::Literal(value) => Ok(value.clone()),
            ExprKind::Interpolated(parts) => {
                let mut result = Vec::new();
                for part in parts {
                    let value = self.eval(part)?;
                    result.extend(string_bytes(&value, part.line, &mut self.warnings));
                }
                Ok(Value::String(result))
            }
            ExprKind::Group(inner) => self.eval(inner),
            ExprKind::Variable(_) if expr.is_globals() => Ok(self.globals_snapshot()),
            ExprKind::Variable(name) => Ok(self.read(name, line)),
            ExprKind::DynamicVariable(name) => {
                let name = self.variable_name(name)?;
                Ok(self.read(&name, line))
            }
            ExprKind::List(_) | ExprKind::Destructure(_) => {
                Err(Error::parse("Cannot read a destructuring pattern", line))
            }
            ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
                let mut array = PhpArray::default();
                for element in elements {
                    let ArrayElement::Entry(key, expr) = element else {
                        let ArrayElement::Unpack(expr) = element else {
                            unreachable!()
                        };
                        let Value::Array(source) = self.eval(expr)? else {
                            return Err(Error::new(
                                ErrorKind::Error,
                                "Only arrays and Traversables can be unpacked",
                                expr.line,
                            ));
                        };
                        for (key, value) in source.entries() {
                            match key {
                                ArrayKey::Int(_) => array.append(value.clone(), expr.line)?,
                                ArrayKey::String(_) => array.insert(key.clone(), value.clone()),
                            }
                        }
                        continue;
                    };
                    let key = match key {
                        Some(key) => Some(ArrayKey::from_value(
                            &self.eval(key)?,
                            key.line,
                            &mut self.warnings,
                        )?),
                        None => None,
                    };
                    let value = self.eval(expr)?;
                    if let Some(key) = key {
                        array.insert(key, value);
                    } else {
                        array.append(value, line)?;
                    }
                }
                Ok(Value::Array(Rc::new(array)))
            }
            ExprKind::Index(base, Some(_)) if base.is_globals() => {
                let mut location = self.location(expr)?;
                self.read_location(&mut location, line, false, None)
            }
            ExprKind::Index(base, key) => {
                let value = self.eval(base)?;
                let key = key.as_ref().ok_or_else(|| {
                    Error::new(ErrorKind::Error, "Cannot use [] for reading", line)
                })?;
                let key = self.eval(key)?;
                read_offset(
                    value,
                    &mut Offset::new(Some(key)),
                    line,
                    Access::Read,
                    &mut self.warnings,
                )
            }
            ExprKind::Isset(args) => {
                for arg in args {
                    if self.coalesce_read(arg, true)? == Value::Null {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            ExprKind::Empty(arg) => Ok(Value::Bool(
                !self
                    .coalesce_read(arg, true)?
                    .to_bool(line, &mut self.warnings),
            )),
            ExprKind::Constant(name) => self.read_constant(name, line),
            ExprKind::Unary(op, expr) => {
                let value = self.eval(expr)?;
                match op.as_str() {
                    "!" => Ok(Value::Bool(!value.to_bool(line, &mut self.warnings))),
                    "~" => match value {
                        Value::String(s) => Ok(Value::String(s.into_iter().map(|b| !b).collect())),
                        Value::Int(_) | Value::Float(_) => {
                            Ok(Value::Int(!value.integer(line, &mut self.warnings)?))
                        }
                        _ => Err(Error::new(
                            ErrorKind::TypeError,
                            "Cannot perform bitwise not on operand",
                            line,
                        )),
                    },
                    "+" => value.number(line, &mut self.warnings),
                    "-" => match value.number(line, &mut self.warnings)? {
                        Value::Int(n) => Ok(n
                            .checked_neg()
                            .map(Value::Int)
                            .unwrap_or(Value::Float(-(n as f64)))),
                        Value::Float(n) => Ok(Value::Float(-n)),
                        _ => unreachable!(),
                    },
                    _ => unreachable!(),
                }
            }
            ExprKind::Assign(target, op, expr) => {
                if matches!(target.kind, ExprKind::Destructure(_)) {
                    let value = self.eval(expr)?;
                    return self.assign_target(target, value);
                }
                let mut location = self.location(target)?;
                let value = if op == "=" {
                    self.eval(expr)?
                } else if op == "??=" {
                    match self.read_location(&mut location, line, true, None)? {
                        Value::Null => self.eval(expr)?,
                        value => return Ok(value),
                    }
                } else {
                    let lhs = self.read_location(
                        &mut location,
                        line,
                        false,
                        Some("Cannot use assign-op operators with string offsets"),
                    )?;
                    let rhs = self.eval(expr)?;
                    binary(&op[..op.len() - 1], lhs, rhs, line, &mut self.warnings)?
                };
                self.write_location(&mut location, value, line)
            }
            ExprKind::Increment(target, delta, prefix) => {
                let mut location = self.location(target)?;
                let old = self.read_location(
                    &mut location,
                    line,
                    false,
                    Some("Cannot increment/decrement string offsets"),
                )?;
                let new = increment(&old, *delta, line, &mut self.warnings)?;
                self.write_location(&mut location, new.clone(), line)?;
                Ok(if *prefix { new } else { old })
            }
            ExprKind::Conditional(cond, yes, no) => {
                let value = self.eval(cond)?;
                if value.to_bool(cond.line, &mut self.warnings) {
                    match yes {
                        Some(yes) => self.eval(yes),
                        None => Ok(value),
                    }
                } else {
                    self.eval(no)
                }
            }
            ExprKind::Binary(op, left, right) => {
                if op == "??" {
                    let lhs = self.coalesce_read(left, false)?;
                    return if lhs == Value::Null {
                        self.eval(right)
                    } else {
                        Ok(lhs)
                    };
                }
                let lhs = self.eval(left)?;
                if matches!(op.as_str(), "&&" | "and" | "||" | "or") {
                    let truth = lhs.to_bool(line, &mut self.warnings);
                    if truth == matches!(op.as_str(), "||" | "or") {
                        return Ok(Value::Bool(truth));
                    }
                    let rhs = self.eval(right)?;
                    return Ok(Value::Bool(rhs.to_bool(line, &mut self.warnings)));
                }
                let rhs = self.eval(right)?;
                binary(op, lhs, rhs, line, &mut self.warnings)
            }
        }
    }
}
fn binary(
    op: &str,
    left: Value,
    right: Value,
    line: usize,
    warnings: &mut Vec<String>,
) -> Result<Value, Error> {
    use Value::*;
    match op {
        "." => {
            let mut bytes = string_bytes(&left, line, warnings);
            bytes.extend(string_bytes(&right, line, warnings));
            return Ok(String(bytes));
        }
        "&&" | "and" => return Ok(Bool(left.is_truthy() && right.is_truthy())),
        "||" | "or" => return Ok(Bool(left.is_truthy() || right.is_truthy())),
        "xor" => {
            return Ok(Bool(
                left.to_bool(line, warnings) ^ right.to_bool(line, warnings),
            ));
        }
        "===" => return Ok(Bool(left == right)),
        "!==" => return Ok(Bool(left != right)),
        "==" | "!=" | "<>" | "<" | "<=" | ">" | ">=" | "<=>" => {
            let order = left.compare(&right);
            return Ok(match op {
                "==" => Bool(order == Some(Ordering::Equal)),
                "!=" | "<>" => Bool(order != Some(Ordering::Equal)),
                "<" => Bool(order == Some(Ordering::Less)),
                "<=" => Bool(matches!(order, Some(Ordering::Less | Ordering::Equal))),
                ">" => Bool(order == Some(Ordering::Greater)),
                ">=" => Bool(matches!(order, Some(Ordering::Greater | Ordering::Equal))),
                _ => Int(match order {
                    Some(Ordering::Less) => -1,
                    Some(Ordering::Equal) => 0,
                    _ => 1,
                }),
            });
        }
        _ => {}
    }
    if op == "+"
        && let (Array(a), Array(b)) = (&left, &right)
    {
        let mut result = a.as_ref().clone();
        for (key, value) in b.entries() {
            if result.get(key).is_none() {
                result.insert(key.clone(), value.clone());
            }
        }
        return Ok(Array(Rc::new(result)));
    }
    if matches!(op, "&" | "|" | "^") {
        if let (String(a), String(b)) = (&left, &right) {
            let len = if op == "|" {
                a.len().max(b.len())
            } else {
                a.len().min(b.len())
            };
            return Ok(String(
                (0..len)
                    .map(|i| {
                        let a = a.get(i).copied().unwrap_or(0);
                        let b = b.get(i).copied().unwrap_or(0);
                        match op {
                            "&" => a & b,
                            "|" => a | b,
                            _ => a ^ b,
                        }
                    })
                    .collect(),
            ));
        }
        let a = left.integer(line, warnings)?;
        let b = right.integer(line, warnings)?;
        return Ok(Int(match op {
            "&" => a & b,
            "|" => a | b,
            _ => a ^ b,
        }));
    }
    if matches!(op, "<<" | ">>") {
        let a = left.integer(line, warnings)?;
        let b = right.integer(line, warnings)?;
        if b < 0 {
            return Err(Error::new(
                ErrorKind::ArithmeticError,
                "Bit shift by negative number",
                line,
            ));
        }
        return Ok(Int(if b >= 64 {
            if op == ">>" && a < 0 { -1 } else { 0 }
        } else if op == "<<" {
            a.wrapping_shl(b as u32)
        } else {
            a >> b
        }));
    }
    let left = left.number(line, warnings)?;
    let right = right.number(line, warnings)?;
    let a = left.as_float();
    let b = right.as_float();
    if (op == "/" || op == "%") && b == 0.0 {
        return Err(Error::new(
            ErrorKind::DivisionByZeroError,
            "Division by zero",
            line,
        ));
    }
    if op == "%" {
        let a = left.integer(line, warnings)?;
        let b = right.integer(line, warnings)?;
        if b == 0 {
            return Err(Error::new(
                ErrorKind::DivisionByZeroError,
                "Modulo by zero",
                line,
            ));
        }
        return Ok(Int(if b == -1 { 0 } else { a % b }));
    }
    if let (Int(a), Int(b)) = (&left, &right) {
        let integer = match op {
            "+" => a.checked_add(*b),
            "-" => a.checked_sub(*b),
            "*" => a.checked_mul(*b),
            "/" if a.checked_rem(*b) == Some(0) => a.checked_div(*b),
            "**" => u32::try_from(*b).ok().and_then(|b| a.checked_pow(b)),
            _ => None,
        };
        if let Some(n) = integer {
            return Ok(Int(n));
        }
    }
    Ok(Float(match op {
        "+" => a + b,
        "-" => a - b,
        "*" => a * b,
        "/" => a / b,
        "**" => a.powf(b),
        _ => unreachable!("Unknown operator: {op}"),
    }))
}

fn increment(
    value: &Value,
    delta: i64,
    line: usize,
    warnings: &mut Vec<String>,
) -> Result<Value, Error> {
    use Value::*;
    match value {
        Bool(_) | Null if matches!(value, Bool(_)) || delta < 0 => {
            warnings.push(format!(
                "{} on type {} has no effect on line {line}",
                if delta > 0 { "Increment" } else { "Decrement" },
                if matches!(value, Bool(_)) {
                    "bool"
                } else {
                    "null"
                }
            ));
            Ok(value.clone())
        }
        String(s) if crate::value::numeric_string(s).is_none() => {
            warnings.push(format!(
                "Deprecated: {} on non-numeric string on line {line}",
                if delta > 0 { "Increment" } else { "Decrement" }
            ));
            if delta < 0 {
                return Ok(value.clone());
            }
            if s.is_empty() {
                return Ok(String(b"1".to_vec()));
            }
            let mut bytes = s.clone();
            let mut carry = None;
            for byte in bytes.iter_mut().rev() {
                carry = match *byte {
                    b'0'..=b'8' | b'a'..=b'y' | b'A'..=b'Y' => {
                        *byte += 1;
                        return Ok(String(bytes));
                    }
                    b'9' => {
                        *byte = b'0';
                        Some(b'1')
                    }
                    b'z' => {
                        *byte = b'a';
                        Some(b'a')
                    }
                    b'Z' => {
                        *byte = b'A';
                        Some(b'A')
                    }
                    _ => return Ok(String(bytes)),
                };
            }
            if let Some(carry) = carry {
                bytes.insert(0, carry);
            }
            Ok(String(bytes))
        }
        _ => binary("+", value.clone(), Int(delta), line, warnings),
    }
}

fn string_bytes(value: &Value, line: usize, warnings: &mut Vec<String>) -> Vec<u8> {
    value.warn_nan("string", line, warnings);
    if matches!(value, Value::Array(_)) {
        warnings.push(format!("Array to string conversion on line {line}"));
    }
    value.to_bytes()
}
