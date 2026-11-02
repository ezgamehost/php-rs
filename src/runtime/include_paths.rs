// Altered Rust adaptation of PHP 8.5.0 main/fopen_wrappers.c,
// main/streams/streams.c, and ext/standard/{basic_functions,streamsfuncs}.c.
// Copyright (c) The PHP Group. See THIRD_PARTY_NOTICES.md.
use super::*;
use std::{fs, path::PathBuf};

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    Get,
    Set,
    Resolve,
}

impl Runtime {
    pub(super) fn include_path_bytes(&self) -> &[u8] {
        self.include_path
            .as_deref()
            .unwrap_or(if cfg!(windows) { b".;" } else { b".:" })
    }

    pub(super) fn include_path_builtin(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(operation, Operation::Get) {
            return Ok(Value::String(self.include_path_bytes().to_vec().into()));
        }
        let value = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        if value.contains(&0) {
            let parameter = if matches!(operation, Operation::Set) {
                "include_path"
            } else {
                "filename"
            };
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 (${parameter}) must not contain any null bytes"),
                line,
            ));
        }
        match operation {
            Operation::Set => {
                if value.is_empty() {
                    return Ok(Value::Bool(false));
                }
                let old = self.include_path_bytes().to_vec();
                self.include_path = Some(value);
                Ok(Value::String(old.into()))
            }
            Operation::Resolve => Ok(self
                .resolve_include(&value, name, line)?
                .map_or(Value::Bool(false), |path| {
                    Value::String(path.as_os_str().as_encoded_bytes().to_vec().into())
                })),
            Operation::Get => unreachable!(),
        }
    }

    pub(super) fn resolve_include(
        &mut self,
        filename: &[u8],
        name: &str,
        line: usize,
    ) -> Result<Option<PathBuf>, Error> {
        if filename.contains(&0) {
            return Ok(None);
        }
        if protocol_end(filename).is_some() {
            return self.resolve_include_candidate(filename, name, line);
        }
        let path = crate::source::path_from_bytes(filename);
        if path.is_absolute()
            || filename.starts_with(b"./")
            || filename.starts_with(b"../")
            || self.include_path_bytes().is_empty()
        {
            return Ok(fs::canonicalize(self.working_directory().join(path)).ok());
        }
        let include_path = self.include_path_bytes().to_vec();
        let separator = if cfg!(windows) { b';' } else { b':' };
        let mut remaining = include_path.as_slice();
        while !remaining.is_empty() {
            // A wrapper's :// is part of the entry, not a path separator.
            // PHP treats the literal '..://' prefix as an ordinary entry.
            let search_start = protocol_end(remaining)
                .filter(|&n| &remaining[..n] != b"..")
                .map_or(0, |n| n + 3);
            let end = remaining[search_start..]
                .iter()
                .position(|&b| b == separator)
                .map(|n| n + search_start);
            let entry = &remaining[..end.unwrap_or(remaining.len())];
            let length = entry.len().saturating_add(filename.len()).saturating_add(2);
            if length < max_path_length() {
                let mut candidate = entry.to_vec();
                // Empty entries are '/filename' in PHP's resolver. A trailing
                // separator produces no entry; it is not an implicit cwd.
                candidate.push(b'/');
                candidate.extend_from_slice(filename);
                if let Some(path) = self.resolve_include_candidate(&candidate, name, line)? {
                    return Ok(Some(path));
                }
            }
            remaining = end.map_or(&[], |end| &remaining[end + 1..]);
        }
        // Use the executing source's directory, not the request's initial cwd.
        // Command-line code has no filename slash and therefore no fallback.
        let source = self.source.name.clone();
        if let Some(end) = source.iter().rposition(|&b| b == b'/').filter(|&n| n > 0)
            && end.saturating_add(filename.len()).saturating_add(2) < max_path_length()
        {
            let mut candidate = source[..=end].to_vec();
            candidate.extend_from_slice(filename);
            return self.resolve_include_candidate(&candidate, name, line);
        }
        Ok(None)
    }

    fn resolve_include_candidate(
        &mut self,
        candidate: &[u8],
        name: &str,
        line: usize,
    ) -> Result<Option<PathBuf>, Error> {
        let mut pending = None;
        let path = if protocol_end(candidate).is_some() {
            self.stat_path_bytes(candidate, name, line, &mut pending)?
        } else {
            Some(candidate)
        };
        let result = path.and_then(|path| {
            fs::canonicalize(
                self.working_directory()
                    .join(crate::source::path_from_bytes(path)),
            )
            .ok()
        });
        pending.map_or(Ok(result), Err)
    }
}

fn protocol_end(bytes: &[u8]) -> Option<usize> {
    let length = bytes
        .iter()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
        .count();
    (length > 1 && bytes[length..].starts_with(b"://")).then_some(length)
}

fn max_path_length() -> usize {
    if cfg!(target_vendor = "apple") {
        1024
    } else if cfg!(windows) {
        32768
    } else {
        4096
    }
}
