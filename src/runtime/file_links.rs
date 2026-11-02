// Altered Rust adaptation of PHP 8.5.0 ext/standard/{link,file}.c,
// main/fopen_wrappers.c and Zend/zend_virtual_cwd.c.
// Copyright (c) The PHP Group. See THIRD_PARTY_NOTICES.md.
use super::*;
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    Hard,
    Symbolic,
    Read,
    Info,
    Realpath,
}

impl Runtime {
    pub(super) fn file_link(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let creation = matches!(operation, Operation::Hard | Operation::Symbolic);
        let target = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        check_path(
            &target,
            name,
            1,
            if creation { "target" } else { "path" },
            line,
        )?;
        let link = if creation {
            let link = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
            check_path(&link, name, 2, "link", line)?;
            Some(link)
        } else {
            None
        };
        let cwd = self.working_directory();
        let path = cwd.join(crate::source::path_from_bytes(&target));
        let result = match operation {
            Operation::Realpath => {
                let expanded = if target.is_empty() {
                    checked_path(cwd.as_os_str().as_encoded_bytes(), &cwd)
                } else {
                    checked_path(&target, &cwd)
                };
                return Ok(expanded
                    .and_then(|path| resolve_path(&path, &mut 0, true))
                    .map_or(Value::Bool(false), path_value));
            }
            Operation::Read => {
                if target.is_empty() {
                    Err(io::Error::from_raw_os_error(2))
                } else {
                    fs::read_link(path).map(|path| {
                        let mut bytes = path.as_os_str().as_encoded_bytes().to_vec();
                        bytes.truncate(max_path() - 1);
                        Value::String(bytes.into())
                    })
                }
            }
            Operation::Info => {
                if target.is_empty() {
                    Err(io::Error::from_raw_os_error(2))
                } else {
                    link_info(&path)
                }
            }
            Operation::Hard | Operation::Symbolic => {
                let link = link.unwrap();
                let expanded = expand(&link, &cwd);
                let target_base = if matches!(operation, Operation::Symbolic) {
                    expanded.as_deref().and_then(Path::parent).unwrap_or(&cwd)
                } else {
                    &cwd
                };
                if expanded.is_none() || expand(&target, target_base).is_none() {
                    self.warn(format!(
                        "{name}(): No such file or directory on line {line}"
                    ))?;
                    return Ok(Value::Bool(false));
                }
                if matches!(operation, Operation::Symbolic) {
                    // Targets remain relative to the link's directory, and keep
                    // exactly the spelling supplied by the caller.
                    symbolic_link(&crate::source::path_from_bytes(&target), &expanded.unwrap())
                } else {
                    // Non-ZTS PHP validates expanded paths but invokes link()
                    // with the original names, including final symlinks.
                    hard_link(&path, &cwd.join(crate::source::path_from_bytes(&link)))
                }
                .map(|()| Value::Bool(true))
            }
        };
        match result {
            Ok(value) => Ok(value),
            Err(error) => {
                self.warn(format!(
                    "{name}(): {} on line {line}",
                    streams::io_message(&error)
                ))?;
                Ok(if matches!(operation, Operation::Info) {
                    Value::Int(-1)
                } else {
                    Value::Bool(false)
                })
            }
        }
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
        Err(Error::new(
            ErrorKind::ValueError,
            format!(
                "{name}(): Argument #{position} (${parameter}) must not contain any null bytes"
            ),
            line,
        ))
    } else {
        Ok(())
    }
}

fn link_info(path: &Path) -> io::Result<Value> {
    let metadata = fs::symlink_metadata(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(Value::Int(metadata.dev() as i64))
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

fn path_value(path: PathBuf) -> Value {
    Value::String(path.as_os_str().as_encoded_bytes().to_vec().into())
}

fn max_path() -> usize {
    if cfg!(target_vendor = "apple") {
        1024
    } else if cfg!(windows) {
        32768
    } else {
        4096
    }
}

fn expand(path: &[u8], base: &Path) -> Option<PathBuf> {
    if path.is_empty() {
        return None;
    }
    let path_buf = checked_path(path, base)?;
    let mut links = 0;
    let mut result = resolve_path(&path_buf, &mut links, false)?;
    if path.ends_with(b"/") {
        result.push("");
    }
    Some(result)
}

fn checked_path(path: &[u8], base: &Path) -> Option<PathBuf> {
    let raw = crate::source::path_from_bytes(path);
    if path.len() >= max_path() - 1
        || (!raw.is_absolute()
            && path.len() + base.as_os_str().as_encoded_bytes().len() + 1 >= max_path() - 1)
    {
        return None;
    }
    Some(base.join(raw))
}

fn resolve_path(path: &Path, links: &mut usize, strict: bool) -> Option<PathBuf> {
    let mut result = PathBuf::new();
    let components: Vec<_> = path.components().collect();
    for (index, component) in components.iter().enumerate() {
        match component {
            Component::CurDir => continue,
            Component::ParentDir => {
                result.pop();
            }
            _ => {
                result.push(component.as_os_str());
                let metadata = match fs::symlink_metadata(&result) {
                    Ok(metadata) => Some(metadata),
                    Err(_) if strict => return None,
                    Err(_) => None,
                };
                if metadata.is_some_and(|metadata| metadata.file_type().is_symlink()) {
                    *links += 1;
                    if *links > 32 {
                        return None;
                    }
                    let target = fs::read_link(&result).ok()?;
                    let target = result.parent()?.join(target);
                    if target.as_os_str().as_encoded_bytes().len() >= max_path() - 1 {
                        return None;
                    }
                    result = resolve_path(&target, links, strict)?;
                }
            }
        }
        if index + 1 < components.len()
            && fs::metadata(&result).is_ok_and(|metadata| !metadata.is_dir())
        {
            return None;
        }
    }
    let raw = path.as_os_str().as_encoded_bytes();
    if (raw.ends_with(b"/") || raw.ends_with(b"/."))
        && fs::metadata(&result).is_ok_and(|metadata| !metadata.is_dir())
    {
        return None;
    }
    Some(result)
}

fn hard_link(target: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::ffi::{CString, c_char, c_int};
        unsafe extern "C" {
            fn link(target: *const c_char, destination: *const c_char) -> c_int;
        }
        let target = CString::new(target.as_os_str().as_encoded_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        let destination = CString::new(destination.as_os_str().as_encoded_bytes())
            .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
        // PHP calls link(), whose final-symlink behavior differs from Rust's
        // linkat()-based implementation on some Unix platforms. Both C strings
        // remain valid for the duration of this call.
        if unsafe { link(target.as_ptr(), destination.as_ptr()) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
    #[cfg(not(unix))]
    {
        fs::hard_link(target, destination)
    }
}

fn symbolic_link(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        let resolved = link.parent().unwrap_or(Path::new(".")).join(target);
        if resolved.is_dir() {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
}
