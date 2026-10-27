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

    fn working_directory(&self) -> PathBuf {
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
        self.execute_document(source, self.memory_source(b"Standard input code"))
    }

    /// Execute PHP code without an opening tag, as with the CLI's -r option.
    pub fn execute_code(&mut self, code: &[u8]) -> Result<Value, Error> {
        self.execute_document(
            &code_document(code),
            self.memory_source(b"Command line code"),
        )
    }

    /// Execute a file with its canonical path as the source identity.
    pub fn execute_file(&mut self, path: impl AsRef<Path>) -> Result<Value, Error> {
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
        self.execute_document(&source, Rc::new(Source::file(&path)))
    }

    fn execute_document(&mut self, code: &[u8], source: Rc<Source>) -> Result<Value, Error> {
        self.scope.reconcile(std::mem::take(&mut self.variables));
        self.static_variables.remove(&None);
        let result = self.execute_unit(code, source, Value::Null);
        self.variables = self
            .scope
            .iter()
            .map(|(name, value)| (name.clone(), value.borrow().clone()))
            .collect();
        result
    }

    fn execute_unit(
        &mut self,
        code: &[u8],
        source: Rc<Source>,
        fallthrough: Value,
    ) -> Result<Value, Error> {
        let (mut statements, warnings, strict) =
            parser::parse_source(lexer::lex(code)?, source.clone())?;
        if let Some(path) = &source.path {
            self.record_include(path.clone());
        }
        self.warnings.extend(warnings);
        self.scope.reserve_variables(&statements);
        self.prepare_functions(&mut statements)?;
        let previous_source = std::mem::replace(&mut self.source, source);
        let previous_strict = std::mem::replace(&mut self.strict_types, strict);
        let previous_function = self.current_function.take();
        let previous_arguments = self.current_arguments.take();
        let previous_statics = self.static_variables.remove(&None);
        let result = self.block(&statements);
        self.source = previous_source;
        self.strict_types = previous_strict;
        self.current_function = previous_function;
        self.current_arguments = previous_arguments;
        self.static_variables.remove(&None);
        if let Some(statics) = previous_statics {
            self.static_variables.insert(None, statics);
        }
        match result? {
            Flow::Return(value) => Ok(value),
            _ => Ok(fallthrough),
        }
    }

    pub(super) fn eval_code(&mut self, code: &[u8], line: usize) -> Result<Value, Error> {
        if code.is_empty() {
            return Ok(Value::Bool(false));
        }
        let source = Rc::new(self.source.evaluated(line));
        self.execute_unit(&code_document(code), source, Value::Null)
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
        let resolved = self.resolve_include(name);
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
                self.warnings.push(format!(
                    "{label}({}): Failed to open stream: {error} on line {line}",
                    String::from_utf8_lossy(name)
                ));
                return self.include_failure(label, name, required, line);
            }
        };
        if once {
            self.record_include(path.clone());
        }
        self.execute_unit(&code, Rc::new(Source::file(&path)), Value::Int(1))
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
                format!("Failed opening required '{name}'"),
                line,
            ))
        } else {
            self.warnings.push(format!(
                "{label}(): Failed opening '{name}' for inclusion on line {line}"
            ));
            Ok(Value::Bool(false))
        }
    }

    fn record_include(&mut self, path: PathBuf) {
        if self.included_set.insert(path.clone()) {
            self.included_files.push(path);
        }
    }

    fn resolve_include(&self, name: &[u8]) -> std::io::Result<PathBuf> {
        let path = path_from_bytes(name);
        // With the default include path, search the current working directory
        // before the executing file's directory. Explicit ./ and ../ paths
        // are always relative to the working directory.
        if path.is_absolute() || name.starts_with(b"./") || name.starts_with(b"../") {
            return fs::canonicalize(self.working_directory().join(path));
        }
        fs::canonicalize(self.working_directory().join(&path))
            .or_else(|_| fs::canonicalize(self.source.directory.join(path)))
    }
}

fn code_document(code: &[u8]) -> Vec<u8> {
    let mut document = Vec::with_capacity(code.len() + 6);
    document.extend_from_slice(b"<?php ");
    document.extend_from_slice(code);
    document
}
