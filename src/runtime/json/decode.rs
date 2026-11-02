use super::*;

enum Token {
    Punct(u8),
    Value(Value),
    End,
}

struct Scanner<'a> {
    input: &'a [u8],
    cursor: usize,
    flags: i64,
    validate: bool,
}

impl Scanner<'_> {
    fn next(&mut self) -> Result<Token, i64> {
        while self
            .input
            .get(self.cursor)
            .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.cursor += 1;
        }
        let Some(&byte) = self.input.get(self.cursor) else {
            return Ok(Token::End);
        };
        if matches!(byte, b'[' | b']' | b'{' | b'}' | b':' | b',') {
            self.cursor += 1;
            return Ok(Token::Punct(byte));
        }
        if byte == b'"' {
            return self.string().map(|s| Token::Value(Value::String(s.into())));
        }
        for (literal, value) in [
            (b"null".as_slice(), Value::Null),
            (b"true", Value::Bool(true)),
            (b"false", Value::Bool(false)),
        ] {
            if self.input[self.cursor..].starts_with(literal) {
                self.cursor += literal.len();
                return Ok(Token::Value(value));
            }
        }
        if byte == b'-' || byte.is_ascii_digit() {
            return self.number().map(Token::Value);
        }
        if byte < 32 {
            return Err(3);
        }
        let mut cursor = self.cursor;
        Err(if utf8::next_char(self.input, &mut cursor).is_some() {
            4
        } else {
            5
        })
    }

    fn number(&mut self) -> Result<Value, i64> {
        let start = self.cursor;
        if self.input[self.cursor] == b'-' {
            self.cursor += 1;
        }
        match self.input.get(self.cursor) {
            Some(b'0') => self.cursor += 1,
            Some(b'1'..=b'9') => {
                while self.input.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                    self.cursor += 1;
                }
            }
            _ => return Err(4),
        }
        let mut integer = true;
        if self.input.get(self.cursor) == Some(&b'.') {
            integer = false;
            self.cursor += 1;
            let first = self.cursor;
            while self.input.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                self.cursor += 1;
            }
            if first == self.cursor {
                return Err(4);
            }
        }
        if matches!(self.input.get(self.cursor), Some(b'e' | b'E')) {
            integer = false;
            self.cursor += 1;
            if matches!(self.input.get(self.cursor), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            let first = self.cursor;
            while self.input.get(self.cursor).is_some_and(u8::is_ascii_digit) {
                self.cursor += 1;
            }
            if first == self.cursor {
                return Err(4);
            }
        }
        if self.validate {
            return Ok(Value::Null);
        }
        let text = std::str::from_utf8(&self.input[start..self.cursor]).unwrap();
        if integer {
            if let Ok(n) = text.parse::<i64>() {
                return Ok(Value::Int(n));
            }
            if self.flags & 2 != 0 {
                return Ok(Value::String(text.as_bytes().to_vec().into()));
            }
        }
        Ok(Value::Float(text.parse().map_err(|_| 4i64)?))
    }

    fn hex_unit(&mut self) -> Result<u32, i64> {
        let mut code = 0;
        for _ in 0..4 {
            let byte = *self.input.get(self.cursor).ok_or(4i64)?;
            let digit = (byte as char).to_digit(16).ok_or(4i64)?;
            code = code * 16 + digit;
            self.cursor += 1;
        }
        Ok(code)
    }

    fn string(&mut self) -> Result<Vec<u8>, i64> {
        self.cursor += 1;
        let mut output = Vec::new();
        loop {
            let byte = *self.input.get(self.cursor).ok_or(3i64)?;
            if byte == b'"' {
                self.cursor += 1;
                return Ok(output);
            }
            if byte < 32 {
                return Err(3);
            }
            let code = if byte == b'\\' {
                self.cursor += 1;
                let byte = *self.input.get(self.cursor).ok_or(4i64)?;
                self.cursor += 1;
                match byte {
                    b'"' | b'\\' | b'/' => u32::from(byte),
                    b'b' => 8,
                    b'f' => 12,
                    b'n' => 10,
                    b'r' => 13,
                    b't' => 9,
                    b'u' => {
                        let mut code = self.hex_unit()?;
                        if (0xd800..=0xdbff).contains(&code) {
                            if !self.input[self.cursor..].starts_with(b"\\u") {
                                return Err(10);
                            }
                            self.cursor += 2;
                            let low = self.hex_unit().map_err(|_| 10i64)?;
                            if !(0xdc00..=0xdfff).contains(&low) {
                                return Err(10);
                            }
                            code = 0x10000 + ((code - 0xd800) << 10) + low - 0xdc00;
                        } else if (0xdc00..=0xdfff).contains(&code) {
                            return Err(10);
                        }
                        code
                    }
                    _ => return Err(4),
                }
            } else {
                let start = self.cursor;
                match utf8::next_char(self.input, &mut self.cursor) {
                    Some(code) => code,
                    None => {
                        // Unlike the encoder, the scanner consumes malformed
                        // UTF-8 one byte at a time and replaces each bad byte.
                        self.cursor = start + 1;
                        if self.flags & SUBSTITUTE_UTF8 != 0 {
                            0xfffd
                        } else if self.flags & IGNORE_UTF8 != 0 {
                            continue;
                        } else {
                            return Err(5);
                        }
                    }
                }
            };
            if !self.validate {
                let mut bytes = [0; 4];
                output.extend(
                    char::from_u32(code)
                        .unwrap()
                        .encode_utf8(&mut bytes)
                        .as_bytes(),
                );
            }
        }
    }
}

