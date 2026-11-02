// Altered Rust adaptation of PHP 8.5.0 ext/standard/scanf.c, string.c and file.c.
// Copyright (c) The PHP Group. scanf.c derives from Tcl 8.3.0; see
// THIRD_PARTY_NOTICES.md for the PHP and Tcl copyright and license notices.
use super::*;
use std::ffi::c_char;

impl Runtime {
    pub(super) fn scanf(
        &mut self,
        stream: bool,
        name: &str,
        args: &[Option<Value>],
        extra: PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        let mut input = if stream {
            if !matches!(first, Value::Resource(_)) {
                return Err(builtins::type_error(first, name, 1, "resource", line));
            }
            Vec::new()
        } else {
            self.native_string(first, name, 1, line)?
        };
        let format = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
        if extra
            .iter()
            .any(|(key, _)| matches!(key, ArrayKey::String(_)))
        {
            return Err(Error::new(
                ErrorKind::ArgumentCountError,
                format!("{name}() does not accept unknown named parameters"),
                line,
            ));
        }
        if stream {
            let Value::Resource(resource) = first else {
                unreachable!()
            };
            if !matches!(*resource.kind.borrow(), crate::resource::Kind::Stream(_)) {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    "fscanf(): supplied resource is not a valid File-Handle resource",
                    line,
                ));
            }
            let Some(text) = self.stream_read_line(first, name, None, line)? else {
                return Ok(Value::Bool(false));
            };
            input = text;
        }
        let variables: Vec<_> = extra
            .iter()
            .map(|(_, value)| {
                if let Value::Reference(reference) = value {
                    reference
                } else {
                    cell(value)
                }
            })
            .collect();
        let (tokens, slots) = compile(c_string(&format), variables.len(), line)?;
        let mut values = vec![Value::Null; slots];
        let mut scanner = Scanner {
            input: c_string(&input),
            offset: 0,
            underflow: false,
        };
        let mut conversions = 0;
        let mut pending = None;
        for token in tokens {
            match token {
                Token::Whitespace => scanner.skip_space(),
                Token::Literal(byte) => {
                    let Some(&actual) = scanner.input.get(scanner.offset) else {
                        scanner.underflow = true;
                        break;
                    };
                    scanner.offset += 1;
                    if actual != byte {
                        break;
                    }
                }
                Token::Field { index, width, kind } => {
                    let Some(value) = scanner.field(width, &kind) else {
                        break;
                    };
                    if let Some(index) = index {
                        if variables.is_empty() {
                            values[index] = value;
                        } else {
                            self.assign_native_reference(
                                &variables[index],
                                value,
                                &mut pending,
                                line,
                            )?;
                        }
                    }
                    conversions += 1;
                }
            }
        }
        if let Some(error) = pending {
            return Err(error);
        }
        if scanner.underflow && conversions == 0 {
            return Ok(if variables.is_empty() {
                Value::Null
            } else {
                Value::Int(-1)
            });
        }
        if !variables.is_empty() {
            return Ok(Value::Int(conversions));
        }
        let mut result = PhpArray::default();
        for value in values {
            result.append(value, line)?;
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn assign_native_reference(
        &mut self,
        reference: &variables::Cell,
        value: Value,
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<(), Error> {
        if self.reference_is_typed(reference) {
            // Typed native assignments install the checked value before
            // destroying the previous one. Conversion failures leave it intact.
            if let Err(error) =
                self.write_reference_value(reference, value, self.strict_types, line)
            {
                if error.kind == ErrorKind::UnwindExit {
                    return Err(error);
                }
                self.set_error(error, pending);
            }
            *pending = self
                .drain_destructors(pending.take().map_or(Ok(()), Err), line)
                .err();
        } else {
            // Untyped ZEND_TRY_ASSIGN_REF_* uses zval_ptr_safe_dtor first:
            // destructors see null here and can modify later output variables.
            *reference.borrow_mut() = Value::Null;
            *pending = self
                .drain_destructors(pending.take().map_or(Ok(()), Err), line)
                .err();
            if pending
                .as_ref()
                .is_some_and(|e| e.kind == ErrorKind::UnwindExit)
            {
                return Err(pending.take().unwrap());
            }
            *reference.borrow_mut() = value;
        }
        if pending
            .as_ref()
            .is_some_and(|e| e.kind == ErrorKind::UnwindExit)
        {
            return Err(pending.take().unwrap());
        }
        // Ordinary exceptions do not stop the native scanner's assignment loop.
        Ok(())
    }
}

fn c_string(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len())]
}

