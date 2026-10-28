use crate::{Error, Value};
mod strings;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Kind {
    Text(Vec<u8>),
    Interpolated(Vec<StringPart>),
    Value(Value),
    Variable(Vec<u8>),
    Word(Vec<u8>),
    Cast(String),
    Symbol(String),
    End,
}
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub kind: Kind,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum StringPart {
    Bytes(Vec<u8>),
    Expression(Vec<Token>),
}

pub(crate) fn lex(source: &[u8]) -> Result<Vec<Token>, Error> {
    Lexer {
        source,
        pos: 0,
        line: 1,
        php: false,
        interpolation: false,
        tokens: Vec::new(),
    }
    .scan()
}
struct Lexer<'a> {
    source: &'a [u8],
    pos: usize,
    line: usize,
    php: bool,
    interpolation: bool,
    tokens: Vec<Token>,
}
impl Lexer<'_> {
    fn cast(&mut self) -> Option<String> {
        let mut end = self.pos + 1;
        while matches!(self.source.get(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        let start = end;
        while self.source.get(end).is_some_and(u8::is_ascii_alphabetic) {
            end += 1;
        }
        let name = String::from_utf8(self.source[start..end].to_ascii_lowercase()).ok()?;
        if ![
            "int", "integer", "float", "double", "real", "bool", "boolean", "string", "binary",
            "array", "object", "unset", "void",
        ]
        .contains(&name.as_str())
        {
            return None;
        }
        while matches!(self.source.get(end), Some(b' ' | b'\t')) {
            end += 1;
        }
        if self.source.get(end) != Some(&b')') {
            return None;
        }
        self.pos = end + 1;
        Some(name)
    }
    fn starts(&self, s: &[u8]) -> bool {
        self.source[self.pos..].starts_with(s)
    }
    fn bump(&mut self) -> u8 {
        let c = self.source[self.pos];
        self.pos += 1;
        if c == b'\n' {
            self.line += 1;
        }
        c
    }
    fn emit(&mut self, kind: Kind, line: usize) {
        self.tokens.push(Token { kind, line });
    }
    fn opening(&self) -> usize {
        if self.starts(b"<?=") {
            3
        } else if self
            .source
            .get(self.pos..self.pos + 5)
            .is_some_and(|s| s.eq_ignore_ascii_case(b"<?php"))
            && self
                .source
                .get(self.pos + 5)
                .is_some_and(u8::is_ascii_whitespace)
        {
            5
        } else {
            0
        }
    }
    fn scan(&mut self) -> Result<Vec<Token>, Error> {
        let mut depth = usize::from(self.interpolation);
        while self.pos < self.source.len() {
            let line = self.line;
            if !self.php {
                let start = self.pos;
                while self.pos < self.source.len() && self.opening() == 0 {
                    self.bump();
                }
                if self.pos > start {
                    self.emit(Kind::Text(self.source[start..self.pos].to_vec()), line);
                }
                let opening = self.opening();
                if opening > 0 {
                    if opening == 3 {
                        self.emit(Kind::Word(b"echo".to_vec()), self.line);
                    }
                    self.pos += opening;
                    self.php = true;
                }
                continue;
            }
            let c = self.source[self.pos];
            if self.interpolation {
                if c == b'{' {
                    depth += 1;
                }
                if c == b'}' {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        self.emit(Kind::End, line);
                        return Ok(std::mem::take(&mut self.tokens));
                    }
                }
            }
            if c.is_ascii_whitespace() {
                self.bump();
                continue;
            }
            if self.starts(b"?>") {
                self.pos += 2;
                self.php = false;
                self.emit(Kind::Symbol(";".into()), line);
                if self.starts(b"\r\n") {
                    self.bump();
                    self.bump();
                } else if self.starts(b"\n") {
                    self.bump();
                }
                continue;
            }
            if self.starts(b"//") || c == b'#' {
                while self.pos < self.source.len()
                    && self.source[self.pos] != b'\n'
                    && !self.starts(b"?>")
                {
                    self.bump();
                }
                continue;
            }
            if self.starts(b"/*") {
                self.pos += 2;
                while self.pos < self.source.len() && !self.starts(b"*/") {
                    self.bump();
                }
                if self.pos == self.source.len() {
                    return Err(Error::parse("Unterminated comment", line));
                }
                self.pos += 2;
                continue;
            }
            let kind = if c == b'('
                && let Some(name) = self.cast()
            {
                Kind::Cast(name)
            } else if self.starts(b"<<<") {
                self.heredoc()?
            } else if (c == b'b' || c == b'B')
                && matches!(self.source.get(self.pos + 1), Some(b'\'' | b'"'))
            {
                self.bump();
                self.string()?
            } else if c == b'\'' || c == b'"' {
                self.string()?
            } else if c.is_ascii_digit()
                || c == b'.'
                    && self
                        .source
                        .get(self.pos + 1)
                        .is_some_and(u8::is_ascii_digit)
            {
                Kind::Value(self.number()?)
            } else if c == b'$' {
                self.bump();
                if self.source.get(self.pos).is_some_and(|c| ident_start(*c)) {
                    Kind::Variable(self.identifier())
                } else {
                    Kind::Symbol("$".into())
                }
            } else if ident_start(c) || c == b'\\' {
                let start = self.pos;
                if c == b'\\' {
                    self.bump();
                }
                loop {
                    if !self.source.get(self.pos).is_some_and(|c| ident_start(*c)) {
                        return Err(Error::parse("Expected qualified name segment", line));
                    }
                    self.identifier();
                    if self.source.get(self.pos) != Some(&b'\\') {
                        break;
                    }
                    self.bump();
                }
                Kind::Word(self.source[start..self.pos].to_vec())
            } else {
                let operators = [
                    "...", "::", "?->", "->", "=>", "[", "]", "<<=", ">>=", "<<", ">>", "&=", "|=",
                    "^=", "===", "!==", "<=>", "**=", "??=", "**", "==", "!=", "<>", "<=", ">=",
                    "&&", "||", "|>", "??", "+=", "-=", "*=", "/=", "%=", ".=", "++", "--", "(",
                    ")", "{", "}", ";", ",", "+", "-", "*", "/", "%", ".", "=", "<", ">", "!", "?",
                    ":", "~", "&", "|", "^",
                ];
                let Some(op) = operators.iter().find(|op| self.starts(op.as_bytes())) else {
                    return Err(Error::parse(format!("Unexpected byte 0x{c:02x}"), line));
                };
                self.pos += op.len();
                Kind::Symbol((*op).into())
            };
            self.emit(kind, line);
        }
        if self.interpolation {
            return Err(Error::parse("Unterminated string interpolation", self.line));
        }
        self.emit(Kind::End, self.line);
        Ok(std::mem::take(&mut self.tokens))
    }
    fn identifier(&mut self) -> Vec<u8> {
        let start = self.pos;
        while self
            .source
            .get(self.pos)
            .is_some_and(|c| ident_start(*c) || c.is_ascii_digit())
        {
            self.bump();
        }
        self.source[start..self.pos].to_vec()
    }
    fn digits(&mut self, radix: u32) -> Result<usize, Error> {
        let mut count = 0;
        let mut separator = false;
        while let Some(c) = self.source.get(self.pos).copied() {
            if (c as char).is_digit(radix) {
                count += 1;
                separator = false;
                self.bump();
            } else if c == b'_' {
                if count == 0 || separator {
                    return Err(Error::parse("Invalid numeric separator", self.line));
                }
                separator = true;
                self.bump();
            } else {
                break;
            }
        }
        if separator {
            return Err(Error::parse("Invalid numeric separator", self.line));
        }
        Ok(count)
    }
    fn number(&mut self) -> Result<Value, Error> {
        let start = self.pos;
        let line = self.line;
        for (prefix, radix) in [(b"0x", 16), (b"0b", 2), (b"0o", 8)] {
            if self
                .source
                .get(self.pos..self.pos + 2)
                .is_some_and(|s| s.eq_ignore_ascii_case(prefix))
            {
                self.pos += 2;
                let digits_start = self.pos;
                if self.digits(radix)? == 0 {
                    return Err(Error::parse("Expected digits", line));
                }
                let bytes: Vec<_> = self.source[digits_start..self.pos]
                    .iter()
                    .copied()
                    .filter(|c| *c != b'_')
                    .collect();
                return Ok(integer(&bytes, radix));
            }
        }
        self.digits(10)?;
        let mut float = false;
        if self.starts(b".") {
            float = true;
            self.bump();
            self.digits(10)?;
        }
        if self.starts(b"e") || self.starts(b"E") {
            float = true;
            self.bump();
            if self.starts(b"+") || self.starts(b"-") {
                self.bump();
            }
            if self.digits(10)? == 0 {
                return Err(Error::parse("Expected exponent digits", line));
            }
        }
        let bytes: Vec<_> = self.source[start..self.pos]
            .iter()
            .copied()
            .filter(|c| *c != b'_')
            .collect();
        if float {
            let n = std::str::from_utf8(&bytes)
                .unwrap()
                .parse()
                .map_err(|_| Error::parse("Invalid float", line))?;
            Ok(Value::Float(n))
        } else if bytes.len() > 1 && bytes[0] == b'0' {
            if bytes.iter().any(|c| *c > b'7') {
                return Err(Error::parse("Invalid octal integer", line));
            }
            Ok(integer(&bytes, 8))
        } else {
            Ok(integer(&bytes, 10))
        }
    }
}
fn ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c >= 128
}
fn integer(bytes: &[u8], radix: u32) -> Value {
    let text = std::str::from_utf8(bytes).unwrap();
    if let Ok(n) = i64::from_str_radix(text, radix) {
        Value::Int(n)
    } else {
        Value::Float(text.chars().fold(0.0, |n, c| {
            n * radix as f64 + c.to_digit(radix).unwrap() as f64
        }))
    }
}
