use super::*;

impl Runtime {
    pub(super) fn lookup_constant(&self, name: &[u8]) -> Option<Value> {
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        self.constants
            .get(&key(name))
            .cloned()
            .or_else(|| predefined(name))
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
            self.warnings.push(format!("Deprecated: Constant E_STRICT is deprecated since 8.4, the error level was removed on line {line}"));
        }
        Ok(value)
    }

    pub(super) fn define_constant(&mut self, name: Vec<u8>, value: Value, line: usize) -> bool {
        let key = key(&name);
        if self.constants.contains_key(&key) || predefined(&name).is_some() {
            self.warnings.push(format!(
                "Constant {} already defined, this will be an error in PHP 9 on line {line}",
                String::from_utf8_lossy(&name)
            ));
            return false;
        }
        self.constants.insert(key, value);
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
        b"PHP_RELEASE_VERSION" | b"COUNT_NORMAL" => Value::Int(0),
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
        b"COUNT_RECURSIVE" | b"E_ERROR" | b"ARRAY_FILTER_USE_BOTH" => Value::Int(1),
        b"E_WARNING" | b"ARRAY_FILTER_USE_KEY" => Value::Int(2),
        b"E_PARSE" => Value::Int(4),
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
        _ => return None,
    })
}