fn space(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

enum Token {
    Literal(u8),
    Whitespace,
    Field {
        index: Option<usize>,
        width: usize,
        kind: Field,
    },
}

enum Field {
    Count,
    String(bool), // %c has no initial whitespace skip and a default width of 1.
    Integer(u32, bool),
    Float,
    Set(ScanSet),
}

fn decimal(format: &[u8], offset: &mut usize) -> u64 {
    let mut value = 0u64;
    while let Some(&byte) = format.get(*offset).filter(|b| b.is_ascii_digit()) {
        value = value
            .saturating_mul(10)
            .saturating_add((byte - b'0') as u64);
        *offset += 1;
    }
    value
}

fn compile(format: &[u8], variables: usize, line: usize) -> Result<(Vec<Token>, usize), Error> {
    let error = |message| Error::new(ErrorKind::ValueError, message, line);
    let bad_index = |positional| {
        error(if positional {
            "\"%n$\" argument index out of range"
        } else {
            "Different numbers of variable names and field specifiers"
        })
    };
    let mixed = || error("cannot mix \"%\" and \"%n$\" conversion specifiers");
    let mut tokens = Vec::new();
    let mut assigned = vec![0usize; variables];
    let mut index = 0;
    let mut positional = false;
    let mut sequential = false;
    let mut positional_size = 0;
    let mut offset = 0;
    while let Some(&byte) = format.get(offset) {
        offset += 1;
        if space(byte) {
            tokens.push(Token::Whitespace);
            continue;
        }
        if byte != b'%' {
            tokens.push(Token::Literal(byte));
            continue;
        }
        if format.get(offset) == Some(&b'%') {
            offset += 1;
            tokens.push(Token::Literal(b'%'));
            continue;
        }
        let suppressed = format.get(offset) == Some(&b'*');
        if suppressed {
            offset += 1;
        } else {
            let mut end = offset;
            let position = decimal(format, &mut end);
            if end > offset && format.get(end) == Some(&b'$') {
                positional = true;
                if sequential {
                    return Err(mixed());
                }
                // PHP's validator stores strtoul's result in a C int.
                let position = position as i32;
                if position < 1
                    || (variables != 0 && position as usize > variables)
                    || (variables == 0 && position > 255)
                {
                    return Err(bad_index(true));
                }
                index = position as usize - 1;
                if variables == 0 {
                    positional_size = positional_size.max(index + 1);
                }
                offset = end + 1;
            } else {
                sequential = true;
                if positional {
                    return Err(mixed());
                }
            }
        }
        let width = decimal(format, &mut offset) as usize;
        if matches!(format.get(offset), Some(b'l' | b'L' | b'h')) {
            offset += 1;
        }
        if !suppressed && variables != 0 && index >= variables {
            return Err(bad_index(positional));
        }
        let byte = format.get(offset).copied().unwrap_or(0);
        offset += 1;
        let kind = match byte {
            b'n' => Field::Count,
            b'd' | b'D' => Field::Integer(10, false),
            b'i' => Field::Integer(0, false),
            b'o' => Field::Integer(8, false),
            b'x' | b'X' => Field::Integer(16, false),
            b'u' => Field::Integer(10, true),
            b'f' | b'e' | b'E' | b'g' => Field::Float,
            b's' | b'c' => Field::String(byte == b'c'),
            b'[' => {
                let start = offset;
                if format.get(offset) == Some(&b'^') {
                    offset += 1;
                }
                if format.get(offset) == Some(&b']') {
                    offset += 1;
                }
                while format.get(offset).is_some_and(|&b| b != b']') {
                    offset += 1;
                }
                if offset >= format.len() {
                    return Err(error("Unmatched [ in format string"));
                }
                let set = ScanSet::new(&format[start..=offset]);
                offset += 1;
                Field::Set(set)
            }
            _ => {
                // zend_value_error uses a C-string message, truncated at NUL.
                let message = if byte == 0 {
                    "Bad scan conversion character \"".to_owned()
                } else {
                    format!(
                        "Bad scan conversion character \"{}\"",
                        String::from_utf8_lossy(&[byte])
                    )
                };
                return Err(Error::new(ErrorKind::ValueError, message, line));
            }
        };
        let destination = if suppressed {
            None
        } else {
            if assigned.len() <= index {
                assigned.resize(index + 1, 0);
            }
            assigned[index] += 1;
            index += 1;
            Some(index - 1)
        };
        tokens.push(Token::Field {
            index: destination,
            width,
            kind,
        });
    }
    let slots = if variables != 0 {
        variables
    } else if positional_size != 0 {
        positional_size
    } else {
        index
    };
    assigned.resize(slots, 0);
    for count in assigned {
        if count > 1 {
            return Err(error(
                "Variable is assigned by multiple \"%n$\" conversion specifiers",
            ));
        }
        if count == 0 && positional_size == 0 {
            return Err(error(
                "Variable is not assigned by any conversion specifiers",
            ));
        }
    }
    Ok((tokens, slots))
}

struct ScanSet {
    exclude: bool,
    chars: Vec<u8>,
    ranges: Vec<(c_char, c_char)>,
}

impl ScanSet {
    fn new(format: &[u8]) -> Self {
        let exclude = format[0] == b'^';
        let mut offset = usize::from(exclude);
        let mut set = Self {
            exclude,
            chars: Vec::new(),
            ranges: Vec::new(),
        };
        let mut start = format[offset];
        if matches!(start, b']' | b'-') {
            set.chars.push(start);
            offset += 1;
        }
        while format[offset] != b']' {
            let byte = format[offset];
            if format[offset + 1] == b'-' {
                start = byte;
            } else if byte == b'-' {
                if format[offset + 1] == b']' {
                    set.chars.extend([start, byte]);
                } else {
                    offset += 1;
                    let (a, b) = (start as c_char, format[offset] as c_char);
                    set.ranges.push((a.min(b), a.max(b)));
                }
            } else {
                set.chars.push(byte);
            }
            offset += 1;
        }
        set
    }

    fn contains(&self, byte: u8) -> bool {
        let c = byte as c_char;
        (self.chars.contains(&byte) || self.ranges.iter().any(|&(a, b)| a <= c && c <= b))
            != self.exclude
    }
}

struct Scanner<'a> {
    input: &'a [u8],
    offset: usize,
    underflow: bool,
}

