use crate::{Error, ErrorKind, PhpArray};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

/// PHP strings are byte sequences, not necessarily UTF-8.
#[derive(Debug)]
pub enum Value {
    /// Mutable array bucket used during reference iteration; not a PHP reference.
    Indirect(Rc<RefCell<Value>>),
    Reference(Rc<RefCell<Value>>),
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(crate::PhpString),
    Array(Rc<PhpArray>),
    Closure(Rc<crate::Closure>),
    Object(Rc<crate::PhpObject>),
    Resource(Rc<crate::PhpResource>),
}

impl Clone for Value {
    fn clone(&self) -> Self {
        match self {
            Self::Indirect(value) => value.borrow().clone(),
            Self::Reference(value) => Self::Reference(value.clone()),
            Self::Null => Self::Null,
            Self::Bool(value) => Self::Bool(*value),
            Self::Int(value) => Self::Int(*value),
            Self::Float(value) => Self::Float(*value),
            Self::String(value) => Self::String(value.clone()),
            Self::Array(value) => Self::Array(value.clone()),
            Self::Object(value) => Self::Object(value.clone()),
            Self::Closure(value) => Self::Closure(value.clone()),
            Self::Resource(value) => Self::Resource(value.clone()),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.identical(other, 0).unwrap_or(false)
    }
}

impl Value {
    pub(crate) fn identical(&self, other: &Self, line: usize) -> Result<bool, Error> {
        self.identical_inner(other, line, &mut HashSet::new())
    }

