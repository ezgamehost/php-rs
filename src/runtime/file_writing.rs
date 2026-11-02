// Altered Rust adaptation of PHP 8.5.0 ext/standard/file.c and
// main/streams/streams.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md for its license.
use super::*;
use crate::resource::{Backend, Kind, PhpResource};
use std::io::Read;

impl Runtime {
    pub(super) fn file_put_contents(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        const NAME: &str = "file_put_contents";
        let filename = self.native_string(args[0].as_ref().unwrap(), NAME, 1, line)?;
        self.file_transfer_path(&filename, NAME, line)?;
        let data = args[1].as_ref().unwrap();
        let flags = args[2]
            .as_ref()
            .map(|value| self.native_int(value, NAME, 3, line))
            .transpose()?
            .unwrap_or(0);
        let context = args[3].as_ref();
        if let Some(value) = context
            && !matches!(value, Value::Null | Value::Resource(_))
        {
            return Err(builtins::type_error(
                value,
                NAME,
                4,
                "resource or null",
                line,
            ));
        }
        if let Value::Resource(resource) = data
            && !matches!(*resource.kind.borrow(), Kind::Stream(_))
        {
            return Err(Error::new(
                ErrorKind::TypeError,
                "file_put_contents(): supplied resource is not a valid stream resource",
                line,
            ));
        }
        if flags & 16 == 0 || context.is_some_and(|value| !matches!(value, Value::Null)) {
            self.stream_context(context, NAME, 4, line)?;
        }
        let mode: &[u8] = if flags & 8 != 0 {
            b"ab"
        } else if flags & 2 != 0 {
            if filename.windows(3).any(|bytes| bytes == b"://")
                && !filename
                    .get(..7)
                    .is_some_and(|bytes| bytes.eq_ignore_ascii_case(b"file://"))
            {
                self.warn(format!(
                    "{NAME}(): Exclusive locks may only be set for regular files on line {line}"
                ))?;
                return Ok(Value::Bool(false));
            }
            b"cb"
        } else {
            b"wb"
        };
        if filename.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "Path must not be empty",
                line,
            ));
        }
        let opened = self.open_stream(NAME, &filename, mode, flags & 1 != 0, line)?;
        let Value::Resource(resource) = &opened else {
            return Ok(Value::Bool(false));
        };
        if flags & 2 != 0 {
            let locked = {
                let kind = resource.kind.borrow();
                match &*kind {
                    Kind::Stream(stream) => match &stream.backend {
                        Backend::File(file) => file.lock().is_ok(),
                        Backend::StandardInput(reader) => reader.get_ref().lock().is_ok(),
                        _ => false,
                    },
                    _ => false,
                }
            };
            if !locked {
                drop(opened);
                self.warn(format!(
                    "{NAME}(): Exclusive locks are not supported for this stream on line {line}"
                ))?;
                return Ok(Value::Bool(false));
            }
        }
        if mode[0] == b'c'
            && let Kind::Stream(stream) = &*resource.kind.borrow()
            && let Backend::File(file) = &stream.backend
        {
            let _ = file.set_len(0);
        }
        match data {
            Value::Resource(source) => self.copy_stream_input(source, &opened, NAME, line),
            Value::Array(array) => {
                let mut index = 0;
                let mut count = 0_i64;
                let mut pending = None;
                while let Some((_, value)) = array.entry(index) {
                    let bytes = self.set_string(&value, line, &mut pending);
                    if pending
                        .as_ref()
                        .is_some_and(|error| error.kind == ErrorKind::UnwindExit)
                    {
                        return Err(pending.unwrap());
                    }
                    if !bytes.is_empty() {
                        let length = bytes.len() as i64;
                        let written = self.write_file_bytes(&opened, bytes, line);
                        let written = match written {
                            Ok(written) => written,
                            Err(error) => return Err(pending.unwrap_or(error)),
                        };
                        if written != Some(length) {
                            let warning = self.warn(format!(
                                "{NAME}(): Failed to write {length} bytes to {} on line {line}",
                                String::from_utf8_lossy(&filename)
                            ));
                            if let Some(error) = pending {
                                return Err(error);
                            }
                            warning?;
                            return Ok(Value::Bool(false));
                        }
                        count = count.saturating_add(length);
                    }
                    index += 1;
                }
                pending.map_or(Ok(Value::Int(count)), Err)
            }
            Value::Object(_) | Value::Closure(_) if !self.can_stringify(data) => {
                Ok(Value::Bool(false))
            }
            _ => {
                let bytes = self.string_bytes(data, line)?;
                let length = bytes.len() as i64;
                let written = self.write_file_bytes(&opened, bytes, line)?;
                if let Some(count) = written {
                    if count == length {
                        return Ok(Value::Int(count));
                    }
                    self.warn(format!("{NAME}(): Only {count} of {length} bytes written, possibly out of free disk space on line {line}"))?;
                }
                Ok(Value::Bool(false))
            }
        }
    }

    fn write_file_bytes(
        &mut self,
        stream: &Value,
        bytes: Vec<u8>,
        line: usize,
    ) -> Result<Option<i64>, Error> {
        self.write_transfer_bytes(stream, bytes, "file_put_contents", line)
    }

    fn write_transfer_bytes(
        &mut self,
        stream: &Value,
        bytes: Vec<u8>,
        name: &str,
        line: usize,
    ) -> Result<Option<i64>, Error> {
        let written = self.stream_operation(
            streams::Operation::Write,
            name,
            &[
                Some(stream.clone()),
                Some(Value::String(bytes.into())),
                None,
            ],
            line,
        )?;
        Ok(match written {
            Value::Int(count) => Some(count),
            _ => None,
        })
    }

    pub(super) fn copy_stream_input(
        &mut self,
        source: &Rc<PhpResource>,
        destination: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        // A native plain-file mapping advances the position without probing EOF.
        // Native mmap requires a page-aligned offset; an unaligned offset falls
        // back to reads and probes EOF. Preserve that observable distinction.
        let remaining = mapped_remaining(source);
        let mut count = 0_u64;
        let mut bytes = [0; 8192];
        loop {
            if remaining == Some(count) {
                break;
            }
            let length = remaining.map_or(bytes.len(), |size| {
                (size - count).min(bytes.len() as u64) as usize
            });
            let read = self.read_transfer_chunk(source, &mut bytes[..length], name, line)?;
            let Some(read) = read else {
                return Ok(Value::Bool(false));
            };
            if read == 0 {
                break;
            }
            let mut offset = 0;
            while offset < read {
                let written = self.write_transfer_bytes(
                    destination,
                    bytes[offset..read].to_vec(),
                    name,
                    line,
                )?;
                let Some(written) = written.filter(|&n| n > 0) else {
                    return Ok(Value::Bool(false));
                };
                offset += written as usize;
            }
            count += read as u64;
        }
        Ok(Value::Int(count.min(i64::MAX as u64) as i64))
    }

    pub(super) fn readfile(&mut self, args: &[Option<Value>], line: usize) -> Result<Value, Error> {
        const NAME: &str = "readfile";
        let filename = self.native_string(args[0].as_ref().unwrap(), NAME, 1, line)?;
        self.file_transfer_path(&filename, NAME, line)?;
        let include = args[1]
            .as_ref()
            .map(|v| self.native_bool(v, NAME, 2, line))
            .transpose()?
            .unwrap_or(false);
        self.stream_context(args[2].as_ref(), NAME, 3, line)?;
        if filename.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "Path must not be empty",
                line,
            ));
        }
        let opened = self.open_stream(NAME, &filename, b"rb", include, line)?;
        if !matches!(opened, Value::Resource(_)) {
            return Ok(Value::Bool(false));
        }
        self.stream_passthru(&opened, NAME, line)
    }

    pub(super) fn stream_passthru(
        &mut self,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        self.validate_stream(value, name, line)?;
        let Value::Resource(resource) = value else {
            unreachable!()
        };
        let remaining = mapped_remaining(resource);
        let mut count = 0_u64;
        let mut bytes = [0; 8192];
        loop {
            if remaining == Some(count) {
                break;
            }
            let length = remaining.map_or(bytes.len(), |size| {
                (size - count).min(bytes.len() as u64) as usize
            });
            match self.read_transfer_chunk(resource, &mut bytes[..length], name, line)? {
                Some(0) => break,
                Some(read) => {
                    self.emit_output(&bytes[..read])?;
                    count = count.saturating_add(read as u64);
                }
                None => {
                    return Ok(Value::Int(if count == 0 {
                        -1
                    } else {
                        count.min(i64::MAX as u64) as i64
                    }));
                }
            }
        }
        Ok(Value::Int(count.min(i64::MAX as u64) as i64))
    }

    fn file_transfer_path(&self, filename: &[u8], name: &str, line: usize) -> Result<(), Error> {
        if filename.contains(&0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 ($filename) must not contain any null bytes"),
                line,
            ));
        }
        Ok(())
    }

    fn read_transfer_chunk(
        &mut self,
        resource: &Rc<PhpResource>,
        bytes: &mut [u8],
        name: &str,
        line: usize,
    ) -> Result<Option<usize>, Error> {
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            return Ok(None);
        };
        // Directory handles reject bulk copying without a stdio read notice.
        if matches!(stream.backend, Backend::Directory(_)) {
            return Ok(None);
        }
        let result = stream.backend.read(bytes);
        match result {
            Ok(count) => {
                stream.position = stream.position.wrapping_add(count as u64);
                stream.eof = count == 0;
                Ok(Some(count))
            }
            Err(error) => {
                drop(kind);
                self.warn(format!(
                    "Notice: {name}(): Read of 8192 bytes failed with errno={} {} on line {line}",
                    error.raw_os_error().unwrap_or(0),
                    streams::io_message(&error)
                ))?;
                Ok(None)
            }
        }
    }
}

fn mapped_remaining(resource: &Rc<PhpResource>) -> Option<u64> {
    let kind = resource.kind.borrow();
    let Kind::Stream(stream) = &*kind else {
        return None;
    };
    let metadata = match &stream.backend {
        Backend::File(file) if stream.mode.starts_with(b"r") || stream.mode.contains(&b'+') => {
            file.metadata().ok()?
        }
        Backend::StandardInput(reader) => reader.get_ref().metadata().ok()?,
        Backend::Temporary(inner) => return mapped_remaining(inner),
        _ => return None,
    };
    if !mapping_aligned(stream.position) || !metadata.is_file() {
        return None;
    }
    metadata
        .len()
        .checked_sub(stream.position)
        .filter(|&size| size > 0)
}

fn mapping_aligned(position: u64) -> bool {
    #[cfg(unix)]
    {
        unsafe extern "C" {
            fn getpagesize() -> std::ffi::c_int;
        }
        // This OS query has no pointer arguments or process state changes.
        let page_size = unsafe { getpagesize() };
        page_size > 0 && position.is_multiple_of(page_size as u64)
    }
    #[cfg(not(unix))]
    {
        let _ = position;
        false
    }
}
