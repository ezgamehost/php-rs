// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_directory.c and its stubs.
// Copyright (c) The PHP Group. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, ast::NativeMethod, object::Class, resource::Kind};

pub(super) struct FileData {
    stream: Option<Value>,
    pub(super) mode: Vec<u8>,
    current: Option<Vec<u8>>,
    csv: Option<Value>,
    index: i64,
    flags: i64,
    max_length: i64,
    pub(super) separator: u8,
    pub(super) enclosure: u8,
    escape: Option<u8>,
    escape_default: bool,
}

impl FileData {
    pub(super) fn close(&mut self) {
        self.stream = None;
    }
}

impl Default for FileData {
    fn default() -> Self {
        Self {
            stream: None,
            mode: Vec::new(),
            current: None,
            csv: None,
            index: 0,
            flags: 0,
            max_length: 0,
            separator: b',',
            enclosure: b'"',
            escape: Some(b'\\'),
            escape_default: true,
        }
    }
}

fn state(object: &PhpObject) -> Rc<RefCell<FileData>> {
    object
        .file_info
        .borrow_mut()
        .as_mut()
        .unwrap()
        .file
        .get_or_insert_with(|| Rc::new(RefCell::new(FileData::default())))
        .clone()
}

fn clear(data: &mut FileData) {
    data.current = None;
    data.csv = None;
}
fn stream(object: &PhpObject, line: usize) -> Result<Value, Error> {
    state(object)
        .borrow()
        .stream
        .clone()
        .ok_or_else(|| Error::new(ErrorKind::Error, "Object not initialized", line))
}
fn eof(value: &Value) -> bool {
    let Value::Resource(resource) = value else {
        unreachable!()
    };
    match &*resource.kind.borrow() {
        Kind::Stream(stream) => stream.at_eof(),
        _ => true,
    }
}
fn int(value: Value) -> i64 {
    let Value::Int(value) = value else {
        unreachable!()
    };
    value
}

