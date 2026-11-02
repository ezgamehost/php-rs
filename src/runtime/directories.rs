// Directory API behavior follows PHP 8.5.0 ext/standard/dir.c and its
// main/streams directory operations. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    native_directory::{Directory, RECORD_SIZE},
    resource::{Backend, Kind, Stream},
};
use std::io::Read;

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    Open,
    Read,
    Rewind,
    Close,
    Scan,
}

impl Runtime {
    pub(super) fn directory_operation(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(operation, Operation::Open | Operation::Scan) {
            return self.open_directory(operation, name, args, line);
        }
        let resource = match args[0].as_ref() {
            None | Some(Value::Null) => {
                self.warn(format!("Deprecated: {name}(): Passing null is deprecated, instead the last opened directory stream should be provided on line {line}"))?;
                self.default_directory
                    .clone()
                    .ok_or_else(|| Error::new(ErrorKind::TypeError, "No resource supplied", line))?
            }
            Some(Value::Resource(resource)) => resource.clone(),
            Some(value) => {
                return Err(builtins::type_error(
                    value,
                    name,
                    1,
                    "resource or null",
                    line,
                ));
            }
        };
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!("{name}(): Argument #1 ($dir_handle) must be an open stream resource"),
                line,
            ));
        };
        let Backend::Directory(directory) = &mut stream.backend else {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!("{name}(): Argument #1 ($dir_handle) must be a valid Directory resource"),
                line,
            ));
        };
        match operation {
            Operation::Read => {
                let mut record = vec![0; RECORD_SIZE];
                match stream.backend.read(&mut record) {
                    Ok(count) if count == RECORD_SIZE => {
                        stream.position = stream.position.wrapping_add(count as u64);
                        let length = record.iter().position(|&b| b == 0).unwrap();
                        record.truncate(length);
                        Ok(Value::String(record.into()))
                    }
                    _ => Ok(Value::Bool(false)),
                }
            }
            Operation::Rewind => {
                directory.rewind();
                stream.eof = false;
                Ok(Value::Null)
            }
            Operation::Close => {
                *kind = Kind::Closed;
                if self
                    .default_directory
                    .as_ref()
                    .is_some_and(|default| Rc::ptr_eq(default, &resource))
                {
                    self.default_directory = None;
                }
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }

    fn open_directory(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let path = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        if path.contains(&0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 ($directory) must not contain any null bytes"),
                line,
            ));
        }
        let scan = matches!(operation, Operation::Scan);
        let order = if scan {
            args[1]
                .as_ref()
                .map(|v| self.native_int(v, name, 2, line))
                .transpose()?
                .unwrap_or(0)
        } else {
            0
        };
        let context_position = if scan { 3 } else { 2 };
        let context = args[context_position - 1].as_ref();
        if let Some(value) = context
            && !matches!(value, Value::Null | Value::Resource(_))
        {
            return Err(builtins::type_error(
                value,
                name,
                context_position,
                "resource or null",
                line,
            ));
        }
        if scan && path.is_empty() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "scandir(): Argument #1 ($directory) must not be empty",
                line,
            ));
        }
        // opendir always resolves the default context; scandir only resolves
        // an explicitly supplied non-null context.
        if !scan || context.is_some_and(|v| !matches!(v, Value::Null)) {
            self.stream_context(context, name, context_position, line)?;
        }
        if path.is_empty() {
            return Ok(Value::Bool(false));
        }
        let directory = match Directory::open_stream(&path, &self.working_directory()) {
            Ok(directory) => directory,
            Err(error) => {
                let message = streams::io_message(&error);
                self.warn(format!(
                    "{name}({}): Failed to open directory: {message} on line {line}",
                    String::from_utf8_lossy(&path)
                ))?;
                if scan {
                    self.warn(format!(
                        "{name}(): (errno {}): {message} on line {line}",
                        error.raw_os_error().unwrap_or(0)
                    ))?;
                }
                return Ok(Value::Bool(false));
            }
        };
        let resource = self.resources.allocate(Kind::Stream(Stream {
            backend: Backend::Directory(directory),
            writable: false,
            eof: false,
            read_buffer: Default::default(),
            position: 0,
            mode: b"r".to_vec(),
            uri: None,
            remove_on_close: None,
            context: None,
        }));
        if !scan {
            self.default_directory = Some(resource.clone());
            return Ok(Value::Resource(resource));
        }
        let mut names = Vec::new();
        let mut kind = resource.kind.borrow_mut();
        let Kind::Stream(stream) = &mut *kind else {
            unreachable!()
        };
        let Backend::Directory(directory) = &mut stream.backend else {
            unreachable!()
        };
        while let Some((name, _)) = directory.read() {
            names.push(name);
        }
        if order != 2 {
            names.sort();
            if order != 0 {
                names.reverse();
            }
        }
        let mut values = PhpArray::default();
        for name in names {
            values.append(Value::String(name.into()), line)?;
        }
        Ok(Value::Array(Rc::new(values)))
    }
}
