use crate::{
    ArrayKey, Error, ErrorKind, PhpArray, Value,
    ast::{Expr, ExprKind, Stmt},
    lexer, parser,
};
use std::{cmp::Ordering, collections::HashMap, rc::Rc};

/// One request's variable scope, output, and diagnostics. Output survives errors.
#[derive(Default)]
pub struct Runtime {
    pub output: Vec<u8>,
    pub warnings: Vec<String>,
    pub variables: HashMap<Vec<u8>, Value>,
}
struct Location {
    name: Vec<u8>,
    keys: Vec<Option<ArrayKey>>,
}
enum Flow {
    Next,
    Break,
    Continue,
    Return(Value),
}
impl Runtime {
    /// Parse the complete document before executing any of it.
    pub fn execute(&mut self, source: &[u8]) -> Result<Value, Error> {
        let stmts = parser::parse(lexer::lex(source)?)?;
        match self.block(&stmts)? {
            Flow::Return(value) => Ok(value),
            _ => Ok(Value::Null),
        }
    }
    fn block(&mut self, stmts: &[Stmt]) -> Result<Flow, Error> {
        for stmt in stmts {
            match self.statement(stmt)? {
                Flow::Next => {}
                flow => return Ok(flow),
            }
        }
        Ok(Flow::Next)
    }
    fn statement(&mut self, stmt: &Stmt) -> Result<Flow, Error> {
        match stmt {
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
                if self.eval(cond)?.is_truthy() {
                    return self.statement(yes);
                }
                if let Some(no) = no {
                    return self.statement(no);
                }
            }
            Stmt::While(cond, body) => {
                while self.eval(cond)?.is_truthy() {
                    match self.statement(body)? {
                        Flow::Next | Flow::Continue => {}
                        Flow::Break => break,
                        flow => return Ok(flow),
                    }
                }
            }
            Stmt::Foreach(iter, key_target, value_target, body) => {
                let value = self.eval(iter)?;
                if let Value::Array(array) = value {
                    for (key, value) in array.entries() {
                        let location = self.location(value_target, false)?;
                        self.write_location(&location, value.clone(), value_target.line)?;
                        if let Some(target) = key_target {
                            let location = self.location(target, false)?;
                            self.write_location(&location, key.to_value(), target.line)?;
                        }
                        match self.statement(body)? {
                            Flow::Next | Flow::Continue => {}
                            Flow::Break => break,
                            flow => return Ok(flow),
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
                    let location = self.location(target, true)?;
                    if location.keys.is_empty() {
                        self.variables.remove(&location.name);
                    } else if let Some(value) = self.variables.get_mut(&location.name) {
                        unset_path(value, &location.keys, target.line)?;
                    }
                }
            }
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
            Stmt::Return(value) => {
                return Ok(Flow::Return(match value {
                    Some(expr) => self.eval(expr)?,
                    None => Value::Null,
                }));
            }
        }
        Ok(Flow::Next)
    }
    fn read(&mut self, name: &[u8], line: usize) -> Value {
        if let Some(value) = self.variables.get(name) {
            value.clone()
        } else {
            self.warnings.push(format!(
                "Undefined variable ${} on line {line}",
                String::from_utf8_lossy(name)
            ));
            Value::Null
        }
    }
    fn location(&mut self, expr: &Expr, unsetting: bool) -> Result<Location, Error> {
        match &expr.kind {
            ExprKind::Variable(name) => Ok(Location {
                name: name.clone(),
                keys: Vec::new(),
            }),
            ExprKind::Index(base, key) => {
                let mut location = self.location(base, unsetting)?;
                let key = match key {
                    Some(key) => {
                        let value = self.eval(key)?;
                        Some(if unsetting && value == Value::Null {
                            ArrayKey::String(Vec::new())
                        } else {
                            ArrayKey::from_value(&value, key.line, &mut self.warnings)?
                        })
                    }
                    None => None,
                };
                location.keys.push(key);
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
        location: &Location,
        line: usize,
        silent: bool,
    ) -> Result<Value, Error> {
        let mut value = if silent {
            self.variables
                .get(&location.name)
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            self.read(&location.name, line)
        };
        for key in &location.keys {
            let key = key
                .as_ref()
                .ok_or_else(|| Error::new(ErrorKind::Error, "Cannot use [] for reading", line))?;
            value = read_offset(value, &key.to_value(), line, silent, &mut self.warnings)?;
        }
        Ok(value)
    }
    fn write_location(
        &mut self,
        location: &Location,
        new_value: Value,
        line: usize,
    ) -> Result<(), Error> {
        let value = self
            .variables
            .entry(location.name.clone())
            .or_insert(Value::Null);
        write_path(value, &location.keys, new_value, line, &mut self.warnings)
    }
    fn coalesce_read(&mut self, expr: &Expr) -> Result<Value, Error> {
        match &expr.kind {
            ExprKind::Variable(name) => {
                Ok(self.variables.get(name).cloned().unwrap_or(Value::Null))
            }
            ExprKind::Group(inner) => self.coalesce_read(inner),
            ExprKind::Index(base, Some(key)) => {
                let value = self.coalesce_read(base)?;
                let key = self.eval(key)?;
                read_offset(value, &key, expr.line, true, &mut self.warnings)
            }
            _ => self.eval(expr),
        }
    }
    fn eval(&mut self, expr: &Expr) -> Result<Value, Error> {
        let line = expr.line;
        match &expr.kind {
            ExprKind::Literal(value) => Ok(value.clone()),
            ExprKind::Group(inner) => self.eval(inner),
            ExprKind::Variable(name) => Ok(self.read(name, line)),
            ExprKind::Array(elements) => {
                let mut array = PhpArray::default();
                for (key, expr) in elements {
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
            ExprKind::Index(base, key) => {
                let value = self.eval(base)?;
                let key = key.as_ref().ok_or_else(|| {
                    Error::new(ErrorKind::Error, "Cannot use [] for reading", line)
                })?;
                let key = self.eval(key)?;
                read_offset(value, &key, line, false, &mut self.warnings)
            }
            ExprKind::Isset(args) => {
                for arg in args {
                    if self.coalesce_read(arg)? == Value::Null {
                        return Ok(Value::Bool(false));
                    }
                }
                Ok(Value::Bool(true))
            }
            ExprKind::Empty(arg) => Ok(Value::Bool(!self.coalesce_read(arg)?.is_truthy())),
            ExprKind::Constant(name) => Err(Error::new(
                ErrorKind::Error,
                format!("Undefined constant \"{}\"", String::from_utf8_lossy(name)),
                line,
            )),
            ExprKind::Unary(op, expr) => {
                let value = self.eval(expr)?;
                match op.as_str() {
                    "!" => Ok(Value::Bool(!value.is_truthy())),
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
                let location = self.location(target, false)?;
                let value = if op == "=" {
                    self.eval(expr)?
                } else if op == "??=" {
                    match self.read_location(&location, line, true)? {
                        Value::Null => self.eval(expr)?,
                        value => value,
                    }
                } else {
                    let lhs = self.read_location(&location, line, false)?;
                    let rhs = self.eval(expr)?;
                    binary(&op[..op.len() - 1], lhs, rhs, line, &mut self.warnings)?
                };
                self.write_location(&location, value.clone(), line)?;
                Ok(value)
            }
            ExprKind::Increment(target, delta, prefix) => {
                let location = self.location(target, false)?;
                let old = self.read_location(&location, line, false)?;
                let new = increment(&old, *delta, line, &mut self.warnings)?;
                self.write_location(&location, new.clone(), line)?;
                Ok(if *prefix { new } else { old })
            }
            ExprKind::Conditional(cond, yes, no) => {
                let value = self.eval(cond)?;
                if value.is_truthy() {
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
                    let lhs = self.coalesce_read(left)?;
                    return if lhs == Value::Null {
                        self.eval(right)
                    } else {
                        Ok(lhs)
                    };
                }
                let lhs = self.eval(left)?;
                if (op == "&&" || op == "and") && !lhs.is_truthy() {
                    return Ok(Value::Bool(false));
                }
                if (op == "||" || op == "or") && lhs.is_truthy() {
                    return Ok(Value::Bool(true));
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
        "xor" => return Ok(Bool(left.is_truthy() ^ right.is_truthy())),
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
    if matches!(value, Value::Array(_)) {
        warnings.push(format!("Array to string conversion on line {line}"));
    }
    value.to_bytes()
}
fn read_offset(
    value: Value,
    key: &Value,
    line: usize,
    silent: bool,
    warnings: &mut Vec<String>,
) -> Result<Value, Error> {
    match value {
        Value::Array(array) => {
            let key = ArrayKey::from_value(key, line, warnings)?;
            if let Some(value) = array.get(&key) {
                return Ok(value.clone());
            }
            if !silent {
                warnings.push(format!(
                    "Undefined array key {} on line {line}",
                    String::from_utf8_lossy(&key.to_value().to_bytes())
                ));
            }
            Ok(Value::Null)
        }
        Value::String(_) => Err(Error::new(
            ErrorKind::TypeError,
            "String offsets are not implemented",
            line,
        )),
        _ => {
            if !silent {
                warnings.push(format!(
                    "Trying to access array offset on a scalar on line {line}"
                ));
            }
            Ok(Value::Null)
        }
    }
}
fn write_path(
    value: &mut Value,
    keys: &[Option<ArrayKey>],
    new_value: Value,
    line: usize,
    warnings: &mut Vec<String>,
) -> Result<(), Error> {
    let Some((key, rest)) = keys.split_first() else {
        *value = new_value;
        return Ok(());
    };
    if matches!(value, Value::Null | Value::Bool(false)) {
        if matches!(value, Value::Bool(false)) {
            warnings.push(format!(
                "Deprecated: Automatic conversion of false to array on line {line}"
            ));
        }
        *value = Value::Array(Rc::new(PhpArray::default()));
    }
    let Value::Array(array) = value else {
        return Err(Error::new(
            ErrorKind::Error,
            "Cannot use a scalar value as an array",
            line,
        ));
    };
    let array = Rc::make_mut(array);
    let key = match key {
        Some(key) => key.clone(),
        None => array.append_key(line)?,
    };
    if rest.is_empty() {
        array.insert(key, new_value);
        return Ok(());
    }
    if array.get(&key).is_none() {
        array.insert(key.clone(), Value::Null);
    }
    write_path(
        array.get_mut(&key).unwrap(),
        rest,
        new_value,
        line,
        warnings,
    )
}
fn unset_path(value: &mut Value, keys: &[Option<ArrayKey>], line: usize) -> Result<(), Error> {
    let Some((key, rest)) = keys.split_first() else {
        return Ok(());
    };
    let key = key
        .as_ref()
        .ok_or_else(|| Error::new(ErrorKind::Error, "Cannot use [] for unsetting", line))?;
    match value {
        Value::Array(array) => {
            let array = Rc::make_mut(array);
            if rest.is_empty() {
                array.remove(key);
            } else if let Some(value) = array.get_mut(key) {
                unset_path(value, rest, line)?;
            }
            Ok(())
        }
        Value::Null => Ok(()),
        _ => Err(Error::new(
            ErrorKind::Error,
            "Cannot unset offset in a non-array variable",
            line,
        )),
    }
}
