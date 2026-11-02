use super::*;
use crate::resource::{Backend, Kind, Stream};
use std::{
    fs::OpenOptions,
    io::{BufRead, Cursor, Read, Seek, SeekFrom, Write},
};

#[derive(Debug, Clone, Copy)]
pub(super) enum Operation {
    Close,
    Read,
    GetChar,
    Truncate,
    Write,
    Eof,
    Tell,
    Seek,
    Rewind,
    Flush,
    Metadata,
    Stat,
}

impl Runtime {
    fn stream_metadata(&mut self, value: &Value, name: &str, line: usize) -> Result<Value, Error> {
        self.validate_stream(value, name, line)?;
        let Value::Resource(resource) = value else {
            unreachable!()
        };
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            unreachable!()
        };
        let (wrapper, stream_type, seekable, unread) = match &mut stream.backend {
            Backend::File(file) => ("plainfile", "STDIO", file.stream_position().is_ok(), 0),
            Backend::StandardInput(reader) => (
                "PHP",
                "STDIO",
                stream.position != u64::MAX,
                reader.buffer().len(),
            ),
            Backend::StandardOutput(_) => ("PHP", "STDIO", stream.position != u64::MAX, 0),
            Backend::Memory(_) => ("PHP", "MEMORY", true, 0),
            Backend::Temporary(_) => ("PHP", "TEMP", true, 0),
            Backend::Directory(directory) => {
                let glob = directory.glob().is_some();
                (
                    if glob { "glob" } else { "plainfile" },
                    if glob { "glob" } else { "dir" },
                    true,
                    0,
                )
            }
        };
        let mut metadata = PhpArray::default();
        let mut add = |key: &[u8], value| {
            metadata.insert(ArrayKey::String(key.to_vec()), value);
        };
        if !matches!(stream.backend, Backend::Temporary(_)) {
            add(b"timed_out", Value::Bool(false));
            add(b"blocked", Value::Bool(true));
            add(b"eof", Value::Bool(stream.eof));
        }
        add(
            b"wrapper_type",
            Value::String(wrapper.as_bytes().to_vec().into()),
        );
        add(
            b"stream_type",
            Value::String(stream_type.as_bytes().to_vec().into()),
        );
        add(b"mode", Value::String(stream.mode.clone().into()));
        add(b"unread_bytes", Value::Int(unread as i64));
        add(b"seekable", Value::Bool(seekable));
        if let Some(uri) = &stream.uri {
            add(b"uri", Value::String(uri.clone().into()));
        }
        Ok(Value::Array(Rc::new(metadata)))
    }

    pub(super) fn stream_get_contents(
        &mut self,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let value = args[0].as_ref().unwrap();
        self.validate_stream(value, name, line)?;
        let length = args[1]
            .as_ref()
            .filter(|v| !matches!(v, Value::Null))
            .map(|v| self.native_int_type(v, name, 2, line, "?int"))
            .transpose()?
            .unwrap_or(-1);
        let offset = args[2]
            .as_ref()
            .map(|v| self.native_int(v, name, 3, line))
            .transpose()?
            .unwrap_or(-1);
        if length < -1 {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #2 ($length) must be greater than or equal to -1"),
                line,
            ));
        }
        if offset >= 0 {
            let Value::Resource(resource) = value else {
                unreachable!()
            };
            let position = {
                let kind = resource.kind.borrow();
                if let Kind::Stream(stream) = &*kind {
                    stream.position as i64
                } else {
                    -1
                }
            };
            if position >= 0 && offset != position {
                let result = self.stream_operation(
                    Operation::Seek,
                    name,
                    &[
                        Some(value.clone()),
                        Some(Value::Int(if offset > position {
                            offset - position
                        } else {
                            offset
                        })),
                        Some(Value::Int(i64::from(offset > position))),
                    ],
                    line,
                )?;
                if result != Value::Int(0) {
                    self.warn(format!(
                        "{name}(): Failed to seek to position {offset} in the stream on line {line}"
                    ))?;
                    return Ok(Value::Bool(false));
                }
            }
        }
        self.stream_contents_bytes(value, name, (length >= 0).then_some(length as usize), line)
            .map(|bytes| Value::String(bytes.into()))
    }

    pub(super) fn stream_contents_bytes(
        &mut self,
        value: &Value,
        name: &str,
        limit: Option<usize>,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        self.validate_stream(value, name, line)?;
        let Value::Resource(resource) = value else {
            unreachable!()
        };
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            unreachable!()
        };
        if limit == Some(0) || matches!(stream.backend, Backend::Directory(_)) {
            return Ok(Vec::new());
        }
        if matches!(stream.backend, Backend::File(_) | Backend::StandardInput(_)) {
            self.file_stat_cache = [None, None];
        }
        let mut bytes = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let wanted = limit.map_or(buffer.len(), |limit| {
                (limit - bytes.len()).min(buffer.len())
            });
            if wanted == 0 {
                break;
            }
            match stream.backend.read(&mut buffer[..wanted]) {
                Ok(0) => {
                    stream.eof = true;
                    break;
                }
                Ok(count) => {
                    stream.position = stream.position.wrapping_add(count as u64);
                    bytes.try_reserve(count).map_err(|_| {
                        Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
                    })?;
                    bytes.extend_from_slice(&buffer[..count]);
                }
                Err(error) => {
                    drop(kind);
                    self.warn(format!("Notice: {name}(): Read of 8192 bytes failed with errno={} {} on line {line}", error.raw_os_error().unwrap_or(0), io_message(&error)))?;
                    break;
                }
            }
        }
        Ok(bytes)
    }

    pub(super) fn fgets(
        &mut self,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let stream = args[0].as_ref().unwrap();
        self.validate_stream(stream, name, line)?;
        let length = args[1]
            .as_ref()
            .filter(|v| !matches!(v, Value::Null))
            .map(|v| self.native_int_type(v, name, 2, line, "?int"))
            .transpose()?;
        if length.is_some_and(|n| n <= 0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #2 ($length) must be greater than 0"),
                line,
            ));
        }
        self.stream_read_line(stream, name, length.map(|n| n as usize - 1), line)
            .map(|text| text.map_or(Value::Bool(false), |text| Value::String(text.into())))
    }

    pub(super) fn stream_read_line(
        &mut self,
        value: &Value,
        name: &str,
        limit: Option<usize>,
        line: usize,
    ) -> Result<Option<Vec<u8>>, Error> {
        self.validate_stream(value, name, line)?;
        let Value::Resource(resource) = value else {
            unreachable!()
        };
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            unreachable!()
        };
        if matches!(stream.backend, Backend::Directory(_)) {
            return Ok(None);
        }
        if matches!(stream.backend, Backend::File(_) | Backend::StandardInput(_)) {
            self.file_stat_cache = [None, None];
        }
        let mut text = Vec::new();
        while limit.is_none_or(|limit| text.len() < limit) {
            let mut byte = [0];
            match stream.backend.read(&mut byte) {
                Ok(0) => {
                    stream.eof = true;
                    break;
                }
                Ok(_) => {
                    stream.position = stream.position.wrapping_add(1);
                    text.push(byte[0]);
                    if byte[0] == b'\n' {
                        break;
                    }
                }
                Err(error) => {
                    drop(kind);
                    self.warn(format!("Notice: {name}(): Read of 8192 bytes failed with errno={} {} on line {line}", error.raw_os_error().unwrap_or(0), io_message(&error)))?;
                    return Ok((!text.is_empty()).then_some(text));
                }
            }
        }
        Ok((!text.is_empty()).then_some(text))
    }

    pub(super) fn validate_stream(
        &self,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<(), Error> {
        let Value::Resource(resource) = value else {
            return Err(builtins::type_error(value, name, 1, "resource", line));
        };
        if !matches!(*resource.kind.borrow(), Kind::Stream(_)) {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!("{name}(): Argument #1 ($stream) must be an open stream resource"),
                line,
            ));
        }
        Ok(())
    }

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
        self.open_stream("fopen", &filename, &mode, include, line)
    }

    pub(super) fn stream_context(
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
                if !matches!(*resource.kind.borrow(), Kind::Context(_)) {
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
            self.warn(format!(
                "unlink({}): {} on line {line}",
                String::from_utf8_lossy(&filename),
                io_message(&error)
            ))?;
            Ok(Value::Bool(false))
        } else {
            self.file_stat_cache = [None, None];
            Ok(Value::Bool(true))
        }
    }

    pub(super) fn open_stream(
        &mut self,
        name: &str,
        filename: &[u8],
        mode: &[u8],
        include: bool,
        line: usize,
    ) -> Result<Value, Error> {
        self.open_stream_with_create_mode(name, filename, mode, include, None, line)
    }

    pub(super) fn open_stream_with_create_mode(
        &mut self,
        name: &str,
        filename: &[u8],
        mode: &[u8],
        include: bool,
        creation_mode: Option<u32>,
        line: usize,
    ) -> Result<Value, Error> {
        #[cfg(not(unix))]
        let _ = creation_mode;
        if filename.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "Path must not be empty",
                line,
            ));
        }
        let resolved_include = if include {
            self.resolve_include(filename, name, line)?
        } else {
            None
        };
        let filename = resolved_include
            .as_ref()
            .map_or(filename, |path| path.as_os_str().as_encoded_bytes());
        let mode = mode.split(|byte| *byte == 0).next().unwrap();
        let standard = match filename {
            b"php://stdin" => Some((0, b"STDIN".as_slice())),
            b"php://stdout" => Some((1, b"STDOUT".as_slice())),
            b"php://stderr" => Some((2, b"STDERR".as_slice())),
            _ => None,
        };
        if let Some((index, constant)) = standard {
            let stream = if self
                .resources
                .standard(constant)
                .is_some_and(|stream| !stream.is_open())
            {
                None
            } else {
                crate::resource::standard_stream(index).ok()
            };
            return match stream {
                Some(mut stream) => {
                    stream.mode = mode[..mode.len().min(15)].to_vec();
                    Ok(Value::Resource(
                        self.resources.allocate(Kind::Stream(stream)),
                    ))
                }
                None => {
                    self.open_warning(name, filename, "operation failed", line)?;
                    Ok(Value::Bool(false))
                }
            };
        }
        let memory = filename == b"php://memory" || filename == b"php://temp";
        let first = mode.first().copied().unwrap_or(0);
        // These wrappers only distinguish read-only mode from writable mode;
        // ordinary files validate the leading mode character.
        if !memory && !matches!(first, b'r' | b'w' | b'a' | b'x' | b'c') {
            self.open_warning(
                name,
                filename,
                &format!(
                    "`{}' is not a valid mode for fopen",
                    String::from_utf8_lossy(mode)
                ),
                line,
            )?;
            return Ok(Value::Bool(false));
        }
        let plus = mode.contains(&b'+');
        let readable = memory || first == b'r' || plus;
        let writable = if memory {
            mode.iter().any(|b| matches!(b, b'a' | b'w' | b'+'))
        } else {
            first != b'r' || plus
        };
        let backend = if memory {
            Backend::Memory(Cursor::new(Vec::new()))
        } else {
            let Some(bytes) = file_operations::plain_path(filename) else {
                self.warn(format!(
                    "{name}(): Remote host file access not supported, {} on line {line}",
                    String::from_utf8_lossy(filename)
                ))?;
                self.open_warning(name, filename, "no suitable wrapper could be found", line)?;
                return Ok(Value::Bool(false));
            };
            let path = crate::source::path_from_bytes(bytes);
            let resolved = self.working_directory().join(&path);
            let mut options = OpenOptions::new();
            options
                .read(readable)
                .write(writable)
                .append(first == b'a')
                .create(matches!(first, b'w' | b'a' | b'c'))
                .create_new(first == b'x')
                .truncate(first == b'w');
            #[cfg(unix)]
            if let Some(mode) = creation_mode {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(mode);
            }
            match open_local_file(&options, &resolved, creation_mode) {
                Ok(file) => {
                    self.file_stat_cache = [None, None];
                    Backend::File(file)
                }
                Err(error) => {
                    self.open_warning(name, filename, &io_message(&error), line)?;
                    return Ok(Value::Bool(false));
                }
            }
        };
        let recorded_mode = if memory {
            if mode.contains(&b'a') {
                b"a+b".to_vec()
            } else if mode.contains(&b'w') || plus {
                b"w+b".to_vec()
            } else {
                b"rb".to_vec()
            }
        } else {
            mode[..mode.len().min(15)].to_vec()
        };
        let resource = self.resources.allocate(Kind::Stream(Stream {
            mode: recorded_mode.clone(),
            context: None,
            uri: Some(filename.to_vec()),
            remove_on_close: None,
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
                mode: recorded_mode,
                context: None,
                uri: None,
                remove_on_close: None,
                backend,
                writable,
                eof: false,
                position: 0,
            }));
            stream.backend = Backend::Temporary(inner);
        }
        Ok(Value::Resource(resource))
    }

    fn open_warning(
        &mut self,
        name: &str,
        filename: &[u8],
        message: &str,
        line: usize,
    ) -> Result<(), Error> {
        self.warn(format!(
            "{name}({}): Failed to open stream: {message} on line {line}",
            String::from_utf8_lossy(filename)
        ))?;
        Ok(())
    }

    pub(super) fn stream_operation(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let first = args[0].as_ref().unwrap();
        self.validate_stream(first, name, line)?;
        let Value::Resource(resource) = first else {
            unreachable!("validated stream resource")
        };
        let invalid = || {
            Error::new(
                ErrorKind::TypeError,
                format!("{name}(): Argument #1 ($stream) must be an open stream resource"),
                line,
            )
        };
        // Coercion may run user code, so do not hold a resource borrow across it.
        let mut data = Vec::new();
        let mut length = 0;
        let mut offset = 0;
        let mut whence = 0;
        if matches!(operation, Operation::Metadata) {
            return self.stream_metadata(first, name, line);
        }
        if matches!(operation, Operation::Stat) {
            return self.stream_stat(first, name, line);
        }
        if matches!(operation, Operation::GetChar) {
            return self
                .stream_read_line(first, name, Some(1), line)
                .map(|text| text.map_or(Value::Bool(false), |text| Value::String(text.into())));
        }
        match operation {
            Operation::Truncate => {
                length = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
                if length < 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        format!("{name}(): Argument #2 ($size) must be greater than or equal to 0"),
                        line,
                    ));
                }
            }
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
        if let Backend::Directory(directory) = &mut stream.backend {
            match operation {
                Operation::Close => {
                    drop(kind);
                    self.warn(format!("{name}(): cannot close the provided stream, as it must not be manually closed on line {line}"))?;
                    return Ok(Value::Bool(false));
                }
                Operation::Rewind | Operation::Seek => {
                    if whence == 0 && offset < 0 {
                        return Ok(Value::Int(-1));
                    }
                    directory.rewind();
                    stream.eof = false;
                    // The native directory seek callback leaves the logical
                    // stream position untouched, regardless of rewind/offset.
                    return Ok(if matches!(operation, Operation::Rewind) {
                        Value::Bool(true)
                    } else {
                        Value::Int(0)
                    });
                }
                Operation::Read => {
                    if length as usize != crate::native_directory::RECORD_SIZE {
                        return Ok(Value::Bool(false));
                    }
                    let mut record = vec![0; length as usize];
                    return Ok(match stream.backend.read(&mut record) {
                        Ok(count) => {
                            stream.position = stream.position.wrapping_add(count as u64);
                            record.truncate(count);
                            Value::String(record.into())
                        }
                        Err(_) => Value::Bool(false),
                    });
                }
                Operation::Write if !data.is_empty() => {
                    drop(kind);
                    self.warn(format!(
                        "Notice: {name}(): Stream is not writable on line {line}"
                    ))?;
                    return Ok(Value::Bool(false));
                }
                _ => {}
            }
        }
        // Plain streams invalidate PHP's request stat cache when their I/O may
        // change access, modification, or inode-change times.
        if (matches!(operation, Operation::Read)
            || (matches!(operation, Operation::Write) && !data.is_empty()))
            && matches!(
                stream.backend,
                Backend::File(_) | Backend::StandardInput(_) | Backend::StandardOutput(_)
            )
        {
            self.file_stat_cache = [None, None];
        }
        if matches!(operation, Operation::Read)
            && let Backend::StandardInput(reader) = &mut stream.backend
        {
            // php://stdin drains any buffered bytes, then fills at most one
            // additional chunk. It must not wait for the entire requested
            // length on an open pipe.
            let mut bytes = Vec::new();
            let take = reader.buffer().len().min(length as usize);
            bytes.extend_from_slice(&reader.buffer()[..take]);
            reader.consume(take);
            if bytes.len() < length as usize {
                match reader.fill_buf() {
                    Ok(buffer) => {
                        stream.eof = buffer.is_empty();
                        let take = buffer.len().min(length as usize - bytes.len());
                        bytes.extend_from_slice(&buffer[..take]);
                        reader.consume(take);
                    }
                    Err(error) => {
                        drop(kind);
                        self.warn(format!("Notice: {name}(): Read of 8192 bytes failed with errno={} {} on line {line}", error.raw_os_error().unwrap_or(0), io_message(&error)))?;
                        return Ok(Value::Bool(false));
                    }
                }
            }
            stream.position = stream.position.wrapping_add(bytes.len() as u64);
            return Ok(Value::String(bytes.into()));
        }
        Ok(match operation {
            Operation::Metadata | Operation::Stat | Operation::GetChar => unreachable!(),
            Operation::Truncate => match stream.truncate(length as u64) {
                Some(result) => Value::Bool(result.is_ok()),
                None => {
                    drop(kind);
                    self.warn(format!(
                        "{name}(): Can't truncate this stream! on line {line}"
                    ))?;
                    Value::Bool(false)
                }
            },
            Operation::Close => {
                if resource.id == 3
                    && self
                        .resources
                        .standard(b"STDERR")
                        .is_some_and(|standard| Rc::ptr_eq(&standard, resource))
                {
                    self.diagnostics.stderr_closed = true;
                }
                *kind = Kind::Closed;
                Value::Bool(true)
            }
            Operation::Eof => Value::Bool(stream.eof),
            Operation::Flush => Value::Bool(stream.backend.flush().is_ok()),
            Operation::Tell => {
                if stream.position == u64::MAX {
                    Value::Bool(false)
                } else {
                    Value::Int(stream.position as i64)
                }
            }
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
                let position = if let Some(seek) = seek {
                    match stream.backend.seek(seek) {
                        Ok(position) => Some(position),
                        Err(error) => {
                            if error.raw_os_error() == Some(29) {
                                drop(kind);
                                self.warn(format!(
                                    "{name}(): Stream does not support seeking on line {line}"
                                ))?;
                                return Ok(if matches!(operation, Operation::Rewind) {
                                    Value::Bool(false)
                                } else {
                                    Value::Int(-1)
                                });
                            }
                            None
                        }
                    }
                } else {
                    None
                };
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
                } else if let Backend::StandardOutput(stderr) = stream.backend {
                    self.emit_stream_output(&data, stderr);
                    stream.position = stream.position.wrapping_add(data.len() as u64);
                    Value::Int(data.len() as i64)
                } else if !stream.writable
                    && !matches!(stream.backend, Backend::File(_) | Backend::StandardInput(_))
                {
                    Value::Bool(false)
                } else {
                    match stream.write_bytes(&data) {
                        Ok(count) => {
                            // PHP tracks the logical position separately from
                            // O_APPEND's underlying file offset.
                            stream.position = stream.position.wrapping_add(count as u64);
                            Value::Int(count as i64)
                        }
                        Err(error) => {
                            drop(kind);
                            self.warn(format!("Notice: {name}(): Write of {} bytes failed with errno={} {} on line {line}", data.len(), error.raw_os_error().unwrap_or(0), io_message(&error)))?;
                            return Ok(Value::Bool(false));
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
                            stream.position = stream.position.wrapping_add(count as u64);
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
                            let reported = if matches!(
                                stream.backend,
                                Backend::StandardOutput(_) | Backend::File(_)
                            ) {
                                8192
                            } else {
                                wanted
                            };
                            drop(kind);
                            self.warn(format!("Notice: {name}(): Read of {reported} bytes failed with errno={} {} on line {line}", error.raw_os_error().unwrap_or(0), io_message(&error)))?;
                            return Ok(Value::Bool(false));
                        }
                    }
                }
                Value::String(bytes.into())
            }
        })
    }
}

