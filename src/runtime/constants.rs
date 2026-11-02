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
            .or_else(|| self.resources.standard(name).map(Value::Resource))
            .or_else(|| predefined(name))
    }

    pub(super) fn constant_metadata(&self, name: &[u8]) -> Option<Rc<GlobalConstant>> {
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        if let Some(constant) = self.constants.get(&key(name)) {
            return Some(constant.clone());
        }
        let value = self
            .resources
            .standard(name)
            .map(Value::Resource)
            .or_else(|| predefined(name))?;
        let canonical = if [b"true".as_slice(), b"false", b"null"]
            .iter()
            .any(|n| name.eq_ignore_ascii_case(n))
        {
            name.to_ascii_uppercase()
        } else {
            name.to_vec()
        };
        let extension = if matches!(name, b"MT_RAND_PHP" | b"MT_RAND_MT19937") {
            "random"
        } else if name.starts_with(b"ASSERT_")
            || matches!(
                name,
                b"COUNT_NORMAL"
                    | b"COUNT_RECURSIVE"
                    | b"CASE_LOWER"
                    | b"CASE_UPPER"
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
            )
            || math_constant(name).is_some()
            || sort_constant(name).is_some()
            || seek_constant(name).is_some()
            || pathinfo_constant(name).is_some()
            || url_constant(name).is_some()
            || extract_constant(name).is_some()
        {
            "standard"
        } else {
            "Core"
        };
        let attributes =
            if matches!(name, b"E_STRICT" | b"MT_RAND_PHP") || assertion_constant(name).is_some() {
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
                        argument(b"since", if name == b"E_STRICT" { b"8.4" } else { b"8.3" }),
                        argument(
                            b"message",
                            if name == b"E_STRICT" {
                                b"the error level was removed"
                            } else if name == b"MT_RAND_PHP" {
                                b"as it uses a biased non-standard variant of Mt19937"
                            } else {
                                b"as assert_options() is deprecated"
                            },
                        ),
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
        let line = self
            .attribute_evaluation_line
            .filter(|(depth, _)| *depth == self.trace_frames.len())
            .map_or(line, |(_, line)| line);
        let value = self.lookup_constant(name).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!("Undefined constant \"{}\"", String::from_utf8_lossy(name)),
                line,
            )
        })?;
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        if name == b"E_STRICT" {
            self.warn(format!("Deprecated: Constant E_STRICT is deprecated since 8.4, the error level was removed on line {line}"))?;
        } else if assertion_constant(name).is_some() {
            self.warn(format!("Deprecated: Constant {} is deprecated since 8.3, as assert_options() is deprecated on line {line}", String::from_utf8_lossy(name)))?;
        } else if name == b"MT_RAND_PHP" {
            self.warn(format!("Deprecated: Constant MT_RAND_PHP is deprecated since 8.3, as it uses a biased non-standard variant of Mt19937 on line {line}"))?;
        } else if let Some(constant) = self.constants.get(&key(name)).cloned() {
            self.deprecated_constant(&constant.attributes, &constant.name, None, false, line)?;
        }
        Ok(value)
    }

    pub(super) fn define_constant(
        &mut self,
        name: Vec<u8>,
        value: Value,
        line: usize,
    ) -> Result<bool, Error> {
        self.define_constant_with_attributes(name, value, Vec::new(), line)
    }

    pub(super) fn define_constant_with_attributes(
        &mut self,
        name: Vec<u8>,
        value: Value,
        attributes: Vec<Attribute>,
        line: usize,
    ) -> Result<bool, Error> {
        let key = key(&name);
        if self.constants.contains_key(&key) || predefined(&name).is_some() {
            self.warn(format!(
                "Constant {} already defined, this will be an error in PHP 9 on line {line}",
                String::from_utf8_lossy(&name)
            ))?;
            return Ok(false);
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
        Ok(true)
    }
}

fn key(name: &[u8]) -> Vec<u8> {
    let mut key = name.to_vec();
    if let Some(separator) = key.iter().rposition(|byte| *byte == b'\\') {
        key[..separator].make_ascii_lowercase();
    }
    key
}

fn sort_constant(name: &[u8]) -> Option<Value> {
    Some(Value::Int(match name {
        b"SORT_REGULAR" => 0,
        b"SORT_NUMERIC" => 1,
        b"SORT_STRING" => 2,
        b"SORT_DESC" => 3,
        b"SORT_ASC" => 4,
        b"SORT_LOCALE_STRING" => 5,
        b"SORT_NATURAL" => 6,
        b"SORT_FLAG_CASE" => 8,
        _ => return None,
    }))
}

fn extract_constant(name: &[u8]) -> Option<Value> {
    Some(Value::Int(match name {
        b"EXTR_OVERWRITE" => 0,
        b"EXTR_SKIP" => 1,
        b"EXTR_PREFIX_SAME" => 2,
        b"EXTR_PREFIX_ALL" => 3,
        b"EXTR_PREFIX_INVALID" => 4,
        b"EXTR_PREFIX_IF_EXISTS" => 5,
        b"EXTR_IF_EXISTS" => 6,
        b"EXTR_REFS" => 256,
        _ => return None,
    }))
}

