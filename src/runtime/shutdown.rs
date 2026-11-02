// Altered Rust adaptation of PHP 8.5.0's shutdown callback registration and
// invocation rules in ext/standard/basic_functions.c. See THIRD_PARTY_NOTICES.md.
use super::*;

#[derive(Clone)]
pub(super) struct Callback {
    target: functions::CallTarget,
    arguments: Vec<Value>,
}

impl Runtime {
    pub(super) fn register_shutdown(
        &mut self,
        callback: &Value,
        arguments: &PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let target = self
            .native_callback(callback, "register_shutdown_function", 1, false, line)?
            .expect("non-null shutdown callback");
        self.shutdown_functions.push(Callback {
            target,
            arguments: arguments
                .entries()
                .iter()
                .map(|(_, value)| value.dereferenced())
                .collect(),
        });
        Ok(Value::Null)
    }

    /// Finish this request, invoking registered shutdown callbacks once.
    /// Call after reporting an uncaught execution error, if any, and before
    /// consuming output. Unhandled callback errors are reported and returned;
    /// do not report them again. An explicit exit stops the remaining callbacks.
    pub fn shutdown(&mut self) -> Result<(), Error> {
        if self.shutdown_started {
            return Ok(());
        }
        self.shutdown_started = true;
        self.diagnostics.messages = std::mem::take(&mut self.warnings);
        self.diagnostics.output_position = self.output.len();
        let source =
            self.replace_source(Rc::new(crate::source::Source::memory(b"[no active file]")));
        let mut result = Ok(());
        let mut index = 0;
        // Keep the live queue: registrations made by a running callback append
        // after those already registered. The saved target retains class scope
        // and object identity rather than resolving the callable again.
        while index < self.shutdown_functions.len() {
            let callback = self.shutdown_functions[index].clone();
            index += 1;
            self.diagnostics.source = Rc::new(crate::source::Source::memory(b"Unknown"));
            if let Err(error) = self.call_callback(&callback.target, &callback.arguments, 0) {
                if error.kind == ErrorKind::UnwindExit {
                    break;
                }
                if let Err(error) = self.dispatch_uncaught(error) {
                    if error.kind != ErrorKind::UnwindExit {
                        result = Err(error);
                    }
                    break;
                }
            }
        }
        self.shutdown_functions.clear();
        self.replace_source(source);
        self.exit_status.get_or_insert(0);
        self.warnings = std::mem::take(&mut self.diagnostics.messages);
        self.variables = self
            .scope
            .iter()
            .map(|(name, value)| (name.clone(), value.borrow().clone()))
            .collect();
        result
    }
}
