use super::*;

impl Lexer<'_> {
    pub(super) fn string(&mut self) -> Result<Kind, Error> {
        let quote = self.bump();
        self.string_body(Some(quote), quote == b'"')
    }
    fn string_body(&mut self, quote: Option<u8>, interpolate: bool) -> Result<Kind, Error> {
        let line = self.line;
        let mut out = Vec::new();
        let mut parts = Vec::new();
        while self.pos < self.source.len() {
            if quote == Some(self.source[self.pos]) {
                self.bump();
                return Ok(finish(parts, out));
            }
            if interpolate && self.starts(b"{$") {
                flush(&mut parts, &mut out);
                self.bump();
                let mut nested = Lexer {
                    source: self.source,
                    pos: self.pos,
                    line: self.line,
                    php: true,
                    interpolation: true,
                    tokens: Vec::new(),
                    doc_comment: None,
                };
                let tokens = nested.scan()?;
                self.pos = nested.pos;
                self.line = nested.line;
                parts.push(StringPart::Expression(tokens));
                continue;
            }
            if interpolate
                && self.starts(b"$")
                && self
                    .source
                    .get(self.pos + 1)
                    .is_some_and(|c| ident_start(*c))
            {
                flush(&mut parts, &mut out);
                parts.push(StringPart::Expression(self.simple_variable()?));
                continue;
            }
            if interpolate && self.starts(b"${") {
                return Err(Error::parse(
                    "Deprecated ${...} interpolation is not implemented",
                    self.line,
                ));
            }
            let c = self.bump();
            if c != b'\\' {
                out.push(c);
                continue;
            }
            if self.pos == self.source.len() {
                if quote.is_none() {
                    out.push(c);
                }
                break;
            }
            let c = self.bump();
            if Some(c) == quote || c == b'\\' {
                out.push(c);
                continue;
            }
            if !interpolate {
                out.extend_from_slice(&[b'\\', c]);
                continue;
            }
            match c {
                b'n' => out.push(b'\n'),
                b'r' => out.push(b'\r'),
                b't' => out.push(b'\t'),
                b'v' => out.push(11),
                b'e' => out.push(27),
                b'f' => out.push(12),
                b'$' => out.push(b'$'),
                b'u' if self.starts(b"{") => {
                    self.bump();
                    let mut value = 0u32;
                    let mut digits = 0;
                    while let Some(d) = self
                        .source
                        .get(self.pos)
                        .and_then(|c| (*c as char).to_digit(16))
                    {
                        value = value
                            .checked_mul(16)
                            .and_then(|v| v.checked_add(d))
                            .filter(|v| *v <= 0x10ffff)
                            .ok_or_else(|| {
                                Error::parse("Invalid UTF-8 codepoint escape sequence", self.line)
                            })?;
                        self.bump();
                        digits += 1;
                    }
                    if digits == 0 || !self.starts(b"}") {
                        return Err(Error::parse(
                            "Invalid UTF-8 codepoint escape sequence",
                            self.line,
                        ));
                    }
                    self.bump();
                    utf8(value, &mut out);
                }
                b'x' | b'X' => {
                    let mut value = 0;
                    let mut count = 0;
                    while count < 2 {
                        let Some(d) = self
                            .source
                            .get(self.pos)
                            .and_then(|c| (*c as char).to_digit(16))
                        else {
                            break;
                        };
                        value = value * 16 + d;
                        self.bump();
                        count += 1;
                    }
                    if count == 0 {
                        out.extend_from_slice(&[b'\\', c]);
                    } else {
                        out.push(value as u8);
                    }
                }
                b'0'..=b'7' => {
                    let mut value = u32::from(c - b'0');
                    for _ in 0..2 {
                        let Some(c @ b'0'..=b'7') = self.source.get(self.pos).copied() else {
                            break;
                        };
                        value = value * 8 + u32::from(c - b'0');
                        self.bump();
                    }
                    out.push(value as u8);
                }
                _ => out.extend_from_slice(&[b'\\', c]),
            }
        }
        if quote.is_none() {
            Ok(finish(parts, out))
        } else {
            Err(Error::parse("Unterminated string", line))
        }
    }
    fn simple_variable(&mut self) -> Result<Vec<Token>, Error> {
        let line = self.line;
        self.bump();
        let mut tokens = vec![Token {
            doc_comment: None,
            kind: Kind::Variable(self.identifier()),
            line,
        }];
        if self.starts(b"[") {
            self.bump();
            tokens.push(symbol("[", line));
            let kind = if self.starts(b"$") {
                self.bump();
                if !self.source.get(self.pos).is_some_and(|c| ident_start(*c)) {
                    return Err(Error::parse("Expected variable in string offset", line));
                }
                Kind::Variable(self.identifier())
            } else if self.source.get(self.pos).is_some_and(|c| ident_start(*c)) {
                Kind::Value(Value::String(self.identifier().into()))
            } else {
                let start = self.pos;
                if self.starts(b"-") {
                    self.bump();
                }
                let digit_start = self.pos;
                while self.source.get(self.pos).is_some_and(u8::is_ascii_digit) {
                    self.bump();
                }
                if self.pos == digit_start {
                    return Err(Error::parse("Invalid string interpolation offset", line));
                }
                Kind::Value(Value::String(self.source[start..self.pos].to_vec().into()))
            };
            tokens.push(Token {
                doc_comment: None,
                kind,
                line,
            });
            if !self.starts(b"]") {
                return Err(Error::parse("Expected ']' in string interpolation", line));
            }
            self.bump();
            tokens.push(symbol("]", line));
        } else {
            let op = if self.starts(b"?->") { "?->" } else { "->" };
            if self.starts(op.as_bytes())
                && self
                    .source
                    .get(self.pos + op.len())
                    .is_some_and(|c| ident_start(*c))
            {
                self.pos += op.len();
                tokens.push(symbol(op, line));
                tokens.push(Token {
                    doc_comment: None,
                    kind: Kind::Word(self.identifier()),
                    line,
                });
            }
        }
        tokens.push(Token {
            doc_comment: None,
            kind: Kind::End,
            line,
        });
        Ok(tokens)
    }
    pub(super) fn heredoc(&mut self) -> Result<Kind, Error> {
        let start_line = self.line;
        self.pos += 3;
        while matches!(self.source.get(self.pos), Some(b' ' | b'\t')) {
            self.bump();
        }
        let quote = match self.source.get(self.pos) {
            Some(b'\'' | b'"') => Some(self.bump()),
            _ => None,
        };
        if !self.source.get(self.pos).is_some_and(|c| ident_start(*c)) {
            return Err(Error::parse("Expected heredoc identifier", self.line));
        }
        let label = self.identifier();
        if let Some(quote) = quote {
            if self.source.get(self.pos) != Some(&quote) {
                return Err(Error::parse("Unterminated heredoc identifier", self.line));
            }
            self.bump();
        }
        if self.starts(b"\r\n") {
            self.bump();
            self.bump();
        } else if self.starts(b"\n") {
            self.bump();
        } else {
            return Err(Error::parse(
                "Expected newline after heredoc identifier",
                self.line,
            ));
        }
        let body_start = self.pos;
        let body_line = self.line;
        while self.pos < self.source.len() {
            let line_start = self.pos;
            while matches!(self.source.get(self.pos), Some(b' ' | b'\t')) {
                self.bump();
            }
            let indent = &self.source[line_start..self.pos];
            if self.starts(&label)
                && !self
                    .source
                    .get(self.pos + label.len())
                    .is_some_and(|c| ident_start(*c) || c.is_ascii_digit())
            {
                let mut body = &self.source[body_start..line_start];
                if let Some(b) = body.strip_suffix(b"\r\n") {
                    body = b;
                } else if let Some(b) = body.strip_suffix(b"\n") {
                    body = b;
                }
                let body = dedent(body, indent, body_line)?;
                self.pos += label.len();
                if quote == Some(b'\'') {
                    return Ok(Kind::Value(Value::String(body.into())));
                }
                return Lexer {
                    source: &body,
                    pos: 0,
                    line: body_line,
                    php: true,
                    interpolation: false,
                    tokens: Vec::new(),
                    doc_comment: None,
                }
                .string_body(None, true);
            }
            while self.pos < self.source.len() && self.bump() != b'\n' {}
        }
        Err(Error::parse("Unterminated heredoc", start_line))
    }
}
fn flush(parts: &mut Vec<StringPart>, out: &mut Vec<u8>) {
    if !out.is_empty() {
        parts.push(StringPart::Bytes(std::mem::take(out)));
    }
}
fn finish(mut parts: Vec<StringPart>, mut out: Vec<u8>) -> Kind {
    if parts.is_empty() {
        Kind::Value(Value::String(out.into()))
    } else {
        flush(&mut parts, &mut out);
        Kind::Interpolated(parts)
    }
}
fn symbol(text: &str, line: usize) -> Token {
    Token {
        doc_comment: None,
        kind: Kind::Symbol(text.into()),
        line,
    }
}
fn dedent(body: &[u8], indent: &[u8], line: usize) -> Result<Vec<u8>, Error> {
    if indent.is_empty() {
        return Ok(body.to_vec());
    }
    if indent.iter().any(|c| *c != indent[0]) {
        return Err(Error::parse(
            "Tabs and spaces cannot be mixed in heredoc indentation",
            line,
        ));
    }
    let mut out = Vec::new();
    for (offset, bytes) in body.split_inclusive(|c| *c == b'\n').enumerate() {
        let whitespace = bytes
            .iter()
            .take_while(|c| **c == b' ' || **c == b'\t')
            .count();
        let strip = whitespace.min(indent.len());
        if bytes[..strip].iter().any(|c| *c != indent[0]) {
            return Err(Error::parse(
                "Tabs and spaces cannot be mixed in heredoc indentation",
                line + offset,
            ));
        }
        if whitespace < indent.len()
            && !bytes[whitespace..]
                .iter()
                .all(|c| matches!(c, b'\r' | b'\n'))
        {
            return Err(Error::parse(
                "Invalid heredoc body indentation level",
                line + offset,
            ));
        }
        out.extend_from_slice(&bytes[strip..]);
    }
    Ok(out)
}
fn utf8(codepoint: u32, out: &mut Vec<u8>) {
    // PHP permits surrogate codepoints; Rust char intentionally does not.
    if codepoint < 0x80 {
        out.push(codepoint as u8);
    } else if codepoint < 0x800 {
        out.extend_from_slice(&[
            (0xc0 | codepoint >> 6) as u8,
            (0x80 | codepoint & 0x3f) as u8,
        ]);
    } else if codepoint < 0x10000 {
        out.extend_from_slice(&[
            (0xe0 | codepoint >> 12) as u8,
            (0x80 | codepoint >> 6 & 0x3f) as u8,
            (0x80 | codepoint & 0x3f) as u8,
        ]);
    } else {
        out.extend_from_slice(&[
            (0xf0 | codepoint >> 18) as u8,
            (0x80 | codepoint >> 12 & 0x3f) as u8,
            (0x80 | codepoint >> 6 & 0x3f) as u8,
            (0x80 | codepoint & 0x3f) as u8,
        ]);
    }
}
