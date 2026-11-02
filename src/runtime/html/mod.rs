// Altered Rust adaptation of PHP 8.5.0 ext/standard/html.c.
// Copyright (c) The PHP Group. See THIRD_PARTY_NOTICES.md.
use super::*;
mod charset;
mod tables;
use charset::Charset;

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    Encode(bool),
    Decode(bool),
    Table,
}

impl Runtime {
    pub(super) fn html_builtin(
        &mut self,
        op: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(op, Operation::Table) {
            let all = args[0]
                .as_ref()
                .map(|v| self.native_int(v, name, 1, line))
                .transpose()?
                .unwrap_or(0)
                != 0;
            let flags = args[1]
                .as_ref()
                .map(|v| self.native_int(v, name, 2, line))
                .transpose()?
                .unwrap_or(11);
            let hint = args[2]
                .as_ref()
                .map(|v| self.native_string(v, name, 3, line))
                .transpose()?;
            let charset = self.html_charset(hint.as_deref(), name, line)?;
            return Ok(translation_table(all, flags, charset));
        }
        let input = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let flags = args[1]
            .as_ref()
            .map(|v| self.native_int(v, name, 2, line))
            .transpose()?
            .unwrap_or(11);
        let hint = args
            .get(2)
            .and_then(Option::as_ref)
            .filter(|v| !matches!(v, Value::Null))
            .map(|v| self.native_string_type(v, name, 3, line, "?string"))
            .transpose()?;
        let double = args
            .get(3)
            .and_then(Option::as_ref)
            .map(|v| self.native_bool(v, name, 4, line))
            .transpose()?
            .unwrap_or(true);
        if matches!(op, Operation::Encode(_)) && input.is_empty()
            || matches!(op, Operation::Decode(_)) && !input.contains(&b'&')
        {
            return Ok(Value::String(input.into()));
        }
        let charset = if matches!(op, Operation::Decode(false)) {
            Charset::Latin1
        } else {
            self.html_charset(hint.as_deref(), name, line)?
        };
        let output = match op {
            Operation::Encode(all) => {
                if all && charset.partial() {
                    self.warn(format!("Notice: {name}(): Only basic entities substitution is supported for multi-byte encodings other than UTF-8; functionality is equivalent to htmlspecialchars on line {line}"))?;
                }
                encode(&input, all, flags, charset, double, line)?
            }
            Operation::Decode(all) => decode(&input, all, flags, charset, line)?,
            Operation::Table => unreachable!(),
        };
        Ok(Value::String(output.into()))
    }

    fn html_charset(
        &mut self,
        hint: Option<&[u8]>,
        name: &str,
        line: usize,
    ) -> Result<Charset, Error> {
        let hint = hint.unwrap_or(b"").split(|&b| b == 0).next().unwrap();
        if hint.is_empty() {
            return Ok(Charset::Utf8);
        }
        if let Some(charset) = Charset::named(hint) {
            return Ok(charset);
        }
        self.warn(format!(
            "{name}(): Charset \"{}\" is not supported, assuming UTF-8 on line {line}",
            String::from_utf8_lossy(hint)
        ))?;
        Ok(Charset::Utf8)
    }
}

fn basic(code: u32, doctype: i64) -> Option<&'static [u8]> {
    Some(match code {
        34 => b"quot",
        38 => b"amp",
        39 if doctype == 0 => b"#039",
        39 => b"apos",
        60 => b"lt",
        62 => b"gt",
        _ => return None,
    })
}
fn quotes(code: u32, flags: i64) -> bool {
    !(code == 39 && flags & 1 == 0 || code == 34 && flags & 2 == 0)
}
fn rows(doctype: i64) -> &'static [(u32, u32, &'static [u8])] {
    if doctype == 48 {
        tables::E5
    } else {
        tables::E4
    }
}
fn for_code(code: u32, doctype: i64) -> &'static [(u32, u32, &'static [u8])] {
    let table = rows(doctype);
    let start = table.partition_point(|row| row.0 < code);
    let end = start + table[start..].partition_point(|row| row.0 == code);
    &table[start..end]
}