pub(super) fn io_message(error: &std::io::Error) -> String {
    let message = error.to_string();
    message
        .split(" (os error ")
        .next()
        .unwrap_or(&message)
        .to_owned()
}

// An exclusive first attempt lets cross-device rename create a private file
// without changing process umask or modifying an existing destination's mode.
fn open_local_file(
    options: &OpenOptions,
    path: &std::path::Path,
    creation_mode: Option<u32>,
) -> std::io::Result<std::fs::File> {
    #[cfg(unix)]
    if let Some(mode) = creation_mode {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        loop {
            match options.clone().create_new(true).mode(mode).open(path) {
                Ok(file) => {
                    file.set_permissions(std::fs::Permissions::from_mode(mode))?;
                    return Ok(file);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    match options.clone().create(false).open(path) {
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                            // A dangling final symlink exists, but its target does
                            // not. Create that target exclusively instead of
                            // repeatedly retrying the symlink's occupied name.
                            match std::fs::read_link(path) {
                                Ok(target) => {
                                    let target = if target.is_absolute() {
                                        target
                                    } else {
                                        path.parent()
                                            .unwrap_or(std::path::Path::new("."))
                                            .join(target)
                                    };
                                    return open_local_file(options, &target, creation_mode);
                                }
                                Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {
                                    continue;
                                }
                                Err(_) => return Err(error),
                            }
                        }
                        result => return result,
                    }
                }
                Err(error) => return Err(error),
            }
        }
    }
    #[cfg(not(unix))]
    let _ = creation_mode;
    options.open(path)
}
