use crate::{Error, ErrorKind};
use std::cmp::Ordering;

/// PHP strings are byte sequences, not necessarily UTF-8.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(Vec<u8>),
}

impl Value {
    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(b) => *b,
            Self::Int(n) => *n != 0,
            Self::Float(n) => *n != 0.0,
            Self::String(s) => !s.is_empty() && s != b"0",
        }
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Null | Self::Bool(false) => Vec::new(),
            Self::Bool(true) => b"1".to_vec(),
            Self::Int(n) => n.to_string().into_bytes(),
            Self::Float(n) => format_float(*n).into_bytes(),
            Self::String(s) => s.clone(),
        }
    }
    pub(crate) fn number(&self, line: usize, warnings: &mut Vec<String>) -> Result<Self, Error> {
        match self {
            Self::Null => Ok(Self::Int(0)),
            Self::Bool(b) => Ok(Self::Int(i64::from(*b))),
            Self::Int(_) | Self::Float(_) => Ok(self.clone()),
            Self::String(s) => {
                let (value, complete) = numeric_prefix(s).ok_or_else(|| {
                    Error::new(
                        ErrorKind::TypeError,
                        "Unsupported operand type: string",
                        line,
                    )
                })?;
                if !complete {
                    warnings.push(format!("A non-numeric value encountered on line {line}"));
                }
                Ok(value)
            }
        }
    }
    pub(crate) fn as_float(&self) -> f64 {
        match self {
            Self::Int(n) => *n as f64,
            Self::Float(n) => *n,
            _ => unreachable!("numeric value required"),
        }
    }
    pub(crate) fn compare(&self, other: &Self) -> Option<Ordering> {
        use Value::*;
        match (self, other) {
            (Bool(_), _) | (_, Bool(_)) => self.is_truthy().partial_cmp(&other.is_truthy()),
            (Null, String(s)) => b"".as_slice().partial_cmp(s.as_slice()),
            (String(s), Null) => s.as_slice().partial_cmp(b"".as_slice()),
            (Null, _) | (_, Null) => self.is_truthy().partial_cmp(&other.is_truthy()),
            (Int(a), Int(b)) => a.partial_cmp(b),
            (Int(_) | Float(_), Int(_) | Float(_)) => {
                self.as_float().partial_cmp(&other.as_float())
            }
            (String(a), String(b)) => match (numeric_string(a), numeric_string(b)) {
                (Some(a), Some(b)) => a.compare(&b),
                _ => a.partial_cmp(b),
            },
            (String(a), _) => match numeric_string(a) {
                Some(a) => a.compare(other),
                None => a.partial_cmp(&other.to_bytes()),
            },
            (_, String(b)) => match numeric_string(b) {
                Some(b) => self.compare(&b),
                None => self.to_bytes().partial_cmp(b),
            },
        }
    }
}

pub(crate) fn numeric_string(bytes: &[u8]) -> Option<Value> {
    let (value, complete) = numeric_prefix(bytes)?;
    complete.then_some(value)
}

fn numeric_prefix(bytes: &[u8]) -> Option<(Value, bool)> {
    let mut pos = 0;
    while bytes.get(pos).is_some_and(u8::is_ascii_whitespace) {
        pos += 1;
    }
    let start = pos;
    if matches!(bytes.get(pos), Some(b'+' | b'-')) {
        pos += 1;
    }
    let mut digits = 0;
    while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
        pos += 1;
        digits += 1;
    }
    if bytes.get(pos) == Some(&b'.') {
        pos += 1;
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if matches!(bytes.get(pos), Some(b'e' | b'E')) {
        let exponent_start = pos;
        pos += 1;
        if matches!(bytes.get(pos), Some(b'+' | b'-')) {
            pos += 1;
        }
        let digits_start = pos;
        while bytes.get(pos).is_some_and(u8::is_ascii_digit) {
            pos += 1;
        }
        if pos == digits_start {
            pos = exponent_start;
        }
    }
    let text = std::str::from_utf8(&bytes[start..pos]).ok()?;
    let value = text
        .parse::<i64>()
        .map(Value::Int)
        .ok()
        .or_else(|| text.parse::<f64>().map(Value::Float).ok())?;
    Some((value, bytes[pos..].iter().all(u8::is_ascii_whitespace)))
}

fn format_float(n: f64) -> String {
    if n.is_nan() {
        return "NAN".into();
    }
    if n.is_infinite() {
        return if n.is_sign_negative() { "-INF" } else { "INF" }.into();
    }
    if n == 0.0 {
        return if n.is_sign_negative() { "-0" } else { "0" }.into();
    }
    // PHP's default precision is 14 significant digits.
    let scientific = format!("{n:.13e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    if !(-4..14).contains(&exponent) {
        let mut mantissa = mantissa
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
        if !mantissa.contains('.') {
            mantissa.push_str(".0");
        }
        format!("{mantissa}E{exponent:+}")
    } else {
        let rounded: f64 = scientific.parse().unwrap();
        let s = format!("{:.*}", (13 - exponent).max(0) as usize, rounded);
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            s
        }
    }
}
