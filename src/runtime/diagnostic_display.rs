use super::*;

impl Runtime {
    /// Report an execution error as an uncaught PHP error. Call once at the
    /// request boundary, before reading/writing displayed output. This may run
    /// a user-defined exception __toString(), including its output or exit().
    /// Execution APIs themselves continue returning errors without reporting them.
    pub fn report_uncaught(&mut self, error: &Error) {
        self.diagnostics.messages = std::mem::take(&mut self.warnings);
        self.diagnostics.output_position = self.output.len();
        let source =
            self.replace_source(Rc::new(crate::source::Source::memory(b"[no active file]")));
        self.report_uncaught_inner(error.clone());
        self.replace_source(source);
        self.warnings = std::mem::take(&mut self.diagnostics.messages);
        self.variables = self
            .scope
            .iter()
            .map(|(name, value)| (name.clone(), value.borrow().clone()))
            .collect();
    }

    fn report_uncaught_inner(&mut self, mut error: Error) {
        loop {
            if error.kind == ErrorKind::UnwindExit {
                return;
            }
            let error_object = self.materialize_error(error);
            let object = error_object.throwable.unwrap();
            let data = object.throwable.borrow().as_ref().unwrap().clone();
            // PHP special-cases the exact internal classes, not their subclasses.
            let label = match object.class.name.as_slice() {
                b"ParseError" => Some((4, "Parse error")),
                b"CompileError" => Some((64, "Fatal error")),
                _ => None,
            };
            if let Some((level, label)) = label {
                self.diagnostics
                    .display(level, label, &data.message, &data.file, data.line);
                return;
            }
            let method = objects::method(&object.class, b"__toString").unwrap();
            let result = self
                .unchecked_method_target(method, object.class.clone(), Some(object), 0)
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
            let mut message = b"Uncaught ".to_vec();
            message.extend(text);
            message.extend_from_slice(b"\n  thrown");
            self.diagnostics
                .display(1, "Fatal error", &message, &data.file, data.line);
            return;
        }
    }
}
