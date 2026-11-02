// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_directory.c and .stub.php.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    PhpObject,
    ast::NativeMethod,
    native_directory::Directory,
    object::Class,
    resource::{Backend, Kind, Stream},
};

#[derive(Default)]
pub(crate) struct DirectoryData {
    pub active: bool,
    pub handle: Option<Rc<crate::PhpResource>>,
    pub entry: Vec<u8>,
    pub entry_type: u8,
    pub index: i64,
    pub flags: i64,
    pub sub_path: Option<Vec<u8>>,
}
pub(super) fn state(object: &PhpObject) -> Rc<RefCell<DirectoryData>> {
    object
        .file_info
        .borrow_mut()
        .as_mut()
        .unwrap()
        .directory
        .get_or_insert_with(|| Rc::new(RefCell::new(DirectoryData::default())))
        .clone()
}
pub(super) fn native_iteration(object: &PhpObject) -> bool {
    object.class.is_a(b"DirectoryIterator")
        && [b"rewind".as_slice(), b"valid", b"current", b"key", b"next"]
            .iter()
            .all(|name| {
                objects::method(&object.class, name)
                    .is_some_and(|m| matches!(m.native, Some(NativeMethod::DirectoryIterator)))
            })
}
pub(super) fn has_current(object: &PhpObject) -> bool {
    !state(object).borrow().entry.is_empty()
}
pub(super) fn caches_current(object: &PhpObject) -> bool {
    object.class.is_a(b"FilesystemIterator")
        && matches!(state(object).borrow().flags & 0xf0, 0 | 0x20)
}
fn read(object: &PhpObject, skip: bool) {
    object.file_info.borrow_mut().as_mut().unwrap().filename = None;
    let state = state(object);
    let mut data = state.borrow_mut();
    loop {
        let next = data.handle.as_ref().and_then(|resource| {
            let mut kind = resource.kind.borrow_mut();
            let Kind::Stream(stream) = &mut *kind else {
                return None;
            };
            let Backend::Directory(directory) = &mut stream.backend else {
                return None;
            };
            let next = directory.read();
            if next.is_some() {
                stream.position = stream
                    .position
                    .wrapping_add(crate::native_directory::RECORD_SIZE as u64);
            }
            next
        });
        (data.entry, data.entry_type) = next.unwrap_or_default();
        if !skip || !matches!(data.entry.as_slice(), b"." | b"..") {
            break;
        }
    }
}
fn rewind_handle(object: &PhpObject) {
    let state = state(object);
    let mut data = state.borrow_mut();
    data.index = 0;
    if let Some(handle) = &data.handle {
        let mut kind = handle.kind.borrow_mut();
        if let Kind::Stream(stream) = &mut *kind
            && let Backend::Directory(directory) = &mut stream.backend
        {
            directory.rewind();
            stream.position = 0;
            stream.eof = false;
        }
    }
}