impl Scanner<'_> {
    fn skip_space(&mut self) {
        while self.input.get(self.offset).is_some_and(|&b| space(b)) {
            self.offset += 1;
        }
    }

    fn field(&mut self, width: usize, kind: &Field) -> Option<Value> {
        if matches!(kind, Field::Count) {
            return Some(Value::Int(self.offset as i64));
        }
        if !matches!(kind, Field::String(true) | Field::Set(_)) {
            self.skip_space();
        }
        if self.offset == self.input.len() {
            self.underflow = true;
            return None;
        }
        let start = self.offset;
        match kind {
            Field::String(character) => {
                let width = if width != 0 {
                    width
                } else if *character {
                    1
                } else {
                    usize::MAX
                };
                while self.offset - start < width
                    && self.input.get(self.offset).is_some_and(|&b| !space(b))
                {
                    self.offset += 1;
                }
                Some(Value::String(
                    self.input[start..self.offset].to_vec().into(),
                ))
            }
            Field::Set(set) => {
                let width = if width == 0 { usize::MAX } else { width };
                while self.offset - start < width
                    && self
                        .input
                        .get(self.offset)
                        .is_some_and(|&b| set.contains(b))
                {
                    self.offset += 1;
                }
                (self.offset != start)
                    .then(|| Value::String(self.input[start..self.offset].to_vec().into()))
            }
            Field::Integer(base, unsigned) => self.integer(width, *base, *unsigned),
            Field::Float => self.float(width),
            Field::Count => unreachable!(),
        }
    }

    fn integer(&mut self, width: usize, mut base: u32, unsigned: bool) -> Option<Value> {
        let start = self.offset;
        let width = if width == 0 { 63 } else { width.min(63) };
        let (mut sign, mut no_digits, mut no_zero, mut hex) = (true, true, true, false);
        while self.offset - start < width {
            let Some(&byte) = self.input.get(self.offset) else {
                break;
            };
            match byte {
                b'0' => {
                    if base == 16 {
                        hex = true;
                    }
                    if base == 0 {
                        base = 8;
                        hex = true;
                    }
                    if no_zero {
                        no_zero = false;
                    } else {
                        hex = false;
                    }
                    sign = false;
                    no_digits = false;
                }
                b'1'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => {
                    if base == 0 && byte.is_ascii_digit() {
                        base = 10;
                    }
                    if ((b'8'..=b'9').contains(&byte) && base <= 8)
                        || (byte.is_ascii_alphabetic() && base <= 10)
                    {
                        break;
                    }
                    sign = false;
                    hex = false;
                    no_digits = false;
                }
                b'+' | b'-' if sign => sign = false,
                b'x' | b'X' if hex && self.offset == start + 1 => {
                    base = 16;
                    hex = false;
                }
                _ => break,
            }
            self.offset += 1;
        }
        if no_digits {
            self.underflow = self.offset == self.input.len();
            return None;
        }
        if matches!(self.input[self.offset - 1], b'x' | b'X') {
            self.offset -= 1;
        }
        let mut bytes = &self.input[start..self.offset];
        let negative = bytes[0] == b'-';
        if matches!(bytes[0], b'+' | b'-') {
            bytes = &bytes[1..];
        }
        if base == 16 && bytes.len() > 2 && bytes[0] == b'0' && matches!(bytes[1], b'x' | b'X') {
            bytes = &bytes[2..];
        }
        let mut magnitude = Some(0u64);
        for &byte in bytes {
            let digit = (byte as char).to_digit(base).unwrap();
            magnitude =
                magnitude.and_then(|n| n.checked_mul(base as u64)?.checked_add(digit as u64));
        }
        if unsigned {
            let value = magnitude
                .map(|n| if negative { n.wrapping_neg() } else { n })
                .unwrap_or(u64::MAX);
            Some(if value > i64::MAX as u64 {
                Value::String(value.to_string().into_bytes().into())
            } else {
                Value::Int(value as i64)
            })
        } else {
            let magnitude = magnitude.unwrap_or(u64::MAX);
            let value = if negative {
                if magnitude > i64::MAX as u64 {
                    i64::MIN
                } else {
                    -(magnitude as i64)
                }
            } else {
                magnitude.min(i64::MAX as u64) as i64
            };
            Some(Value::Int(value))
        }
    }

    fn float(&mut self, width: usize) -> Option<Value> {
        let start = self.offset;
        let width = if width == 0 { 63 } else { width.min(63) };
        let (mut sign, mut no_digits, mut point, mut exponent) = (true, true, true, true);
        while self.offset - start < width {
            let Some(&byte) = self.input.get(self.offset) else {
                break;
            };
            match byte {
                b'0'..=b'9' => {
                    sign = false;
                    no_digits = false;
                }
                b'+' | b'-' if sign => sign = false,
                b'.' if point => {
                    sign = false;
                    point = false;
                }
                b'e' | b'E' if !no_digits && exponent => {
                    exponent = false;
                    point = false;
                    sign = true;
                    no_digits = true;
                }
                _ => break,
            }
            self.offset += 1;
        }
        if no_digits {
            if exponent {
                self.underflow = self.offset == self.input.len();
                return None;
            }
            self.offset -= 1;
            if !matches!(self.input[self.offset], b'e' | b'E') {
                self.offset -= 1;
            }
        }
        let text = std::str::from_utf8(&self.input[start..self.offset]).unwrap();
        Some(Value::Float(
            text.parse().expect("scanner accepted a decimal float"),
        ))
    }
}
