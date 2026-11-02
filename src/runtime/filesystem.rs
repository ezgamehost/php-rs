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
            let parameter = if matches!(operation, Operation::Touch) {
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
        let bytes = if filename
            .get(..7)
            .is_some_and(|p| p.eq_ignore_ascii_case(b"file://"))
        {
            &filename[7..]
        } else {
            filename
        };
        let path = self
            .working_directory()
            .join(crate::source::path_from_bytes(bytes));
        if matches!(query, b"isreadable" | b"iswritable" | b"isexecutable") {
            return Ok(Value::Bool(file_info::native_access(
                &path,
                match query {
                    b"isreadable" => 4,
                    b"iswritable" => 2,
                    _ => 1,
                },
            )));
        }
        let link = matches!(query, b"gettype" | b"islink");
        let slot = usize::from(link);
        let cached = self.file_stat_cache[slot]
            .as_ref()
            .filter(|(key, _)| key == filename)
            .map(|(_, m)| m.clone());
        let metadata = cached.map(Ok).unwrap_or_else(|| {
            if link {
                fs::symlink_metadata(&path)
            } else {
                fs::metadata(&path)
            }
        });
        match metadata {
            Ok(metadata) => {
                self.file_stat_cache[slot] = Some((filename.to_vec(), metadata.clone()));
                if link && !metadata.file_type().is_symlink() {
                    self.file_stat_cache[0] = Some((filename.to_vec(), metadata.clone()));
                }
                file_info::metadata_value(&metadata, query, line)
            }
            Err(_) => {
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
