// Altered Rust adaptation of PHP 8.5.0 ext/standard/file.c and
// main/streams/{streams,plain_wrapper}.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use std::{fs, io};

#[derive(Clone, Copy, PartialEq)]
enum Wrapper {
    File,
    Php,
    Glob,
    Data,
    Http,
    Ftp,
}

impl Runtime {
    pub(super) fn stream_is_local(
        &mut self,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        if let Value::Resource(resource) = value {
            if !matches!(*resource.kind.borrow(), crate::resource::Kind::Stream(_)) {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!("{name}(): supplied resource is not a valid stream resource"),
                    line,
                ));
            }
            // All currently constructible streams use local file, PHP or glob
            // wrappers. Network and user wrappers are not yet implemented.
            return Ok(Value::Bool(true));
        }
        // This accepts any string-convertible value, even under strict_types.
        // Wrapper lookup uses a C string, unlike filesystem path validation.
        let path = self.string_bytes(value, line)?;
        let path = path.split(|&byte| byte == 0).next().unwrap();
        let previous = self.native_warning_suppressed;
        let mut pending = None;
        let result = self.transfer_wrapper(path, name, &mut pending, line);
        self.native_warning_suppressed = previous;
        let wrapper = result?;
        if let Some(error) = pending {
            return Err(error);
        }
        Ok(Value::Bool(matches!(
            wrapper,
            Some(Wrapper::File | Wrapper::Php | Wrapper::Glob)
        )))
    }

    pub(super) fn file_operation(
        &mut self,
        rename: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let from = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        check_path(&from, name, 1, "from", line)?;
        let to = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
        check_path(&to, name, 2, "to", line)?;
        let context = args[2].as_ref();
        if let Some(value) = context
            && !matches!(value, Value::Null | Value::Resource(_))
        {
            return Err(builtins::type_error(
                value,
                name,
                3,
                "resource or null",
                line,
            ));
        }
        let previous = self.native_warning_suppressed;
        let mut pending = None;
        let result = (|| {
            let wrapper = self.transfer_wrapper(&from, name, &mut pending, line)?;
            if rename {
                let Some(wrapper) = wrapper else {
                    self.transfer_warning(
                        format!("{name}(): Unable to locate stream wrapper on line {line}"),
                        &mut pending,
                    )?;
                    return Ok(Value::Bool(false));
                };
                if wrapper != Wrapper::File {
                    let label = match wrapper {
                        Wrapper::Php => "PHP",
                        Wrapper::Glob => "glob",
                        Wrapper::Data => "RFC2397",
                        Wrapper::Http => "http",
                        Wrapper::Ftp => "ftp",
                        Wrapper::File => unreachable!(),
                    };
                    self.transfer_warning(
                        format!(
                            "{name}(): {label} wrapper does not support renaming on line {line}"
                        ),
                        &mut pending,
                    )?;
                    return Ok(Value::Bool(false));
                }
                if self.transfer_wrapper(&to, name, &mut pending, line)? != Some(wrapper) {
                    self.transfer_warning(
                        format!(
                            "{name}(): Cannot rename a file across wrapper types on line {line}"
                        ),
                        &mut pending,
                    )?;
                    return Ok(Value::Bool(false));
                }
            }
            if let Err(error) = self.stream_context(context, name, 3, line) {
                self.transfer_exception(error, &mut pending)?;
            }
            if rename {
                self.rename_paths(&from, &to, &mut pending, line)
            } else {
                self.copy_paths(&from, &to, name, &mut pending, false, line)
            }
        })();
        self.native_warning_suppressed = previous;
        if result
            .as_ref()
            .is_err_and(|error| error.kind == ErrorKind::UnwindExit)
        {
            return result;
        }
        pending.map_or(result, Err)
    }

    fn transfer_exception(
        &mut self,
        error: Error,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        if error.kind == ErrorKind::UnwindExit {
            return Err(error);
        }
        self.set_error(error, pending);
        self.native_warning_suppressed = true;
        Ok(())
    }

    fn transfer_warning(
        &mut self,
        message: String,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        if let Err(error) = self.warn(message) {
            self.transfer_exception(error, pending)?;
        }
        Ok(())
    }

    fn transfer_wrapper(
        &mut self,
        filename: &[u8],
        name: &str,
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<Option<Wrapper>, Error> {
        let length = filename
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
            .count();
        if length <= 1
            || filename.get(length) != Some(&b':')
            || !(filename[length..].starts_with(b"://") || filename.starts_with(b"data:"))
        {
            return Ok(Some(Wrapper::File));
        }
        Ok(Some(
            match filename[..length].to_ascii_lowercase().as_slice() {
                b"file" => {
                    if plain_path(filename).is_none() {
                        return Ok(None);
                    }
                    Wrapper::File
                }
                b"php" => Wrapper::Php,
                b"glob" => Wrapper::Glob,
                b"data" => Wrapper::Data,
                b"http" => Wrapper::Http,
                b"ftp" => Wrapper::Ftp,
                _ => {
                    self.transfer_warning(format!("{name}(): Unable to find the wrapper \"{}\" - did you forget to enable it when you configured PHP? on line {line}", String::from_utf8_lossy(&filename[..length.min(31)])), pending)?;
                    Wrapper::File
                }
            },
        ))
    }

    fn transfer_stat(
        &mut self,
        path: &[u8],
        name: &str,
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<Option<fs::Metadata>, Error> {
        if self.transfer_wrapper(path, name, pending, line)? != Some(Wrapper::File) {
            return Ok(None);
        }
        if path.is_empty() {
            return Ok(None);
        }
        Ok(plain_path(path).and_then(|path| {
            fs::metadata(
                self.working_directory()
                    .join(crate::source::path_from_bytes(path)),
            )
            .ok()
        }))
    }

    pub(super) fn transfer_open(
        &mut self,
        path: &[u8],
        mode: &[u8],
        name: &str,
        pending: &mut Option<Error>,
        private_creation: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let wrapper = self.transfer_wrapper(path, name, pending, line)?;
        if wrapper == Some(Wrapper::Php)
            && ![
                b"php://memory".as_slice(),
                b"php://temp",
                b"php://stdin",
                b"php://stdout",
                b"php://stderr",
            ]
            .iter()
            .any(|name| path.eq_ignore_ascii_case(name))
        {
            self.transfer_warning(
                format!("{name}(): Invalid php:// URL specified on line {line}"),
                pending,
            )?;
            self.transfer_warning(
                format!(
                    "{name}({}): Failed to open stream: operation failed on line {line}",
                    String::from_utf8_lossy(path)
                ),
                pending,
            )?;
            return Ok(Value::Bool(false));
        }
        if matches!(
            wrapper,
            Some(Wrapper::Glob | Wrapper::Data | Wrapper::Http | Wrapper::Ftp)
        ) {
            let reason = if wrapper == Some(Wrapper::Glob) {
                "wrapper does not support stream open"
            } else {
                "operation failed"
            };
            self.transfer_warning(
                format!(
                    "{name}({}): Failed to open stream: {reason} on line {line}",
                    String::from_utf8_lossy(path)
                ),
                pending,
            )?;
            return Ok(Value::Bool(false));
        }
        self.open_stream_with_create_mode(
            name,
            path,
            mode,
            false,
            private_creation.then_some(0o600),
            line,
        )
    }

    fn copy_paths(
        &mut self,
        from: &[u8],
        to: &[u8],
        name: &str,
        pending: &mut Option<Error>,
        private_creation: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if let Some(source) = self.transfer_stat(from, name, pending, line)? {
            if source.is_dir() {
                self.transfer_warning(format!("{name}(): The first argument to copy() function cannot be a directory on line {line}"), pending)?;
                return Ok(Value::Bool(false));
            }
            if let Some(destination) = self.transfer_stat(to, name, pending, line)? {
                if destination.is_dir() {
                    self.transfer_warning(format!("{name}(): The second argument to copy() function cannot be a directory on line {line}"), pending)?;
                    return Ok(Value::Bool(false));
                }
                if same_file(&source, &destination).unwrap_or_else(|| {
                    let source = plain_path(from).and_then(|p| {
                        fs::canonicalize(
                            self.working_directory()
                                .join(crate::source::path_from_bytes(p)),
                        )
                        .ok()
                    });
                    let destination = plain_path(to).and_then(|p| {
                        fs::canonicalize(
                            self.working_directory()
                                .join(crate::source::path_from_bytes(p)),
                        )
                        .ok()
                    });
                    source.zip(destination).is_some_and(|(a, b)| a == b)
                }) {
                    return Ok(Value::Bool(false));
                }
            }
        }
        let source = self.transfer_open(from, b"rb", name, pending, false, line)?;
        let Value::Resource(source) = source else {
            return Ok(Value::Bool(false));
        };
        let destination = self.transfer_open(to, b"wb", name, pending, private_creation, line)?;
        if !matches!(destination, Value::Resource(_)) {
            return Ok(Value::Bool(false));
        }
        Ok(Value::Bool(matches!(
            self.copy_stream_input(&source, &destination, name, line)?,
            Value::Int(_)
        )))
    }

    fn rename_paths(
        &mut self,
        from: &[u8],
        to: &[u8],
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<Value, Error> {
        // The rename callback strips only file://, unlike the open-path resolver.
        let from = strip_file_scheme(from);
        let to = strip_file_scheme(to);
        let source = self
            .working_directory()
            .join(crate::source::path_from_bytes(from));
        let target = self
            .working_directory()
            .join(crate::source::path_from_bytes(to));
        let result = if from.is_empty() || to.is_empty() {
            Err(io::Error::from_raw_os_error(2))
        } else {
            fs::rename(&source, &target)
        };
        match result {
            Ok(()) => {
                self.file_stat_cache = [None, None];
                Ok(Value::Bool(true))
            }
            #[cfg(unix)]
            Err(error) if error.raw_os_error() == Some(18) => {
                self.rename_across_devices(from, to, pending, line)
            }
            Err(error) => {
                self.rename_warning(from, to, &error, pending, line)?;
                Ok(Value::Bool(false))
            }
        }
    }

    fn rename_warning(
        &mut self,
        from: &[u8],
        to: &[u8],
        error: &io::Error,
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<(), Error> {
        self.transfer_warning(
            format!(
                "rename({},{}): {} on line {line}",
                String::from_utf8_lossy(from),
                String::from_utf8_lossy(to),
                streams::io_message(error)
            ),
            pending,
        )
    }

    #[cfg(unix)]
    fn rename_across_devices(
        &mut self,
        from: &[u8],
        to: &[u8],
        pending: &mut Option<Error>,
        line: usize,
    ) -> Result<Value, Error> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        // Apply native umask(077) to destination creation without changing
        // the embedding process's global umask or creating a file prematurely.
        if !self
            .copy_paths(from, to, "rename", pending, true, line)?
            .is_truthy()
        {
            self.rename_warning(from, to, &io::Error::last_os_error(), pending, line)?;
            return Ok(Value::Bool(false));
        }
        let metadata = match fs::metadata(
            self.working_directory()
                .join(crate::source::path_from_bytes(from)),
        ) {
            Ok(metadata) => metadata,
            Err(error) => {
                self.rename_warning(from, to, &error, pending, line)?;
                return Ok(Value::Bool(false));
            }
        };
        if let Err(error) = std::os::unix::fs::chown(
            self.working_directory()
                .join(crate::source::path_from_bytes(to)),
            Some(metadata.uid()),
            Some(metadata.gid()),
        ) {
            self.rename_warning(from, to, &error, pending, line)?;
            if error.raw_os_error() != Some(1) {
                return Ok(Value::Bool(false));
            }
        }
        if let Err(error) = fs::set_permissions(
            self.working_directory()
                .join(crate::source::path_from_bytes(to)),
            fs::Permissions::from_mode(metadata.mode()),
        ) {
            self.rename_warning(from, to, &error, pending, line)?;
            if error.raw_os_error() != Some(1) {
                return Ok(Value::Bool(false));
            }
        }
        let _ = fs::remove_file(
            self.working_directory()
                .join(crate::source::path_from_bytes(from)),
        );
        Ok(Value::Bool(true))
    }
}

fn check_path(
    path: &[u8],
    name: &str,
    position: usize,
    parameter: &str,
    line: usize,
) -> Result<(), Error> {
    if path.contains(&0) {
        return Err(Error::new(
            ErrorKind::ValueError,
            format!(
                "{name}(): Argument #{position} (${parameter}) must not contain any null bytes"
            ),
            line,
        ));
    }
    Ok(())
}

fn strip_file_scheme(path: &[u8]) -> &[u8] {
    if path
        .get(..7)
        .is_some_and(|p| p.eq_ignore_ascii_case(b"file://"))
    {
        &path[7..]
    } else {
        path
    }
}

pub(super) fn plain_path(path: &[u8]) -> Option<&[u8]> {
    let bytes = strip_file_scheme(path);
    if bytes.len() == path.len() {
        return Some(path);
    }
    let bytes = if path
        .get(..17)
        .is_some_and(|p| p.eq_ignore_ascii_case(b"file://localhost/"))
    {
        &path[16..]
    } else {
        if !bytes.is_empty() && !bytes.starts_with(b"/") {
            return None;
        }
        bytes
    };
    if bytes.is_empty() {
        return Some(b"/");
    }
    let slashes = bytes.iter().take_while(|&&b| b == b'/').count();
    Some(&bytes[slashes.saturating_sub(1)..])
}

fn same_file(a: &fs::Metadata, b: &fs::Metadata) -> Option<bool> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        (a.ino() != 0 && b.ino() != 0).then(|| a.ino() == b.ino() && a.dev() == b.dev())
    }
    #[cfg(not(unix))]
    {
        let _ = (a, b);
        None
    }
}
