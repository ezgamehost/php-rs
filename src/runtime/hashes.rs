// Altered Rust adaptation of PHP 8.5.0 ext/standard/{md5,sha1}.c.
// Copyright (c) The PHP Group. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    digest::{Algorithm, Digest},
    resource::Kind,
};

impl Runtime {
    pub(super) fn hash_value(
        &mut self,
        algorithm: Algorithm,
        file: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let input = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        if file && input.contains(&0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 ($filename) must not contain any null bytes"),
                line,
            ));
        }
        let binary = args[1]
            .as_ref()
            .map(|value| self.native_bool(value, name, 2, line))
            .transpose()?
            .unwrap_or(false);
        let mut digest = Digest::new(algorithm);
        if !file {
            digest.update(&input);
            return Ok(digest_value(digest, binary));
        }
        let previous = self.native_warning_suppressed;
        let mut pending = None;
        let result: Result<Value, Error> = (|| {
            let opened = self.transfer_open(&input, b"rb", name, &mut pending, false, line)?;
            let Value::Resource(resource) = opened else {
                return Ok(Value::Bool(false));
            };
            let mut bytes = [0; 1024];
            while let Some(read) =
                self.read_transfer_chunk(&resource, &mut bytes, false, name, line)?
            {
                if read == 0 {
                    break;
                }
                digest.update(&bytes[..read]);
            }
            if algorithm == Algorithm::Md5 {
                let kind = resource.kind.borrow();
                if !matches!(&*kind, Kind::Stream(stream) if stream.at_eof()) {
                    return Ok(Value::Bool(false));
                }
            }
            Ok(digest_value(digest, binary))
        })();
        self.native_warning_suppressed = previous;
        if result
            .as_ref()
            .is_err_and(|error| error.kind == ErrorKind::UnwindExit)
        {
            return result;
        }
        pending.map_or(result, Err)
    }
}

fn digest_value(digest: Digest, binary: bool) -> Value {
    let bytes = digest.finish();
    if binary {
        Value::String(bytes.into())
    } else {
        const HEX: &[u8] = b"0123456789abcdef";
        Value::String(
            bytes
                .into_iter()
                .flat_map(|byte| [HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]])
                .collect::<Vec<_>>()
                .into(),
        )
    }
}
