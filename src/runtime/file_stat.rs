// Altered Rust adaptation of PHP 8.5.0 ext/standard/{file,filestat}.c and
// main/streams/{memory,plain_wrapper}.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::resource::{Backend, Kind, Stream};
use std::{fs::Metadata, io};

const NAMES: [&[u8]; 13] = [
    b"dev", b"ino", b"mode", b"nlink", b"uid", b"gid", b"rdev", b"size", b"atime", b"mtime",
    b"ctime", b"blksize", b"blocks",
];

fn stat_array(fields: [i64; 13], line: usize) -> Result<Value, Error> {
    let mut result = PhpArray::default();
    // Numeric fields precede named fields; these values are independent copies.
    for field in fields {
        result.append(Value::Int(field), line)?;
    }
    for (name, field) in NAMES.into_iter().zip(fields) {
        result.insert(ArrayKey::String(name.to_vec()), Value::Int(field));
    }
    Ok(Value::Array(Rc::new(result)))
}

pub(super) fn from_metadata(metadata: &Metadata, line: usize) -> Result<Value, Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        stat_array(
            [
                metadata.dev() as i64,
                metadata.ino() as i64,
                metadata.mode() as i64,
                metadata.nlink() as i64,
                metadata.uid() as i64,
                metadata.gid() as i64,
                metadata.rdev() as i64,
                metadata.size() as i64,
                metadata.atime(),
                metadata.mtime(),
                metadata.ctime(),
                metadata.blksize() as i64,
                metadata.blocks() as i64,
            ],
            line,
        )
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(Error::new(
            ErrorKind::Error,
            "Filesystem metadata is not supported on this platform",
            line,
        ))
    }
}

impl Runtime {
    pub(super) fn stream_stat(
        &mut self,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Value, Error> {
        self.validate_stream(value, name, line)?;
        let Value::Resource(resource) = value else {
            unreachable!()
        };
        let kind = resource.kind.borrow();
        let Kind::Stream(stream) = &*kind else {
            unreachable!()
        };
        stat_stream(stream, line)
    }
}

fn stat_stream(stream: &Stream, line: usize) -> Result<Value, Error> {
    let metadata = match &stream.backend {
        Backend::File(file) => file.metadata(),
        Backend::StandardInput(reader) => reader.get_ref().metadata(),
        Backend::StandardOutput(stderr) => output_metadata(*stderr),
        Backend::Directory(_) => return Ok(Value::Bool(false)),
        Backend::Temporary(resource, _) => {
            let kind = resource.kind.borrow();
            let Kind::Stream(inner) = &*kind else {
                return Ok(Value::Bool(false));
            };
            return stat_stream(inner, line);
        }
        Backend::Memory(buffer) => {
            // PHP synthesizes a regular-file stat record for memory streams.
            let mode = 0o100000 | if stream.writable { 0o666 } else { 0o444 };
            return stat_array(
                [
                    12,
                    0,
                    mode,
                    1,
                    0,
                    0,
                    -1,
                    buffer.get_ref().len() as i64,
                    0,
                    0,
                    0,
                    if cfg!(windows) { 0 } else { -1 },
                    if cfg!(windows) { 0 } else { -1 },
                ],
                line,
            );
        }
    };
    match metadata {
        Ok(metadata) => from_metadata(&metadata, line),
        Err(_) => Ok(Value::Bool(false)),
    }
}

fn output_metadata(stderr: bool) -> io::Result<Metadata> {
    crate::resource::output_file(stderr)?.metadata()
}
