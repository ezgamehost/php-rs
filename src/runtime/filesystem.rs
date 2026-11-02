// Altered Rust adaptation of PHP 8.5.0 ext/standard/{dir,file,filestat}.c
// and main/streams/plain_wrapper.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use std::{fs, io, path::Path};

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    Mkdir,
    Rmdir,
    Touch,
    Chmod,
    Chdir,
    Getcwd,
    Stat(&'static [u8]),
}

impl Runtime {
    pub(super) fn filesystem_operation(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(operation, Operation::Getcwd) {
            return Ok(fs::canonicalize(self.working_directory())
                .map_or(Value::Bool(false), |path| {
                    Value::String(path.as_os_str().as_encoded_bytes().to_vec().into())
                }));
        }
        let filename = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        if let Operation::Stat(query) = operation {
            return self.filesystem_stat(&filename, query, name, line);
        }
        if filename.contains(&0) {
            let parameter = if matches!(operation, Operation::Touch | Operation::Chmod) {
                "filename"
            } else {
                "directory"
            };
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 (${parameter}) must not contain any null bytes"),
                line,
            ));
        }
        let file_url = filename
            .get(..7)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case(b"file://"));
        let bytes = if file_url && !matches!(operation, Operation::Chdir) {
            &filename[7..]
        } else {
            &filename
        };
        let path = self
            .working_directory()
            .join(crate::source::path_from_bytes(bytes));
        match operation {
            Operation::Chmod => {
                let mode = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                let mut pending = None;
                let scheme_len = filename
                    .iter()
                    .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
                    .count();
                let protocol = scheme_len > 1
                    && filename.get(scheme_len) == Some(&b':')
                    && (filename[scheme_len..].starts_with(b"://")
                        || filename.starts_with(b"data:"));
                if protocol {
                    let scheme = &filename[..scheme_len];
                    let scheme_lower = scheme.to_ascii_lowercase();
                    let unsupported = if scheme_lower == b"file" {
                        !bytes.is_empty()
                            && !bytes.starts_with(b"/")
                            && !filename.get(..17).is_some_and(|prefix| {
                                prefix.eq_ignore_ascii_case(b"file://localhost/")
                            })
                    } else if matches!(
                        scheme_lower.as_slice(),
                        b"php" | b"glob" | b"data" | b"http" | b"ftp"
                    ) {
                        true
                    } else {
                        if let Err(error) = self.warn(format!("{name}(): Unable to find the wrapper \"{}\" - did you forget to enable it when you configured PHP? on line {line}", String::from_utf8_lossy(&scheme[..scheme.len().min(31)]))) {
                            if error.kind == ErrorKind::UnwindExit { return Err(error); }
                            // Native wrapper lookup leaves the exception pending
                            // while chmod still attempts ordinary file access.
                            pending = Some(self.materialize_error(error));
                        }
                        false
                    };
                    if unsupported {
                        self.warn(format!(
                            "{name}(): Cannot call chmod() for a non-standard stream on line {line}"
                        ))?;
                        return Ok(Value::Bool(false));
                    }
                }
                let path = self
                    .working_directory()
                    .join(crate::source::path_from_bytes(bytes));
                let result = if bytes.is_empty() {
                    Err(io::Error::from_raw_os_error(2))
                } else {
                    chmod(&path, mode)
                };
                if let Err(error) = result {
                    if let Some(pending) = pending {
                        return Err(pending);
                    }
                    let prefix = if file_url && !bytes.is_empty() {
                        format!("{name}({})", String::from_utf8_lossy(bytes))
                    } else {
                        format!("{name}()")
                    };
                    let operation = if file_url { "Operation failed: " } else { "" };
                    self.warn(format!(
                        "{prefix}: {operation}{} on line {line}",
                        streams::io_message(&error)
                    ))?;
                    return Ok(Value::Bool(false));
                }
                self.file_stat_cache = [None, None];
                if let Some(pending) = pending {
                    return Err(pending);
                }
            }
            Operation::Mkdir => {
                let mode = args[1]
                    .as_ref()
                    .map(|v| self.native_int(v, name, 2, line))
                    .transpose()?
                    .unwrap_or(0o777);
                let recursive = args[2]
                    .as_ref()
                    .map(|v| self.native_bool(v, name, 3, line))
                    .transpose()?
                    .unwrap_or(false);
                self.stream_context(args[3].as_ref(), name, 4, line)?;
                let result = if bytes.is_empty() {
                    Err(io::Error::from_raw_os_error(2))
                } else {
                    make_directory(&path, mode, recursive)
                };
                if let Err(error) = result {
                    let message = if recursive && bytes.is_empty() {
                        "Invalid path".into()
                    } else {
                        streams::io_message(&error)
                    };
                    self.warn(format!("{name}(): {message} on line {line}"))?;
                    return Ok(Value::Bool(false));
                }
            }
            Operation::Rmdir => {
                self.stream_context(args[1].as_ref(), name, 2, line)?;
                let result = if bytes.is_empty() {
                    Err(io::Error::from_raw_os_error(2))
                } else {
                    fs::remove_dir(&path)
                };
                if let Err(error) = result {
                    self.warn(format!(
                        "{name}({}): {} on line {line}",
                        String::from_utf8_lossy(bytes),
                        streams::io_message(&error)
                    ))?;
                    return Ok(Value::Bool(false));
                }
                self.file_stat_cache = [None, None];
            }
            Operation::Chdir => {
                let result = if bytes.is_empty() {
                    Err(io::Error::from_raw_os_error(2))
                } else {
                    let resolved = fs::canonicalize(&path);
                    resolved.and_then(|resolved| {
                        if !resolved.is_dir() {
                            return Err(io::Error::from_raw_os_error(20));
                        }
                        if !file_info::native_access(&resolved, 1) {
                            return Err(io::Error::from_raw_os_error(13));
                        }
                        self.working_directory = Some(resolved);
                        Ok(())
                    })
                };
                if let Err(error) = result {
                    self.warn(format!(
                        "{name}(): {} (errno {}) on line {line}",
                        streams::io_message(&error),
                        error.raw_os_error().unwrap_or(0)
                    ))?;
                    return Ok(Value::Bool(false));
                }
                for cache in &mut self.file_stat_cache {
                    if cache
                        .as_ref()
                        .is_some_and(|(path, _)| !path.starts_with(b"/"))
                    {
                        *cache = None;
                    }
                }
            }
            Operation::Touch => {
                let mut times = [None, None];
                for (index, time) in times.iter_mut().enumerate() {
                    *time = args[index + 1]
                        .as_ref()
                        .filter(|v| !matches!(v, Value::Null))
                        .map(|v| self.native_int_type(v, name, index + 2, line, "?int"))
                        .transpose()?;
                }
                if filename.is_empty() {
                    return Ok(Value::Bool(false));
                }
                if times[0].is_none() && times[1].is_some() {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "touch(): Argument #2 ($mtime) cannot be null when argument #3 ($atime) is an integer",
                        line,
                    ));
                }
                let prefix = if file_url {
                    format!("touch({})", String::from_utf8_lossy(bytes))
                } else {
                    "touch()".into()
                };
                if !file_info::native_access(&path, 0)
                    && let Err(error) = fs::File::create(&path)
                {
                    self.warn(format!(
                        "{prefix}: Unable to create file {} because {} on line {line}",
                        String::from_utf8_lossy(bytes),
                        streams::io_message(&error)
                    ))?;
                    return Ok(Value::Bool(false));
                }
                if let Err(error) = touch(
                    &path,
                    times[0].map(|mtime| (times[1].unwrap_or(mtime), mtime)),
                ) {
                    let operation = if file_url { "Operation" } else { "Utime" };
                    self.warn(format!(
                        "{prefix}: {operation} failed: {} on line {line}",
                        streams::io_message(&error)
                    ))?;
                    return Ok(Value::Bool(false));
                }
                self.file_stat_cache = [None, None];
            }
            Operation::Getcwd | Operation::Stat(_) => unreachable!(),
        }
        Ok(Value::Bool(true))
    }

    fn stat_path_bytes<'a>(
        &mut self,
        filename: &'a [u8],
        name: &str,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Result<Option<&'a [u8]>, Error> {
        let length = filename
            .iter()
            .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
            .count();
        let protocol = length > 1
            && filename.get(length) == Some(&b':')
            && (filename[length..].starts_with(b"://") || filename.starts_with(b"data:"));
        if !protocol {
            return Ok(Some(filename));
        }
        let scheme = &filename[..length];
        if scheme.eq_ignore_ascii_case(b"file") {
            let bytes = if filename
                .get(..17)
                .is_some_and(|p| p.eq_ignore_ascii_case(b"file://localhost/"))
            {
                &filename[16..]
            } else {
                let bytes = &filename[7..];
                if !bytes.is_empty() && !bytes.starts_with(b"/") {
                    return Ok(None);
                }
                bytes
            };
            if bytes.is_empty() {
                return Ok(Some(b"/"));
            }
            let slashes = bytes.iter().take_while(|&&b| b == b'/').count();
            return Ok(Some(&bytes[slashes.saturating_sub(1)..]));
        }
        if matches!(
            scheme.to_ascii_lowercase().as_slice(),
            b"php" | b"glob" | b"data" | b"http" | b"ftp"
        ) {
            // These built-in wrappers do not provide local file stat callbacks.
            return Ok(None);
        }
        if let Err(error) = self.warn(format!(
            "{name}(): Unable to find the wrapper \"{}\" - did you forget to enable it when you configured PHP? on line {line}",
            String::from_utf8_lossy(&scheme[..scheme.len().min(31)]),
        )) {
            if error.kind == ErrorKind::UnwindExit { return Err(error); }
            self.set_error(error, pending);
        }
        // Unknown protocols fall back to a plain filename, even after an exception.
        Ok(Some(filename))
    }

    fn filesystem_stat(
        &mut self,
        filename: &[u8],
        query: &[u8],
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        let quiet = query.starts_with(b"is");
        if filename.is_empty() || filename.contains(&0) {
            if !quiet && !filename.is_empty() {
                self.warn(format!(
                    "{name}(): Filename contains null byte on line {line}"
                ))?;
            }
            return Ok(Value::Bool(false));
        }
        let access = matches!(query, b"isreadable" | b"iswritable" | b"isexecutable");
        let link = matches!(query, b"gettype" | b"islink" | b"lstat");
        let slot = usize::from(link);
        let cached = self.file_stat_cache[slot]
            .as_ref()
            .filter(|(key, _)| !access && key == filename)
            .map(|(_, m)| m.clone());
        let mut pending = None;
        let metadata = if let Some(metadata) = cached {
            Ok(metadata)
        } else {
            let bytes = self.stat_path_bytes(filename, name, line, &mut pending)?;
            // A wrapper warning can run user code that changes the working directory.
            let path = bytes.map(|bytes| {
                self.working_directory()
                    .join(crate::source::path_from_bytes(bytes))
            });
            if access {
                let value = Value::Bool(path.is_some_and(|path| {
                    file_info::native_access(
                        &path,
                        match query {
                            b"isreadable" => 4,
                            b"iswritable" => 2,
                            _ => 1,
                        },
                    )
                }));
                return pending.map_or(Ok(value), Err);
            }
            match path {
                Some(path) if link => fs::symlink_metadata(path),
                Some(path) => fs::metadata(path),
                None => Err(io::Error::from(io::ErrorKind::Unsupported)),
            }
        };
        match metadata {
            Ok(metadata) => {
                self.file_stat_cache[slot] = Some((filename.to_vec(), metadata.clone()));
                if link && !metadata.file_type().is_symlink() {
                    self.file_stat_cache[0] = Some((filename.to_vec(), metadata.clone()));
                }
                if let Some(error) = pending {
                    return Err(error);
                }
                if matches!(query, b"stat" | b"lstat") {
                    file_stat::from_metadata(&metadata, line)
                } else {
                    file_info::metadata_value(&metadata, query, line)
                }
            }
            Err(_) => {
                if let Some(error) = pending {
                    return Err(error);
                }
                if !quiet {
                    self.warn(format!(
                        "{name}(): {}stat failed for {} on line {line}",
                        if link { "L" } else { "" },
                        String::from_utf8_lossy(filename)
                    ))?;
                }
                Ok(Value::Bool(false))
            }
        }
    }
}

