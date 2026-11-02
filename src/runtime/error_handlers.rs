// Altered Rust adaptation of PHP 8.5.0's Zend error-handler registration
// and dispatch rules. See THIRD_PARTY_NOTICES.md.
use super::*;
use builtins::Builtin;

impl Runtime {
    pub(super) fn attribute_deprecation(
        &mut self,
        message: &[u8],
        level: i64,
        line: usize,
        internal: bool,
    ) -> Result<(), Error> {
        self.diagnostics.last_fatal_trace = None;
        let (file, line) = self.attribute_constant_location.as_ref().map_or_else(
            || (self.diagnostics.source.name.clone(), line),
            |(source, line)| (source.name.clone(), *line),
        );
        if !self.dispatch_error_handler(level, message, &file, line, internal.then_some(true))? {
            self.diagnostics.output_position = self.output.len();
            self.diagnostics
                .display(level, "Deprecated", message, &file, line as i64);
            if self.diagnostics.reporting & level != 0 {
                self.diagnostics.messages.push(format!(
                    "Deprecated: {} on line {line}",
                    String::from_utf8_lossy(message)
                ));
            }
        }
        Ok(())
    }
    pub(super) fn error_handler_builtin(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        match builtin {
            Builtin::ErrorGetLast => Ok(self
                .diagnostics
                .last_error
                .as_ref()
                .map_or(Value::Null, |error| {
                    error.value(self.diagnostics.last_fatal_trace.as_ref())
                })),
            Builtin::ErrorClearLast => {
                self.diagnostics.last_error = None;
                self.diagnostics.last_fatal_trace = None;
                Ok(Value::Null)
            }
            Builtin::SetErrorHandler => {
                let value = args[0].as_ref().unwrap();
                self.native_callback(value, "set_error_handler", 1, true, line)?;
                let mask = args[1]
                    .as_ref()
                    .map(|value| self.native_int(value, "set_error_handler", 2, line))
                    .transpose()?
                    .unwrap_or(30719);
                let previous = self.error_handler.take();
                self.error_handlers
                    .push((previous.clone(), self.error_handler_reporting));
                if !matches!(value, Value::Null) {
                    self.error_handler = Some(value.clone());
                    self.error_handler_reporting = mask as i32 as i64;
                }
                Ok(previous.unwrap_or(Value::Null))
            }
            Builtin::GetErrorHandler => Ok(self.error_handler.clone().unwrap_or(Value::Null)),
            Builtin::RestoreErrorHandler => {
                self.error_handler = None;
                if let Some((handler, mask)) = self.error_handlers.pop() {
                    self.error_handler = handler;
                    self.error_handler_reporting = mask;
                }
                Ok(Value::Bool(true))
            }
            Builtin::TriggerError => {
                let message =
                    self.native_string(args[0].as_ref().unwrap(), "trigger_error", 1, line)?;
                let level = args[1]
                    .as_ref()
                    .map(|value| self.native_int(value, "trigger_error", 2, line))
                    .transpose()?
                    .unwrap_or(1024);
                if !matches!(level, 256 | 512 | 1024 | 16384) {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "trigger_error(): Argument #2 ($error_level) must be one of E_USER_ERROR, E_USER_WARNING, E_USER_NOTICE, or E_USER_DEPRECATED",
                        line,
                    ));
                }
                if level == 256 {
                    self.warn(format!("Deprecated: Passing E_USER_ERROR to trigger_error() is deprecated since 8.4, throw an exception or call exit with a string message instead on line {line}"))?;
                }
                let file = self.diagnostics.source.name.clone();
                self.diagnostics.last_fatal_trace = (level == 256
                    && self.fatal_backtraces.unwrap_or(true))
                .then(|| self.capture_trace());
                if !self.dispatch_error_handler(level, &message, &file, line, None)? {
                    let label = match level {
                        256 => "Fatal error",
                        512 => "Warning",
                        1024 => "Notice",
                        _ => "Deprecated",
                    };
                    self.diagnostics.output_position = self.output.len();
                    let previous = self.diagnostics.events.len();
                    self.diagnostics
                        .display(level, label, &message, &file, line as i64);
                    if self.diagnostics.events.len() > previous
                        && let Some(trace) = &self.diagnostics.last_fatal_trace
                    {
                        let event = self.diagnostics.events.last_mut().unwrap();
                        event.rendered.extend_from_slice(b"Stack trace:\n");
                        event.rendered.extend(traces::trace_string(trace));
                        event.rendered.push(b'\n');
                    }
                    if self.diagnostics.reporting & level != 0 {
                        self.diagnostics.messages.push(format!(
                            "{label}: {} on line {line}",
                            String::from_utf8_lossy(&message)
                        ));
                    }
                    if level == 256 {
                        self.exit_status = Some(255);
                        return Err(Error::new(ErrorKind::UnwindExit, "", line));
                    }
                }
                Ok(Value::Bool(true))
            }
            _ => unreachable!(),
        }
    }

    fn dispatch_error_handler(
        &mut self,
        level: i64,
        message: &[u8],
        file: &[u8],
        line: usize,
        internal: Option<bool>,
    ) -> Result<bool, Error> {
        if self.folding_constants
            || level & (1 | 4 | 16 | 32 | 64 | 128) != 0
            || self.error_handler_reporting & level == 0
        {
            return Ok(false);
        }
        let Some(handler) = self.error_handler.take() else {
            return Ok(false);
        };
        // Do not push the active handler onto the registration stack. Calls to
        // restore_error_handler() inside this callback use the existing stack.
        let compiling = std::mem::replace(&mut self.diagnostics.compiling, false);
        let source = self.diagnostics.source.clone();
        let result = (|| {
            let target = self
                .resolve_native_callback(&handler, line)
                .map_err(|error| {
                    if error.throwable.is_some() || error.kind == ErrorKind::UnwindExit {
                        return error;
                    }
                    let name = self.callable_name(&handler, line).unwrap_or_default();
                    Error::new(
                        ErrorKind::Error,
                        format!(
                            "Invalid callback {}, {}",
                            String::from_utf8_lossy(&name),
                            self.callback_error(&handler, &error)
                        ),
                        line,
                    )
                })?;
            let values = [
                Value::Int(level),
                Value::String(message.to_vec().into()),
                Value::String(file.to_vec().into()),
                Value::Int(line as i64),
            ];
            let internal = internal
                .unwrap_or_else(|| self.trace_frames.last().is_some_and(|frame| frame.internal));
            self.call_callback_at(&target, &values, line, internal)
        })();
        self.diagnostics.compiling = compiling;
        self.diagnostics.source = source;
        if self.error_handler.is_none() {
            self.error_handler = Some(handler);
        }
        result.map(|value| !matches!(value.dereferenced(), Value::Bool(false)))
    }
}

impl WarningSink for Runtime {
    fn warn(&mut self, message: String) -> Result<(), Error> {
        self.diagnostics.last_fatal_trace = None;
        let level = self.diagnostics.level(&message);
        let text = message
            .strip_prefix("Deprecated: ")
            .or_else(|| message.strip_prefix("Notice: "))
            .or_else(|| message.strip_prefix("Warning: "))
            .unwrap_or(&message);
        let (text, line) = text
            .rsplit_once(" on line ")
            .and_then(|(text, line)| line.parse::<usize>().ok().map(|line| (text, line)))
            .unwrap_or((text, 0));
        let file = self.diagnostics.source.name.clone();
        if !self.dispatch_error_handler(level, text.as_bytes(), &file, line, None)? {
            self.diagnostics.output_position = self.output.len();
            self.diagnostics.push_after_handler(message);
        }
        Ok(())
    }
}
