// Altered Rust adaptation of PHP 8.5.0 Zend/zend_ini_scanner.l.
// Copyright (c) Zend Technologies Ltd.; see THIRD_PARTY_NOTICES.md.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Kind {
    End,
    Eol,
    Section,
    Label,
    Offset,
    Raw,
    Constant,
    Number,
    String,
    Whitespace,
    True,
    False,
    Null,
    Variable,
    VarName,
    Fallback,
    Quoted,
    Char(u8),
}
impl Kind {
    pub fn string_part(self) -> bool {
        matches!(
            self,
            Self::Raw
                | Self::Constant
                | Self::Number
                | Self::String
                | Self::Whitespace
                | Self::Variable
                | Self::Char(b'"')
        )
    }
    pub fn name(self) -> String {
        match self {
            Self::End => "end of file",
            Self::Eol => "END_OF_LINE",
            Self::Section => "TC_SECTION",
            Self::Label => "TC_LABEL",
            Self::Offset => "TC_OFFSET",
            Self::Raw => "TC_RAW",
            Self::Constant => "TC_CONSTANT",
            Self::Number => "TC_NUMBER",
            Self::String => "TC_STRING",
            Self::Whitespace => "TC_WHITESPACE",
            Self::True => "BOOL_TRUE",
            Self::False => "BOOL_FALSE",
            Self::Null => "NULL_NULL",
            Self::Variable => "TC_DOLLAR_CURLY",
            Self::VarName => "TC_VARNAME",
            Self::Fallback => "TC_FALLBACK",
            Self::Quoted => "TC_QUOTED_STRING",
            Self::Char(byte) => return format!("'{}'", char::from(byte)),
        }
        .into()
    }
}
pub(super) struct Token {
    pub kind: Kind,
    pub bytes: Vec<u8>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Initial,
    Value,
    Raw,
    Section,
    SectionRaw,
    Offset,
    Quote,
    VarName,
    Fallback,
}
pub(super) struct Scanner<'a> {
    input: &'a [u8],
    pos: usize,
    state: State,
    stack: Vec<State>,
    raw: bool,
    pub line: usize,
}
fn spaces(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}
fn label(b: u8) -> bool {
    !b"=\n\r\t;&|^$~(){}!\"[]\0".contains(&b)
}
fn trim(bytes: &[u8]) -> Vec<u8> {
    let start = bytes
        .iter()
        .position(|b| !matches!(b, b' ' | b'\t'))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t' | b'\r' | b'\n'))
        .map_or(start, |n| n + 1);
    bytes[start..end.max(start)].to_vec()
}
impl<'a> Scanner<'a> {
    pub fn new(input: &'a [u8], mode: i32) -> Self {
        let pos = if input.len() > 3 && input.starts_with(b"\xef\xbb\xbf") {
            3
        } else {
            0
        };
        Self {
            input,
            pos,
            state: State::Initial,
            stack: Vec::new(),
            raw: mode == 1,
            line: 1,
        }
    }
    fn token(&mut self, kind: Kind, len: usize) -> Token {
        let bytes = self.input[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Token { kind, bytes }
    }
    fn push(&mut self, state: State) {
        self.stack.push(self.state);
        self.state = state;
    }
    fn pop(&mut self) {
        self.state = self.stack.pop().unwrap_or(State::Initial);
    }
    pub fn next(&mut self) -> Token {
        loop {
            let s = &self.input[self.pos..];
            if s.is_empty() {
                let kind = if matches!(self.state, State::Value | State::Raw) {
                    self.state = State::Initial;
                    Kind::Eol
                } else {
                    Kind::End
                };
                return self.token(kind, 0);
            }
            let ws = s.iter().take_while(|b| spaces(**b)).count();
            let nl = |s: &[u8]| -> usize {
                if s.starts_with(b"\r\n") {
                    2
                } else if matches!(s.first(), Some(b'\r' | b'\n')) {
                    1
                } else {
                    0
                }
            };
            // Comments and line endings consume preceding horizontal whitespace.
            if matches!(self.state, State::Initial | State::Value | State::Raw) {
                let rest = &s[ws..];
                let end = if rest.first() == Some(&b';') {
                    rest.iter()
                        .position(|b| matches!(b, b'\r' | b'\n'))
                        .unwrap_or(rest.len())
                } else {
                    0
                };
                if rest.first() == Some(&b';') || nl(rest) != 0 {
                    self.pos += ws + end + nl(&rest[end..]);
                    self.line += 1;
                    self.state = State::Initial;
                    return Token {
                        kind: Kind::Eol,
                        bytes: Vec::new(),
                    };
                }
            }
            match self.state {
                State::Initial => {
                    if s[0] == b'[' {
                        self.state = if self.raw {
                            State::SectionRaw
                        } else {
                            State::Section
                        };
                        return self.token(Kind::Section, 1);
                    }
                    if s.get(ws) == Some(&b'=') {
                        let len = ws + 1 + s[ws + 1..].iter().take_while(|b| spaces(**b)).count();
                        self.state = if self.raw { State::Raw } else { State::Value };
                        return self.token(Kind::Char(b'='), len);
                    }
                    let len = s.iter().take_while(|b| label(**b)).count();
                    if len > 0 && s.get(len) == Some(&b'[') {
                        let bytes = trim(&s[..len]);
                        self.pos += len + 1;
                        self.pos += self.input[self.pos..]
                            .iter()
                            .take_while(|b| spaces(**b))
                            .count();
                        self.state = State::Offset;
                        return Token {
                            kind: Kind::Offset,
                            bytes,
                        };
                    }
                    if let Some((kind, n, bytes)) = boolean(s)
                        && n >= len
                    {
                        self.pos += n;
                        return Token { kind, bytes };
                    }
                    if len > ws {
                        let mut t = self.token(Kind::Label, len);
                        t.bytes = trim(&t.bytes);
                        return t;
                    }
                    if ws > 0 {
                        self.pos += ws;
                        continue;
                    }
                    if b":,.[]\"'()&|^+-/*=%$!~<>?@{}".contains(&s[0]) {
                        return self.token(Kind::Char(s[0]), 1);
                    }
                    return self.token(Kind::End, 1);
                }
                State::Raw => {
                    if ws > 0 {
                        self.pos += ws;
                        continue;
                    }
                    let len = s
                        .iter()
                        .position(|b| matches!(b, b'\n' | b'\r'))
                        .unwrap_or(s.len());
                    let mut comment = None;
                    for (i, b) in s[..len].iter().enumerate().skip(1) {
                        if *b == b';' && comment.is_none() {
                            comment = Some(i);
                        }
                        if *b == b'"' && s[0] == b'"' {
                            comment = None;
                        }
                    }
                    let mut bytes = trim(&s[..comment.unwrap_or(len)]);
                    if bytes.len() > 1 && bytes.starts_with(b"\"") && bytes.ends_with(b"\"") {
                        bytes = bytes[1..bytes.len() - 1].to_vec();
                    }
                    self.pos += len;
                    return Token {
                        kind: Kind::Raw,
                        bytes,
                    };
                }
                State::Section | State::SectionRaw if s[0] == b']' => {
                    let len = 1 + s[1..].iter().take_while(|b| spaces(**b)).count();
                    self.pos += len + nl(&s[len..]);
                    self.line += 1;
                    self.state = State::Initial;
                    return Token {
                        kind: Kind::Char(b']'),
                        bytes: Vec::new(),
                    };
                }
                State::SectionRaw => {
                    let len = s
                        .iter()
                        .take_while(|b| !matches!(b, b']' | b'\n' | b'\r'))
                        .count();
                    return self.token(if len > 0 { Kind::Raw } else { Kind::End }, len);
                }
                State::Offset if s.get(ws) == Some(&b']') => {
                    self.state = State::Initial;
                    return self.token(Kind::Char(b']'), ws + 1);
                }
                State::VarName => {
                    if s.starts_with(b":-") {
                        self.state = State::Fallback;
                        return self.token(Kind::Fallback, 2);
                    }
                    if s[0] == b'}' {
                        self.pop();
                        return self.token(Kind::Char(b'}'), 1);
                    }
                    // The native single-character name rule enters fallback before
                    // returning a name when its first character is followed by :-.
                    if label(s[0]) && s[1..].starts_with(b":-") {
                        self.state = State::Fallback;
                        return self.token(Kind::Fallback, 3);
                    }
                    let mut len = 0;
                    while len < s.len() && label(s[len]) && !s[len..].starts_with(b":-") {
                        len += 1;
                    }
                    if len > 0 {
                        let mut token = self.token(Kind::VarName, len);
                        token.bytes = trim(&token.bytes);
                        return token;
                    }
                    return self.token(Kind::End, 1);
                }
                State::Fallback if s[0] == b'}' => {
                    self.pop();
                    return self.token(Kind::Char(b'}'), 1);
                }
                State::Quote => {
                    if s[0] == b'"' {
                        self.pop();
                        return self.token(
                            Kind::Char(b'"'),
                            1 + s[1..].iter().take_while(|b| spaces(**b)).count(),
                        );
                    }
                    if s.starts_with(b"${") {
                        self.push(State::VarName);
                        return self.token(Kind::Variable, 2);
                    }
                    let mut bytes = Vec::new();
                    let mut i = 0;
                    while i < s.len() && s[i] != b'"' && !s[i..].starts_with(b"${") {
                        let b = s[i];
                        i += 1;
                        if b == b'\\' && i < s.len() {
                            let escaped = s[i];
                            if escaped == b'"'
                                && (i + 1 == s.len() || matches!(s[i + 1], b'\r' | b'\n'))
                            {
                                bytes.push(b'\\');
                                break;
                            }
                            if !matches!(escaped, b'"' | b'\\' | b'$') {
                                bytes.push(b'\\');
                            }
                            bytes.push(escaped);
                            i += 1;
                            if escaped == b'\n' || (escaped == b'\r' && s.get(i) != Some(&b'\n')) {
                                self.line += 1;
                            }
                        } else {
                            bytes.push(b);
                            if b == b'\n' || (b == b'\r' && s.get(i) != Some(&b'\n')) {
                                self.line += 1;
                            }
                        }
                    }
                    self.pos += i;
                    return Token {
                        kind: Kind::Quoted,
                        bytes,
                    };
                }
                _ => {}
            }
            if s.starts_with(b"${") {
                self.push(State::VarName);
                return self.token(Kind::Variable, 2);
            }
            if s.get(ws) == Some(&b'"') {
                self.push(State::Quote);
                return self.token(Kind::Char(b'"'), ws + 1);
            }
            if s[0] == b'\''
                && self.state != State::Fallback
                && let Some(end) = s[1..].iter().position(|b| *b == b'\'')
                && end > 0
            {
                let bytes = s[1..end + 1].to_vec();
                self.pos += end + 2;
                return Token {
                    kind: Kind::Raw,
                    bytes,
                };
            }
            if self.state == State::Value {
                if b"&|^~()!".contains(&s[0]) {
                    return self.token(
                        Kind::Char(s[0]),
                        1 + s[1..].iter().take_while(|b| spaces(**b)).count(),
                    );
                }
                if s[0] == b'=' {
                    self.state = State::Initial;
                    return self.token(Kind::Eol, 0);
                }
            }
            // re2c selects the longest match, with constants/numbers winning ties.
            let mut len = 0;
            while len < s.len() {
                let b = s[len];
                let excluded: &[u8] = match self.state {
                    State::Value => b"$= \t\n\r;&|^~()!\"'\0",
                    State::Fallback => b"$\n\r;\"'}\\",
                    _ => b"$\n\r;\"']\\",
                };
                if (b == b'$' && s.get(len + 1).is_some_and(|b| !matches!(b, b'{' | 0)))
                    || (b == b'\\' && self.state != State::Value && len + 1 < s.len())
                {
                    len += if b == b'$' && s.get(len + 1) == Some(&b'\\') && len + 2 < s.len() {
                        3
                    } else {
                        2
                    };
                } else if excluded.contains(&b) {
                    break;
                } else {
                    len += 1;
                }
            }
            let constant = if s[0].is_ascii_alphabetic() || s[0] == b'_' {
                s.iter()
                    .take_while(|b| b.is_ascii_alphanumeric() || **b == b'_')
                    .count()
            } else {
                0
            };
            let number = number_len(s);
            if self.state == State::Value
                && let Some((kind, n, bytes)) = boolean(s)
                && n >= len.max(constant).max(number).max(ws)
            {
                self.pos += n;
                return Token { kind, bytes };
            }
            if constant > 0 && constant >= len.max(number).max(ws) {
                return self.token(Kind::Constant, constant);
            }
            if number > 0 && number >= len.max(ws) {
                return self.token(Kind::Number, number);
            }
            if len > 0 && len >= ws {
                return self.token(Kind::String, len);
            }
            if ws > 0 {
                return self.token(Kind::Whitespace, ws);
            }
            if self.state == State::Value {
                self.state = State::Initial;
                return self.token(Kind::Eol, 1);
            }
            return self.token(Kind::End, 1);
        }
    }
}
fn boolean(s: &[u8]) -> Option<(Kind, usize, Vec<u8>)> {
    for (word, kind) in [
        (b"true".as_slice(), Kind::True),
        (b"on", Kind::True),
        (b"yes", Kind::True),
        (b"false", Kind::False),
        (b"off", Kind::False),
        (b"none", Kind::False),
        (b"no", Kind::False),
        (b"null", Kind::Null),
    ] {
        if s.get(..word.len())
            .is_some_and(|s| s.eq_ignore_ascii_case(word))
        {
            let len = word.len() + s[word.len()..].iter().take_while(|b| spaces(**b)).count();
            return Some((
                kind,
                len,
                if kind == Kind::True {
                    b"1".to_vec()
                } else {
                    Vec::new()
                },
            ));
        }
    }
    None
}
fn number_len(s: &[u8]) -> usize {
    let start = usize::from(s[0] == b'-');
    let digits = s[start..].iter().take_while(|b| b.is_ascii_digit()).count();
    let mut len = if digits > 0 { start + digits } else { 0 };
    // The native DNUM rule does not include a minus sign or exponent.
    if start == 0 && s.get(digits) == Some(&b'.') {
        let fraction = s[digits + 1..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits + fraction > 0 {
            len = digits + 1 + fraction;
        }
    }
    len
}
