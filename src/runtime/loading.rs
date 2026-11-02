use super::*;
use crate::{
    ast::IncludeKind,
    source::{Source, path_from_bytes},
};
use std::{
    fs,
    path::{Path, PathBuf},
};

impl Runtime {
    /// Set this request's working directory without changing the host process.
    pub fn set_working_directory(&mut self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let path = fs::canonicalize(self.working_directory().join(path))?;
        if !path.is_dir() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotADirectory,
                "Working directory is not a directory",
            ));
        }
        self.working_directory = Some(path);
        Ok(())
    }

    pub(super) fn working_directory(&self) -> PathBuf {
        self.working_directory
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    }

    fn memory_source(&self, name: &[u8]) -> Rc<Source> {
        Rc::new(Source {
            name: name.to_vec(),
            directory: self.working_directory(),
            path: None,
        })
    }

    /// Execute a PHP document as standard input, preserving request state.
    pub fn execute(&mut self, source: &[u8]) -> Result<Value, Error> {
        self.execute_document(source, self.memory_source(b"Standard input code"), true)
    }

    /// Execute PHP code without an opening tag, as with the CLI's -r option.
    pub fn execute_code(&mut self, code: &[u8]) -> Result<Value, Error> {
        self.execute_document(
            &code_document(code),
            self.memory_source(b"Command line code"),
            false,
        )
    }

    /// Execute a file with its canonical path as the source identity.
    pub fn execute_file(&mut self, path: impl AsRef<Path>) -> Result<Value, Error> {
        if self.exit_status.is_some() {
            return Ok(Value::Null);
        }
        self.uncaught_handler_allowed = false;
        let requested = path.as_ref();
        let path = fs::canonicalize(self.working_directory().join(requested)).map_err(|error| {
            Error::new(
                ErrorKind::Error,
                format!("Cannot open {}: {error}", requested.display()),
                1,
            )
        })?;
        let source = fs::read(&path).map_err(|error| {
            Error::new(
                ErrorKind::Error,
                format!("Cannot read {}: {error}", path.display()),
                1,
            )
        })?;
        self.execute_document(&source, Rc::new(Source::file(&path)), true)
    }

    fn execute_document(
        &mut self,
        code: &[u8],
        source: Rc<Source>,
        handle_exceptions: bool,
    ) -> Result<Value, Error> {
        if self.exit_status.is_some() {
            return Ok(Value::Null);
        }
        self.scope.reconcile(std::mem::take(&mut self.variables));
        self.diagnostics.output_position = self.output.len();
        self.diagnostics.messages = std::mem::take(&mut self.warnings);
        self.static_variables.remove(&None);
        self.uncaught_handler_allowed = false;
        let result = self.execute_unit(code, source, Value::Null, None, handle_exceptions);
        self.warnings = std::mem::take(&mut self.diagnostics.messages);
        self.variables = self
            .scope
            .iter()
            .map(|(name, value)| (name.clone(), value.borrow().clone()))
            .collect();
        match result {
            // Ordinary exit signals are consumed here. A class-linking fatal
            // retains its diagnostic for embedders without becoming catchable
            // inside PHP or being reported a second time by the CLI.
            Err(error) if error.kind == ErrorKind::UnwindExit && error.message.is_empty() => {
                Ok(Value::Null)
            }
            Err(mut error) if error.kind == ErrorKind::UnwindExit && error.throwable.is_some() => {
                // The internal unwind has reached the embedding boundary.
                // Preserve the public class-linking error kind while retaining
                // the CompileError payload for native diagnostic rendering.
                self.uncaught_handler_allowed = false;
                error.kind = ErrorKind::Error;
                Err(error)
            }
            result => result,
        }
    }

    fn execute_unit(
        &mut self,
        code: &[u8],
        source: Rc<Source>,
        fallthrough: Value,
        frame: Option<traces::TraceFrame>,
        handle_exceptions: bool,
    ) -> Result<Value, Error> {
        let (mut statements, warnings, strict) = lexer::lex(code)
            .and_then(|tokens| parser::parse_source(tokens, source.clone()))
            .map_err(|error| self.materialize_error_from(error, &source.name))?;
        if let Some(path) = &source.path {
            self.record_include(path.clone());
        }
        let previous_compiling = std::mem::replace(&mut self.diagnostics.compiling, true);
        let diagnostic_source = std::mem::replace(&mut self.diagnostics.source, source.clone());
        let prepared = (|| {
            for warning in warnings {
                self.warn(warning)?;
            }
            self.scope.reserve_variables(&statements);
            self.prepare_functions(&mut statements)
        })()
        .map_err(|error| self.materialize_error_from(error, &source.name));
        self.diagnostics.compiling = previous_compiling;
        self.diagnostics.source = diagnostic_source;
        prepared?;
        // Only execution of a compiled entry script enables the uncaught
        // handler. PHP's CLI -r and failures compiling the entry script bypass
        // it; nested eval/include exceptions use the enclosing script boundary.
        if handle_exceptions {
            self.uncaught_handler_allowed = true;
        }
        let previous_source = self.replace_source(source);
        let previous_strict = std::mem::replace(&mut self.strict_types, strict);
        let previous_function = self.current_function.take();
        let previous_returns_reference =
            std::mem::replace(&mut self.current_returns_reference, false);
        let previous_closure = self.current_closure.take();
        let previous_arguments = self.current_arguments.take();
        let previous_statics = self.static_variables.remove(&None);
        let has_frame = frame.is_some();
        if let Some(frame) = frame {
            self.trace_frames.push(frame);
        }
        self.executing_units += 1;
        let result = self.block(&statements);
        self.executing_units -= 1;
        if has_frame {
            self.trace_frames.pop();
        }
        self.replace_source(previous_source);
        self.strict_types = previous_strict;
        self.current_function = previous_function;
        self.current_returns_reference = previous_returns_reference;
        self.current_closure = previous_closure;
        self.current_arguments = previous_arguments;
        self.static_variables.remove(&None);
        if let Some(statics) = previous_statics {
            self.static_variables.insert(None, statics);
        }
        match result? {
            Flow::Return(value, _) => Ok(value),
            _ => Ok(fallthrough),
        }
    }

    pub(super) fn eval_code(&mut self, code: &[u8], line: usize) -> Result<Value, Error> {
        if code.is_empty() {
            return Ok(Value::Bool(false));
        }
        let source = Rc::new(self.source.evaluated(line));
        self.execute_unit(
            &code_document(code),
            source,
            Value::Null,
            Some(traces::TraceFrame::unit(self, b"eval", Some(line), None)),
            false,
        )
    }

    pub(super) fn include_file(
        &mut self,
        kind: IncludeKind,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let required = matches!(kind, IncludeKind::Require | IncludeKind::RequireOnce);
        let once = matches!(kind, IncludeKind::IncludeOnce | IncludeKind::RequireOnce);
        let label = match kind {
            IncludeKind::Include => "include",
            IncludeKind::IncludeOnce => "include_once",
            IncludeKind::Require => "require",
            IncludeKind::RequireOnce => "require_once",
        };
        if name.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "Path must not be empty",
                line,
            ));
        }
        if let Some(end) = name.iter().position(|b| *b == 0) {
            return self.include_failure(label, &name[..end], required, line);
        }
        let resolved = self
            .resolve_include(name, label, line)?
            .map(Ok)
            .unwrap_or_else(|| {
                fs::canonicalize(self.working_directory().join(path_from_bytes(name)))
            });
        let loaded = match resolved {
            Ok(path) => {
                if once && self.included_set.contains(&path) {
                    return Ok(Value::Bool(true));
                }
                fs::read(&path).map(|source| (path, source))
            }
            Err(error) => Err(error),
        };
        let (path, code) = match loaded {
            Ok(loaded) => loaded,
            Err(error) => {
                let frame = traces::TraceFrame::unit(
                    self,
                    label.as_bytes(),
                    Some(line),
                    Some(PhpArray::default()),
                );
                self.trace_frames.push(frame);
                let result = (|| {
                    self.warn(format!(
                        "{label}({}): Failed to open stream: {} on line {line}",
                        String::from_utf8_lossy(name),
                        streams::io_message(&error)
                    ))?;
                    self.include_failure(label, name, required, line)
                })()
                .map_err(|error| self.materialize_error(error));
                self.trace_frames.pop();
                return result;
            }
        };
        if once {
            self.record_include(path.clone());
        }
        let source = Rc::new(Source::file(&path));
        let mut arguments = PhpArray::default();
        arguments.append(Value::String(source.name.clone().into()), line)?;
        let frame = traces::TraceFrame::unit(self, label.as_bytes(), Some(line), Some(arguments));
        self.execute_unit(&code, source, Value::Int(1), Some(frame), false)
    }

    fn include_failure(
        &mut self,
        label: &str,
        name: &[u8],
        required: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let name = String::from_utf8_lossy(name);
        if required {
            Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Failed opening required '{name}' (include_path='{}')",
                    String::from_utf8_lossy(self.include_path_bytes())
                ),
                line,
            ))
        } else {
            self.warn(format!(
                "{label}(): Failed opening '{name}' for inclusion (include_path='{}') on line {line}", String::from_utf8_lossy(self.include_path_bytes())
            ))?;
            Ok(Value::Bool(false))
        }
    }

    fn record_include(&mut self, path: PathBuf) {
        if self.included_set.insert(path.clone()) {
            self.included_files.push(path);
        }
    }

    pub(super) fn default_autoload(
        &mut self,
        name: &[u8],
        extensions: &[u8],
        line: usize,
    ) -> Result<(), Error> {
        let mut base = name.to_ascii_lowercase();
        for byte in &mut base {
            if *byte == b'\\' {
                *byte = std::path::MAIN_SEPARATOR as u8;
            }
        }
        let mut extensions = extensions.split(|b| *b == b',').peekable();
        while let Some(extension) = extensions.next() {
            if extension.is_empty() && extensions.peek().is_none() {
                break;
            }
            let mut filename = base.clone();
            filename.extend(extension);
            let path = self
                .resolve_include(&filename, "spl_autoload", line)?
                .or_else(|| {
                    fs::canonicalize(self.working_directory().join(path_from_bytes(&filename))).ok()
                });
            let Some(path) = path else {
                continue;
            };
            if self.included_set.contains(&path) {
                continue;
            }
            let Ok(code) = fs::read(&path) else {
                continue;
            };
            self.record_include(path.clone());
            // zend_execute uses the nearest user frame's symbol table, but
            // the loaded top-level code has no lexical class scope.
            let context = self.class_context.take();
            let result = self.execute_unit(
                &code,
                Rc::new(Source::file(&path)),
                Value::Int(1),
                None,
                false,
            );
            self.class_context = context;
            result?;
            if self.find_class(name).is_some() {
                break;
            }
        }
        Ok(())
    }
}

fn code_document(code: &[u8]) -> Vec<u8> {
    let mut document = Vec::with_capacity(code.len() + 6);
    document.extend_from_slice(b"<?php ");
    document.extend_from_slice(code);
    document
}
