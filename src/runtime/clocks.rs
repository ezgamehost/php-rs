// Altered Rust adaptation of PHP 8.5.0 ext/standard/{microtime,hrtime}.c
// and ext/date/php_date.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use std::{
    sync::OnceLock,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy)]
pub(super) enum Operation {
    Time,
    Microtime,
    Hrtime,
}

impl Runtime {
    pub(super) fn clock_builtin(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        // PHP captures the monotonic timestamp before argument conversion, which
        // may invoke a user error handler for a deprecated null argument.
        let monotonic = if matches!(operation, Operation::Hrtime) {
            static ORIGIN: OnceLock<Instant> = OnceLock::new();
            ORIGIN.get_or_init(Instant::now).elapsed().as_nanos() as u64
        } else {
            0
        };
        let numeric = args
            .first()
            .and_then(Option::as_ref)
            .map(|value| self.native_bool(value, name, 1, line))
            .transpose()?
            .unwrap_or(false);
        if matches!(operation, Operation::Hrtime) {
            if numeric {
                return Ok(Value::Int(monotonic as i64));
            }
            let mut result = PhpArray::default();
            result.append(Value::Int((monotonic / 1_000_000_000) as i64), line)?;
            result.append(Value::Int((monotonic % 1_000_000_000) as i64), line)?;
            return Ok(Value::Array(Rc::new(result)));
        }
        let nanoseconds = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos() as i128,
            Err(error) => -(error.duration().as_nanos() as i128),
        };
        let seconds = nanoseconds.div_euclid(1_000_000_000) as i64;
        if matches!(operation, Operation::Time) {
            return Ok(Value::Int(seconds));
        }
        let micros = nanoseconds.rem_euclid(1_000_000_000) / 1_000;
        Ok(if numeric {
            Value::Float(seconds as f64 + micros as f64 / 1_000_000.0)
        } else {
            Value::String(format!("0.{micros:06}00 {seconds}").into_bytes().into())
        })
    }
}
