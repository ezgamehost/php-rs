use super::*;

impl Runtime {
    pub(super) fn compile_fatal_at(
        &mut self,
        message: &str,
        source: &crate::source::Source,
        line: usize,
    ) -> Error {
        self.diagnostics.last_fatal_trace = None;
        self.diagnostics.output_position = self.output.len();
        self.diagnostics.display(
            64,
            "Fatal error",
            message.as_bytes(),
            &source.name,
            line as i64,
        );
        self.exit_status = Some(255);
        Error::new(ErrorKind::UnwindExit, "", line)
    }

    /// Configure native fatal backtraces. PHP's PHPT runner disables these by
    /// default; ordinary requests use PHP 8.5's enabled default.
    pub fn set_fatal_error_backtraces(&mut self, enabled: bool) {
        self.fatal_backtraces = Some(enabled);
    }

    pub(super) fn native_fatal(&mut self, message: &[u8], line: usize, capture: bool) -> Error {
        self.native_fatal_at(message, &self.source.name.clone(), line, 1, capture)
    }

    pub(super) fn native_fatal_at(
        &mut self,
        message: &[u8],
        file: &[u8],
        line: usize,
        level: i64,
        capture: bool,
    ) -> Error {
        self.diagnostics.last_fatal_trace =
            (capture && self.fatal_backtraces.unwrap_or(true)).then(|| self.capture_trace());
        self.diagnostics.output_position = self.output.len();
        let previous = self.diagnostics.events.len();
        self.diagnostics
            .display(level, "Fatal error", message, file, line as i64);
        if self.diagnostics.events.len() > previous
            && let Some(trace) = &self.diagnostics.last_fatal_trace
        {
            let event = self.diagnostics.events.last_mut().unwrap();
            event.rendered.extend_from_slice(b"Stack trace:\n");
            event.rendered.extend(traces::trace_string(trace));
            event.rendered.push(b'\n');
        }
        self.exit_status = Some(255);
        Error::new(ErrorKind::UnwindExit, "", line)
    }

    // An exception pending when Zend raises a native fatal is rendered as a
    // warning first. This bypasses PHP error and exception handlers.
    pub(super) fn report_pending_exception(&mut self, mut error: Error) -> Result<(), Error> {
        loop {
            if error.kind == ErrorKind::UnwindExit {
                return Err(error);
            }
            let materialized = self.materialize_error(error);
            let object = materialized.throwable.unwrap();
            let method = objects::method(&object.class, b"__toString").unwrap();
            let result = self
                .unchecked_method_target(method, object.class.clone(), Some(object.clone()), 0)
                .and_then(|target| self.invoke_at(target, &[], 0, false, true));
            let text = match result {
                Ok(value) => value.to_bytes(),
                Err(next) => {
                    error = next;
                    continue;
                }
            };
            object.set_throwable_property(b"string", Value::String(text.clone().into()));
            let data = object.throwable_snapshot();
            let file = if data.file.is_empty() {
                b"Unknown".as_slice()
            } else {
                &data.file
            };
            let mut message = b"Uncaught ".to_vec();
            message.extend(text);
            message.extend_from_slice(b"\n  thrown");
            self.diagnostics.last_fatal_trace = None;
            self.diagnostics.output_position = self.output.len();
            self.diagnostics
                .display(2, "Warning", &message, file, data.line as u32 as i64);
            return Ok(());
        }
    }

    /// Report an execution error as an uncaught PHP error. Call once at the
    /// request boundary, before reading/writing displayed output. This may run
    /// a registered exception handler or user-defined exception __toString(),
    /// including their output or exit(). A handled exception preserves an existing
    /// exit status, or sets it to 0 if no earlier status was set.
    /// Execution APIs themselves continue returning errors without reporting them.
    pub fn report_uncaught(&mut self, error: &Error) {
        self.diagnostics.messages = std::mem::take(&mut self.warnings);
        self.diagnostics.output_position = self.output.len();
        let source =
            self.replace_source(Rc::new(crate::source::Source::memory(b"[no active file]")));
        if self.uncaught_handler_allowed {
            let _ = self.dispatch_uncaught(error.clone());
        } else {
            self.report_uncaught_inner(error.clone());
        }
        self.replace_source(source);
        self.warnings = std::mem::take(&mut self.diagnostics.messages);
        self.variables = self
            .scope
            .iter()
            .map(|(name, value)| (name.clone(), value.borrow().clone()))
            .collect();
    }

    pub(super) fn report_uncaught_inner(&mut self, mut error: Error) {
        loop {
            if error.kind == ErrorKind::UnwindExit {
                return;
            }
            self.diagnostics.last_fatal_trace = None;
            let error_object = self.materialize_error(error);
            let object = error_object.throwable.unwrap();
            let data = object.throwable_snapshot();
            // PHP special-cases the exact internal classes, not their subclasses.
            let label = match object.class.name.as_slice() {
                b"ParseError" => Some((4, "Parse error")),
                b"CompileError" => Some((64, "Fatal error")),
                _ => None,
            };
            if let Some((level, label)) = label {
                self.exit_status = Some(255);
                self.diagnostics.display(
                    level,
                    label,
                    &data.message,
                    &data.file,
                    data.line as u32 as i64,
                );
                return;
            }
            let method = objects::method(&object.class, b"__toString").unwrap();
            let result = self
                .unchecked_method_target(method, object.class.clone(), Some(object.clone()), 0)
                .and_then(|target| self.invoke_at(target, &[], 0, false, true))
                .map(|value| value.dereferenced());
            let text = match result {
                Ok(Value::String(text)) => text.to_vec(),
                Ok(_) => unreachable!("Throwable::__toString has a string return type"),
                Err(next) => {
                    // With no active user frame, a throw during fatal reporting
                    // becomes the new uncaught error; exit() terminates reporting.
                    error = next;
                    continue;
                }
            };
            object.set_throwable_property(b"string", Value::String(text.clone().into()));
            let data = object.throwable_snapshot();
            let file = if data.file.is_empty() {
                b"Unknown".as_slice()
            } else {
                &data.file
            };
            let mut message = b"Uncaught ".to_vec();
            message.extend(text);
            message.extend_from_slice(b"\n  thrown");
            self.diagnostics
                .display(1, "Fatal error", &message, file, data.line as u32 as i64);
            self.exit_status = Some(255);
            return;
        }
    }
}
