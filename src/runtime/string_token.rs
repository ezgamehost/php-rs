// Altered Rust adaptation of PHP 8.5.0 ext/standard/string.c strtok.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md for its license.
use super::*;

impl Runtime {
    pub(super) fn string_token(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let string = self.native_string(args[0].as_ref().unwrap(), "strtok", 1, line)?;
        let token = args[1]
            .as_ref()
            .filter(|v| !matches!(v, Value::Null))
            .map(|v| self.native_string_type(v, "strtok", 2, line, "?string"))
            .transpose()?;
        // Coercion may itself call strtok; update and borrow the request state
        // only after both parameters have been converted.
        let token = if let Some(token) = token {
            self.string_token_state = Some((string, 0));
            token
        } else {
            string
        };
        let Some((text, cursor)) = &mut self.string_token_state else {
            self.diagnostics.push(format!(
                "strtok(): Both arguments must be provided when starting tokenization on line {line}"
            ));
            return Ok(Value::Bool(false));
        };
        if *cursor >= text.len() {
            return Ok(Value::Bool(false));
        }
        let mut delimiters = [false; 256];
        for byte in token {
            delimiters[byte as usize] = true;
        }
        let mut start = *cursor;
        while start < text.len() && delimiters[text[start] as usize] {
            start += 1;
        }
        if start == text.len() {
            // Consuming a delimiter-only remainder releases the source. A
            // later continuation warns, unlike an already exhausted cursor.
            self.string_token_state = None;
            return Ok(Value::Bool(false));
        }
        let mut end = start + 1;
        while end < text.len() && !delimiters[text[end] as usize] {
            end += 1;
        }
        let result = text[start..end].to_vec();
        *cursor = end + 1;
        Ok(Value::String(result.into()))
    }
}
