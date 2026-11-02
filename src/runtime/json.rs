// Altered Rust adaptation of PHP 8.5.0 ext/json. Copyright (c) The PHP Group;
// see THIRD_PARTY_NOTICES.md for sources and attribution.
use super::*;
use crate::ast::Argument;
mod decode;
mod encode;

const IGNORE_UTF8: i64 = 1 << 20;
const SUBSTITUTE_UTF8: i64 = 1 << 21;
const THROW: i64 = 1 << 22;
const PARTIAL: i64 = 512;

#[derive(Debug, Clone, Copy)]
pub(super) enum Operation {
    Encode,
    Decode,
    Validate,
    LastError,
    LastMessage,
}

pub(super) fn constant(name: &[u8]) -> Option<i64> {
    Some(match name {
        b"JSON_HEX_TAG" | b"JSON_OBJECT_AS_ARRAY" => 1,
        b"JSON_HEX_AMP" | b"JSON_BIGINT_AS_STRING" => 2,
        b"JSON_HEX_APOS" => 4,
        b"JSON_HEX_QUOT" => 8,
        b"JSON_FORCE_OBJECT" => 16,
        b"JSON_NUMERIC_CHECK" => 32,
        b"JSON_UNESCAPED_SLASHES" => 64,
        b"JSON_PRETTY_PRINT" => 128,
        b"JSON_UNESCAPED_UNICODE" => 256,
        b"JSON_PARTIAL_OUTPUT_ON_ERROR" => PARTIAL,
        b"JSON_PRESERVE_ZERO_FRACTION" => 1024,
        b"JSON_UNESCAPED_LINE_TERMINATORS" => 2048,
        b"JSON_INVALID_UTF8_IGNORE" => IGNORE_UTF8,
        b"JSON_INVALID_UTF8_SUBSTITUTE" => SUBSTITUTE_UTF8,
        b"JSON_THROW_ON_ERROR" => THROW,
        b"JSON_ERROR_NONE" => 0,
        b"JSON_ERROR_DEPTH" => 1,
        b"JSON_ERROR_STATE_MISMATCH" => 2,
        b"JSON_ERROR_CTRL_CHAR" => 3,
        b"JSON_ERROR_SYNTAX" => 4,
        b"JSON_ERROR_UTF8" => 5,
        b"JSON_ERROR_RECURSION" => 6,
        b"JSON_ERROR_INF_OR_NAN" => 7,
        b"JSON_ERROR_UNSUPPORTED_TYPE" => 8,
        b"JSON_ERROR_INVALID_PROPERTY_NAME" => 9,
        b"JSON_ERROR_UTF16" => 10,
        b"JSON_ERROR_NON_BACKED_ENUM" => 11,
        _ => return None,
    })
}

fn error_message(code: i64) -> &'static str {
    match code {
        0 => "No error",
        1 => "Maximum stack depth exceeded",
        2 => "State mismatch (invalid or malformed JSON)",
        3 => "Control character error, possibly incorrectly encoded",
        4 => "Syntax error",
        5 => "Malformed UTF-8 characters, possibly incorrectly encoded",
        6 => "Recursion detected",
        7 => "Inf and NaN cannot be JSON encoded",
        8 => "Type is not supported",
        9 => "The decoded property name is invalid",
        10 => "Single unpaired UTF-16 surrogate in unicode escape",
        11 => "Non-backed enums have no default serialization",
        _ => "Unknown error",
    }
}

impl Runtime {
    fn json_exception(&mut self, code: i64, line: usize) -> Error {
        let class = self.find_class(b"JsonException").unwrap();
        let args: Vec<_> = [
            Value::String(error_message(code).as_bytes().to_vec().into()),
            Value::Int(code),
        ]
        .into_iter()
        .map(|value| {
            Argument::Positional(Expr {
                kind: ExprKind::Literal(value),
                line,
            })
        })
        .collect();
        match self.instantiate_class(class, &args, true, line) {
            Ok(value) => self.throw_value(value, line),
            Err(error) => error,
        }
    }

    pub(super) fn json_builtin(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(operation, Operation::LastError) {
            return Ok(Value::Int(self.json_last_error));
        }
        if matches!(operation, Operation::LastMessage) {
            return Ok(Value::String(
                error_message(self.json_last_error)
                    .as_bytes()
                    .to_vec()
                    .into(),
            ));
        }
        if matches!(operation, Operation::Encode) {
            let flags = args[1]
                .as_ref()
                .map(|v| self.native_int(v, name, 2, line))
                .transpose()?
                .unwrap_or(0);
            let depth = args[2]
                .as_ref()
                .map(|v| self.native_int(v, name, 3, line))
                .transpose()?
                .unwrap_or(512);
            return self.json_encode(args[0].as_ref().unwrap(), flags, depth as i32, line);
        }
        let input = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let validate = matches!(operation, Operation::Validate);
        let associate = if !validate {
            args[1]
                .as_ref()
                .filter(|v| !matches!(v, Value::Null))
                .map(|v| self.native_bool(v, name, 2, line))
                .transpose()?
        } else {
            None
        };
        let depth_index = if validate { 1 } else { 2 };
        let depth = args[depth_index]
            .as_ref()
            .map(|v| self.native_int(v, name, depth_index + 1, line))
            .transpose()?
            .unwrap_or(512);
        let mut flags = args[depth_index + 1]
            .as_ref()
            .map(|v| self.native_int(v, name, depth_index + 2, line))
            .transpose()?
            .unwrap_or(0);
        if validate && flags != 0 && flags != IGNORE_UTF8 {
            return Err(Error::new(
                ErrorKind::ValueError,
                "json_validate(): Argument #3 ($flags) must be a valid flag (allowed flags: JSON_INVALID_UTF8_IGNORE)",
                line,
            ));
        }
        if flags & THROW == 0 {
            self.json_last_error = 0;
        }
        let result = if input.is_empty() {
            Err(4)
        } else {
            if depth <= 0 || depth > i64::from(i32::MAX) {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!(
                        "{name}(): Argument #{} ($depth) must be {}",
                        depth_index + 1,
                        if depth <= 0 {
                            "greater than 0"
                        } else {
                            "less than 2147483647"
                        }
                    ),
                    line,
                ));
            }
            if let Some(associate) = associate {
                flags = if associate { flags | 1 } else { flags & !1 };
            }
            decode::parse(self, &input, flags, depth as usize, validate)
        };
        match result {
            Ok(value) => Ok(if validate { Value::Bool(true) } else { value }),
            Err(code) => {
                if flags & THROW != 0 {
                    return Err(self.json_exception(code, line));
                }
                self.json_last_error = code;
                Ok(if validate {
                    Value::Bool(false)
                } else {
                    Value::Null
                })
            }
        }
    }
}
