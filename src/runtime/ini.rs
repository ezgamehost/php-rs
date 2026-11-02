// Altered Rust adaptation of PHP 8.5.0 Zend/zend_ini_parser.y,
// Zend/zend_ini_scanner.l, and ext/standard/basic_functions.c.
// Copyright (c) Zend Technologies Ltd. and The PHP Group.
// See THIRD_PARTY_NOTICES.md for licenses and attribution.
use super::*;
mod scanner;
use scanner::{Kind, Scanner, Token};

pub(super) fn constant(name: &[u8]) -> Option<i64> {
    match name {
        b"INI_SCANNER_NORMAL" => Some(0),
        b"INI_SCANNER_RAW" => Some(1),
        b"INI_SCANNER_TYPED" => Some(2),
        _ => None,
    }
}

impl Runtime {
    pub(super) fn parse_ini(
        &mut self,
        file: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let mut input = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        if file && input.contains(&0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 ($filename) must not contain any null bytes"),
                line,
            ));
        }
        let sections = args[1]
            .as_ref()
            .map(|v| self.native_bool(v, name, 2, line))
            .transpose()?
            .unwrap_or(false);
        let mode = args[2]
            .as_ref()
            .map(|v| self.native_int(v, name, 3, line))
            .transpose()?
            .unwrap_or(0) as i32;
        let filename = if file {
            if input.is_empty() {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!("{name}(): Argument #1 ($filename) must not be empty"),
                    line,
                ));
            }
            let filename = String::from_utf8_lossy(&input).into_owned();
            let opened = self.open_stream(name, &input, b"rb", true, line)?;
            if !matches!(opened, Value::Resource(_)) {
                return Ok(Value::Bool(false));
            }
            input = self.stream_contents_bytes(&opened, name, None, line)?;
            filename
        } else {
            // The native string entry point uses strlen; file buffers retain NULs.
            if let Some(end) = input.iter().position(|b| *b == 0) {
                input.truncate(end);
            }
            "Unknown".into()
        };
        if !(0..=2).contains(&mode) {
            self.warn(format!("Invalid scanner mode on line {line}"))?;
            return Ok(Value::Bool(false));
        }
        let mut parser = Parser::new(self, &input, mode, line);
        match parser.parse(sections) {
            Ok(value) => Ok(Value::Array(Rc::new(value))),
            Err(message) => {
                let ini_line = parser.scanner.line;
                self.warn(format!(
                    "{message} in {filename} on line {ini_line}\n on line {line}"
                ))?;
                Ok(Value::Bool(false))
            }
        }
    }
}

struct Item {
    value: Value,
    number: bool,
}
impl Item {
    fn string(bytes: Vec<u8>) -> Self {
        Self {
            value: Value::String(bytes.into()),
            number: false,
        }
    }
    fn bytes(&self) -> Vec<u8> {
        self.value.to_bytes()
    }
}

struct Parser<'a> {
    runtime: &'a mut Runtime,
    scanner: Scanner<'a>,
    token: Token,
    mode: i32,
    depth: usize,
    line: usize,
}

