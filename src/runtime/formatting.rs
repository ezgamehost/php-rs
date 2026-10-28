use super::{builtins::Builtin, *};
use crate::ast::CastKind;

impl Runtime {
    pub(super) fn formatted_print(
        &mut self,
        builtin: Builtin,
        name: &str,
        args: &[Option<Value>],
        extra: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let format = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let array_arguments = matches!(builtin, Builtin::Vprintf | Builtin::Vsprintf);
        let values = if array_arguments {
            builtins::require_array(args[1].as_ref().unwrap(), name, 2, line)?
        } else {
            if extra
                .entries()
                .iter()
                .any(|(key, _)| matches!(key, ArrayKey::String(_)))
            {
                return Err(Error::new(
                    ErrorKind::ArgumentCountError,
                    format!("{name}() does not accept unknown named parameters"),
                    line,
                ));
            }
            extra
        };
        let values: Vec<_> = values
            .entries()
            .iter()
            .map(|(_, value)| value.dereferenced())
            .collect();
        let mut parser = FormatParser {
            bytes: &format,
            pos: 0,
            next: 0,
            missing: 0,
            line,
        };
        let mut output = Vec::new();
        while parser.pos < format.len() {
            let byte = format[parser.pos];
            parser.pos += 1;
            if byte != b'%' {
                output.push(byte);
                continue;
            }
            if parser.peek() == Some(b'%') {
                output.push(b'%');
                parser.pos += 1;
                continue;
            }
            let index = parser.position()?;
            let mut left = false;
            let mut plus = false;
            let mut pad = b' ';
            loop {
                match parser.peek() {
                    Some(b' ' | b'0') => pad = format[parser.pos],
                    Some(b'-') => left = true,
                    Some(b'+') => plus = true,
                    Some(b'\'') => {
                        parser.pos += 1;
                        pad = parser
                            .peek()
                            .ok_or_else(|| parser.error("Missing padding character"))?;
                    }
                    _ => break,
                }
                parser.pos += 1;
            }
            let Some(width) = parser.dimension(&values, false)? else {
                continue;
            };
            let mut precision = None;
            let mut explicit = false;
            if parser.peek() == Some(b'.') {
                parser.pos += 1;
                explicit = parser
                    .peek()
                    .is_some_and(|c| c == b'*' || c.is_ascii_digit());
                let Some(value) = parser.dimension(&values, true)? else {
                    continue;
                };
                precision = Some(value);
            }
            if parser.peek() == Some(b'l') {
                parser.pos += 1;
            }
            let index = parser.argument(index);
            let Some(value) = values.get(index) else {
                parser.missing = parser.missing.max(index + 1);
                continue;
            };
            let spec = parser
                .peek()
                .ok_or_else(|| parser.error("Missing format specifier at end of string"))?;
            parser.pos += 1;
            if precision == Some(-1) && !b"gGhH".contains(&spec) {
                return Err(parser.error("Precision -1 is only supported for %g, %G, %h and %H"));
            }
            let mut signed = false;
            let mut field_width = width as usize;
            let mut bytes = match spec {
                b's' => {
                    let mut bytes = self.string_bytes(value, line)?;
                    if explicit {
                        bytes.truncate(precision.unwrap() as usize);
                    }
                    bytes
                }
                b'd' | b'u' | b'b' | b'o' | b'x' | b'X' | b'c' => {
                    let integer =
                        casting::cast(CastKind::Int, value.clone(), line, &mut self.diagnostics)?;
                    let Value::Int(n) = integer else {
                        unreachable!()
                    };
                    if spec == b'c' {
                        field_width = 0;
                        vec![n as u8]
                    } else {
                        if matches!(spec, b'd' | b'u') && left && pad == b'0' {
                            pad = b' ';
                        }
                        signed = spec == b'd' && (n < 0 || plus);
                        let text = match spec {
                            b'd' if plus => format!("{n:+}"),
                            b'd' => n.to_string(),
                            b'u' => (n as u64).to_string(),
                            b'b' => format!("{:b}", n as u64),
                            b'o' => format!("{:o}", n as u64),
                            b'x' => format!("{:x}", n as u64),
                            b'X' => format!("{:X}", n as u64),
                            _ => unreachable!(),
                        };
                        // Zend's base conversion passes explicit precision with a zero length.
                        if explicit && b"boxX".contains(&spec) {
                            Vec::new()
                        } else {
                            text.into_bytes()
                        }
                    }
                }
                b'e' | b'E' | b'f' | b'F' | b'g' | b'G' | b'h' | b'H' => {
                    let number =
                        casting::cast(CastKind::Float, value.clone(), line, &mut self.diagnostics)?
                            .as_float();
                    let mut precision = precision.unwrap_or(6);
                    if precision > 53 {
                        self.diagnostics.push(format!("Notice: {name}(): Requested precision of {precision} digits was truncated to PHP maximum of 53 digits on line {line}"));
                        precision = 53;
                    }
                    if !number.is_finite() {
                        field_width = 0;
                        if number.is_nan() {
                            b"NaN".to_vec()
                        } else if number.is_sign_negative() {
                            b"-INF".to_vec()
                        } else {
                            b"INF".to_vec()
                        }
                    } else {
                        let text = format_double(number, spec, precision);
                        signed = text.starts_with('-') || plus;
                        if plus && !text.starts_with('-') {
                            format!("+{text}").into_bytes()
                        } else {
                            text.into_bytes()
                        }
                    }
                }
                b'%' => {
                    field_width = 0;
                    vec![b'%']
                }
                _ => {
                    return Err(
                        parser.error(format!("Unknown format specifier \"{}\"", char::from(spec)))
                    );
                }
            };
            let padding = field_width.saturating_sub(bytes.len());
            let length = output.len().checked_add(bytes.len().max(field_width));
            if length.is_none_or(|len| len >= i32::MAX as usize) {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!("Field width {field_width} is too long"),
                    line,
                ));
            }
            output
                .try_reserve(bytes.len().max(field_width))
                .map_err(|_| {
                    Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
                })?;
            if !left {
                if pad == b'0' && signed && !bytes.is_empty() {
                    output.push(bytes.remove(0));
                }
                output.resize(output.len() + padding, pad);
            }
            output.extend(bytes);
            if left {
                output.resize(output.len() + padding, pad);
            }
        }
        if parser.missing > 0 {
            return Err(if array_arguments {
                parser.error(format!(
                    "The arguments array must contain {} items, {} given",
                    parser.missing,
                    values.len()
                ))
            } else {
                Error::new(
                    ErrorKind::ArgumentCountError,
                    format!(
                        "{} arguments are required, {} given",
                        parser.missing + 1,
                        values.len() + 1
                    ),
                    line,
                )
            });
        }
        if matches!(builtin, Builtin::Printf | Builtin::Vprintf) {
            let length = output.len();
            self.output.extend(output);
            Ok(Value::Int(length as i64))
        } else {
            Ok(Value::String(output.into()))
        }
    }
}