fn named(name: &[u8], all: bool, doctype: i64) -> Option<(u32, u32)> {
    if !all || doctype == 16 {
        return Some((
            match name {
                b"quot" => 34,
                b"amp" => 38,
                b"lt" => 60,
                b"gt" => 62,
                b"apos" if doctype != 0 => 39,
                _ => return None,
            },
            0,
        ));
    }
    if doctype == 32 && name == b"apos" {
        return Some((39, 0));
    }
    let table = if doctype == 48 {
        tables::D5
    } else {
        tables::D4
    };
    table
        .binary_search_by(|row| row.0.cmp(name))
        .ok()
        .map(|i| (table[i].1, table[i].2))
}

fn allowed(code: u32, doctype: i64) -> bool {
    let basic = (0x20..=0x7e).contains(&code) || matches!(code, 9 | 10 | 13);
    match doctype {
        0 => basic || (0xa0..=0xd7ff).contains(&code) || (0xe000..=0x10ffff).contains(&code),
        48 => {
            basic
                || code == 12
                || (0xa0..=0xd7ff).contains(&code)
                || ((0xe000..=0x10ffff).contains(&code)
                    && code & 0xffff < 0xfffe
                    && !(0xfdd0..=0xfdef).contains(&code))
        }
        _ => {
            matches!(code, 9 | 10 | 13)
                || (0x20..=0xd7ff).contains(&code)
                || ((0xe000..=0x10ffff).contains(&code) && !matches!(code, 0xfffe | 0xffff))
        }
    }
}
fn numeric_allowed(code: u32, doctype: i64) -> bool {
    match doctype {
        0 => code <= 0x10ffff,
        48 => {
            (0x20..=0x7e).contains(&code)
                || matches!(code, 9 | 10 | 12)
                || ((0xa0..=0x10ffff).contains(&code)
                    && code & 0xffff < 0xfffe
                    && !(0xfdd0..=0xfdef).contains(&code))
        }
        _ => allowed(code, doctype),
    }
}

// Returns code points, the index after ';', and whether this is numeric.
fn entity(input: &[u8], begin: usize, all: bool, doctype: i64) -> Option<(u32, u32, usize, bool)> {
    let mut cursor = begin + 1;
    if input.get(cursor) == Some(&b'#') {
        cursor += 1;
        let hex = matches!(input.get(cursor), Some(b'x' | b'X'));
        if hex {
            cursor += 1;
        }
        let base = if hex { 16 } else { 10 };
        let digit = |byte: &u8| (*byte as char).to_digit(base);
        input.get(cursor).and_then(digit)?;
        // strtol with base 16 accepts an optional 0x prefix after the entity's x.
        if hex
            && input.get(cursor) == Some(&b'0')
            && matches!(input.get(cursor + 1), Some(b'x' | b'X'))
            && input.get(cursor + 2).and_then(digit).is_some()
        {
            cursor += 2;
        }
        let mut code = 0u32;
        while let Some(value) = input.get(cursor).and_then(digit) {
            code = code.saturating_mul(base).saturating_add(value);
            cursor += 1;
        }
        if code > 0x10ffff || input.get(cursor) != Some(&b';') {
            return None;
        }
        Some((code, 0, cursor + 1, true))
    } else {
        let start = cursor;
        while input.get(cursor).is_some_and(u8::is_ascii_alphanumeric) {
            cursor += 1;
        }
        if cursor == start || input.get(cursor) != Some(&b';') {
            return None;
        }
        let (first, second) = named(&input[start..cursor], all, doctype)?;
        Some((first, second, cursor + 1, false))
    }
}

fn append(output: &mut Vec<u8>, bytes: &[u8], line: usize) -> Result<(), Error> {
    output
        .try_reserve(bytes.len())
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate HTML string", line))?;
    output.extend_from_slice(bytes);
    Ok(())
}
fn emit_entity(output: &mut Vec<u8>, name: &[u8], line: usize) -> Result<(), Error> {
    append(output, b"&", line)?;
    append(output, name, line)?;
    append(output, b";", line)
}

