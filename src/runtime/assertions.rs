// Altered Rust adaptation of PHP 8.5.0's ext/standard/assert.c.
// See THIRD_PARTY_NOTICES.md.
use super::*;

pub(super) struct Options {
    pub active: bool,
    callback: Option<Value>,
    bail: bool,
    warning: bool,
    exception: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            active: true,
            callback: None,
            bail: false,
            warning: true,
            exception: true,
        }
    }
}

impl Runtime {
    pub(super) fn assert_options(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let option = self.native_int(args[0].as_ref().unwrap(), "assert_options", 1, line)?;
        if !(1..=5).contains(&option) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "assert_options(): Argument #1 ($option) must be an ASSERT_* constant",
                line,
            ));
        }
        if option == 2 {
            let old = self
                .assertion_options
                .callback
                .clone()
                .unwrap_or(Value::Null);
            if let Some(value) = &args[1] {
                self.assertion_options.callback =
                    (!matches!(value, Value::Null)).then(|| value.clone());
            }
            return Ok(old);
        }
        let previous = match option {
            1 => self.assertion_options.active,
            3 => self.assertion_options.bail,
            4 => self.assertion_options.warning,
            _ => self.assertion_options.exception,
        };
        if let Some(value) = &args[1] {
            let text = self.string_bytes(value, line)?;
            let value = ini_bool(&text);
            match option {
                1 => self.assertion_options.active = value,
                3 => self.assertion_options.bail = value,
                4 => self.assertion_options.warning = value,
                _ => self.assertion_options.exception = value,
            }
        }
        Ok(Value::Int(i64::from(previous)))
    }

    pub(super) fn assert_value(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let description = match args[1].as_ref() {
            None | Some(Value::Null) => None,
            Some(Value::Object(object)) if object.class.is_a(b"Throwable") => {
                Some(Value::Object(object.clone()))
            }
            Some(value) => Some(Value::String(
                self.native_string_type(value, "assert", 2, line, "Throwable|string|null")?
                    .into(),
            )),
        };
        if args[0].as_ref().unwrap().to_bool(line, self)? {
            return Ok(Value::Bool(true));
        }
        if let Some(value @ Value::Object(_)) = description {
            return Err(self.throw_value(value, line));
        }
        let message = match description {
            Some(Value::String(bytes)) => Some(bytes.to_vec()),
            _ => None,
        };
        let mut pending = if let Some(callback) = self.assertion_options.callback.clone() {
            let result = (|| {
                let target = self
                    .resolve_native_callback(&callback, line)
                    .map_err(|error| {
                        if error.throwable.is_some() || error.kind == ErrorKind::UnwindExit {
                            return error;
                        }
                        let name = self.callable_name(&callback, line).unwrap_or_default();
                        Error::new(
                            ErrorKind::Error,
                            format!(
                                "Invalid callback {}, {}",
                                String::from_utf8_lossy(&name),
                                self.callback_error(&callback, &error)
                            ),
                            line,
                        )
                    })?;
                let mut values = vec![
                    Value::String(self.source.name.clone().into()),
                    Value::Int(line as i64),
                    Value::Null,
                ];
                if let Some(message) = &message {
                    values.push(Value::String(message.clone().into()));
                }
                self.call_callback_at(&target, &values, line, true)
            })();
            match result {
                Err(error) if error.kind == ErrorKind::UnwindExit => return Err(error),
                Err(error) => Some(self.materialize_error(error)),
                Ok(_) => None,
            }
        } else {
            None
        };
        if self.assertion_options.exception {
            // zend_throw_exception accepts a C string, including its NUL boundary.
            let message = message
                .as_deref()
                .unwrap_or_default()
                .split(|b| *b == 0)
                .next()
                .unwrap();
            let error = self.materialize_error(Error::new(
                ErrorKind::AssertionError,
                String::from_utf8_lossy(message),
                line,
            ));
            {
                let object = error.throwable.as_ref().unwrap();
                let mut data = object.throwable.borrow_mut();
                let data = data.as_mut().unwrap();
                data.message = message.to_vec();
                data.code = 1;
                data.previous = pending.and_then(|error| error.throwable);
            }
            if self.assertion_options.bail {
                self.report_uncaught_inner(error);
                return Err(Error::new(ErrorKind::UnwindExit, "", line));
            }
            return Err(error);
        }
        if self.assertion_options.warning {
            let message = message.as_deref().unwrap_or(b"Assertion");
            let message = message.split(|b| *b == 0).next().unwrap();
            self.defer_warning(
                format!(
                    "assert(): {} failed on line {line}",
                    String::from_utf8_lossy(message)
                ),
                &mut pending,
            )?;
        }
        if self.assertion_options.bail {
            if let Some(error) = pending {
                self.report_pending_exception(error)?;
            }
            self.exit_status = Some(0);
            return Err(Error::new(ErrorKind::UnwindExit, "", line));
        }
        if let Some(error) = pending {
            return Err(error);
        }
        Ok(Value::Bool(false))
    }
}

fn ini_bool(text: &[u8]) -> bool {
    if [b"true".as_slice(), b"yes", b"on"]
        .iter()
        .any(|v| text.eq_ignore_ascii_case(v))
    {
        return true;
    }
    let text = text.trim_ascii_start();
    let text = text
        .strip_prefix(b"-")
        .or_else(|| text.strip_prefix(b"+"))
        .unwrap_or(text);
    text.iter()
        .take_while(|b| b.is_ascii_digit())
        .any(|b| *b != b'0')
}
