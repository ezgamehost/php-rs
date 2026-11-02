// Altered Rust adaptation of PHP 8.5.0 main/php_open_temporary_file.c,
// main/streams/plain_wrapper.c and ext/standard/file.c.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::builtins::Builtin;
use super::*;
use crate::resource::{Backend, Kind, Stream};
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Path, PathBuf},
};

impl Runtime {
    fn system_temporary_directory(&mut self) -> Vec<u8> {
        self.temporary_directory
            .get_or_insert_with(|| {
                if let Some(value) = std::env::var_os("TMPDIR").filter(|value| !value.is_empty()) {
                    let mut bytes = value.as_encoded_bytes().to_vec();
                    if bytes.ends_with(b"/") {
                        bytes.pop();
                    }
                    return bytes;
                }
                if cfg!(target_os = "macos") {
                    b"/var/tmp/".to_vec()
                } else {
                    b"/tmp".to_vec()
                }
            })
            .clone()
    }

    pub(super) fn temporary_file_operation(
        &mut self,
        operation: Builtin,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(operation, Builtin::TemporaryDirectory) {
            return Ok(Value::String(self.system_temporary_directory().into()));
        }
        let named = matches!(operation, Builtin::TemporaryName);
        let (directory, prefix) = if named {
            let directory = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
            if directory.contains(&0) {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "tempnam(): Argument #1 ($directory) must not contain any null bytes",
                    line,
                ));
            }
            let prefix = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
            if prefix.contains(&0) {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "tempnam(): Argument #2 ($prefix) must not contain any null bytes",
                    line,
                ));
            }
            let mut prefix = path_parts::basename(&prefix).to_vec();
            prefix.truncate(63);
            (directory, prefix)
        } else {
            (Vec::new(), b"php".to_vec())
        };
        let cwd = self.working_directory();
        let mut opened = if directory.is_empty() {
            None
        } else {
            Some(create_temporary(
                &cwd.join(crate::source::path_from_bytes(&directory)),
                &prefix,
            ))
        };
        if opened.as_ref().is_some_and(|result| result.is_err()) {
            self.warn(format!(
                "Notice: {name}(): file created in the system's temporary directory on line {line}"
            ))?;
            opened = None;
        }
        let opened = opened.unwrap_or_else(|| {
            let directory = self.system_temporary_directory();
            if directory.is_empty() {
                return Err(io::Error::from(io::ErrorKind::NotFound));
            }
            create_temporary(
                &cwd.join(crate::source::path_from_bytes(&directory)),
                &prefix,
            )
        });
        let Ok((file, path)) = opened else {
            return Ok(Value::Bool(false));
        };
        let bytes = path.as_os_str().as_encoded_bytes().to_vec();
        if named {
            drop(file);
            return Ok(Value::String(bytes.into()));
        }
        Ok(Value::Resource(self.resources.allocate(Kind::Stream(
            Stream {
                backend: Backend::File(file),
                writable: true,
                eof: false,
                read_buffer: Default::default(),
                position: 0,
                mode: b"r+b".to_vec(),
                uri: Some(bytes),
                context: None,
                remove_on_close: Some(path),
            },
        ))))
    }
}

fn create_temporary(directory: &Path, prefix: &[u8]) -> io::Result<(File, PathBuf)> {
    let directory = fs::canonicalize(directory)?;
    let mut entropy = [0; 14];
    for _ in 0..128 {
        random::fill(&mut entropy).map_err(io::Error::other)?;
        let mut random = u64::from_ne_bytes(entropy[..8].try_into().unwrap());
        let mut name = prefix.to_vec();
        // PHP adds thirteen lower-case base32 characters before mkstemp's six.
        for _ in 0..13 {
            name.push(b"0123456789abcdefghijklmnopqrstuv"[(random % 32) as usize]);
            random /= 32;
        }
        for byte in &entropy[8..] {
            name.push(
                b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
                    [usize::from(*byte) % 62],
            );
        }
        let path = directory.join(crate::source::path_from_bytes(&name));
        if path.as_os_str().as_encoded_bytes().len() >= crate::native_directory::RECORD_SIZE - 1 {
            return Err(io::Error::from(io::ErrorKind::InvalidInput));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&path) {
            Ok(file) => return Ok((file, path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::from(io::ErrorKind::AlreadyExists))
}