struct FormatParser<'a> {
    bytes: &'a [u8],
    pos: usize,
    next: usize,
    missing: usize,
    line: usize,
}
impl FormatParser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }
    fn error(&self, message: impl Into<String>) -> Error {
        Error::new(ErrorKind::ValueError, message, self.line)
    }
    fn number(&mut self) -> i64 {
        let mut number = 0i64;
        while let Some(byte) = self.peek().filter(u8::is_ascii_digit) {
            number = number
                .saturating_mul(10)
                .saturating_add((byte - b'0') as i64);
            self.pos += 1;
        }
        number
    }
    fn position(&mut self) -> Result<Option<usize>, Error> {
        let start = self.pos;
        let number = self.number();
        if self.peek() != Some(b'$') {
            self.pos = start;
            return Ok(None);
        }
        self.pos += 1;
        if !(1..i32::MAX as i64).contains(&number) {
            return Err(self.error(
                "Argument number specifier must be greater than zero and less than 2147483647",
            ));
        }
        Ok(Some(number as usize - 1))
    }
    fn argument(&mut self, index: Option<usize>) -> usize {
        index.unwrap_or_else(|| {
            let index = self.next;
            self.next += 1;
            index
        })
    }
    fn dimension(&mut self, values: &[Value], precision: bool) -> Result<Option<i64>, Error> {
        let label = if precision { "Precision" } else { "Width" };
        let star = self.peek() == Some(b'*');
        let number = if star {
            self.pos += 1;
            let position = self.position()?;
            let index = self.argument(position);
            let Some(value) = values.get(index) else {
                self.missing = self.missing.max(index + 1);
                return Ok(None);
            };
            let Value::Int(number) = value else {
                return Err(self.error(format!("{label} must be an integer")));
            };
            *number
        } else {
            self.number()
        };
        let minimum = if precision && star { -1 } else { 0 };
        if number < minimum || number > i32::MAX as i64 || (!star && number == i32::MAX as i64) {
            return Err(self.error(format!("{label} must be between {minimum} and 2147483647")));
        }
        Ok(Some(number))
    }
}

fn format_double(number: f64, spec: u8, precision: i64) -> String {
    let upper = matches!(spec, b'E' | b'G' | b'H');
    let marker = if upper { 'E' } else { 'e' };
    if matches!(spec, b'f' | b'F') {
        return format!("{number:.*}", precision as usize);
    }
    if matches!(spec, b'e' | b'E') {
        let scientific = format!("{number:.*e}", precision as usize);
        let (mantissa, exponent) = scientific.split_once('e').unwrap();
        let exponent: i32 = exponent.parse().unwrap();
        return format!("{mantissa}{marker}{exponent:+}");
    }
    let digits = precision.max(1) as usize;
    let scientific = if precision == -1 {
        format!("{number:e}")
    } else {
        format!("{number:.*e}", digits - 1)
    };
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    let mut mantissa = mantissa.to_owned();
    if mantissa.contains('.') {
        mantissa = mantissa
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
    }
    // gcvt uses the rounded exponent to choose fixed or scientific notation.
    if exponent < -4 || exponent >= if precision == -1 { 17 } else { digits as i32 } {
        if !mantissa.contains('.') {
            mantissa.push_str(".0");
        }
        format!("{mantissa}{marker}{exponent:+}")
    } else {
        // Expand the rounded decimal digits without parsing back through a binary float.
        let negative = mantissa.starts_with('-');
        let raw = mantissa.trim_start_matches('-').replace('.', "");
        let point = exponent + 1;
        let body = if point <= 0 {
            format!("0.{}{raw}", "0".repeat((-point) as usize))
        } else if point as usize >= raw.len() {
            format!("{raw}{}", "0".repeat(point as usize - raw.len()))
        } else {
            format!("{}.{}", &raw[..point as usize], &raw[point as usize..])
        };
        if negative { format!("-{body}") } else { body }
    }
}