fn seek_constant(name: &[u8]) -> Option<Value> {
    Some(Value::Int(match name {
        b"SEEK_SET" => 0,
        b"SEEK_CUR" => 1,
        b"SEEK_END" => 2,
        _ => return None,
    }))
}

fn assertion_constant(name: &[u8]) -> Option<i64> {
    Some(match name {
        b"ASSERT_ACTIVE" => 1,
        b"ASSERT_CALLBACK" => 2,
        b"ASSERT_BAIL" => 3,
        b"ASSERT_WARNING" => 4,
        b"ASSERT_EXCEPTION" => 5,
        _ => return None,
    })
}

fn predefined(name: &[u8]) -> Option<Value> {
    if matches!(name, b"MT_RAND_PHP" | b"MT_RAND_MT19937") {
        return Some(Value::Int(i64::from(name == b"MT_RAND_PHP")));
    }
    if let Some(value) = assertion_constant(name) {
        return Some(Value::Int(value));
    }
    if let Some(value) = pathinfo_constant(name) {
        return Some(value);
    }
    if let Some(value) = url_constant(name) {
        return Some(value);
    }
    if let Some(value) = seek_constant(name) {
        return Some(value);
    }

    if let Some(value) = extract_constant(name) {
        return Some(value);
    }

    if let Some(value) = sort_constant(name) {
        return Some(value);
    }
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
        b"PHP_OS" => Value::String(platform_os().as_bytes().to_vec().into()),
        b"PHP_OS_FAMILY" => Value::String(
            match platform_os() {
                "WINNT" => "Windows",
                "Darwin" => "Darwin",
                "Linux" => "Linux",
                "FreeBSD" | "OpenBSD" | "NetBSD" | "DragonFly" => "BSD",
                "SunOS" => "Solaris",
                _ => "Unknown",
            }
            .as_bytes()
            .to_vec()
            .into(),
        ),
        b"PHP_VERSION" => Value::String(b"8.5.0".to_vec().into()),
        b"PHP_VERSION_ID" => Value::Int(80500),
        b"PHP_MAJOR_VERSION" | b"PHP_INT_SIZE" => Value::Int(8),
        b"PHP_MINOR_VERSION" => Value::Int(5),
        b"PHP_RELEASE_VERSION" | b"COUNT_NORMAL" | b"STR_PAD_LEFT" | b"CASE_LOWER" => Value::Int(0),
        b"CASE_UPPER" => Value::Int(1),
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

fn url_constant(name: &[u8]) -> Option<Value> {
    Some(Value::Int(match name {
        b"PHP_URL_SCHEME" => 0,
        b"PHP_URL_HOST" => 1,
        b"PHP_URL_PORT" => 2,
        b"PHP_URL_USER" => 3,
        b"PHP_URL_PASS" => 4,
        b"PHP_URL_PATH" => 5,
        b"PHP_URL_QUERY" => 6,
        b"PHP_URL_FRAGMENT" => 7,
        b"PHP_QUERY_RFC1738" => 1,
        b"PHP_QUERY_RFC3986" => 2,
        _ => return None,
    }))
}

fn pathinfo_constant(name: &[u8]) -> Option<Value> {
    Some(Value::Int(match name {
        b"PATHINFO_DIRNAME" => 1,
        b"PATHINFO_BASENAME" => 2,
        b"PATHINFO_EXTENSION" => 4,
        b"PATHINFO_FILENAME" => 8,
        b"PATHINFO_ALL" => 15,
        b"SCANDIR_SORT_ASCENDING" => 0,
        b"SCANDIR_SORT_DESCENDING" => 1,
        b"SCANDIR_SORT_NONE" => 2,
        b"FILE_USE_INCLUDE_PATH" => 1,
        b"FILE_IGNORE_NEW_LINES" => 2,
        b"FILE_SKIP_EMPTY_LINES" => 4,
        b"FILE_NO_DEFAULT_CONTEXT" => 16,
        b"FILE_APPEND" => 8,
        b"LOCK_SH" => 1,
        b"LOCK_EX" => 2,
        b"LOCK_UN" => 3,
        b"LOCK_NB" => 4,
        _ => return None,
    }))
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
        b"M_EULER" => consts::EULER_GAMMA,
        b"M_SQRT2" => consts::SQRT_2,
        b"M_SQRT1_2" => consts::FRAC_1_SQRT_2,
        b"M_SQRT3" => 3.0_f64.sqrt(),
        _ => return None,
    })
}

fn platform_os() -> &'static str {
    match std::env::consts::OS {
        "macos" | "ios" | "tvos" | "watchos" | "visionos" => "Darwin",
        "linux" | "android" => "Linux",
        "windows" => "WINNT",
        "freebsd" => "FreeBSD",
        "openbsd" => "OpenBSD",
        "netbsd" => "NetBSD",
        "dragonfly" => "DragonFly",
        "solaris" | "illumos" => "SunOS",
        "aix" => "AIX",
        "haiku" => "Haiku",
        name => name,
    }
}
