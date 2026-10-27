use crate::{
    Error, Value,
    ast::{Argument, Expr, ExprKind, FunctionDecl, Parameter, Stmt},
    lexer::{Kind, StringPart, Token},
};

use std::rc::Rc;

pub(crate) fn parse(tokens: Vec<Token>) -> Result<Vec<Stmt>, Error> {
    Parser {
        tokens,
        pos: 0,
        loops: 0,
    }
    .program()
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    loops: usize,
}
impl Parser {
    fn token(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn is(&self, s: &str) -> bool {
        match &self.token().kind {
            Kind::Symbol(t) => t == s,
            Kind::Word(t) => t.eq_ignore_ascii_case(s.as_bytes()),
            _ => false,
        }
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.is(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, s: &str) -> Result<(), Error> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(Error::parse(format!("Expected '{s}'"), self.token().line))
        }
    }
    fn program(&mut self) -> Result<Vec<Stmt>, Error> {
        let mut result = Vec::new();
        while self.token().kind != Kind::End {
            result.push(self.statement()?);
        }
        Ok(result)
    }
    fn statement(&mut self) -> Result<Stmt, Error> {
        if self.eat("function") {
            return self.function();
        }
        if let Kind::Text(bytes) = self.token().kind.clone() {
            self.pos += 1;
            return Ok(Stmt::Text(bytes));
        }
        if self.eat(";") {
            return Ok(Stmt::Block(Vec::new()));
        }
        if self.eat("{") {
            let mut stmts = Vec::new();
            while !self.eat("}") {
                if self.token().kind == Kind::End {
                    return Err(Error::parse("Unclosed block", self.token().line));
                }
                stmts.push(self.statement()?);
            }
            return Ok(Stmt::Block(stmts));
        }
        if self.eat("echo") {
            let mut values = vec![self.expr(0)?];
            while self.eat(",") {
                values.push(self.expr(0)?);
            }
            self.expect(";")?;
            return Ok(Stmt::Echo(values));
        }
        if self.eat("if") {
            return self.if_statement();
        }
        if self.eat("while") {
            self.expect("(")?;
            let cond = self.expr(0)?;
            self.expect(")")?;
            self.loops += 1;
            let body = self.statement()?;
            self.loops -= 1;
            return Ok(Stmt::While(cond, Box::new(body)));
        }
        if self.eat("foreach") {
            self.expect("(")?;
            let iter = self.expr(0)?;
            self.expect("as")?;
            let first = self.expr(0)?;
            let (key, value) = if self.eat("=>") {
                (Some(first), self.expr(0)?)
            } else {
                (None, first)
            };
            if !value.is_writable() || key.as_ref().is_some_and(|key| !key.is_writable()) {
                return Err(Error::parse(
                    "Invalid foreach assignment target",
                    value.line,
                ));
            }
            self.expect(")")?;
            self.loops += 1;
            let body = self.statement()?;
            self.loops -= 1;
            return Ok(Stmt::Foreach(iter, key, value, Box::new(body)));
        }
        if self.eat("unset") {
            self.expect("(")?;
            let mut targets = Vec::new();
            loop {
                let target = self.expr(0)?;
                if !target.is_writable() {
                    return Err(Error::parse("Invalid unset target", target.line));
                }
                targets.push(target);
                if !self.eat(",") || self.is(")") {
                    break;
                }
            }
            self.expect(")")?;
            self.expect(";")?;
            return Ok(Stmt::Unset(targets));
        }
        if self.is("break") || self.is("continue") {
            if self.loops == 0 {
                return Err(Error::parse(
                    "Loop control outside a loop",
                    self.token().line,
                ));
            }
            let is_break = self.eat("break");
            if !is_break {
                self.expect("continue")?;
            }
            self.expect(";")?;
            return Ok(if is_break {
                Stmt::Break
            } else {
                Stmt::Continue
            });
        }
        if self.eat("return") {
            let value = if self.is(";") {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.expect(";")?;
            return Ok(Stmt::Return(value));
        }
        let expr = self.expr(0)?;
        self.expect(";")?;
        Ok(Stmt::Expr(expr))
    }
    fn function(&mut self) -> Result<Stmt, Error> {
        let line = self.token().line;
        let Kind::Word(name) = self.token().kind.clone() else {
            return Err(Error::parse("Expected function name", line));
        };
        self.pos += 1;
        self.expect("(")?;
        let mut parameters = Vec::new();
        while !self.eat(")") {
            let variadic = self.eat("...");
            let Kind::Variable(name) = self.token().kind.clone() else {
                return Err(Error::parse(
                    "Expected parameter variable",
                    self.token().line,
                ));
            };
            self.pos += 1;
            if parameters.iter().any(|p: &Parameter| p.name == name) {
                return Err(Error::parse("Redefinition of parameter", line));
            }
            let default = if self.eat("=") {
                Some(self.expr(0)?)
            } else {
                None
            };
            if variadic && default.is_some() {
                return Err(Error::parse(
                    "Variadic parameter cannot have a default value",
                    line,
                ));
            }
            if default
                .as_ref()
                .is_some_and(|e| !e.is_constant_expression())
            {
                return Err(Error::parse(
                    "Constant expression contains invalid operations",
                    line,
                ));
            }
            parameters.push(Parameter {
                name,
                default,
                variadic,
            });
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
            if variadic && !self.is(")") {
                return Err(Error::parse(
                    "Only the last parameter can be variadic",
                    line,
                ));
            }
        }
        self.expect("{")?;
        let outer_loops = std::mem::replace(&mut self.loops, 0);
        let mut body = Vec::new();
        while !self.eat("}") {
            if self.token().kind == Kind::End {
                return Err(Error::parse("Unclosed function body", line));
            }
            body.push(self.statement()?);
        }
        self.loops = outer_loops;
        Ok(Stmt::Function(Rc::new(FunctionDecl {
            name,
            parameters,
            body,
            line,
        })))
    }
    fn arguments(&mut self) -> Result<Vec<Argument>, Error> {
        let mut args = Vec::new();
        let mut named = false;
        let mut unpacked = false;
        while !self.eat(")") {
            let line = self.token().line;
            if self.eat("...") {
                if named {
                    return Err(Error::parse(
                        "Cannot use argument unpacking after named arguments",
                        line,
                    ));
                }
                unpacked = true;
                args.push(Argument::Unpack(self.expr(0)?));
            } else if let Kind::Word(name) = self.token().kind.clone()
                && self
                    .tokens
                    .get(self.pos + 1)
                    .is_some_and(|t| t.kind == Kind::Symbol(":".into()))
            {
                named = true;
                self.pos += 2;
                args.push(Argument::Named(name, self.expr(0)?));
            } else {
                if named || unpacked {
                    return Err(Error::parse(
                        "Cannot use positional argument after named argument or unpacking",
                        line,
                    ));
                }
                args.push(Argument::Positional(self.expr(0)?));
            }
            if !self.eat(",") {
                self.expect(")")?;
                break;
            }
        }
        Ok(args)
    }
    fn if_statement(&mut self) -> Result<Stmt, Error> {
        self.expect("(")?;
        let cond = self.expr(0)?;
        self.expect(")")?;
        let yes = self.statement()?;
        let no = if self.eat("elseif") {
            Some(Box::new(self.if_statement()?))
        } else if self.eat("else") {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        Ok(Stmt::If(cond, Box::new(yes), no))
    }
    fn array_elements(&mut self, close: &str) -> Result<Vec<(Option<Expr>, Expr)>, Error> {
        let mut elements = Vec::new();
        while !self.eat(close) {
            let first = self.expr(0)?;
            let (key, value) = if self.eat("=>") {
                (Some(first), self.expr(0)?)
            } else {
                (None, first)
            };
            elements.push((key, value));
            if !self.eat(",") {
                self.expect(close)?;
                break;
            }
        }
        Ok(elements)
    }
    fn expr(&mut self, min: u8) -> Result<Expr, Error> {
        let token = self.token().clone();
        let line = token.line;
        self.pos += 1;
        let kind = match token.kind {
            Kind::Value(value) => ExprKind::Literal(value),
            Kind::Interpolated(parts) => {
                let mut expressions = Vec::new();
                for part in parts {
                    expressions.push(match part {
                        StringPart::Bytes(bytes) => Expr {
                            kind: ExprKind::Literal(Value::String(bytes)),
                            line,
                        },
                        StringPart::Expression(tokens) => {
                            let mut parser = Parser {
                                tokens,
                                pos: 0,
                                loops: 0,
                            };
                            let expr = parser.expr(0)?;
                            if parser.token().kind != Kind::End
                                || !matches!(
                                    expr.kind,
                                    ExprKind::Variable(_)
                                        | ExprKind::Index(_, _)
                                        | ExprKind::Call(_, _)
                                )
                            {
                                return Err(Error::parse(
                                    "Invalid string interpolation expression",
                                    expr.line,
                                ));
                            }
                            expr
                        }
                    });
                }
                ExprKind::Interpolated(expressions)
            }
            Kind::Variable(name) => ExprKind::Variable(name),
            Kind::Word(name) => match name.to_ascii_lowercase().as_slice() {
                b"array" => {
                    self.expect("(")?;
                    ExprKind::Array(self.array_elements(")")?)
                }
                b"isset" => {
                    self.expect("(")?;
                    let mut args = vec![self.expr(0)?];
                    while self.eat(",") {
                        args.push(self.expr(0)?);
                    }
                    self.expect(")")?;
                    if args.iter().any(|e| !e.is_writable()) {
                        return Err(Error::parse("Cannot use isset() on an expression", line));
                    }
                    ExprKind::Isset(args)
                }
                b"empty" => {
                    self.expect("(")?;
                    let arg = self.expr(0)?;
                    self.expect(")")?;
                    ExprKind::Empty(Box::new(arg))
                }
                b"true" => ExprKind::Literal(Value::Bool(true)),
                b"false" => ExprKind::Literal(Value::Bool(false)),
                b"null" => ExprKind::Literal(Value::Null),
                _ => ExprKind::Constant(name),
            },
            Kind::Symbol(op) if op == "[" => ExprKind::Array(self.array_elements("]")?),
            Kind::Symbol(op) if op == "(" => {
                let expr = self.expr(0)?;
                self.expect(")")?;
                ExprKind::Group(Box::new(expr))
            }
            Kind::Symbol(op) if ["+", "-", "!", "~"].contains(&op.as_str()) => {
                ExprKind::Unary(op, Box::new(self.expr(19)?))
            }
            Kind::Symbol(op) if op == "++" || op == "--" => {
                let expr = self.expr(21)?;
                if !expr.is_writable() {
                    return Err(Error::parse("Expected variable", line));
                }
                ExprKind::Increment(Box::new(expr), if op == "++" { 1 } else { -1 }, true)
            }
            _ => return Err(Error::parse("Expected expression", line)),
        };
        let mut lhs = Expr { kind, line };
        loop {
            if min <= 22 && self.eat("(") {
                let args = self.arguments()?;
                lhs = Expr {
                    kind: ExprKind::Call(Box::new(lhs), args),
                    line,
                };
                continue;
            }
            if min <= 22 && self.eat("[") {
                let key = if self.is("]") {
                    None
                } else {
                    Some(Box::new(self.expr(0)?))
                };
                self.expect("]")?;
                lhs = Expr {
                    kind: ExprKind::Index(Box::new(lhs), key),
                    line,
                };
                continue;
            }
            if min <= 21 && (self.is("++") || self.is("--")) {
                let delta = if self.eat("++") {
                    1
                } else {
                    self.expect("--")?;
                    -1
                };
                if !lhs.is_writable() {
                    return Err(Error::parse("Expected variable", line));
                }
                lhs = Expr {
                    kind: ExprKind::Increment(Box::new(lhs), delta, false),
                    line,
                };
                continue;
            }
            if min <= 5 && self.eat("?") {
                let yes = if self.is(":") {
                    None
                } else {
                    Some(Box::new(self.expr(0)?))
                };
                if let ExprKind::Conditional(_, previous_yes, _) = &lhs.kind
                    && (previous_yes.is_some() || yes.is_some())
                {
                    return Err(Error::parse(
                        "Unparenthesized ternary chain is not supported",
                        line,
                    ));
                }
                self.expect(":")?;
                let no = self.expr(6)?;
                lhs = Expr {
                    kind: ExprKind::Conditional(Box::new(lhs), yes, Box::new(no)),
                    line,
                };
                continue;
            }
            let op = match &self.token().kind {
                Kind::Symbol(op) => op.clone(),
                Kind::Word(op) => String::from_utf8_lossy(op).to_ascii_lowercase(),
                _ => break,
            };
            let Some((power, right)) = binding_power(&op) else {
                break;
            };
            if power < min {
                break;
            }
            if (power == 12 || power == 13)
                && let ExprKind::Binary(previous, _, _) = &lhs.kind
                && binding_power(previous).is_some_and(|(p, _)| p == power)
            {
                return Err(Error::parse(
                    "Comparison operators are not associative",
                    line,
                ));
            }
            self.pos += 1;
            let rhs = self.expr(if right { power } else { power + 1 })?;
            let kind = if power == 4 {
                if !lhs.is_writable() {
                    return Err(Error::parse("Cannot assign to expression", line));
                }
                ExprKind::Assign(Box::new(lhs), op, Box::new(rhs))
            } else {
                ExprKind::Binary(op, Box::new(lhs), Box::new(rhs))
            };
            lhs = Expr { kind, line };
        }
        Ok(lhs)
    }
}
fn binding_power(op: &str) -> Option<(u8, bool)> {
    Some(match op {
        "or" => (1, false),
        "xor" => (2, false),
        "and" => (3, false),
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | ".=" | "**=" | "??=" | "&=" | "|=" | "^="
        | "<<=" | ">>=" => (4, true),
        "??" => (6, true),
        "||" => (7, false),
        "&&" => (8, false),
        "==" | "!=" | "<>" | "===" | "!==" | "<=>" => (12, false),
        "<" | "<=" | ">" | ">=" => (13, false),
        "|" => (9, false),
        "^" => (10, false),
        "&" => (11, false),
        "." => (14, false),
        "<<" | ">>" => (15, false),
        "+" | "-" => (16, false),
        "*" | "/" | "%" => (17, false),
        "**" => (20, true),
        _ => return None,
    })
}
