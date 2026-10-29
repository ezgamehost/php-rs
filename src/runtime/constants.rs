use super::*;
use crate::ast::Attribute;

#[derive(Clone)]
pub(crate) struct GlobalConstant {
    pub name: Vec<u8>,
    pub value: Value,
    pub source: Option<Rc<crate::source::Source>>,
    pub strict_types: bool,
    pub attributes: Vec<Attribute>,
    pub extension: Option<&'static str>,
}

impl GlobalConstant {
    pub fn is_deprecated(&self) -> bool {
        self.attributes
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(b"Deprecated"))
    }
}

impl Runtime {
    pub(super) fn lookup_constant(&self, name: &[u8]) -> Option<Value> {
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        self.constants
            .get(&key(name))
            .map(|constant| constant.value.clone())
            .or_else(|| predefined(name))
    }

    pub(super) fn constant_metadata(&self, name: &[u8]) -> Option<Rc<GlobalConstant>> {
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        if let Some(constant) = self.constants.get(&key(name)) {
            return Some(constant.clone());
        }
        let value = predefined(name)?;
        let canonical = if [b"true".as_slice(), b"false", b"null"]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
        {
            name.to_ascii_uppercase()
        } else {
            name.to_vec()
        };
        let extension = if matches!(
            name,
            b"COUNT_NORMAL"
                | b"COUNT_RECURSIVE"
                | b"ARRAY_FILTER_USE_BOTH"
                | b"ARRAY_FILTER_USE_KEY"
                | b"STR_PAD_LEFT"
                | b"STR_PAD_RIGHT"
                | b"STR_PAD_BOTH"
                | b"DIRECTORY_SEPARATOR"
                | b"PATH_SEPARATOR"
                | b"INF"
                | b"NAN"
                | b"PHP_ROUND_HALF_UP"
                | b"PHP_ROUND_HALF_DOWN"
                | b"PHP_ROUND_HALF_EVEN"
                | b"PHP_ROUND_HALF_ODD"
        ) || math_constant(name).is_some()
        {
            "standard"
        } else {
            "Core"
        };
        let attributes = if name == b"E_STRICT" {
            let argument = |name: &[u8], value: &[u8]| {
                crate::ast::Argument::Named(
                    name.to_vec(),
                    Expr {
                        kind: ExprKind::Literal(Value::String(value.to_vec().into())),
                        line: 0,
                    },
                )
            };
            vec![Attribute {
                name: b"Deprecated".to_vec(),
                arguments: vec![
                    argument(b"since", b"8.4"),
                    argument(b"message", b"the error level was removed"),
                ],
                line: 0,
            }]
        } else {
            Vec::new()
        };
        Some(Rc::new(GlobalConstant {
            name: canonical,
            value,
            source: None,
            strict_types: false,
            attributes,
            extension: Some(extension),
        }))
    }

    pub(super) fn read_constant(&mut self, name: &[u8], line: usize) -> Result<Value, Error> {
        let value = self.lookup_constant(name).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!("Undefined constant \"{}\"", String::from_utf8_lossy(name)),
                line,
            )
        })?;
        if name.strip_prefix(b"\\").unwrap_or(name) == b"E_STRICT" {
            self.diagnostics.push(format!("Deprecated: Constant E_STRICT is deprecated since 8.4, the error level was removed on line {line}"));
        }
        Ok(value)
    }

    pub(super) fn define_constant(&mut self, name: Vec<u8>, value: Value, line: usize) -> bool {
        self.define_constant_with_attributes(name, value, Vec::new(), line)
    }

    pub(super) fn define_constant_with_attributes(
        &mut self,
        name: Vec<u8>,
        value: Value,
        attributes: Vec<Attribute>,
        line: usize,
    ) -> bool {
        let key = key(&name);
        if self.constants.contains_key(&key) || predefined(&name).is_some() {
            self.diagnostics.push(format!(
                "Constant {} already defined, this will be an error in PHP 9 on line {line}",
                String::from_utf8_lossy(&name)
            ));
            return false;
        }
        self.constants.insert(
            key,
            Rc::new(GlobalConstant {
                name,
                value,
                source: Some(self.source.clone()),
                strict_types: self.strict_types,
                attributes,
                extension: None,
            }),
        );
        true
    }
}

fn key(name: &[u8]) -> Vec<u8> {
    let mut key = name.to_vec();
    if let Some(separator) = key.iter().rposition(|byte| *byte == b'\\') {
        key[..separator].make_ascii_lowercase();
    }
    key
}

