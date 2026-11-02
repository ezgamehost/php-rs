// Altered Rust adaptation of PHP 8.5.0's Zend exception-handler registration
// and dispatch rules. See THIRD_PARTY_NOTICES.md.
use super::*;
use builtins::Builtin;

impl Runtime {
    pub(super) fn exception_handler_builtin(
        &mut self,
        builtin: Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        match builtin {
            Builtin::SetExceptionHandler => {
                let value = args[0].as_ref().unwrap();
                // Validate before modifying the stack, but retain the original
                // callable value. PHP resolves it again at dispatch time.
                self.native_callback(value, "set_exception_handler", 1, true, line)?;
                let previous = self.exception_handler.take();
                self.exception_handlers.push(previous.clone());
                self.exception_handler = (!matches!(value, Value::Null)).then(|| value.clone());
                Ok(previous.unwrap_or(Value::Null))
            }
            Builtin::GetExceptionHandler => {
                Ok(self.exception_handler.clone().unwrap_or(Value::Null))
            }
            Builtin::RestoreExceptionHandler => {
                self.exception_handler = self.exception_handlers.pop().flatten();
                Ok(Value::Bool(true))
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn dispatch_uncaught(&mut self, error: Error) -> Result<(), Error> {
        if error.kind == ErrorKind::UnwindExit {
            return Err(error);
        }
        let Some(handler) = self.exception_handler.take() else {
            self.report_uncaught_inner(error.clone());
            return Err(error);
        };
        self.exception_handlers.push(Some(handler.clone()));
        let error = self.materialize_error(error);
        let exception = Value::Object(error.throwable.unwrap());
        let source =
            self.replace_source(Rc::new(crate::source::Source::memory(b"[no active file]")));
        self.diagnostics.source = Rc::new(crate::source::Source::memory(b"Unknown"));
        let result = match self.resolve_native_callback(&handler, 0) {
            Ok(target) => self.call_callback(&target, &[exception], 0),
            Err(error) if error.throwable.is_none() && error.kind != ErrorKind::UnwindExit => {
                // Re-resolution may fail after the registering class scope has
                // unwound, or after referenced members of a callback change.
                // At this boundary PHP reports a fatal callback error directly.
                let name = self.callable_name(&handler, 0).unwrap_or_default();
                let message = format!(
                    "Invalid callback {}, {}",
                    String::from_utf8_lossy(&name),
                    self.callback_error(&handler, &error),
                );
                let previous = self.diagnostics.events.len();
                self.diagnostics
                    .display(1, "Fatal error", message.as_bytes(), b"Unknown", 0);
                if self.diagnostics.events.len() > previous {
                    self.diagnostics
                        .events
                        .last_mut()
                        .unwrap()
                        .rendered
                        .extend_from_slice(b"Stack trace:\n#0 {main}\n");
                }
                self.exit_status = Some(255);
                self.replace_source(source);
                return Err(error);
            }
            Err(error) => Err(error),
        };
        self.replace_source(source);
        let result = match result {
            Ok(_) => {
                self.exit_status.get_or_insert(0);
                Ok(())
            }
            Err(error) => {
                // The active handler is cleared during invocation. A handler
                // explicitly installed by that invocation can handle its throw.
                self.dispatch_uncaught(error)
            }
        };
        if self.exception_handler.is_none() {
            self.exception_handler = self.exception_handlers.pop().flatten();
        }
        result
    }
}
