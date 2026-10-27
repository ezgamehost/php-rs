use crate::{
    Error, Value,
    ast::{
        Argument, ArrayElement, CastKind, Expr, ExprKind, FunctionDecl, IncludeKind, MatchArm,
        Parameter, Stmt, SwitchCase, TypeDecl,
    },
    lexer::{Kind, StringPart, Token},
};

use std::{collections::HashSet, rc::Rc};
mod jumps;
mod targets;
mod types;
mod variables;

pub(crate) fn parse(tokens: Vec<Token>) -> Result<(Vec<Stmt>, Vec<String>, bool), Error> {
    parse_source(tokens, Rc::new(crate::source::Source::default()))
}

pub(crate) fn parse_source(
    tokens: Vec<Token>,
    source: Rc<crate::source::Source>,
) -> Result<(Vec<Stmt>, Vec<String>, bool), Error> {
    let mut parser = Parser {
        tokens,
        pos: 0,
        control_depth: 0,
        warnings: Vec::new(),
        strict_types: false,
        return_type: None,
        static_names: HashSet::new(),
        source,
        function_name: Vec::new(),
    };
    let statements = parser.program()?;
    targets::validate(&statements)?;
    jumps::validate(&statements)?;
    Ok((statements, parser.warnings, parser.strict_types))
}
struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    control_depth: usize,
    warnings: Vec<String>,
    strict_types: bool,
    return_type: Option<TypeDecl>,
    static_names: HashSet<Vec<u8>>,
    source: Rc<crate::source::Source>,
    function_name: Vec<u8>,
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
        let mut first = true;
        while self.token().kind != Kind::End {
            if self.eat("declare") {
                self.strict_declaration(first)?;
                continue;
            }
            if self.eat(";") {
                continue;
            }
            first = false;
            result.push(if self.eat("const") {
                self.constant_declaration()?
            } else {
                self.statement()?
            });
        }
        Ok(result)
    }
    fn constant_declaration(&mut self) -> Result<Stmt, Error> {
        let mut constants = Vec::new();
        loop {
            let line = self.token().line;
            let Kind::Word(name) = self.token().kind.clone() else {
                return Err(Error::parse("Expected constant name", line));
            };
            if name.contains(&b'\\')
                || is_magic_constant(&name)
                || [b"true".as_slice(), b"false", b"null"]
                    .iter()
                    .any(|literal| name.eq_ignore_ascii_case(literal))
            {
                return Err(Error::parse("Invalid constant name", line));
            }
            self.pos += 1;
            self.expect("=")?;
            let value = self.expr(0)?;
            if !value.is_constant_expression() {
                return Err(Error::parse(
                    "Constant expression contains invalid operations",
                    line,
                ));
            }
            constants.push((name, value));
            if !self.eat(",") {
                break;
            }
        }
        self.expect(";")?;
        Ok(Stmt::Const(constants))
    }
    fn statement(&mut self) -> Result<Stmt, Error> {
        let line = self.token().line;
        if self.eat("goto") {
            let name = self.label_name()?;
            self.expect(";")?;
            return Ok(Stmt::Goto(name, line));
        }
        if matches!(self.token().kind, Kind::Word(_))
            && matches!(self.tokens.get(self.pos + 1).map(|t| &t.kind), Some(Kind::Symbol(s)) if s == ":")
        {
            let name = self.label_name()?;
            self.expect(":")?;
            return Ok(Stmt::Label(name, line));
        }
        if self.eat("global") {
            return self.global_declaration();
        }
        if self.eat("static") {
            return self.static_declaration();
        }
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
            return self.if_statement(true);
        }
        if self.eat("switch") {
            return self.switch_statement();
        }
        if self.eat("while") {
            self.expect("(")?;
            let cond = self.expr(0)?;
            self.expect(")")?;
            let body = self.loop_body(Some("endwhile"))?;
            return Ok(Stmt::While(cond, Box::new(body)));
        }
        if self.eat("do") {
            let body = self.loop_body(None)?;
            self.expect("while")?;
            self.expect("(")?;
            let cond = self.expr(0)?;
            self.expect(")")?;
            self.expect(";")?;
            return Ok(Stmt::DoWhile(Box::new(body), cond));
        }
        if self.eat("for") {
            self.expect("(")?;
            let init = self.expression_list(";")?;
            let cond = self.expression_list(";")?;
            if cond
                .last()
                .is_some_and(|expr| matches!(expr.kind, ExprKind::Cast(CastKind::Void, _)))
            {
                return Err(Error::parse(
                    "The final for condition cannot discard its value",
                    self.token().line,
                ));
            }
            let step = self.expression_list(")")?;
            let body = self.loop_body(Some("endfor"))?;
            return Ok(Stmt::For(init, cond, step, Box::new(body)));
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
            let value = targets::assignment_target(value)?;
            if key.as_ref().is_some_and(|key| !key.is_writable()) {
                return Err(Error::parse(
                    "Invalid foreach assignment target",
                    value.line,
                ));
            }
            self.expect(")")?;
            let body = self.loop_body(Some("endforeach"))?;
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
            let line = self.token().line;
            if self.control_depth == 0 {
                return Err(Error::parse(
                    "Loop control outside a loop",
                    self.token().line,
                ));
            }
            let is_break = self.eat("break");
            if !is_break {
                self.expect("continue")?;
            }
            let level = if self.is(";") {
                1
            } else {
                let expr = self.expr(0)?;
                let mut operand = &expr;
                while let ExprKind::Group(inner) = &operand.kind {
                    operand = inner;
                }
                let ExprKind::Literal(Value::Int(level)) = operand.kind else {
                    return Err(Error::parse(
                        "Loop control requires a positive integer literal",
                        expr.line,
                    ));
                };
                if level <= 0 || level as u64 > self.control_depth as u64 {
                    return Err(Error::parse("Invalid loop control depth", expr.line));
                }
                level as usize
            };
            self.expect(";")?;
            return Ok(if is_break {
                Stmt::Break(level)
            } else {
                Stmt::Continue(level, line)
            });
        }
        if self.eat("return") {
            let value = if self.is(";") {
                None
            } else {
                Some(self.expr(0)?)
            };
            self.validate_return(value.as_ref())?;
            self.expect(";")?;
            return Ok(Stmt::Return(value));
        }
        let expr = self.discardable_expression()?;
        self.expect(";")?;
        Ok(Stmt::Expr(expr))
    }
    fn discardable_expression(&mut self) -> Result<Expr, Error> {
        if matches!(&self.token().kind, Kind::Cast(name) if name == "void") {
            let line = self.token().line;
            self.pos += 1;
            return Ok(Expr {
                kind: ExprKind::Cast(CastKind::Void, Box::new(self.expr(0)?)),
                line,
            });
        }
        self.expr(0)
    }
    fn switch_statement(&mut self) -> Result<Stmt, Error> {
        self.expect("(")?;
        let subject = self.expr(0)?;
        self.expect(")")?;
        let alternative = self.eat(":");
        if !alternative {
            self.expect("{")?;
        }
        let end = if alternative { "endswitch" } else { "}" };
        self.eat(";");
        self.control_depth += 1;
        let mut cases = Vec::new();
        let mut has_default = false;
        while !self.eat(end) {
            let condition = if self.eat("case") {
                Some(self.expr(0)?)
            } else {
                self.expect("default")?;
                if has_default {
                    return Err(Error::parse(
                        "Switch statements may only contain one default clause",
                        self.token().line,
                    ));
                }
                has_default = true;
                None
            };
            let semicolon_line = if self.is(";") {
                let line = self.token().line;
                self.pos += 1;
                Some(line)
            } else {
                self.expect(":")?;
                None
            };
            let mut body = Vec::new();
            while !self.is("case") && !self.is("default") && !self.is(end) {
                body.push(self.statement()?);
            }
            cases.push(SwitchCase {
                condition,
                body,
                semicolon_line,
            });
        }
        self.control_depth -= 1;
        if alternative {
            self.expect(";")?;
        }
        Ok(Stmt::Switch(subject, cases))
    }
    fn expression_list(&mut self, end: &str) -> Result<Vec<Expr>, Error> {
        let mut expressions = Vec::new();
        if self.eat(end) {
            return Ok(expressions);
        }
        loop {
            expressions.push(self.discardable_expression()?);
            if !self.eat(",") {
                self.expect(end)?;
                return Ok(expressions);
            }
        }
    }
    fn loop_body(&mut self, end: Option<&str>) -> Result<Stmt, Error> {
        self.control_depth += 1;
        let body = if let Some(end) = end.filter(|_| self.is(":")) {
            self.expect(":")?;
            let mut body = Vec::new();
            while !self.eat(end) {
                body.push(self.statement()?);
            }
            self.expect(";")?;
            Stmt::Block(body)
        } else {
            self.statement()?
        };
        self.control_depth -= 1;
        Ok(body)
    }
    fn function(&mut self) -> Result<Stmt, Error> {
        let line = self.token().line;
        let Kind::Word(name) = self.token().kind.clone() else {
            return Err(Error::parse("Expected function name", line));
        };
        if name.contains(&b'\\') || is_magic_constant(&name) {
            return Err(Error::parse(
                "Function declarations require an unqualified name",
                line,
            ));
        }
        self.pos += 1;
        let outer_function_name = std::mem::replace(&mut self.function_name, name.clone());
        self.expect("(")?;
        let mut parameters = Vec::new();
        while !self.eat(")") {
            let ty = if self.is("?") || matches!(self.token().kind, Kind::Word(_)) {
                Some(self.type_declaration(false)?)
            } else {
                None
            };
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
                ty,
                implicit_nullable: false,
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
        let return_type = if self.eat(":") {
            Some(self.type_declaration(true)?)
        } else {
            None
        };
        self.validate_defaults(&name, &mut parameters, line)?;
        self.expect("{")?;
        let outer_return_type = std::mem::replace(&mut self.return_type, return_type.clone());
        let outer_static_names = std::mem::take(&mut self.static_names);
        let outer_control_depth = std::mem::replace(&mut self.control_depth, 0);
        let mut body = Vec::new();
        while !self.eat("}") {
            if self.token().kind == Kind::End {
                return Err(Error::parse("Unclosed function body", line));
            }
            body.push(self.statement()?);
        }
        self.control_depth = outer_control_depth;
        self.return_type = outer_return_type;
        self.static_names = outer_static_names;
        self.function_name = outer_function_name;
        Ok(Stmt::Function(Rc::new(FunctionDecl {
            name,
            parameters,
            body,
            line,
            return_type,
            strict_types: self.strict_types,
            source: self.source.clone(),
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
    fn if_statement(&mut self, allow_alternative: bool) -> Result<Stmt, Error> {
        self.expect("(")?;
        let cond = self.expr(0)?;
        self.expect(")")?;
        if self.is(":") {
            if !allow_alternative {
                return Err(Error::parse(
                    "Cannot mix if statement syntaxes",
                    self.token().line,
                ));
            }
            return self.alternative_if(cond);
        }
        let yes = self.statement()?;
        let no = if self.eat("elseif") {
            Some(Box::new(self.if_statement(false)?))
        } else if self.eat("else") {
            Some(Box::new(self.statement()?))
        } else {
            None
        };
        Ok(Stmt::If(cond, Box::new(yes), no))
    }
    fn alternative_if(&mut self, mut condition: Expr) -> Result<Stmt, Error> {
        let mut branches = Vec::new();
        loop {
            self.expect(":")?;
            let mut body = Vec::new();
            while !self.is("elseif") && !self.is("else") && !self.is("endif") {
                body.push(self.statement()?);
            }
            branches.push((condition, Stmt::Block(body)));
            if !self.eat("elseif") {
                break;
            }
            self.expect("(")?;
            condition = self.expr(0)?;
            self.expect(")")?;
        }
        let mut alternative = if self.eat("else") {
            self.expect(":")?;
            let mut body = Vec::new();
            while !self.is("endif") {
                body.push(self.statement()?);
            }
            Some(Box::new(Stmt::Block(body)))
        } else {
            None
        };
        self.expect("endif")?;
        self.expect(";")?;
        for (condition, body) in branches.into_iter().rev() {
            alternative = Some(Box::new(Stmt::If(condition, Box::new(body), alternative)));
        }
        Ok(*alternative.unwrap())
    }
    fn array_elements(&mut self, close: &str) -> Result<Vec<ArrayElement>, Error> {
        let mut elements = Vec::new();
        while !self.eat(close) {
            if self.eat(",") {
                elements.push(ArrayElement::Hole);
                continue;
            }
            let element = if self.eat("...") {
                ArrayElement::Unpack(self.expr(0)?)
            } else {
                let first = self.expr(0)?;
                if self.eat("=>") {
                    ArrayElement::Entry(Some(first), self.expr(0)?)
                } else {
                    ArrayElement::Entry(None, first)
                }
            };
            elements.push(element);
            if !self.eat(",") {
                self.expect(close)?;
                break;
            }
        }
        Ok(elements)
    }
    fn match_expression(&mut self) -> Result<ExprKind, Error> {
        self.expect("(")?;
        let subject = self.expr(0)?;
        self.expect(")")?;
        self.expect("{")?;
        let mut arms = Vec::new();
        let mut has_default = false;
        while !self.eat("}") {
            let mut conditions = Vec::new();
            if self.eat("default") {
                if has_default {
                    return Err(Error::parse(
                        "Match expressions may only contain one default arm",
                        self.token().line,
                    ));
                }
                has_default = true;
                self.eat(",");
            } else {
                loop {
                    conditions.push(self.expr(0)?);
                    if !self.eat(",") || self.is("=>") {
                        break;
                    }
                }
            }
            self.expect("=>")?;
            let result = self.expr(0)?;
            arms.push(MatchArm { conditions, result });
            if !self.eat(",") {
                self.expect("}")?;
                break;
            }
        }
        Ok(ExprKind::Match(Box::new(subject), arms))
    }
    fn expr(&mut self, min: u8) -> Result<Expr, Error> {
        let token = self.token().clone();
        let line = token.line;
        self.pos += 1;
        let kind = match token.kind {
            Kind::Cast(name) => {
                let (kind, canonical) = match name.as_str() {
                    "int" | "integer" => (CastKind::Int, "int"),
                    "float" | "double" => (CastKind::Float, "float"),
                    "bool" | "boolean" => (CastKind::Bool, "bool"),
                    "string" | "binary" => (CastKind::String, "string"),
                    "array" => (CastKind::Array, "array"),
                    _ => {
                        return Err(Error::parse(
                            format!("Unsupported cast ({name}) in this context"),
                            line,
                        ));
                    }
                };
                if name != canonical {
                    self.warnings.push(format!("Deprecated: Non-canonical cast ({name}) is deprecated, use the ({canonical}) cast instead on line {line}"));
                }
                ExprKind::Cast(kind, Box::new(self.expr(19)?))
            }
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
                                control_depth: 0,
                                warnings: Vec::new(),
                                strict_types: false,
                                return_type: None,
                                static_names: HashSet::new(),
                                source: self.source.clone(),
                                function_name: self.function_name.clone(),
                            };
                            let expr = parser.expr(0)?;
                            self.warnings.append(&mut parser.warnings);
                            if parser.token().kind != Kind::End
                                || !matches!(
                                    expr.kind,
                                    ExprKind::Variable(_)
                                        | ExprKind::DynamicVariable(_)
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
            Kind::Symbol(op) if op == "$" => self.dynamic_variable()?,
            Kind::Word(name) => match name.to_ascii_lowercase().as_slice() {
                b"__line__" => ExprKind::Literal(Value::Int(line as i64)),
                b"__file__" => ExprKind::Literal(Value::String(self.source.name.clone())),
                b"__dir__" => ExprKind::Literal(Value::String(
                    self.source
                        .directory
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                )),
                b"__function__" | b"__method__" => {
                    ExprKind::Literal(Value::String(self.function_name.clone()))
                }
                b"__class__" | b"__namespace__" | b"__trait__" => {
                    ExprKind::Literal(Value::String(Vec::new()))
                }
                b"eval" => {
                    self.expect("(")?;
                    let code = self.expr(0)?;
                    self.expect(")")?;
                    ExprKind::Eval(Box::new(code))
                }
                b"include" | b"include_once" | b"require" | b"require_once" => {
                    let kind = match name.to_ascii_lowercase().as_slice() {
                        b"include" => IncludeKind::Include,
                        b"include_once" => IncludeKind::IncludeOnce,
                        b"require" => IncludeKind::Require,
                        _ => IncludeKind::RequireOnce,
                    };
                    ExprKind::Include(kind, Box::new(self.expr(0)?))
                }
                b"match" => self.match_expression()?,
                b"print" => ExprKind::Print(Box::new(self.expr(4)?)),
                b"array" => {
                    self.expect("(")?;
                    ExprKind::LongArray(self.array_elements(")")?)
                }
                b"list" => {
                    self.expect("(")?;
                    ExprKind::List(self.array_elements(")")?)
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
            if matches!(lhs.kind, ExprKind::Match(_, _)) && (self.is("[") || self.is("(")) {
                return Err(Error::parse(
                    "Parenthesize a match expression before dereferencing it",
                    self.token().line,
                ));
            }
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
                if op == "=" {
                    lhs = targets::assignment_target(lhs)?;
                } else if !lhs.is_writable() {
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

fn is_magic_constant(name: &[u8]) -> bool {
    matches!(
        name.to_ascii_lowercase().as_slice(),
        b"__line__"
            | b"__file__"
            | b"__dir__"
            | b"__function__"
            | b"__method__"
            | b"__class__"
            | b"__namespace__"
            | b"__trait__"
    )
}