impl<'a> Parser<'a> {
    fn new(runtime: &'a mut Runtime, input: &'a [u8], mode: i32, line: usize) -> Self {
        let mut scanner = Scanner::new(input, mode);
        let token = scanner.next();
        Self {
            runtime,
            scanner,
            token,
            mode,
            depth: 0,
            line,
        }
    }
    fn advance(&mut self) -> Token {
        std::mem::replace(&mut self.token, self.scanner.next())
    }
    fn error(&self, expected: &str) -> String {
        format!(
            "syntax error, unexpected {}{}",
            self.token.kind.name(),
            expected
        )
    }
    fn expect(&mut self, kind: Kind, expected: &str) -> Result<(), String> {
        if self.token.kind != kind {
            return Err(self.error(expected));
        }
        self.advance();
        Ok(())
    }
    fn key(&mut self, bytes: Vec<u8>) -> ArrayKey {
        ArrayKey::from_value(&Value::String(bytes.into()), self.line, self.runtime).unwrap()
    }
    fn parse(&mut self, sections: bool) -> Result<PhpArray, String> {
        let mut result = PhpArray::default();
        let mut section: Option<(ArrayKey, PhpArray)> = None;
        loop {
            match self.token.kind {
                Kind::End => break,
                Kind::Eol => {
                    self.advance();
                }
                Kind::Section => {
                    self.advance();
                    let title = self.strings(true)?.bytes();
                    // Reduce the section before fetching the next statement.
                    if self.token.kind != Kind::Char(b']') {
                        return Err(self.error(", expecting ']'"));
                    }
                    if sections {
                        if let Some((key, entries)) = section.take() {
                            result.insert(key, Value::Array(Rc::new(entries)));
                        }
                        let key = self.key(title);
                        result.insert(key.clone(), Value::Array(Rc::new(PhpArray::default())));
                        section = Some((key, PhpArray::default()));
                    }
                    self.advance();
                }
                Kind::Label | Kind::Offset => {
                    let label = self.advance();
                    let offset = if label.kind == Kind::Offset {
                        let offset = self.strings(false)?.bytes();
                        self.expect(Kind::Char(b']'), ", expecting ']'")?;
                        self.expect(Kind::Char(b'='), ", expecting '='")?;
                        Some(offset)
                    } else if self.token.kind == Kind::Char(b'=') {
                        self.advance();
                        None
                    } else {
                        continue;
                    };
                    let mut value = match self.token.kind {
                        Kind::Eol => {
                            self.advance();
                            Item::string(Vec::new())
                        }
                        Kind::True | Kind::False | Kind::Null => {
                            let token = self.advance();
                            Item {
                                value: if self.mode == 2 {
                                    match token.kind {
                                        Kind::True => Value::Bool(true),
                                        Kind::Null => Value::Null,
                                        _ => Value::Bool(false),
                                    }
                                } else {
                                    Value::String(token.bytes.into())
                                },
                                number: false,
                            }
                        }
                        _ => self.expression()?,
                    };
                    if self.mode == 2 && value.number {
                        let bytes = value.bytes();
                        if let Some(number) = crate::value::numeric_string(&bytes) {
                            // Integer overflow is kept as a string, unlike decimal floating point.
                            let integer = bytes.iter().all(|b| {
                                b.is_ascii_digit() || matches!(b, b'-' | b'+' | b' ' | b'\t')
                            });
                            if !integer || matches!(number, Value::Int(_)) {
                                value.value = number;
                            }
                        }
                    }
                    let key = if offset.is_some()
                        && !(label.bytes.len() > 1 && label.bytes[0] == b'0')
                        && matches!(
                            crate::value::numeric_string(&label.bytes),
                            Some(Value::Int(_))
                        ) {
                        // Array-entry labels use strtoul(base 0), unlike scalar keys.
                        let bytes = &label.bytes;
                        let negative = bytes.first() == Some(&b'-');
                        let digits = bytes
                            .strip_prefix(b"-")
                            .or_else(|| bytes.strip_prefix(b"+"))
                            .unwrap_or(bytes);
                        let radix = if digits.first() == Some(&b'0') { 8 } else { 10 };
                        let n = digits
                            .iter()
                            .take_while(|b| b.is_ascii_digit() && **b - b'0' < radix)
                            .fold(0i64, |n, b| {
                                n.wrapping_mul(radix.into())
                                    .wrapping_add(i64::from(*b - b'0'))
                            });
                        ArrayKey::Int(if negative { n.wrapping_neg() } else { n })
                    } else {
                        self.key(label.bytes)
                    };
                    let entries = section
                        .as_mut()
                        .map(|(_, entries)| entries)
                        .unwrap_or(&mut result);
                    if let Some(offset) = offset {
                        let mut nested = match entries.get(&key) {
                            Some(Value::Array(a)) => (*a).clone(),
                            _ => PhpArray::default(),
                        };
                        if offset.is_empty() {
                            // Native add_next_index_zval silently rejects an exhausted index.
                            let _ = nested.append(value.value, self.line);
                        } else {
                            let key = ArrayKey::from_value(
                                &Value::String(offset.into()),
                                self.line,
                                self.runtime,
                            )
                            .unwrap();
                            nested.insert(key, value.value);
                        }
                        entries.insert(key, Value::Array(Rc::new(nested)));
                    } else {
                        entries.insert(key, value.value);
                    }
                }
                _ => return Err(self.error("")),
            }
        }
        if let Some((key, entries)) = section {
            result.insert(key, Value::Array(Rc::new(entries)));
        }
        Ok(result)
    }
    fn expression(&mut self) -> Result<Item, String> {
        let mut left = self.unary()?;
        while let Kind::Char(op @ (b'|' | b'&' | b'^')) = self.token.kind {
            self.advance();
            let right = self.unary()?;
            let a = integer(&left.bytes());
            let b = integer(&right.bytes());
            left = self.operation(match op {
                b'|' => a | b,
                b'&' => a & b,
                _ => a ^ b,
            });
        }
        Ok(left)
    }
    fn operation(&self, value: i32) -> Item {
        Item {
            value: if self.mode == 2 {
                Value::Int(value.into())
            } else {
                Value::String(value.to_string().into_bytes().into())
            },
            number: false,
        }
    }
    fn unary(&mut self) -> Result<Item, String> {
        self.depth += 1;
        if self.depth > 512 {
            return Err("memory exhausted".into());
        }
        let result = match self.token.kind {
            Kind::Char(op @ (b'~' | b'!')) => {
                self.advance();
                let value = self.unary()?;
                let n = integer(&value.bytes());
                self.operation(if op == b'~' { !n } else { i32::from(n == 0) })
            }
            Kind::Char(b'(') => {
                self.advance();
                let value = self.expression()?;
                self.expect(Kind::Char(b')'), ", expecting ')'")?;
                value
            }
            _ if self.token.kind.string_part() => self.strings(false)?,
            _ => return Err(self.error("")),
        };
        self.depth -= 1;
        Ok(result)
    }
    fn strings(&mut self, literal: bool) -> Result<Item, String> {
        let mut result: Option<Item> = None;
        while self.token.kind.string_part() {
            let part = match self.token.kind {
                Kind::Variable => self.variable()?,
                Kind::Char(b'"') => {
                    self.advance();
                    let mut bytes = Vec::new();
                    while matches!(self.token.kind, Kind::Quoted | Kind::Variable) {
                        if self.token.kind == Kind::Variable {
                            bytes.extend(self.variable()?.bytes());
                        } else {
                            bytes.extend(self.advance().bytes);
                        }
                    }
                    self.expect(
                        Kind::Char(b'"'),
                        ", expecting TC_DOLLAR_CURLY or TC_QUOTED_STRING or '\"'",
                    )?;
                    Item::string(bytes)
                }
                _ => {
                    let token = self.advance();
                    if token.kind == Kind::Constant && !literal {
                        Item::string(
                            self.runtime
                                .lookup_constant(&token.bytes)
                                .map(|v| v.to_bytes())
                                .unwrap_or(token.bytes),
                        )
                    } else {
                        Item {
                            value: Value::String(token.bytes.into()),
                            number: token.kind == Kind::Number && !literal,
                        }
                    }
                }
            };
            if let Some(result) = &mut result {
                let mut bytes = result.bytes();
                bytes.extend(part.bytes());
                result.value = Value::String(bytes.into());
            } else {
                result = Some(part);
            }
        }
        Ok(result.unwrap_or_else(|| Item::string(Vec::new())))
    }
    fn variable(&mut self) -> Result<Item, String> {
        self.depth += 1;
        if self.depth > 512 {
            return Err("memory exhausted".into());
        }
        self.advance();
        if self.token.kind != Kind::VarName {
            return Err(self.error(", expecting TC_VARNAME"));
        }
        let name = self.advance().bytes;
        let fallback = if self.token.kind == Kind::Fallback {
            self.advance();
            Some(self.strings(false)?.bytes())
        } else {
            None
        };
        self.expect(
            Kind::Char(b'}'),
            if fallback.is_some() {
                ", expecting '}'"
            } else {
                ", expecting TC_FALLBACK or '}'"
            },
        )?;
        self.depth -= 1;
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let value = std::env::var_os(std::ffi::OsStr::from_bytes(&name))
            .map(|v| v.into_vec())
            .or(fallback)
            .unwrap_or_default();
        Ok(Item::string(value))
    }
}

fn integer(bytes: &[u8]) -> i32 {
    let bytes = bytes.trim_ascii_start();
    let negative = bytes.first() == Some(&b'-');
    let bytes = if matches!(bytes.first(), Some(b'+' | b'-')) {
        &bytes[1..]
    } else {
        bytes
    };
    let limit = if negative {
        1u64 << 63
    } else {
        i64::MAX as u64
    };
    let n = bytes
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .fold(0u64, |n, b| {
            n.saturating_mul(10)
                .saturating_add(u64::from(*b - b'0'))
                .min(limit)
        }) as i32;
    if negative { n.wrapping_neg() } else { n }
}