impl Runtime {
    pub(super) fn builtin_directory_iterator_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, parent, interface, methods) = match name {
            b"directoryiterator" => (
                "DirectoryIterator",
                "SplFileInfo",
                Some("SeekableIterator"),
                r#"
                public function __construct(string $directory) {}
                public function getFilename(): string {}
                public function getExtension(): string {}
                public function getBasename(string $suffix = ""): string {}
                public function isDot(): bool {}
                public function rewind(): void {}
                public function valid(): bool {}
                public function key(): mixed {}
                public function current(): mixed {}
                public function next(): void {}
                public function seek(int $offset): void {}
                public function __toString(): string {}
            "#,
            ),
            b"filesystemiterator" => (
                "FilesystemIterator",
                "DirectoryIterator",
                None,
                r#"
                public const int CURRENT_MODE_MASK=240, CURRENT_AS_PATHNAME=32, CURRENT_AS_FILEINFO=0, CURRENT_AS_SELF=16,
                    KEY_MODE_MASK=3840, KEY_AS_PATHNAME=0, FOLLOW_SYMLINKS=16384, KEY_AS_FILENAME=256, NEW_CURRENT_AND_KEY=256,
                    OTHER_MODE_MASK=28672, SKIP_DOTS=4096, UNIX_PATHS=8192;
                public function __construct(string $directory, int $flags = 4096) {}
                public function rewind(): void {}
                public function key(): string {}
                public function current(): string|SplFileInfo|FilesystemIterator {}
                public function getFlags(): int {}
                public function setFlags(int $flags): void {}
            "#,
            ),
            b"recursivedirectoryiterator" => (
                "RecursiveDirectoryIterator",
                "FilesystemIterator",
                Some("RecursiveIterator"),
                r#"
                public function __construct(string $directory, int $flags = 0) {}
                public function hasChildren(bool $allowLinks = false): bool {}
                public function getChildren(): RecursiveDirectoryIterator {}
                public function getSubPath(): string {}
                public function getSubPathname(): string {}
            "#,
            ),
            _ => return None,
        };
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let code = format!(
            "<?php class {canonical} extends {parent} {} {{ {methods} }}",
            interface.map_or(String::new(), |i| format!("implements {i}"))
        );
        let (mut statements, _, _) = parser::parse(lexer::lex(code.as_bytes()).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::DirectoryIterator);
            if !method.name.ends_with(b"::__construct") && !method.name.ends_with(b"::__toString") {
                method.tentative_return_type = method.return_type.take();
            }
            for parameter in &mut method.parameters {
                if parameter.name == b"flags" && parameter.default.is_some() {
                    let constant = |name: &[u8]| Expr {
                        line: 0,
                        kind: ExprKind::ScopedMember(
                            Box::new(Expr {
                                line: 0,
                                kind: ExprKind::Constant(b"FilesystemIterator".to_vec()),
                            }),
                            Box::new(Expr {
                                line: 0,
                                kind: ExprKind::Literal(Value::String(name.to_vec().into())),
                            }),
                        ),
                    };
                    let mut default = Expr {
                        line: 0,
                        kind: ExprKind::Binary(
                            "|".into(),
                            Box::new(constant(b"KEY_AS_PATHNAME")),
                            Box::new(constant(b"CURRENT_AS_FILEINFO")),
                        ),
                    };
                    if name == b"filesystemiterator" {
                        default = Expr {
                            line: 0,
                            kind: ExprKind::Binary(
                                "|".into(),
                                Box::new(default),
                                Box::new(constant(b"SKIP_DOTS")),
                            ),
                        };
                    }
                    parameter.default = Some(default);
                }
            }
        }
        let parent = self.find_class(parent.as_bytes()).unwrap();
        let mut methods = declaration.methods.clone();
        let names: Vec<_> = methods
            .iter()
            .map(|m| {
                m.name
                    .rsplit(|b| *b == b':')
                    .next()
                    .unwrap()
                    .to_ascii_lowercase()
            })
            .collect();
        methods.extend(
            parent
                .methods
                .iter()
                .filter(|m| {
                    !names.contains(
                        &m.name
                            .rsplit(|b| *b == b':')
                            .next()
                            .unwrap()
                            .to_ascii_lowercase(),
                    )
                })
                .cloned(),
        );
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods,
            declaration,
            parent: Some(parent),
            interfaces: interface
                .map(|i| vec![self.find_class(i.as_bytes()).unwrap()])
                .unwrap_or_default(),
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

    fn directory_open(
        &mut self,
        object: &Rc<PhpObject>,
        path: Vec<u8>,
        flags: i64,
        method: &str,
        line: usize,
    ) -> Result<(), Error> {
        let mut stored_path = path.clone();
        if stored_path.len() > 1 && stored_path.ends_with(b"/") {
            stored_path.pop();
        }
        {
            let mut info = object.file_info.borrow_mut();
            let info = info.as_mut().unwrap();
            info.path = stored_path;
            info.filename = None;
        }
        let state = state(object);
        {
            let mut data = state.borrow_mut();
            data.active = true;
            data.flags = flags;
            data.index = 0;
            data.entry.clear();
        }
        self.resources.default_context();
        let resolved = self
            .working_directory()
            .join(crate::source::path_from_bytes(
                path.strip_prefix(b"file://").unwrap_or(&path),
            ));
        let directory = Directory::open(&resolved).map_err(|error| {
            self.array_iterator_exception(
                b"UnexpectedValueException",
                format!(
                    "{method}({}): Failed to open directory: {}",
                    String::from_utf8_lossy(&path),
                    streams::io_message(&error)
                ),
                line,
            )
        })?;
        state.borrow_mut().handle = Some(self.resources.allocate(Kind::Stream(Stream {
            backend: Backend::Directory(directory),
            writable: false,
            eof: false,
            position: 0,
        })));
        read(object, flags & 0x1000 != 0);
        Ok(())
    }
    pub(super) fn clone_directory_iterator(
        &mut self,
        original: &PhpObject,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<(), Error> {
        let original_data = original.file_info.borrow();
        let Some(info) = original_data.as_ref() else {
            return Ok(());
        };
        let Some(directory) = &info.directory else {
            return Ok(());
        };
        let data = directory.borrow();
        if !data.active {
            object.file_info.borrow_mut().as_mut().unwrap().directory =
                Some(Rc::new(RefCell::new(DirectoryData {
                    flags: data.flags,
                    ..Default::default()
                })));
            return Ok(());
        }
        let (path, flags, index) = (info.path.clone(), data.flags, data.index);
        drop(data);
        drop(original_data);
        object.file_info.borrow_mut().as_mut().unwrap().directory =
            Some(Rc::new(RefCell::new(DirectoryData::default())));
        self.directory_open(object, path, flags, "clone", line)?;
        for _ in 0..index {
            read(object, flags & 0x1000 != 0);
        }
        state(object).borrow_mut().index = index;
        Ok(())
    }
    pub(super) fn directory_iterator_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        tree: bool,
        native: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let state = state(object);
        if !native && !tree && state.borrow().handle.is_none() {
            return Err(Error::new(ErrorKind::Error, "Object not initialized", line));
        }
        match name {
            b"rewind" => {
                rewind_handle(object);
                let skip = tree && state.borrow().flags & 0x1000 != 0;
                read(object, skip);
                Ok(Value::Null)
            }
            b"next" => {
                let skip = (!native || tree) && state.borrow().flags & 0x1000 != 0;
                let index = state.borrow().index;
                state.borrow_mut().index = index.wrapping_add(1);
                read(object, skip);
                Ok(Value::Null)
            }
            b"valid" => Ok(Value::Bool(!state.borrow().entry.is_empty())),
            b"key" if !tree => Ok(Value::Int(state.borrow().index)),
            b"key" => {
                if state.borrow().flags & 0xf00 == 0x100 {
                    Ok(Value::String(state.borrow().entry.clone().into()))
                } else {
                    self.directory_filename(object, line)
                        .map(|v| Value::String(v.into()))
                }
            }
            b"current" if !tree => Ok(Value::Object(object.clone())),
            b"current" => {
                let flags = state.borrow().flags;
                match flags & 0xf0 {
                    0x20 => self
                        .directory_filename(object, line)
                        .map(|v| Value::String(v.into())),
                    0 => self.directory_current_info(object, line),
                    _ => Ok(Value::Object(object.clone())),
                }
            }
            _ => unreachable!(),
        }
    }
    pub(super) fn directory_filename(
        &mut self,
        object: &PhpObject,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        let mut data = object.file_info.borrow_mut();
        let data = data.as_mut().unwrap();
        data.ensure_filename()
            .ok_or_else(|| Error::new(ErrorKind::Error, "Object not initialized", line))
    }
    fn directory_current_info(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let filename = self.directory_filename(object, line)?;
        if state(object).borrow().entry.is_empty() {
            return Err(self.array_iterator_exception(
                b"RuntimeException",
                "Could not open file".into(),
                line,
            ));
        }
        let class = object
            .file_info
            .borrow()
            .as_ref()
            .unwrap()
            .info_class
            .clone()
            .unwrap_or_else(|| self.find_class(b"SplFileInfo").unwrap());
        self.create_file_info(class, filename, line)
    }
    pub(super) fn native_directory_iterator_method(&mut self, line: usize) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let function = self.current_function.clone().unwrap();
        let name = function.rsplit(|b| *b == b':').next().unwrap();
        let tree = function.starts_with(b"filesystemiterator::");
        let state = state(&object);
        if name == b"__construct" {
            let Value::String(path) = self.lookup(b"directory").unwrap() else {
                unreachable!()
            };
            let path = path.to_vec();
            let canonical = if tree {
                "FilesystemIterator::__construct"
            } else if function.starts_with(b"recursivedirectoryiterator::") {
                "RecursiveDirectoryIterator::__construct"
            } else {
                "DirectoryIterator::__construct"
            };
            if path.is_empty() || path.contains(&0) {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    format!(
                        "{canonical}(): Argument #1 ($directory) must {}",
                        if path.is_empty() {
                            "not be empty"
                        } else {
                            "not contain any null bytes"
                        }
                    ),
                    line,
                ));
            }
            if state.borrow().active
                || object
                    .file_info
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .filename
                    .is_some()
            {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Directory object is already initialized",
                    line,
                ));
            }
            let flags = match self.lookup(b"flags") {
                Some(Value::Int(flags)) => flags,
                _ => 0x10,
            };
            self.directory_open(&object, path, flags, canonical, line)?;
            return Ok(Value::Null);
        }
        match name {
            b"getflags" => Ok(Value::Int(state.borrow().flags & 0x7ff0)),
            b"setflags" => {
                let Value::Int(flags) = self.lookup(b"flags").unwrap() else {
                    unreachable!()
                };
                let old = state.borrow().flags;
                state.borrow_mut().flags = (old & !0x7ff0) | (flags & 0x7ff0);
                Ok(Value::Null)
            }
            b"getsubpath" => Ok(Value::String(
                state.borrow().sub_path.clone().unwrap_or_default().into(),
            )),
            b"getsubpathname" => {
                let data = state.borrow();
                let mut path = data.sub_path.clone().unwrap_or_default();
                if data.sub_path.is_some() {
                    path.push(b'/');
                }
                path.extend_from_slice(&data.entry);
                Ok(Value::String(path.into()))
            }
            b"haschildren" => {
                let (entry, kind, flags) = {
                    let data = state.borrow();
                    (data.entry.clone(), data.entry_type, data.flags)
                };
                if entry.is_empty() || matches!(entry.as_slice(), b"." | b"..") {
                    return Ok(Value::Bool(false));
                }
                if kind == 4 {
                    return Ok(Value::Bool(true));
                }
                if kind == 8 {
                    return Ok(Value::Bool(false));
                }
                let filename = self.directory_filename(&object, line)?;
                let link = self.file_info_stat(&filename, b"islink", line)?.is_truthy();
                let follow = self.lookup(b"allowLinks").is_some_and(|v| v.is_truthy())
                    || flags & 0x4000 != 0;
                if link && !follow {
                    return Ok(Value::Bool(false));
                }
                self.file_info_stat(&filename, b"isdir", line)
            }
            b"getchildren" => {
                let filename = self.directory_filename(&object, line)?;
                let flags = state.borrow().flags;
                let args = [Value::String(filename.into()), Value::Int(flags)];
                let args: Vec<_> = args
                    .into_iter()
                    .map(|v| {
                        crate::ast::Argument::Positional(Expr {
                            line,
                            kind: ExprKind::Literal(v),
                        })
                    })
                    .collect();
                let Value::Object(child) =
                    self.instantiate_class(object.class.clone(), &args, true, line)?
                else {
                    unreachable!()
                };
                let sub = {
                    let data = state.borrow();
                    let mut sub = data.sub_path.clone().unwrap_or_default();
                    if !sub.is_empty() {
                        sub.push(b'/');
                    }
                    sub.extend_from_slice(&data.entry);
                    sub
                };
                state_for_child(&child).borrow_mut().sub_path = Some(sub);
                {
                    let info = object.file_info.borrow();
                    let info = info.as_ref().unwrap();
                    let mut dest = child.file_info.borrow_mut();
                    let dest = dest.as_mut().unwrap();
                    dest.info_class = info.info_class.clone();
                    dest.file_class = info.file_class.clone();
                }
                Ok(Value::Object(child))
            }
            b"seek" => {
                if state.borrow().handle.is_none() {
                    return Err(Error::new(ErrorKind::Error, "Object not initialized", line));
                }
                let Value::Int(offset) = self.lookup(b"offset").unwrap() else {
                    unreachable!()
                };
                if state.borrow().index > offset {
                    self.directory_call(&object, b"rewind", line)?;
                }
                while state.borrow().index < offset {
                    if !self.directory_call(&object, b"valid", line)?.is_truthy() {
                        return Err(self.array_iterator_exception(
                            b"OutOfBoundsException",
                            format!("Seek position {offset} is out of range"),
                            line,
                        ));
                    }
                    self.directory_call(&object, b"next", line)?;
                }
                Ok(Value::Null)
            }
            b"getfilename" | b"__tostring" | b"getextension" | b"getbasename" | b"isdot" => {
                if state.borrow().handle.is_none() {
                    return Err(Error::new(ErrorKind::Error, "Object not initialized", line));
                }
                let mut entry = state.borrow().entry.clone();
                if name == b"isdot" {
                    return Ok(Value::Bool(matches!(entry.as_slice(), b"." | b"..")));
                }
                if name == b"getextension" {
                    entry = entry
                        .iter()
                        .rposition(|b| *b == b'.')
                        .map_or_else(Vec::new, |i| entry[i + 1..].to_vec());
                }
                if name == b"getbasename" {
                    let Value::String(suffix) = self.lookup(b"suffix").unwrap() else {
                        unreachable!()
                    };
                    let suffix = suffix.to_vec();
                    if suffix.len() < entry.len() && entry.ends_with(&suffix) {
                        entry.truncate(entry.len() - suffix.len());
                    }
                }
                Ok(Value::String(entry.into()))
            }
            _ => self.directory_iterator_step(&object, name, tree, false, line),
        }
    }
    fn directory_call(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let method = objects::method(&object.class, name).unwrap();
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        self.invoke_at(target, &[], line, false, true)
    }
}
fn state_for_child(object: &PhpObject) -> Rc<RefCell<DirectoryData>> {
    state(object)
}