impl Runtime {
    pub(super) fn builtin_file_object_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, parent, interfaces, body) = match name {
            b"splfileobject" => (
                "SplFileObject",
                "SplFileInfo",
                vec!["RecursiveIterator", "SeekableIterator"],
                FILE_METHODS,
            ),
            b"spltempfileobject" => (
                "SplTempFileObject",
                "SplFileObject",
                Vec::new(),
                "public function __construct(int $maxMemory=2 * 1024 * 1024) {}",
            ),
            _ => return None,
        };
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let code = format!(
            "<?php class {canonical} extends {parent} {} {{ {body} }}",
            if interfaces.is_empty() {
                String::new()
            } else {
                format!("implements {}", interfaces.join(", "))
            }
        );
        let (mut statements, _, _) = parser::parse(lexer::lex(code.as_bytes()).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::FileObject);
            if !method.name.ends_with(b"::__construct") && !method.name.ends_with(b"::__toString") {
                method.tentative_return_type = method.return_type.take();
            }
        }
        let parent = self.find_class(parent.as_bytes()).unwrap();
        let mut methods = declaration.methods.clone();
        for method in &parent.methods {
            let short = method.name.rsplit(|b| *b == b':').next().unwrap();
            if !methods.iter().any(|m| {
                m.name
                    .rsplit(|b| *b == b':')
                    .next()
                    .unwrap()
                    .eq_ignore_ascii_case(short)
            }) {
                methods.push(method.clone());
            }
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods,
            declaration,
            parent: Some(parent),
            interfaces: interfaces
                .iter()
                .map(|i| self.find_class(i.as_bytes()).unwrap())
                .collect(),
            properties: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic: false,
            defaults: RefCell::default(),
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        });
        self.builtin_classes
            .borrow_mut()
            .insert(name.to_vec(), class.clone());
        Some(class)
    }

    fn file_object_open(
        &mut self,
        object: &Rc<PhpObject>,
        temporary: bool,
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let data = state(object);
        if data.borrow().stream.is_some() {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot call constructor twice",
                line,
            ));
        }
        let name = if temporary {
            "SplTempFileObject::__construct"
        } else {
            "SplFileObject::__construct"
        };
        let (filename, mode, include, context) = if temporary {
            let memory = int(self.lookup(b"maxMemory").unwrap());
            (
                if memory < 0 {
                    b"php://memory".to_vec()
                } else if supplied == 0 {
                    b"php://temp".to_vec()
                } else {
                    format!("php://temp/maxmemory:{memory}").into_bytes()
                },
                b"wb".to_vec(),
                false,
                Value::Null,
            )
        } else {
            (
                self.lookup(b"filename").unwrap().to_bytes(),
                self.lookup(b"mode").unwrap().to_bytes(),
                self.lookup(b"useIncludePath").unwrap().is_truthy(),
                self.lookup(b"context").unwrap(),
            )
        };
        if filename.contains(&0) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!("{name}(): Argument #1 ($filename) must not contain any null bytes"),
                line,
            ));
        }
        if file_operations::plain_path(&filename).is_some_and(|p| {
            self.working_directory()
                .join(crate::source::path_from_bytes(p))
                .is_dir()
        }) {
            return Err(self.array_iterator_exception(
                b"LogicException",
                "Cannot use SplFileObject with directories".into(),
                line,
            ));
        }
        self.stream_context(Some(&context), name, 4, line)?;
        let previous = self
            .native_warning_exception
            .replace(b"RuntimeException".to_vec());
        let result = self.open_stream(name, &filename, &mode, include, line);
        self.native_warning_exception = previous;
        let opened = result?;
        if !matches!(opened, Value::Resource(_)) {
            return Err(self.array_iterator_exception(
                b"RuntimeException",
                format!("Cannot open file '{}'", String::from_utf8_lossy(&filename)),
                line,
            ));
        }
        let original_path = match &opened {
            Value::Resource(resource) => match &*resource.kind.borrow() {
                Kind::Stream(stream) => stream.uri.clone(),
                _ => None,
            },
            _ => None,
        };
        {
            let mut data = data.borrow_mut();
            data.stream = Some(opened);
            data.mode = mode;
            data.separator = b',';
            data.enclosure = b'"';
            data.escape = Some(b'\\');
            data.escape_default = true;
        }
        let mut info = object.file_info.borrow_mut();
        let info = info.as_mut().unwrap();
        info.set_filename(original_path.clone().unwrap_or_else(|| filename.clone()));
        info.filename = Some(filename);
        info.original_path = original_path;
        if temporary {
            info.path.clear();
        }
        Ok(Value::Null)
    }

    fn file_object_read(
        &mut self,
        object: &Rc<PhpObject>,
        silent: bool,
        add: i64,
        csv: bool,
        line: usize,
    ) -> Result<bool, Error> {
        let resource = stream(object, line)?;
        let state = state(object);
        let (max, flags) = {
            let mut data = state.borrow_mut();
            clear(&mut data);
            (data.max_length, data.flags)
        };
        if eof(&resource) {
            if silent {
                return Ok(false);
            }
            let filename = object
                .file_info
                .borrow()
                .as_ref()
                .unwrap()
                .filename
                .clone()
                .unwrap_or_default();
            return Err(self.array_iterator_exception(
                b"RuntimeException",
                format!(
                    "Cannot read from file {}",
                    String::from_utf8_lossy(&filename)
                ),
                line,
            ));
        }
        let mut text = self
            .stream_read_line(
                &resource,
                "SplFileObject::fgets",
                (max > 0).then_some(max as usize),
                line,
            )?
            .unwrap_or_default();
        if !csv && flags & 1 != 0 && text.ends_with(b"\n") {
            text.pop();
            if text.ends_with(b"\r") {
                text.pop();
            }
        }
        let mut data = state.borrow_mut();
        data.current = Some(text);
        data.index = data.index.wrapping_add(add);
        Ok(true)
    }

    fn file_object_read_csv(
        &mut self,
        object: &Rc<PhpObject>,
        control: (u8, u8, Option<u8>),
        silent: bool,
        line: usize,
    ) -> Result<bool, Error> {
        let data = state(object);
        loop {
            let add = i64::from(data.borrow().current.is_some());
            if !self.file_object_read(object, silent, add, true, line)? {
                return Ok(false);
            }
            let data = data.borrow();
            let text = data.current.as_ref().unwrap();
            let empty = text.is_empty()
                || data.flags & 9 == 9 && matches!(text.as_slice(), b"\n" | b"\r\n");
            if !empty || data.flags & 4 == 0 {
                break;
            }
        }
        let text = data.borrow().current.clone().unwrap();
        let resource = stream(object, line)?;
        let value = csv::parse_record(text, control.0, control.1, control.2, line, || {
            self.stream_read_line(&resource, "SplFileObject::fgetcsv", None, line)
        })?;
        data.borrow_mut().csv = Some(value);
        Ok(true)
    }

    fn file_object_read_current(
        &mut self,
        object: &Rc<PhpObject>,
        silent: bool,
        line: usize,
    ) -> Result<bool, Error> {
        let data = state(object);
        loop {
            let (flags, control, add) = {
                let data = data.borrow();
                (
                    data.flags,
                    (data.separator, data.enclosure, data.escape),
                    i64::from(data.current.is_some()),
                )
            };
            let success = if flags & 8 != 0 {
                self.file_object_read_csv(object, control, silent, line)?
            } else {
                let method = objects::method(&object.class, b"getCurrentLine").unwrap();
                if !matches!(method.native, Some(NativeMethod::FileObject)) {
                    clear(&mut data.borrow_mut());
                    if eof(&stream(object, line)?) {
                        return Ok(false);
                    }
                    let target = self.unchecked_method_target(
                        method,
                        object.class.clone(),
                        Some(object.clone()),
                        line,
                    )?;
                    let value = self.invoke_at(target, &[], line, false, true)?;
                    let Value::String(text) = value else {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            format!(
                                "{}::getCurrentLine(): Return value must be of type string, {} returned",
                                object.class_name,
                                builtins::debug_type(&value)
                            ),
                            line,
                        ));
                    };
                    let mut data = data.borrow_mut();
                    if data.current.is_some() || data.csv.is_some() {
                        data.index = data.index.wrapping_add(1);
                    }
                    clear(&mut data);
                    data.current = Some(text.to_vec());
                    true
                } else {
                    self.file_object_read(object, silent, add, false, line)?
                }
            };
            let empty = data.borrow().current.as_ref().is_some_and(Vec::is_empty);
            if !success || flags & 4 == 0 || !empty {
                return Ok(success);
            }
            clear(&mut data.borrow_mut());
        }
    }

    fn file_object_rewind(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<(), Error> {
        let resource = stream(object, line)?;
        let result = self.stream_operation(
            streams::Operation::Rewind,
            "SplFileObject::rewind",
            &[Some(resource)],
            line,
        )?;
        if !result.is_truthy() {
            return Err(self.array_iterator_exception(
                b"RuntimeException",
                "Cannot rewind file".into(),
                line,
            ));
        }
        let data = state(object);
        {
            let mut data = data.borrow_mut();
            clear(&mut data);
            data.index = 0;
        }
        if data.borrow().flags & 2 != 0 {
            self.file_object_read_current(object, true, line)?;
        }
        Ok(())
    }

    pub(super) fn native_file_object_method(
        &mut self,
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let function = self.current_function.clone().unwrap();
        let name = function.rsplit(|b| *b == b':').next().unwrap();
        if name == b"__construct" {
            return self.file_object_open(
                &object,
                function.starts_with(b"spltempfileobject"),
                supplied,
                line,
            );
        }
        let data = state(&object);
        match name {
            b"getflags" => return Ok(Value::Int(data.borrow().flags & 15)),
            b"setflags" => {
                data.borrow_mut().flags = int(self.lookup(b"flags").unwrap());
                return Ok(Value::Null);
            }
            b"getmaxlinelen" => return Ok(Value::Int(data.borrow().max_length)),
            b"setmaxlinelen" => {
                let n = int(self.lookup(b"maxLength").unwrap());
                if n < 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "SplFileObject::setMaxLineLen(): Argument #1 ($maxLength) must be greater than or equal to 0",
                        line,
                    ));
                }
                data.borrow_mut().max_length = n;
                return Ok(Value::Null);
            }
            b"key" => return Ok(Value::Int(data.borrow().index)),
            b"haschildren" => return Ok(Value::Bool(false)),
            b"getchildren" => return Ok(Value::Null),
            b"valid" => {
                let data = data.borrow();
                return Ok(Value::Bool(if data.flags & 2 != 0 {
                    data.current.is_some() || data.csv.is_some()
                } else {
                    data.stream.as_ref().is_some_and(|s| !eof(s))
                }));
            }
            b"setcsvcontrol" | b"getcsvcontrol" | b"fgetcsv" | b"fputcsv" => {
                return self.file_object_csv(&object, name, supplied, line);
            }
            _ => {}
        }
        let resource = stream(&object, line)?;
        match name {
            b"rewind" => {
                self.file_object_rewind(&object, line)?;
                Ok(Value::Null)
            }
            b"next" => {
                clear(&mut data.borrow_mut());
                if data.borrow().flags & 2 != 0 {
                    self.file_object_read_current(&object, true, line)?;
                }
                let n = data.borrow().index;
                data.borrow_mut().index = n.wrapping_add(1);
                Ok(Value::Null)
            }
            b"fgets" | b"getcurrentline" => {
                self.file_object_read(&object, false, 1, false, line)?;
                Ok(Value::String(data.borrow().current.clone().unwrap().into()))
            }
            b"current" | b"__tostring" => {
                if data.borrow().current.is_none() && data.borrow().csv.is_none() {
                    self.file_object_read_current(&object, name != b"__tostring", line)?;
                }
                let data = data.borrow();
                if name == b"current"
                    && (data.current.is_none() || data.flags & 8 != 0)
                    && let Some(csv) = &data.csv
                {
                    return Ok(csv.clone());
                }
                Ok(data
                    .current
                    .clone()
                    .map_or(Value::Bool(false), |text| Value::String(text.into())))
            }
            b"seek" => {
                let position = int(self.lookup(b"line").unwrap());
                if position < 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "SplFileObject::seek(): Argument #1 ($line) must be greater than or equal to 0",
                        line,
                    ));
                }
                self.file_object_rewind(&object, line)?;
                for _ in 0..position {
                    if !self.file_object_read_current(&object, true, line)? {
                        return Ok(Value::Null);
                    }
                }
                if position > 0 && data.borrow().flags & 2 == 0 {
                    let mut data = data.borrow_mut();
                    data.index = data.index.wrapping_add(1);
                    clear(&mut data);
                }
                Ok(Value::Null)
            }
            b"fpassthru" => self.stream_passthru(&resource, "SplFileObject::fpassthru", line),
            b"fscanf" => {
                let add = i64::from(data.borrow().current.is_some());
                self.file_object_read(&object, false, add, false, line)?;
                let text = Value::String(data.borrow().current.clone().unwrap().into());
                let Value::Array(vars) = self.lookup(b"vars").unwrap() else {
                    unreachable!()
                };
                self.scanf(
                    false,
                    "SplFileObject::fscanf",
                    &[Some(text), self.lookup(b"format")],
                    (*vars).clone(),
                    line,
                )
            }
            b"flock" => self.file_object_lock(&resource, line),
            _ => {
                let (operation, canonical, args) = match name {
                    b"eof" => (streams::Operation::Eof, "eof", vec![]),
                    b"ftell" => (streams::Operation::Tell, "ftell", vec![]),
                    b"fflush" => (streams::Operation::Flush, "fflush", vec![]),
                    b"fstat" => (streams::Operation::Stat, "fstat", vec![]),
                    b"fread" => (
                        streams::Operation::Read,
                        "fread",
                        vec![self.lookup(b"length")],
                    ),
                    b"fwrite" => (
                        streams::Operation::Write,
                        "fwrite",
                        vec![self.lookup(b"data"), self.lookup(b"length")],
                    ),
                    b"ftruncate" => (
                        streams::Operation::Truncate,
                        "ftruncate",
                        vec![self.lookup(b"size")],
                    ),
                    b"fseek" => {
                        clear(&mut data.borrow_mut());
                        (
                            streams::Operation::Seek,
                            "fseek",
                            vec![self.lookup(b"offset"), self.lookup(b"whence")],
                        )
                    }
                    b"fgetc" => {
                        clear(&mut data.borrow_mut());
                        (streams::Operation::GetChar, "fgetc", vec![])
                    }
                    _ => unreachable!("file method"),
                };
                let mut arguments = vec![Some(resource)];
                arguments.extend(args);
                let result = self
                    .stream_operation(
                        operation,
                        &format!("SplFileObject::{canonical}"),
                        &arguments,
                        line,
                    )
                    .map_err(|mut error| {
                        error.message = error
                            .message
                            .replace("Argument #2", "Argument #1")
                            .replace("Argument #3", "Argument #2");
                        error
                    })?;
                if name == b"fgetc" && matches!(&result, Value::String(s) if s.to_vec() == b"\n") {
                    let n = data.borrow().index;
                    data.borrow_mut().index = n.wrapping_add(1);
                }
                Ok(result)
            }
        }
    }

    fn file_object_csv(
        &mut self,
        object: &Rc<PhpObject>,
        method: &[u8],
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let data = state(object);
        if method == b"getcsvcontrol" {
            let data = data.borrow();
            let mut result = PhpArray::default();
            for bytes in [
                vec![data.separator],
                vec![data.enclosure],
                data.escape.into_iter().collect(),
            ] {
                result.append(Value::String(bytes.into()), line)?;
            }
            return Ok(Value::Array(Rc::new(result)));
        }
        let write = method == b"fputcsv";
        let set = method == b"setcsvcontrol";
        let name = if set {
            "SplFileObject::setCsvControl"
        } else if write {
            "SplFileObject::fputcsv"
        } else {
            "SplFileObject::fgetcsv"
        };
        let offset = usize::from(write);
        let (mut separator, mut enclosure, mut escape, default) = {
            let d = data.borrow();
            if set {
                (b',', b'"', Some(b'\\'), true)
            } else {
                (d.separator, d.enclosure, d.escape, d.escape_default)
            }
        };
        for (index, parameter, dest) in [
            (1 + offset, b"separator".as_slice(), &mut separator),
            (2 + offset, b"enclosure".as_slice(), &mut enclosure),
        ] {
            if supplied >= index {
                let bytes = self.lookup(parameter).unwrap().to_bytes();
                if bytes.len() != 1 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        format!(
                            "{name}(): Argument #{index} (${}) must be a single character",
                            String::from_utf8_lossy(parameter)
                        ),
                        line,
                    ));
                }
                *dest = bytes[0];
            }
        }
        if supplied >= 3 + offset {
            let bytes = self.lookup(b"escape").unwrap().to_bytes();
            if bytes.len() > 1 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!(
                        "{name}(): Argument #{} ($escape) must be empty or a single character",
                        3 + offset
                    ),
                    line,
                ));
            }
            escape = bytes.first().copied();
        } else if default {
            self.warn(format!("Deprecated: {name}(): the $escape parameter must be provided{} on line {line}", if set { " as its default value will change" } else { ", as its default value will change, either explicitly or via SplFileObject::setCsvControl()" }))?;
        }
        if set {
            let mut data = data.borrow_mut();
            data.separator = separator;
            data.enclosure = enclosure;
            data.escape = escape;
            if supplied >= 3 {
                data.escape_default = false;
            }
            return Ok(Value::Null);
        }
        let resource = stream(object, line)?;
        if !write {
            if !self.file_object_read_csv(object, (separator, enclosure, escape), true, line)? {
                return Ok(Value::Bool(false));
            }
            return Ok(data.borrow().csv.clone().unwrap());
        }
        self.stream_csv(
            true,
            name,
            &[
                Some(resource),
                self.lookup(b"fields"),
                Some(Value::String(vec![separator].into())),
                Some(Value::String(vec![enclosure].into())),
                Some(Value::String(escape.into_iter().collect::<Vec<_>>().into())),
                self.lookup(b"eol"),
            ],
            line,
        )
    }

    fn file_object_lock(&mut self, resource: &Value, line: usize) -> Result<Value, Error> {
        let operation = int(self.lookup(b"operation").unwrap());
        if !matches!(operation & 3, 1..=3) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "SplFileObject::flock(): Argument #1 ($operation) must be one of LOCK_SH, LOCK_EX, or LOCK_UN",
                line,
            ));
        }
        let Value::Resource(resource) = resource else {
            unreachable!()
        };
        let reference = self.scope.get(b"wouldBlock").cloned();
        let mut pending = None;
        if let Some(reference) = &reference {
            self.assign_native_reference(reference, Value::Int(0), &mut pending, line)?;
        }
        let result = lock_resource(resource, operation);
        if let Some(reference) = &reference
            && result
                .as_ref()
                .is_err_and(|e| e.kind() == std::io::ErrorKind::WouldBlock)
        {
            self.assign_native_reference(reference, Value::Int(1), &mut pending, line)?;
        }
        pending.map_or(Ok(Value::Bool(result.is_ok())), Err)
    }
}