fn encode(
    input: &[u8],
    all: bool,
    flags: i64,
    charset: Charset,
    double: bool,
    line: usize,
) -> Result<Vec<u8>, Error> {
    let doctype = flags & 48;
    let all = all && !charset.partial() && doctype != 16;
    let replacement = if charset == Charset::Utf8 {
        b"\xef\xbf\xbd".as_slice()
    } else {
        b"&#xFFFD;"
    };
    let mut output = Vec::new();
    let mut cursor = 0;
    while cursor < input.len() {
        let begin = cursor;
        let Some(mut code) = charset.next(input, &mut cursor) else {
            if flags & 4 != 0 {
                continue;
            }
            if flags & 8 != 0 {
                append(&mut output, replacement, line)?;
                continue;
            }
            return Ok(Vec::new());
        };
        let original = &input[begin..cursor];
        if code == 38 {
            if !double
                && let Some((code, _, end, numeric)) = entity(input, begin, true, doctype)
                && (!numeric || flags & 128 == 0 || numeric_allowed(code, doctype))
            {
                append(&mut output, &input[begin..end], line)?;
                cursor = end;
            } else {
                emit_entity(&mut output, b"amp", line)?;
            }
            continue;
        }
        if !quotes(code, flags) {
            append(&mut output, original, line)?;
            continue;
        }
        let name = if all {
            code = charset.to_unicode(code);
            if code == 0xffff && !charset.unicode_compatible() {
                append(&mut output, original, line)?;
                continue;
            }
            let entries = for_code(code, doctype);
            let mut name = entries.iter().find(|row| row.1 == 0).map(|row| row.2);
            if entries.iter().any(|row| row.1 != 0) && cursor < input.len() {
                let mut after = cursor;
                if let Some(next) = charset.next(input, &mut after)
                    && let Some(row) = entries.iter().find(|row| row.1 == next && row.1 != 0)
                {
                    name = Some(row.2);
                    cursor = after;
                }
            }
            name
        } else {
            basic(code, doctype)
        };
        if let Some(name) = name {
            emit_entity(&mut output, name, line)?;
        } else if flags & 128 != 0
            && (charset.unicode_compatible() || all || code <= 0x7d)
            && !allowed(code, doctype)
        {
            append(&mut output, replacement, line)?;
        } else {
            append(&mut output, original, line)?;
        }
    }
    Ok(output)
}

fn decode(
    input: &[u8],
    all: bool,
    flags: i64,
    charset: Charset,
    line: usize,
) -> Result<Vec<u8>, Error> {
    let doctype = flags & 48;
    let mut output = Vec::new();
    let mut cursor = 0;
    while cursor < input.len() {
        if input[cursor] == b'&'
            && let Some((first, second, end, numeric)) = entity(input, cursor, all, doctype)
            && (!numeric
                || ((all || basic(first, 16).is_some())
                    && allowed(first, doctype)
                    && !(doctype == 48 && first == 13)))
            && quotes(first, flags)
            && (charset == Charset::Utf8 || second == 0)
            && let Some(code) = charset.map_unicode(first)
        {
            output.try_reserve(8).map_err(|_| {
                Error::new(ErrorKind::Error, "Unable to allocate HTML string", line)
            })?;
            charset.write(code, &mut output);
            if second != 0 {
                charset.write(second, &mut output);
            }
            cursor = end;
        } else {
            append(&mut output, &input[cursor..cursor + 1], line)?;
            cursor += 1;
        }
    }
    Ok(output)
}

fn translation_table(all: bool, flags: i64, charset: Charset) -> Value {
    let doctype = flags & 48;
    let all = all && !charset.partial() && doctype != 16;
    let mut result = PhpArray::default();
    let mut insert = |first: u32, second: u32, name: &[u8]| {
        let mut key = Vec::new();
        charset.write(first, &mut key);
        if second != 0 {
            charset.write(second, &mut key);
        }
        let mut value = Vec::with_capacity(name.len() + 2);
        value.push(b'&');
        value.extend_from_slice(name);
        value.push(b';');
        result.insert(ArrayKey::String(key), Value::String(value.into()));
    };
    if !all {
        for code in [34, 38, 39, 60, 62] {
            if quotes(code, flags) {
                insert(code, 0, basic(code, doctype).unwrap());
            }
        }
    } else if charset.unicode_compatible() {
        for &(first, second, name) in rows(doctype) {
            if (charset == Charset::Utf8 || first <= 255) && quotes(first, flags) {
                insert(first, second, name);
            }
        }
    } else {
        for byte in 0..=255 {
            if !quotes(byte, flags) {
                continue;
            }
            for &(_, second, name) in for_code(charset.to_unicode(byte), doctype) {
                if second == 0 {
                    insert(byte, 0, name);
                } else if let Some(second) = charset.map_unicode(second) {
                    insert(byte, second, name);
                }
            }
        }
    }
    Value::Array(Rc::new(result))
}
