// Altered Rust adaptation of PHP 8.5.0 ext/standard/streamsfuncs.c and
// main/streams/streams.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::resource::{Backend, Kind};
use std::io::Read;

impl Runtime {
    pub(super) fn stream_get_line(
        &mut self,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let value = args[0].as_ref().unwrap();
        self.validate_stream(value, name, line)?;
        let length = self.native_int(args[1].as_ref().unwrap(), name, 2, line)?;
        let ending = args[2]
            .as_ref()
            .map(|value| self.native_string(value, name, 3, line))
            .transpose()?
            .unwrap_or_default();
        if length < 0 {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #2 ($length) must be greater than or equal to 0"),
                line,
            ));
        }
        self.validate_stream(value, name, line)?;
        let limit = if length == 0 { 8192 } else { length as usize };
        let Value::Resource(resource) = value else {
            unreachable!()
        };
        let mut searched: usize = 0;
        let mut exhausted = false;
        loop {
            self.validate_stream(value, name, line)?;
            let mut kind = resource.kind.borrow_mut();
            let Kind::Stream(stream) = &mut *kind else {
                unreachable!()
            };
            let visible = stream.read_buffer.len().min(limit);
            let found = if ending.is_empty() {
                None
            } else {
                // Retain a possible delimiter prefix at a chunk boundary.
                let start = searched.saturating_sub(ending.len() - 1).min(visible);
                stream.read_buffer.bytes()[start..visible]
                    .windows(ending.len())
                    .position(|bytes| bytes == ending)
                    .map(|offset| start + offset)
            };
            if found.is_some() || visible == limit || exhausted {
                if found.is_none() && visible < limit && (!stream.eof || visible == 0) {
                    return Ok(Value::Bool(false));
                }
                let count = found.unwrap_or(visible);
                let bytes = stream.read_buffer.bytes()[..count].to_vec();
                let consumed = count + if found.is_some() { ending.len() } else { 0 };
                stream.read_buffer.consume(consumed);
                stream.position = stream.position.wrapping_add(consumed as u64);
                return Ok(Value::String(bytes.into()));
            }
            if matches!(stream.backend, Backend::Directory(_)) {
                return Ok(Value::Bool(false));
            }
            searched = stream.read_buffer.len();
            let stdio = matches!(
                stream.backend,
                Backend::File(_) | Backend::StandardInput(_) | Backend::StandardOutput(_)
            );
            let capacity = stream.read_buffer.fill_space().map_err(|_| {
                Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
            })?;
            let mut chunk = Vec::new();
            chunk.try_reserve_exact(capacity).map_err(|_| {
                Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
            })?;
            chunk.resize(capacity, 0);
            match stream.backend.read(&mut chunk) {
                Ok(count) => {
                    if count == 0 {
                        stream.eof = true;
                        exhausted = true;
                    } else {
                        stream.read_buffer.append(&chunk[..count]).map_err(|_| {
                            Error::new(ErrorKind::Error, "Unable to allocate string storage", line)
                        })?;
                    }
                    if stdio {
                        self.file_stat_cache = [None, None];
                    }
                }
                Err(error) => {
                    drop(kind);
                    let warning = self.warn(format!(
                        "Notice: {name}(): Read of {capacity} bytes failed with errno={} {} on line {line}",
                        error.raw_os_error().unwrap_or(0), streams::io_message(&error)
                    ));
                    if stdio {
                        if error.raw_os_error() != Some(9)
                            && let Kind::Stream(stream) = &mut *resource.kind.borrow_mut()
                        {
                            stream.eof = true;
                        }
                        self.file_stat_cache = [None, None];
                    }
                    warning?;
                    exhausted = true;
                }
            }
        }
    }
}