fn predefined(name: &[u8]) -> Option<Value> {
    if name.eq_ignore_ascii_case(b"true") {
        return Some(Value::Bool(true));
    }
    if name.eq_ignore_ascii_case(b"false") {
        return Some(Value::Bool(false));
    }
    if name.eq_ignore_ascii_case(b"null") {
        return Some(Value::Null);
    }
    Some(match name {
        b"PHP_VERSION" => Value::String(b"8.5.0".to_vec().into()),
        b"PHP_VERSION_ID" => Value::Int(80500),
        b"PHP_MAJOR_VERSION" | b"PHP_INT_SIZE" => Value::Int(8),
        b"PHP_MINOR_VERSION" => Value::Int(5),
        b"PHP_RELEASE_VERSION" | b"COUNT_NORMAL" | b"STR_PAD_LEFT" => Value::Int(0),
        b"PHP_EXTRA_VERSION" => Value::String(Vec::new().into()),
        b"PHP_INT_MAX" => Value::Int(i64::MAX),
        b"PHP_INT_MIN" => Value::Int(i64::MIN),
        b"PHP_FLOAT_DIG" => Value::Int(15),
        b"PHP_FLOAT_MIN" => Value::Float(f64::MIN_POSITIVE),
        b"PHP_FLOAT_MAX" => Value::Float(f64::MAX),
        b"PHP_FLOAT_EPSILON" => Value::Float(f64::EPSILON),
        b"INF" => Value::Float(f64::INFINITY),
        b"NAN" => Value::Float(f64::NAN),
        b"PHP_EOL" => Value::String(if cfg!(windows) {
            b"\r\n".to_vec().into()
        } else {
            b"\n".to_vec().into()
        }),
        b"DIRECTORY_SEPARATOR" => {
            Value::String(std::path::MAIN_SEPARATOR.to_string().into_bytes().into())
        }
        b"PATH_SEPARATOR" => Value::String(if cfg!(windows) {
            b";".to_vec().into()
        } else {
            b":".to_vec().into()
        }),
        b"COUNT_RECURSIVE" | b"E_ERROR" | b"ARRAY_FILTER_USE_BOTH" | b"STR_PAD_RIGHT" => {
            Value::Int(1)
        }
        b"E_WARNING" | b"ARRAY_FILTER_USE_KEY" | b"STR_PAD_BOTH" => Value::Int(2),
        b"E_PARSE" => Value::Int(4),
        b"PHP_ROUND_HALF_UP" => Value::Int(1),
        b"PHP_ROUND_HALF_DOWN" => Value::Int(2),
        b"PHP_ROUND_HALF_EVEN" => Value::Int(3),
        b"PHP_ROUND_HALF_ODD" => Value::Int(4),
        b"E_NOTICE" => Value::Int(8),
        b"E_CORE_ERROR" => Value::Int(16),
        b"E_CORE_WARNING" => Value::Int(32),
        b"E_COMPILE_ERROR" => Value::Int(64),
        b"E_COMPILE_WARNING" => Value::Int(128),
        b"E_USER_ERROR" => Value::Int(256),
        b"E_USER_WARNING" => Value::Int(512),
        b"E_USER_NOTICE" => Value::Int(1024),
        b"E_STRICT" => Value::Int(2048),
        b"E_RECOVERABLE_ERROR" => Value::Int(4096),
        b"E_DEPRECATED" => Value::Int(8192),
        b"E_USER_DEPRECATED" => Value::Int(16384),
        b"E_ALL" => Value::Int(30719),
        b"DEBUG_BACKTRACE_PROVIDE_OBJECT" => Value::Int(1),
        b"DEBUG_BACKTRACE_IGNORE_ARGS" => Value::Int(2),
        _ => return math_constant(name).map(Value::Float),
    })
}

fn math_constant(name: &[u8]) -> Option<f64> {
    use std::f64::consts;
    Some(match name {
        b"M_E" => consts::E,
        b"M_LOG2E" => consts::LOG2_E,
        b"M_LOG10E" => consts::LOG10_E,
        b"M_LN2" => consts::LN_2,
        b"M_LN10" => consts::LN_10,
        b"M_PI" => consts::PI,
        b"M_PI_2" => consts::FRAC_PI_2,
        b"M_PI_4" => consts::FRAC_PI_4,
        b"M_1_PI" => consts::FRAC_1_PI,
        b"M_2_PI" => consts::FRAC_2_PI,
        b"M_SQRTPI" => 1.772453850905516,
        b"M_2_SQRTPI" => consts::FRAC_2_SQRT_PI,
        b"M_LNPI" => 1.1447298858494002,
        b"M_EULER" => 0.5772156649015329,
        b"M_SQRT2" => consts::SQRT_2,
        b"M_SQRT1_2" => consts::FRAC_1_SQRT_2,
        b"M_SQRT3" => 3.0_f64.sqrt(),
        _ => return None,
    })
}
