// Altered Rust adaptation of PHP 8.5.0 ext/standard/file.c and
// main/streams/streams.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md for its license.
use super::*;
use crate::resource::{Backend, Kind};
use std::io::{Read, Seek, SeekFrom};

impl Runtime {
    pub(super) fn file_get_contents(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        const NAME: &str = "file_get_contents";
        let filename = self.native_string(args[0].as_ref().unwrap(), NAME, 1, line)?;
        if filename.contains(&0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "file_get_contents(): Argument #1 ($filename) must not contain any null bytes",
                line,
            ));
        }
        let include = args[1]
            .as_ref()
            .map(|v| self.native_bool(v, NAME, 2, line))
            .transpose()?
            .unwrap_or(false);
        let context = args[2].as_ref();
        if let Some(value) = context
            && !matches!(value, Value::Null | Value::Resource(_))
        {
            return Err(builtins::type_error(
                value,
                NAME,
                3,
                "resource or null",
                line,
            ));
        }
        let offset = args[3]
            .as_ref()
            .map(|v| self.native_int(v, NAME, 4, line))
            .transpose()?
            .unwrap_or(0);
        let length = args[4]
            .as_ref()
            .filter(|v| !matches!(v, Value::Null))
            .map(|v| self.native_int_type(v, NAME, 5, line, "?int"))
            .transpose()?;
        if length.is_some_and(|n| n < 0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "file_get_contents(): Argument #5 ($length) must be greater than or equal to 0",
                line,
            ));
        }
        self.stream_context(context, NAME, 3, line)?;
        if filename.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "Path must not be empty",
                line,
            ));
        }
        let opened = self.open_stream(NAME, &filename, b"rb", include, line)?;
        let Value::Resource(resource) = opened else {
            return Ok(Value::Bool(false));
        };
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            unreachable!()
        };
        if offset != 0 {
            let seek = if offset > 0 {
                SeekFrom::Start(offset as u64)
            } else {
                SeekFrom::End(offset)
            };
            match stream.backend.seek(seek) {
                Ok(position) => stream.position = position,
                Err(error) => {
                    if error.raw_os_error() == Some(29) {
                        self.diagnostics.push(format!(
                            "{NAME}(): Stream does not support seeking on line {line}"
                        ));
                    }
                    self.diagnostics.push(format!(
                        "{NAME}(): Failed to seek to position {offset} in the stream on line {line}"
                    ));
                    return Ok(Value::Bool(false));
                }
            }
        }
        if length == Some(0) {
            return Ok(Value::String(Vec::new().into()));
        }
        let limit = length.map(|n| n as usize);
        let small = limit.is_some_and(|n| n < 32768);
        let mut capacity = if small {
            limit.unwrap()
        } else {
            let size = match &stream.backend {
                Backend::File(file) => file.metadata().ok().map(|m| m.len()),
                Backend::StandardInput(reader) => reader.get_ref().metadata().ok().map(|m| m.len()),
                _ => None,
            };
            let estimate = size
                .filter(|&n| n > 0)
                .map(|n| n.saturating_sub(stream.position).saturating_add(8192) as usize)
                .unwrap_or(8192);
            limit.map_or(estimate, |limit| estimate.min(limit))
        };
        let mut bytes = Vec::new();
        let mut used = 0;
        loop {
            bytes.try_reserve(capacity - bytes.len()).map_err(|_| {
                Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
            })?;
            bytes.resize(capacity, 0);
            let wanted = capacity - used;
            // This API disables stdio buffering for its complete transfer.
            let read = match &mut stream.backend {
                Backend::StandardInput(reader) => reader.get_mut().read(&mut bytes[used..]),
                backend => backend.read(&mut bytes[used..]),
            };
            match read {
                Ok(0) => break,
                Ok(count) => {
                    used += count;
                    stream.position = stream.position.wrapping_add(count as u64);
                    if limit == Some(used) {
                        break;
                    }
                    if !small && used.saturating_add(2048) >= capacity {
                        capacity = capacity.checked_add(8192).ok_or_else(|| {
                            Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
                        })?;
                        if let Some(limit) = limit {
                            capacity = capacity.min(limit);
                        }
                    }
                }
                Err(error) => {
                    self.diagnostics.push(format!("Notice: {NAME}(): Read of {wanted} bytes failed with errno={} {} on line {line}", error.raw_os_error().unwrap_or(0), streams::io_message(&error)));
                    break;
                }
            }
        }
        bytes.truncate(used);
        Ok(Value::String(bytes.into()))
    }
}