    fn identical_inner(
        &self,
        other: &Self,
        line: usize,
        seen: &mut HashSet<usize>,
    ) -> Result<bool, Error> {
        Ok(match (self, other) {
            (Self::Reference(value) | Self::Indirect(value), other) => {
                value.borrow().identical_inner(other, line, seen)?
            }
            (value, Self::Reference(other) | Self::Indirect(other)) => {
                value.identical_inner(&other.borrow(), line, seen)?
            }
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Object(a), Self::Object(b)) => a == b,
            (Self::Closure(a), Self::Closure(b)) => a == b,
            (Self::Resource(a), Self::Resource(b)) => Rc::ptr_eq(a, b),
            (Self::Array(a), Self::Array(b)) => {
                if Rc::ptr_eq(a, b) {
                    return Ok(true);
                }
                let identity = Rc::as_ptr(a) as usize;
                if !seen.insert(identity) {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Nesting level too deep - recursive dependency?",
                        line,
                    ));
                }
                let mut equal = a.len() == b.len();
                if equal {
                    for ((ka, va), (kb, vb)) in a.entries().iter().zip(b.entries()) {
                        if ka != kb || !va.identical_inner(vb, line, seen)? {
                            equal = false;
                            break;
                        }
                    }
                }
                seen.remove(&identity);
                equal
            }
            _ => false,
        })
    }

    /// Read a PHP value without carrying an array slot's reference binding.
    pub(crate) fn dereferenced(&self) -> Self {
        match self {
            Self::Reference(value) | Self::Indirect(value) => value.borrow().dereferenced(),
            value => value.clone(),
        }
    }

    /// Array duplication drops reference containers that have no other owner.
    pub(crate) fn copy_array_entry(&self) -> Self {
        match self {
            Self::Indirect(value) => value.borrow().copy_array_entry(),
            Self::Reference(value) if Rc::strong_count(value) == 1 => value.borrow().clone(),
            value => value.clone(),
        }
    }

    pub(crate) fn is_shared_reference(&self) -> bool {
        match self {
            Self::Indirect(slot) => slot.borrow().is_shared_reference(),
            Self::Reference(reference) => Rc::strong_count(reference) > 1,
            _ => false,
        }
    }

    pub(crate) fn warn_nan(
        &self,
        kind: &str,
        line: usize,
        warnings: &mut dyn crate::diagnostics::WarningSink,
    ) {
        if let Self::Indirect(value) | Self::Reference(value) = self {
            value.borrow().warn_nan(kind, line, warnings);
            return;
        }
        if matches!(self, Self::Float(n) if n.is_nan()) {
            warnings.push(format!(
                "unexpected NAN value was coerced to {kind} on line {line}"
            ));
        }
    }

    pub(crate) fn to_bool(
        &self,
        line: usize,
        warnings: &mut dyn crate::diagnostics::WarningSink,
    ) -> bool {
        self.warn_nan("bool", line, warnings);
        self.is_truthy()
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Self::Reference(value) | Self::Indirect(value) => value.borrow().is_truthy(),
            Self::Closure(_) | Self::Object(_) | Self::Resource(_) => true,
            Self::Null => false,
            Self::Array(a) => !a.is_empty(),
            Self::Bool(b) => *b,
            Self::Int(n) => *n != 0,
            Self::Float(n) => *n != 0.0,
            Self::String(s) => !s.is_empty() && s.bytes().as_slice() != b"0",
        }
    }
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            Self::Reference(value) | Self::Indirect(value) => value.borrow().to_bytes(),
            Self::Null | Self::Bool(false) => Vec::new(),
            Self::Bool(true) => b"1".to_vec(),
            Self::Int(n) => n.to_string().into_bytes(),
            Self::Resource(resource) => format!("Resource id #{}", resource.id).into_bytes(),
            Self::Float(n) => format_float(*n).into_bytes(),
            Self::String(s) => s.to_vec(),
            Self::Array(_) => b"Array".to_vec(),
            Self::Closure(_) | Self::Object(_) => b"Object".to_vec(),
        }
    }
    pub(crate) fn number(
        &self,
        line: usize,
        warnings: &mut dyn crate::diagnostics::WarningSink,
    ) -> Result<Self, Error> {
        match self {
            Self::Reference(value) | Self::Indirect(value) => value.borrow().number(line, warnings),
            Self::Object(object) => Err(Error::new(
                ErrorKind::TypeError,
                format!("Unsupported operand type: {}", object.class_name),
                line,
            )),
            Self::Closure(_) => Err(Error::new(
                ErrorKind::TypeError,
                "Unsupported operand type: Closure",
                line,
            )),
            Self::Resource(_) => Err(Error::new(
                ErrorKind::TypeError,
                "Unsupported operand type: resource",
                line,
            )),
            Self::Array(_) => Err(Error::new(
                ErrorKind::TypeError,
                "Unsupported operand type: array",
                line,
            )),
            Self::Null => Ok(Self::Int(0)),
            Self::Bool(b) => Ok(Self::Int(i64::from(*b))),
            Self::Int(_) | Self::Float(_) => Ok(self.clone()),
            Self::String(s) => {
                let s = s.bytes();
                let (value, complete) = numeric_prefix(&s).ok_or_else(|| {
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
    pub(crate) fn integer(
        &self,
        line: usize,
        warnings: &mut dyn crate::diagnostics::WarningSink,
    ) -> Result<i64, Error> {
        match self.number(line, warnings)? {
            Self::Int(n) => Ok(n),
            Self::Float(n) => {
                let converted = float_to_int(n);
                if !n.is_finite() || !(-9223372036854775808.0..9223372036854775808.0).contains(&n) {
                    warnings.push(format!(
                        "The float {} is not representable as an int, cast occurred on line {line}",
                        format_float(n)
                    ));
                }
                // PHP's range predicate admits NaN, so it also receives the
                // precision diagnostic. Infinity and finite overflow do not.
                if n != converted as f64
                    && (n.is_nan() || (-9223372036854775808.0..9223372036854775808.0).contains(&n))
                {
                    warnings.push(format!("Deprecated: Implicit conversion from float {} to int loses precision on line {line}", format_float(n)));
                }
                Ok(converted)
            }
            _ => unreachable!(),
        }
    }
    pub(crate) fn as_float(&self) -> f64 {
        match self {
            Self::Int(n) => *n as f64,
            Self::Float(n) => *n,
            _ => unreachable!("numeric value required"),
        }
    }
}

pub(crate) fn numeric_string(bytes: &[u8]) -> Option<Value> {
    let (value, complete) = numeric_prefix(bytes)?;
    complete.then_some(value)
}

pub(crate) fn numeric_prefix(bytes: &[u8]) -> Option<(Value, bool)> {
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

/// Zend converts finite doubles modulo 2^64; Rust's numeric casts saturate.
pub(crate) fn float_to_int(n: f64) -> i64 {
    if !n.is_finite() {
        return 0;
    }
    let n = n.trunc() % 18446744073709551616.0;
    if n >= 9223372036854775808.0 {
        (n - 18446744073709551616.0) as i64
    } else if n < -9223372036854775808.0 {
        (n + 18446744073709551616.0) as i64
    } else {
        n as i64
    }
}
