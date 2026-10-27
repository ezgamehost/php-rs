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
            _ => self.eval(expr),
        }
    }
    fn eval(&mut self, expr: &Expr) -> Result<Value, Error> {
        let line = expr.line;
        match &expr.kind {
            ExprKind::Literal(value) => Ok(value.clone()),
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
                    "+" => value.number(line),
                    "-" => binary("-", Value::Int(0), value, line),
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
                    binary(&op[..op.len() - 1], lhs, rhs, line)?
                };
                self.variables.insert(name.clone(), value.clone());
                Ok(value)
            }
            ExprKind::Increment(name, delta, prefix) => {
                let old = self.read(name, line);
                let new = binary("+", old.clone(), Value::Int(*delta), line)?;
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
                binary(op, lhs, rhs, line)
            }
        }
    }
}
fn binary(op: &str, left: Value, right: Value, line: usize) -> Result<Value, Error> {
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
    let left = left.number(line)?;
    let right = right.number(line)?;
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
        let a = match left {
            Int(n) => n,
            _ => a as i64,
        };
        let b = match right {
            Int(n) => n,
            _ => b as i64,
        };
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
