use super::{builtins::Builtin, *};

impl Runtime {
    pub(super) fn string_builtin(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        match builtin {
            Builtin::Chr => {
                let code = self.native_int(first, "chr", 1, line)?;
                if !(0..=255).contains(&code) {
                    self.diagnostics.push(format!("Deprecated: chr(): Providing a value not in-between 0 and 255 is deprecated, this is because a byte value must be in the [0, 255] interval. The value used will be constrained using % 256 on line {line}"));
                }
                Ok(Value::String(vec![code as u8].into()))
            }
            Builtin::Ord => {
                let bytes = self.native_string(first, "ord", 1, line)?;
                if bytes.len() != 1 {
                    let message = if bytes.is_empty() {
                        "Providing an empty string is deprecated"
                    } else {
                        "Providing a string that is not one byte long is deprecated. Use ord($str[0]) instead"
                    };
                    self.diagnostics
                        .push(format!("Deprecated: ord(): {message} on line {line}"));
                }
                Ok(Value::Int(bytes.first().copied().unwrap_or(0) as i64))
            }
            Builtin::StrRev => {
                let mut bytes = self.native_string(first, "strrev", 1, line)?;
                bytes.reverse();
                Ok(Value::String(bytes.into()))
            }
            Builtin::StrRepeat => {
                let bytes = self.native_string(first, "str_repeat", 1, line)?;
                let times = self.native_int(args[1].as_ref().unwrap(), "str_repeat", 2, line)?;
                if times < 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "str_repeat(): Argument #2 ($times) must be greater than or equal to 0",
                        line,
                    ));
                }
                if bytes.is_empty() || times == 0 {
                    return Ok(Value::String(Vec::new().into()));
                }
                let length = bytes
                    .len()
                    .checked_mul(times as usize)
                    .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
                let mut result = Vec::new();
                result.try_reserve_exact(length).map_err(|_| {
                    Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
                })?;
                result.extend_from_slice(&bytes);
                while result.len() < length {
                    let count = result.len().min(length - result.len());
                    result.extend_from_within(..count);
                }
                Ok(Value::String(result.into()))
            }
            _ => unreachable!(),
        }
    }
}