struct Frame {
    object: bool,
    // 0: initial value/key or end; 1: value/key after comma;
    // 2: colon; 3: object value; 4: comma or end.
    state: u8,
    key: Vec<u8>,
    array: PhpArray,
    instance: Option<Value>,
    count: i64,
}

impl Frame {
    fn new(object: bool) -> Self {
        Self {
            object,
            state: 0,
            key: Vec::new(),
            array: PhpArray::default(),
            instance: None,
            count: 0,
        }
    }
    fn attach(
        &mut self,
        rt: &mut Runtime,
        value: Value,
        flags: i64,
        validate: bool,
    ) -> Result<(), i64> {
        self.state = 4;
        if validate {
            return Ok(());
        }
        if self.object && flags & 1 == 0 {
            if self.key.first() == Some(&0) {
                return Err(9);
            }
            let instance = self
                .instance
                .get_or_insert_with(|| rt.object_from_properties(PhpArray::default()));
            let Value::Object(object) = instance else {
                unreachable!()
            };
            object
                .properties
                .borrow_mut()
                .insert(ArrayKey::String(std::mem::take(&mut self.key)), value);
        } else {
            let key = if self.object {
                let bytes = std::mem::take(&mut self.key);
                match std::str::from_utf8(&bytes)
                    .ok()
                    .and_then(|s| s.parse::<i64>().ok().filter(|n| n.to_string() == s))
                {
                    Some(n) => ArrayKey::Int(n),
                    None => ArrayKey::String(bytes),
                }
            } else {
                ArrayKey::Int(self.count)
            };
            self.array.insert(key, value);
        }
        self.count += 1;
        Ok(())
    }
    fn finish(self, rt: &mut Runtime, flags: i64, validate: bool) -> Value {
        if validate {
            Value::Null
        } else if self.object && flags & 1 == 0 {
            self.instance
                .unwrap_or_else(|| rt.object_from_properties(PhpArray::default()))
        } else {
            Value::Array(Rc::new(self.array))
        }
    }
}

// An explicit grammar stack avoids tying accepted JSON depth to the host stack.
// Validation uses the same scanner and transitions without constructing values.
pub(super) fn parse(
    rt: &mut Runtime,
    input: &[u8],
    flags: i64,
    max_depth: usize,
    validate: bool,
) -> Result<Value, i64> {
    let mut scanner = Scanner {
        input,
        cursor: 0,
        flags,
        validate,
    };
    let mut stack: Vec<Frame> = Vec::new();
    let mut root = None;
    loop {
        let token = scanner.next()?;
        if let Some(root) = root {
            return if matches!(token, Token::End) {
                Ok(root)
            } else {
                Err(4)
            };
        }
        if let Some(frame) = stack.last_mut() {
            let close = if frame.object { b'}' } else { b']' };
            let other_close = if frame.object { b']' } else { b'}' };
            if matches!(frame.state, 0 | 4)
                && matches!(token, Token::Punct(c) if c == close || c == other_close)
            {
                if matches!(token, Token::Punct(c) if c != close) {
                    return Err(2);
                }
                let value = stack.pop().unwrap().finish(rt, flags, validate);
                if let Some(parent) = stack.last_mut() {
                    parent.attach(rt, value, flags, validate)?;
                } else {
                    root = Some(value);
                }
                continue;
            }
            if frame.state == 4 {
                if !matches!(token, Token::Punct(b',')) {
                    return Err(4);
                }
                frame.state = 1;
                continue;
            }
            if frame.object && frame.state <= 1 {
                let Token::Value(Value::String(key)) = token else {
                    return Err(4);
                };
                frame.key = key.to_vec();
                frame.state = 2;
                continue;
            }
            if frame.state == 2 {
                if !matches!(token, Token::Punct(b':')) {
                    return Err(4);
                }
                frame.state = 3;
                continue;
            }
        }
        match token {
            Token::Punct(byte @ (b'[' | b'{')) => {
                if stack.len() + 1 >= max_depth {
                    return Err(1);
                }
                stack.push(Frame::new(byte == b'{'));
            }
            Token::Value(value) => {
                if let Some(frame) = stack.last_mut() {
                    frame.attach(rt, value, flags, validate)?;
                } else {
                    root = Some(value);
                }
            }
            _ => return Err(4),
        }
    }
}
