use super::*;
use crate::resource::{Backend, Kind, Stream};
use std::{
    fs::OpenOptions,
    io::{Cursor, Read, Seek, SeekFrom, Write},
};

#[derive(Debug, Clone, Copy)]
pub(super) enum Operation {
    Close,
    Read,
    Write,
    Eof,
    Tell,
    Seek,
    Rewind,
    Flush,
}

impl Runtime {
    pub(super) fn fopen(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        let filename = self.native_string(args[0].as_ref().unwrap(), "fopen", 1, line)?;
        if filename.contains(&0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "fopen(): Argument #1 ($filename) must not contain any null bytes",
                line,
            ));
        }
        let mode = self.native_string(args[1].as_ref().unwrap(), "fopen", 2, line)?;
        let include = args[2]
            .as_ref()
            .map(|v| self.native_bool(v, "fopen", 3, line))
            .transpose()?
            .unwrap_or(false);
        self.stream_context(args[3].as_ref(), "fopen", 4, line)?;
        if filename.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "Path must not be empty",
                line,
            ));
        }
        self.open_stream(&filename, &mode, include, line)
    }

    fn stream_context(
        &mut self,
        value: Option<&Value>,
        name: &str,
        position: usize,
        line: usize,
    ) -> Result<(), Error> {
        match value {
            None | Some(Value::Null) => {
                self.resources.default_context();
            }
            Some(Value::Resource(resource)) => {
                if !matches!(*resource.kind.borrow(), Kind::Context) {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "{name}(): supplied resource is not a valid Stream-Context resource"
                        ),
                        line,
                    ));
                }
            }
            Some(value) => {
                return Err(builtins::type_error(
                    value,
                    name,
                    position,
                    "resource or null",
                    line,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn file_path_operation(
        &mut self,
        unlink: bool,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = if unlink { "unlink" } else { "file_exists" };
        let filename = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        if filename.contains(&0) {
            return if unlink {
                Err(Error::new(
                    ErrorKind::ValueError,
                    "unlink(): Argument #1 ($filename) must not contain any null bytes",
                    line,
                ))
            } else {
                Ok(Value::Bool(false))
            };
        }
        if unlink {
            self.stream_context(args[1].as_ref(), name, 2, line)?;
        }
        let bytes = filename.strip_prefix(b"file://").unwrap_or(&filename);
        let path = self
            .working_directory()
            .join(crate::source::path_from_bytes(bytes));
        if !unlink {
            return Ok(Value::Bool(!filename.is_empty() && path.exists()));
        }
        let result = if filename.is_empty() {
            Err(std::io::Error::from_raw_os_error(2))
        } else {
            std::fs::remove_file(path)
        };
        if let Err(error) = result {
            self.diagnostics.push(format!(
                "unlink({}): {} on line {line}",
                String::from_utf8_lossy(&filename),
                io_message(&error)
            ));
            Ok(Value::Bool(false))
        } else {
            Ok(Value::Bool(true))
        }
    }

    fn open_stream(
        &mut self,
        filename: &[u8],
        mode: &[u8],
        include: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let mode = mode.split(|byte| *byte == 0).next().unwrap();
        let memory = filename == b"php://memory" || filename == b"php://temp";
        let first = mode.first().copied().unwrap_or(0);
        // These wrappers only distinguish read-only mode from writable mode;
        // ordinary files validate the leading mode character.
        if !memory && !matches!(first, b'r' | b'w' | b'a' | b'x' | b'c') {
            self.open_warning(
                filename,
                &format!(
                    "`{}' is not a valid mode for fopen",
                    String::from_utf8_lossy(mode)
                ),
                line,
            );
            return Ok(Value::Bool(false));
        }
        let plus = mode.contains(&b'+');
        let readable = memory || first == b'r' || plus;
        let writable = first != b'r' || plus;
        let backend = if memory {
            Backend::Memory(Cursor::new(Vec::new()))
        } else {
            let bytes = filename.strip_prefix(b"file://").unwrap_or(filename);
            let path = crate::source::path_from_bytes(bytes);
            let mut resolved = self.working_directory().join(&path);
            if include
                && !path.is_absolute()
                && !bytes.starts_with(b"./")
                && !bytes.starts_with(b"../")
                && !resolved.exists()
            {
                let source_path = self.source.directory.join(path);
                if source_path.exists() {
                    resolved = source_path;
                }
            }
            let mut options = OpenOptions::new();
            options
                .read(readable)
                .write(writable)
                .append(first == b'a')
                .create(matches!(first, b'w' | b'a' | b'c'))
                .create_new(first == b'x')
                .truncate(first == b'w');
            match options.open(resolved) {
                Ok(file) => Backend::File(file),
                Err(error) => {
                    self.open_warning(filename, &io_message(&error), line);
                    return Ok(Value::Bool(false));
                }
            }
        };
        let resource = self.resources.allocate(Kind::Stream(Stream {
            backend,
            writable,
            eof: false,
            position: 0,
        }));
        if filename == b"php://temp" {
            // PHP's temporary stream owns a separate registered backing stream.
            let mut kind = resource.kind.borrow_mut();
            let Kind::Stream(stream) = &mut *kind else {
                unreachable!()
            };
            let backend = std::mem::replace(
                &mut stream.backend,
                Backend::Memory(Cursor::new(Vec::new())),
            );
            let inner = self.resources.allocate(Kind::Stream(Stream {
                backend,
                writable,
                eof: false,
                position: 0,
            }));
            stream.backend = Backend::Temporary(inner);
        }
        Ok(Value::Resource(resource))
    }

    fn open_warning(&mut self, filename: &[u8], message: &str, line: usize) {
        self.diagnostics.push(format!(
            "fopen({}): Failed to open stream: {message} on line {line}",
            String::from_utf8_lossy(filename)
        ));
    }

    pub(super) fn stream_operation(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        let Value::Resource(resource) = first else {
            return Err(builtins::type_error(first, name, 1, "resource", line));
        };
        let invalid = || {
            Error::new(
                ErrorKind::TypeError,
                format!("{name}(): Argument #1 ($stream) must be an open stream resource"),
                line,
            )
        };
        if !matches!(*resource.kind.borrow(), Kind::Stream(_)) {
            return Err(invalid());
        }
        // Coercion may run user code, so do not hold a resource borrow across it.
        let mut data = Vec::new();
        let mut length = 0;
        let mut offset = 0;
        let mut whence = 0;
        match operation {
            Operation::Read => {
                length = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                if length <= 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        format!("{name}(): Argument #2 ($length) must be greater than 0"),
                        line,
                    ));
                }
            }
            Operation::Write => {
                data = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
                if let Some(value) = args[2].as_ref().filter(|v| !matches!(v, Value::Null)) {
                    let limit = self.native_int_type(value, name, 3, line, "?int")?.max(0) as usize;
                    data.truncate(limit);
                }
            }
            Operation::Seek => {
                offset = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                whence = args[2]
                    .as_ref()
                    .map(|v| self.native_int(v, name, 3, line))
                    .transpose()?
                    .unwrap_or(0) as i32;
            }
            _ => {}
        }
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            return Err(invalid());
        };
        Ok(match operation {
            Operation::Close => {
                *kind = Kind::Closed;
                Value::Bool(true)
            }
            Operation::Eof => Value::Bool(stream.eof),
            Operation::Flush => Value::Bool(stream.backend.flush().is_ok()),
            Operation::Tell => Value::Int(stream.position as i64),
            Operation::Rewind | Operation::Seek => {
                let seek = match whence {
                    0 if offset >= 0 => Some(SeekFrom::Start(offset as u64)),
                    1 => stream
                        .position
                        .checked_add_signed(offset)
                        .map(SeekFrom::Start),
                    2 => Some(SeekFrom::End(offset)),
                    _ => None,
                };
                let position = seek.and_then(|seek| stream.backend.seek(seek).ok());
                let success = position.is_some();
                if success {
                    stream.eof = false;
                    stream.position = position.unwrap();
                } else if (0..=2).contains(&whence)
                    && offset < 0
                    && let Backend::Memory(buffer) = &mut stream.backend
                {
                    buffer.set_position(0);
                }
                if matches!(operation, Operation::Rewind) {
                    Value::Bool(success)
                } else {
                    Value::Int(if success { 0 } else { -1 })
                }
            }
            Operation::Write => {
                if data.is_empty() {
                    Value::Int(0)
                } else if !stream.writable && !matches!(stream.backend, Backend::File(_)) {
                    Value::Bool(false)
                } else {
                    match stream.backend.write(&data) {
                        Ok(count) => {
                            // PHP tracks the logical position separately from
                            // O_APPEND's underlying file offset.
                            stream.position += count as u64;
                            Value::Int(count as i64)
                        }
                        Err(error) => {
                            self.diagnostics.push(format!("Notice: {name}(): Write of {} bytes failed with errno={} {} on line {line}", data.len(), error.raw_os_error().unwrap_or(0), io_message(&error)));
                            Value::Bool(false)
                        }
                    }
                }
            }
            Operation::Read => {
                let mut bytes = Vec::new();
                let mut buffer = [0; 8192];
                while bytes.len() < length as usize {
                    let wanted = buffer.len().min(length as usize - bytes.len());
                    match stream.backend.read(&mut buffer[..wanted]) {
                        Ok(0) => {
                            stream.eof = true;
                            break;
                        }
                        Ok(count) => {
                            stream.position += count as u64;
                            bytes.try_reserve(count).map_err(|_| {
                                Error::new(
                                    ErrorKind::Error,
                                    "Unable to allocate string storage",
                                    line,
                                )
                            })?;
                            bytes.extend_from_slice(&buffer[..count]);
                        }
                        Err(error) => {
                            self.diagnostics.push(format!("Notice: {name}(): Read of {wanted} bytes failed with errno={} {} on line {line}", error.raw_os_error().unwrap_or(0), io_message(&error)));
                            return Ok(Value::Bool(false));
                        }
                    }
                }
                Value::String(bytes.into())
            }
        })
    }
}

fn io_message(error: &std::io::Error) -> String {
    let message = error.to_string();
    message
        .split(" (os error ")
        .next()
        .unwrap_or(&message)
        .to_owned()
}
