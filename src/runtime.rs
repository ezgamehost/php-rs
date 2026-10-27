use crate::{
    Error, ErrorKind, Value,
    ast::{Expr, ExprKind, Stmt},
    lexer, parser,
};
use std::{cmp::Ordering, collections::HashMap};

/// One request's variable scope, output, and diagnostics. Output survives errors.
#[derive(Default)]
pub struct Runtime {
    pub output: Vec<u8>,
    pub warnings: Vec<String>,
    pub variables: HashMap<Vec<u8>, Value>,
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
                    self.output.extend(v.to_bytes());
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
    fn coalesce_read(&mut self, expr: &Expr) -> Result<Value, Error> {
        match &expr.kind {
            ExprKind::Variable(name) => {
                Ok(self.variables.get(name).cloned().unwrap_or(Value::Null))
            }
            ExprKind::Group(inner) => self.coalesce_read(inner),
            _ => self.eval(expr),
        }
    }
    fn eval(&mut self, expr: &Expr) -> Result<Value, Error> {
        let line = expr.line;
        match &expr.kind {
            ExprKind::Literal(value) => Ok(value.clone()),
            ExprKind::Group(inner) => self.eval(inner),
            ExprKind::Variable(name) => Ok(self.read(name, line)),
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
            ExprKind::Assign(name, op, expr) => {
                let value = if op == "=" {
                    self.eval(expr)?
                } else if op == "??=" {
                    match self.variables.get(name).cloned().unwrap_or(Value::Null) {
                        Value::Null => self.eval(expr)?,
                        value => value,
                    }
                } else {
                    let lhs = self.read(name, line);
                    let rhs = self.eval(expr)?;
                    binary(&op[..op.len() - 1], lhs, rhs, line, &mut self.warnings)?
                };
                self.variables.insert(name.clone(), value.clone());
                Ok(value)
            }
            ExprKind::Increment(name, delta, prefix) => {
                let old = self.read(name, line);
                let new = increment(&old, *delta, line, &mut self.warnings)?;
                self.variables.insert(name.clone(), new.clone());
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
            let mut bytes = left.to_bytes();
            bytes.extend(right.to_bytes());
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
