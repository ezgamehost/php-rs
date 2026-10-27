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
    pub(crate) fn number(&self, line: usize) -> Result<Self, Error> {
        match self {
            Self::Null => Ok(Self::Int(0)),
            Self::Bool(b) => Ok(Self::Int(i64::from(*b))),
            Self::Int(_) | Self::Float(_) => Ok(self.clone()),
            Self::String(s) => numeric_string(s).ok_or_else(|| {
                Error::new(
                    ErrorKind::TypeError,
                    "Unsupported operand type: string",
                    line,
                )
            }),
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
    let s = std::str::from_utf8(bytes)
        .ok()?
        .trim_matches(|c: char| c.is_ascii_whitespace());
    if s.is_empty() {
        return None;
    }
    // Rust accepts inf/NaN; PHP does not classify them as numeric strings.
    if !s.bytes().any(|c| c.is_ascii_digit())
        || s.bytes()
            .any(|c| !c.is_ascii_digit() && !b"+-.eE".contains(&c))
    {
        return None;
    }
    s.parse::<i64>()
        .map(Value::Int)
        .ok()
        .or_else(|| s.parse::<f64>().map(Value::Float).ok())
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
