// Integer-prefix and settype rules adapted from PHP 8.5.0 ext/standard/type.c.
// Copyright (c) The PHP Group. Altered Rust implementation; see
// THIRD_PARTY_NOTICES.md for the PHP license.
use super::*;
use crate::{ast::CastKind, value::numeric_whitespace};

impl Runtime {
    pub(super) fn settype(
        &mut self,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        line: usize,
    ) -> Result<Value, Error> {
        let mut args = args.into_iter();
        let first = args.next().unwrap().unwrap();
        let reference = if let Some(reference) = &references[0] {
            drop(first);
            reference.clone()
        } else {
            cell(first)
        };
        let type_value = args.next().unwrap().unwrap();
        let name = self.native_string(&type_value, "settype", 2, line)?;
        drop(type_value);
        let kind = match name.to_ascii_lowercase().as_slice() {
            b"int" | b"integer" => CastKind::Int,
            b"float" | b"double" => CastKind::Float,
            b"string" => CastKind::String,
            b"array" => CastKind::Array,
            b"object" => CastKind::Object,
            b"bool" | b"boolean" => CastKind::Bool,
            b"null" => CastKind::Void,
            name => {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    if name == b"resource" {
                        "Cannot convert to resource type"
                    } else {
                        "settype(): Argument #2 ($type) must be a valid type"
                    },
                    line,
                ));
            }
        };
        // Type-name coercion may mutate the variable. Read its current value
        // afterwards and release the borrow before invoking Stringable code.
        let value = reference.borrow().dereferenced();
        let mut pending = None;
        let converted = match kind {
            CastKind::String => match self.string_bytes(&value, line) {
                Ok(bytes) => Value::String(bytes.into()),
                Err(error) if error.kind == ErrorKind::UnwindExit => return Err(error),
                Err(error) => {
                    // Native convert_to_string installs an empty string even
                    // when conversion throws, then attempts typed assignment.
                    pending = Some(self.materialize_error(error));
                    Value::String(Vec::new().into())
                }
            },
            CastKind::Object => self.cast_object(value, line),
            CastKind::Void => {
                value.warn_nan("null", line, &mut self.diagnostics);
                Value::Null
            }
            _ => casting::cast(kind, value, line, &mut self.diagnostics)?,
        };
        let result = self
            .write_reference_value(&reference, converted, self.strict_types, line)
            .map(|_| Value::Bool(true));
        match (result, pending) {
            (Err(error), Some(previous)) => {
                let error = self.materialize_error(error);
                if let (Some(current), Some(previous)) = (&error.throwable, previous.throwable) {
                    throwables::append_previous(current, &previous);
                }
                Err(error)
            }
            (Ok(_), Some(error)) => Err(error),
            (result, None) => result,
        }
    }

    pub(super) fn scalar_value(
        &mut self,
        kind: CastKind,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let value = args[0].as_ref().unwrap().dereferenced();
        if kind == CastKind::Int {
            let base = args[1]
                .as_ref()
                .map(|v| self.native_int(v, name, 2, line))
                .transpose()?
                .unwrap_or(10);
            if base != 10
                && let Value::String(bytes) = &value
            {
                return Ok(Value::Int(parse_integer(&bytes.bytes(), base)));
            }
        }
        if kind == CastKind::String {
            return Ok(Value::String(self.string_bytes(&value, line)?.into()));
        }
        casting::cast(kind, value, line, &mut self.diagnostics)
    }
}

fn parse_integer(mut bytes: &[u8], base: i64) -> i64 {
    while bytes.first().is_some_and(numeric_whitespace) {
        bytes = &bytes[1..];
    }
    let negative = bytes.first() == Some(&b'-');
    if matches!(bytes.first(), Some(b'+' | b'-')) {
        bytes = &bytes[1..];
    }
    // PHP removes binary prefixes itself before calling strtol. The base
    // comparison occurs before the C API narrows it to a 32-bit integer.
    let mut radix = base as i32;
    if matches!(base, 0 | 2) && (bytes.starts_with(b"0b") || bytes.starts_with(b"0B")) {
        bytes = &bytes[2..];
        radix = 2;
    } else if matches!(radix, 0 | 16)
        && (bytes.starts_with(b"0x") || bytes.starts_with(b"0X"))
        && bytes.get(2).is_some_and(u8::is_ascii_hexdigit)
    {
        bytes = &bytes[2..];
        radix = 16;
    } else if radix == 0 {
        radix = if bytes.first() == Some(&b'0') { 8 } else { 10 };
    }
    if !(2..=36).contains(&radix) {
        return 0;
    }
    let limit = i64::MAX as u64 + u64::from(negative);
    let mut number = 0u64;
    for &byte in bytes {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'z' => byte - b'a' + 10,
            b'A'..=b'Z' => byte - b'A' + 10,
            _ => break,
        };
        if i32::from(digit) >= radix {
            break;
        }
        number = number
            .saturating_mul(radix as u64)
            .saturating_add(u64::from(digit));
        if number > limit {
            return if negative { i64::MIN } else { i64::MAX };
        }
    }
    if negative {
        number.wrapping_neg() as i64
    } else {
        number as i64
    }
}