fn lock_resource(resource: &crate::PhpResource, operation: i64) -> std::io::Result<()> {
    use crate::resource::Backend;
    let kind = resource.kind.borrow();
    let Kind::Stream(stream) = &*kind else {
        return Err(std::io::ErrorKind::InvalidInput.into());
    };
    let file = match &stream.backend {
        Backend::File(file) => file,
        Backend::StandardInput(reader) => reader.get_ref(),
        Backend::Temporary(inner, _) => return lock_resource(inner, operation),
        _ => return Err(std::io::ErrorKind::Unsupported.into()),
    };
    if operation & 3 == 3 {
        file.unlock()
    } else if operation & 4 != 0 {
        (if operation & 3 == 1 {
            file.try_lock_shared()
        } else {
            file.try_lock()
        })
        .map_err(|e| match e {
            std::fs::TryLockError::WouldBlock => std::io::ErrorKind::WouldBlock.into(),
            std::fs::TryLockError::Error(e) => e,
        })
    } else if operation & 3 == 1 {
        file.lock_shared()
    } else {
        file.lock()
    }
}

const FILE_METHODS: &str = r#"public const int DROP_NEW_LINE=1, READ_AHEAD=2, SKIP_EMPTY=4, READ_CSV=8;
public function __construct(string $filename, string $mode = "r", bool $useIncludePath = false, $context = null) {}
public function rewind(): void {}
public function eof(): bool {}
public function valid(): bool {}
public function fgets(): string {}
public function fread(int $length): string|false {}
public function fgetcsv(string $separator = ",", string $enclosure = "\"", string $escape = "\\"): array|false {}
public function fputcsv(array $fields, string $separator = ",", string $enclosure = "\"", string $escape = "\\", string $eol = "\n"): int|false {}
public function setCsvControl(string $separator = ",", string $enclosure = "\"", string $escape = "\\"): void {}
public function getCsvControl(): array {}
public function flock(int $operation, &$wouldBlock = null): bool {}
public function fflush(): bool {}
public function ftell(): int|false {}
public function fseek(int $offset, int $whence = SEEK_SET): int {}
public function fgetc(): string|false {}
public function fpassthru(): int {}
public function fscanf(string $format, mixed &...$vars): array|int|null {}
public function fwrite(string $data, ?int $length = null): int|false {}
public function fstat(): array {}
public function ftruncate(int $size): bool {}
public function current(): string|array|false {}
public function key(): int {}
public function next(): void {}
public function setFlags(int $flags): void {}
public function getFlags(): int {}
public function setMaxLineLen(int $maxLength): void {}
public function getMaxLineLen(): int {}
public function hasChildren(): false {}
public function getChildren(): null {}
public function seek(int $line): void {}
public function getCurrentLine(): string {}
public function __toString(): string {}"#;