fn make_directory(path: &Path, mode: i64, recursive: bool) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(mode as u32);
    }
    #[cfg(not(unix))]
    let _ = mode;
    if !recursive {
        return builder.create(path);
    }
    // CWD_EXPAND collapses dot components before recursive directory creation.
    // Resolve existing prefixes so a parent component follows symlink targets.
    let mut expanded = std::path::PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                expanded.pop();
            }
            _ => {
                expanded.push(component.as_os_str());
                if let Ok(resolved) = fs::canonicalize(&expanded) {
                    expanded = resolved;
                }
            }
        }
    }
    let path = expanded.as_path();
    // PHP reports EEXIST for the final component, even for recursive mkdir.
    // Create only missing ancestors, then perform the final mkdir separately.
    let mut pending = Vec::new();
    let mut parent = path.parent();
    while let Some(path) = parent {
        if path.exists() {
            break;
        }
        pending.push(path);
        parent = path.parent();
    }
    for path in pending.into_iter().rev() {
        if let Err(error) = builder.create(path)
            && error.kind() != io::ErrorKind::AlreadyExists
        {
            return Err(error);
        }
    }
    builder.create(path)
}

#[cfg(all(unix, target_pointer_width = "64"))]
fn touch(path: &Path, times: Option<(i64, i64)>) -> io::Result<()> {
    use std::{
        ffi::{CString, c_char, c_int},
        os::unix::ffi::OsStrExt,
    };
    #[repr(C)]
    struct Times {
        access: i64,
        modification: i64,
    }
    unsafe extern "C" {
        fn utime(path: *const c_char, times: *const Times) -> c_int;
    }
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
    let times = times.map(|(access, modification)| Times {
        access,
        modification,
    });
    let pointer = times
        .as_ref()
        .map_or(std::ptr::null(), |t| t as *const Times);
    // SAFETY: 64-bit Unix time_t fields match the native utimbuf. Both inputs
    // remain live for the call; a null times pointer requests the current time.
    if unsafe { utime(path.as_ptr(), pointer) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(all(unix, target_pointer_width = "64")))]
fn touch(_: &Path, _: Option<(i64, i64)>) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}

#[cfg(unix)]
fn chmod(path: &Path, mode: i64) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(mode as u32))
}

#[cfg(not(unix))]
fn chmod(_: &Path, _: i64) -> io::Result<()> {
    Err(io::Error::from(io::ErrorKind::Unsupported))
}
