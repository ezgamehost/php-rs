// Altered Rust adaptation of PHP 8.5.0 ext/standard/string.c's
// str_increment/str_decrement, copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn string_step(
        &mut self,
        increment: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let mut bytes = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let invalid = |reason: &str| {
            Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 ($string) {reason}"),
                line,
            )
        };
        if bytes.is_empty() {
            return Err(invalid("must not be empty"));
        }
        if !bytes.iter().all(u8::is_ascii_alphanumeric) {
            return Err(invalid(
                "must be composed only of alphanumeric ASCII characters",
            ));
        }
        if !increment && (bytes[0] == b'0' || (bytes.len() == 1 && matches!(bytes[0], b'a' | b'A')))
        {
            return Err(invalid(&format!(
                "\"{}\" is out of decrement range",
                String::from_utf8_lossy(&bytes)
            )));
        }
        let mut carry = true;
        for byte in bytes.iter_mut().rev() {
            if increment {
                *byte = match *byte {
                    b'z' => b'a',
                    b'Z' => b'A',
                    b'9' => b'0',
                    other => {
                        carry = false;
                        other + 1
                    }
                };
            } else {
                *byte = match *byte {
                    b'a' => b'z',
                    b'A' => b'Z',
                    b'0' => b'9',
                    other => {
                        carry = false;
                        other - 1
                    }
                };
            }
            if !carry {
                break;
            }
        }
        if increment && carry {
            let prefix = if bytes[0] == b'0' { b'1' } else { bytes[0] };
            bytes.insert(0, prefix);
        } else if !increment && (carry || (bytes[0] == b'0' && bytes.len() > 1)) {
            bytes.remove(0);
        }
        Ok(Value::String(bytes.into()))
    }
}
