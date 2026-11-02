// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_directory.c and .stub.php,
// with metadata behavior from ext/standard/filestat.c. See THIRD_PARTY_NOTICES.md.
// Copyright (c) The PHP Group.
use super::*;
use crate::{PhpObject, ast::NativeMethod, object::Class};

#[derive(Clone, Default)]
pub(crate) struct FileInfoData {
    pub(super) directory: Option<Rc<RefCell<directory_iterator::DirectoryData>>>,
    pub(super) filename: Option<Vec<u8>>,
    pub(super) path: Vec<u8>,
    pub(super) info_class: Option<Rc<Class>>,
    pub(super) file_class: Option<Rc<Class>>,
}
impl FileInfoData {
    pub(super) fn set_filename(&mut self, mut filename: Vec<u8>) {
        while filename.len() > 1 && filename.ends_with(b"/") {
            filename.pop();
        }
        let mut end = filename.len();
        while end > 1 && filename[end - 1] != b'/' {
            end -= 1;
        }
        end = end.saturating_sub(1);
        self.path = filename[..end].to_vec();
        self.filename = Some(filename);
    }
    pub(super) fn ensure_filename(&mut self) -> Option<Vec<u8>> {
        if self.filename.is_none()
            && let Some(directory) = &self.directory
            && directory.borrow().active
        {
            let mut filename = self.effective_path();
            if !filename.is_empty() {
                filename.push(b'/');
            }
            filename.extend_from_slice(&directory.borrow().entry);
            self.filename = Some(filename);
        }
        self.filename.clone()
    }
    fn pathname(&mut self) -> Option<Vec<u8>> {
        if let Some(directory) = &self.directory
            && directory.borrow().active
        {
            if directory.borrow().entry.is_empty() {
                return None;
            }
            return self.ensure_filename();
        }
        self.filename.clone()
    }
    fn effective_path(&self) -> Vec<u8> {
        self.directory
            .as_ref()
            .and_then(|directory| directory_iterator::glob_path(&directory.borrow()))
            .unwrap_or_else(|| self.path.clone())
    }
    fn basename(&self) -> Option<&[u8]> {
        self.filename.as_deref().map(|name| {
            let path = self.effective_path();
            if !path.is_empty() && path.len() < name.len() {
                &name[path.len() + 1..]
            } else {
                name
            }
        })
    }
}

impl Runtime {
    pub(super) fn builtin_file_info_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        if name != b"splfileinfo" {
            return None;
        }
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let code = br#"<?php class SplFileInfo implements Stringable {
            public function __construct(string $filename) {}
            public function getPath(): string {}
            public function getFilename(): string {}
            public function getExtension(): string {}
            public function getBasename(string $suffix = ""): string {}
            public function getPathname(): string {}
            public function getPerms(): int|false {}
            public function getInode(): int|false {}
            public function getSize(): int|false {}
            public function getOwner(): int|false {}
            public function getGroup(): int|false {}
            public function getATime(): int|false {}
            public function getMTime(): int|false {}
            public function getCTime(): int|false {}
            public function getType(): string|false {}
            public function isWritable(): bool {}
            public function isReadable(): bool {}
            public function isExecutable(): bool {}
            public function isFile(): bool {}
            public function isDir(): bool {}
            public function isLink(): bool {}
            public function getLinkTarget(): string|false {}
            public function getRealPath(): string|false {}
            public function getFileInfo(?string $class = null): SplFileInfo {}
            public function getPathInfo(?string $class = null): ?SplFileInfo {}
            public function openFile(string $mode = "r", bool $useIncludePath = false, $context = null): SplFileObject {}
            public function setFileClass(string $class = "SplFileObject"): void {}
            public function setInfoClass(string $class = "SplFileInfo"): void {}
            public function __toString(): string {}
            public function __debugInfo(): array {}
            #[Deprecated(since: '8.2')] final public function _bad_state_ex(): void {}
        }"#;
        let (mut statements, _, _) = parser::parse(lexer::lex(code).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::FileInfo);
            let name = method.name.rsplit(|b| *b == b':').next().unwrap();
            if !matches!(name, b"__construct" | b"__toString" | b"_bad_state_ex") {
                method.tentative_return_type = method.return_type.take();
            }
            if matches!(name, b"setFileClass" | b"setInfoClass") {
                let class = if name == b"setFileClass" {
                    b"SplFileObject".as_slice()
                } else {
                    b"SplFileInfo"
                };
                method.parameters[0].default = Some(Expr {
                    line: 0,
                    kind: ExprKind::ClassName(Box::new(Expr {
                        line: 0,
                        kind: ExprKind::Constant(class.to_vec()),
                    })),
                });
            }
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            declaration,
            parent: None,
            interfaces: vec![self.find_class(b"Stringable").unwrap()],
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

    pub(super) fn native_file_info_method(&mut self, line: usize) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let function = self.current_function.clone().unwrap();
        let name = function.rsplit(|b| *b == b':').next().unwrap();
        if name == b"__construct" {
            let Value::String(filename) = self.lookup(b"filename").unwrap() else {
                unreachable!()
            };
            let filename = filename.to_vec();
            if filename.contains(&0) {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "SplFileInfo::__construct(): Argument #1 ($filename) must not contain any null bytes",
                    line,
                ));
            }
            object
                .file_info
                .borrow_mut()
                .as_mut()
                .unwrap()
                .set_filename(filename);
            return Ok(Value::Null);
        }
        if matches!(
            name,
            b"setfileclass" | b"setinfoclass" | b"getfileinfo" | b"getpathinfo" | b"openfile"
        ) {
            return self.file_info_factory(&object, name, line);
        }
        if name == b"_bad_state_ex" {
            return Err(Error::new(
                ErrorKind::Error,
                "The parent constructor was not called: the object is in an invalid state",
                line,
            ));
        }
        let pathname = {
            let mut payload = object.file_info.borrow_mut();
            let data = payload.as_mut().unwrap();
            if matches!(
                name,
                b"getpathname" | b"__tostring" | b"__debuginfo" | b"getrealpath"
            ) {
                data.pathname()
            } else if matches!(
                name,
                b"getpath" | b"getfilename" | b"getextension" | b"getbasename"
            ) {
                None
            } else {
                data.ensure_filename()
            }
        };
        let data = object.file_info.borrow().as_ref().unwrap().clone();
        match name {
            b"getpath" => Ok(Value::String(data.effective_path().into())),
            b"getpathname" | b"__tostring" => {
                Ok(Value::String(pathname.unwrap_or_default().into()))
            }
            b"__debuginfo" => {
                let mut props = object.snapshot();
                props.insert(
                    ArrayKey::String(b"\0SplFileInfo\0pathName".to_vec()),
                    Value::String(pathname.unwrap_or_default().into()),
                );
                if let Some(name) = data.basename() {
                    props.insert(
                        ArrayKey::String(b"\0SplFileInfo\0fileName".to_vec()),
                        Value::String(name.to_vec().into()),
                    );
                }
                if let Some(directory) = &data.directory
                    && directory.borrow().active
                {
                    props.insert(
                        ArrayKey::String(b"\0DirectoryIterator\0glob".to_vec()),
                        if directory_iterator::glob_path(&directory.borrow()).is_some() {
                            Value::String(data.path.clone().into())
                        } else {
                            Value::Bool(false)
                        },
                    );
                    props.insert(
                        ArrayKey::String(b"\0RecursiveDirectoryIterator\0subPathName".to_vec()),
                        Value::String(
                            directory
                                .borrow()
                                .sub_path
                                .clone()
                                .unwrap_or_default()
                                .into(),
                        ),
                    );
                }
                Ok(Value::Array(Rc::new(props)))
            }
            b"getrealpath" => {
                let Some(filename) = data.filename else {
                    return Ok(Value::Bool(false));
                };
                let path = self
                    .working_directory()
                    .join(crate::source::path_from_bytes(&filename));
                Ok(std::fs::canonicalize(path).map_or(Value::Bool(false), |p| {
                    Value::String(p.as_os_str().as_encoded_bytes().to_vec().into())
                }))
            }
            _ => {
                let Some(filename) = data.filename.as_ref() else {
                    return Err(Error::new(ErrorKind::Error, "Object not initialized", line));
                };
                match name {
                    b"getfilename" => Ok(Value::String(data.basename().unwrap().to_vec().into())),
                    b"getbasename" | b"getextension" => {
                        let mut base = path_parts::basename(data.basename().unwrap()).to_vec();
                        if name == b"getextension" {
                            base = base
                                .iter()
                                .rposition(|b| *b == b'.')
                                .map_or_else(Vec::new, |pos| base[pos + 1..].to_vec());
                        } else {
                            let Value::String(suffix) = self.lookup(b"suffix").unwrap() else {
                                unreachable!()
                            };
                            let suffix = suffix.to_vec();
                            if suffix.len() < base.len() && base.ends_with(&suffix) {
                                base.truncate(base.len() - suffix.len());
                            }
                        }
                        Ok(Value::String(base.into()))
                    }
                    b"getlinktarget" => {
                        let path = self
                            .working_directory()
                            .join(crate::source::path_from_bytes(filename));
                        std::fs::read_link(path)
                            .map(|p| {
                                Value::String(p.as_os_str().as_encoded_bytes().to_vec().into())
                            })
                            .map_err(|error| {
                                self.array_iterator_exception(
                                    b"RuntimeException",
                                    format!(
                                        "Unable to read link {}, error: {}",
                                        String::from_utf8_lossy(filename),
                                        streams::io_message(&error)
                                    ),
                                    line,
                                )
                            })
                    }
                    _ => self.file_info_stat(filename, name, line),
                }
            }
        }
    }

    fn file_info_factory(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let data = object.file_info.borrow().as_ref().unwrap().clone();
        let file = matches!(name, b"setfileclass" | b"openfile");
        let base = if file {
            b"SplFileObject".as_slice()
        } else {
            b"SplFileInfo"
        };
        let selected = if file {
            data.file_class
        } else {
            data.info_class
        };
        let canonical = match name {
            b"setfileclass" => "setFileClass",
            b"setinfoclass" => "setInfoClass",
            b"getfileinfo" => "getFileInfo",
            b"getpathinfo" => "getPathInfo",
            _ => "openFile",
        };
        let argument = self.lookup(b"class").unwrap_or(Value::Null);
        let class = if let Value::String(value) = argument {
            let bytes = value.to_vec();
            let class = self.load_class(&bytes, line)?;
            let required = if name == b"getfileinfo" {
                selected.as_ref().map_or(base, |c| &c.name)
            } else {
                base
            };
            if class.as_ref().is_none_or(|class| !class.is_a(required)) {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    format!(
                        "SplFileInfo::{canonical}(): Argument #1 ($class) must be a class name derived from {}{}, {} given",
                        String::from_utf8_lossy(required),
                        if matches!(name, b"getfileinfo" | b"getpathinfo") {
                            " or null"
                        } else {
                            ""
                        },
                        class.as_ref().map_or_else(
                            || String::from_utf8_lossy(&bytes).into_owned(),
                            |c| c.display_name().into_owned()
                        )
                    ),
                    line,
                ));
            }
            class.unwrap()
        } else {
            match selected {
                Some(class) => class,
                None => self.load_class(base, line)?.ok_or_else(|| {
                    Error::new(
                        ErrorKind::Error,
                        format!("Class \"{}\" not found", String::from_utf8_lossy(base)),
                        line,
                    )
                })?,
            }
        };
        if matches!(name, b"setfileclass" | b"setinfoclass") {
            let mut data = object.file_info.borrow_mut();
            let data = data.as_mut().unwrap();
            if file {
                data.file_class = Some(class);
            } else {
                data.info_class = Some(class);
            }
            return Ok(Value::Null);
        }
        let filename = if name == b"getpathinfo" {
            object.file_info.borrow_mut().as_mut().unwrap().pathname()
        } else {
            if let Some(directory) = &data.directory
                && directory.borrow().active
                && directory.borrow().entry.is_empty()
            {
                return Err(self.array_iterator_exception(
                    b"RuntimeException",
                    "Could not open file".into(),
                    line,
                ));
            }
            object
                .file_info
                .borrow_mut()
                .as_mut()
                .unwrap()
                .ensure_filename()
        };
        let Some(mut filename) = filename else {
            return if name == b"getpathinfo" {
                Ok(Value::Null)
            } else {
                Err(Error::new(ErrorKind::Error, "Object not initialized", line))
            };
        };
        if name == b"getpathinfo" {
            if filename.is_empty() {
                return Ok(Value::Null);
            }
            filename = path_parts::dirname(&filename);
        }
        if !file {
            return self.create_file_info(class, filename, line);
        }
        let result = self.allocate_class_object(class.clone(), line)?;
        let constructor = objects::method(&class, b"__construct").unwrap();
        let args = vec![
            Value::String(filename.into()),
            self.lookup(b"mode").unwrap(),
        ];
        let target =
            self.unchecked_method_target(constructor, class, Some(result.clone()), line)?;
        self.call_callback_at(&target, &args, line, true)?;
        Ok(Value::Object(result))
    }
    pub(super) fn create_file_info(
        &mut self,
        class: Rc<Class>,
        filename: Vec<u8>,
        line: usize,
    ) -> Result<Value, Error> {
        let result = self.allocate_class_object(class.clone(), line)?;
        let constructor = objects::method(&class, b"__construct").unwrap();
        if matches!(constructor.native, Some(NativeMethod::FileInfo)) {
            result
                .file_info
                .borrow_mut()
                .as_mut()
                .unwrap()
                .set_filename(filename);
        } else {
            let args = vec![Value::String(filename.into())];
            let target =
                self.unchecked_method_target(constructor, class, Some(result.clone()), line)?;
            self.call_callback_at(&target, &args, line, true)?;
        }
        Ok(Value::Object(result))
    }

    pub(super) fn file_info_stat(
        &mut self,
        filename: &[u8],
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        if filename.is_empty() {
            return Ok(Value::Bool(false));
        }
        let bytes = filename.strip_prefix(b"file://").unwrap_or(filename);
        let path = self
            .working_directory()
            .join(crate::source::path_from_bytes(bytes));
        if matches!(name, b"isreadable" | b"iswritable" | b"isexecutable") {
            return Ok(Value::Bool(native_access(
                &path,
                match name {
                    b"isreadable" => 4,
                    b"iswritable" => 2,
                    _ => 1,
                },
            )));
        }
        let link = matches!(name, b"gettype" | b"islink");
        let slot = usize::from(link);
        let cached = self.file_stat_cache[slot]
            .as_ref()
            .filter(|(key, _)| key == filename)
            .map(|(_, m)| m.clone());
        let metadata = cached.map(Ok).unwrap_or_else(|| {
            if link {
                std::fs::symlink_metadata(&path)
            } else {
                std::fs::metadata(&path)
            }
        });
        let metadata = match metadata {
            Ok(metadata) => metadata,
            Err(_) if name.starts_with(b"is") => return Ok(Value::Bool(false)),
            Err(_) => {
                let method = match name {
                    b"getperms" => "getPerms",
                    b"getinode" => "getInode",
                    b"getsize" => "getSize",
                    b"getowner" => "getOwner",
                    b"getgroup" => "getGroup",
                    b"getatime" => "getATime",
                    b"getmtime" => "getMTime",
                    b"getctime" => "getCTime",
                    _ => "getType",
                };
                return Err(self.array_iterator_exception(
                    b"RuntimeException",
                    format!(
                        "SplFileInfo::{method}(): {}stat failed for {}",
                        if link { "L" } else { "" },
                        String::from_utf8_lossy(filename)
                    ),
                    line,
                ));
            }
        };
        self.file_stat_cache[slot] = Some((filename.to_vec(), metadata.clone()));
        metadata_value(&metadata, name, line)
    }
}

#[cfg(unix)]
pub(super) fn native_access(path: &std::path::Path, mode: i32) -> bool {
    use std::{
        ffi::{CString, c_char, c_int},
        os::unix::ffi::OsStrExt,
    };
    unsafe extern "C" {
        fn access(path: *const c_char, mode: c_int) -> c_int;
    }
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: the CString remains live and NUL-terminated for this read-only call.
    unsafe { access(path.as_ptr(), mode) == 0 }
}
#[cfg(not(unix))]
pub(super) fn native_access(path: &std::path::Path, mode: i32) -> bool {
    path.metadata()
        .is_ok_and(|m| mode != 2 || !m.permissions().readonly())
}
pub(super) fn metadata_value(
    metadata: &std::fs::Metadata,
    name: &[u8],
    line: usize,
) -> Result<Value, Error> {
    if name == b"isfile" {
        return Ok(Value::Bool(metadata.is_file()));
    }
    if name == b"isdir" {
        return Ok(Value::Bool(metadata.is_dir()));
    }
    if name == b"islink" {
        return Ok(Value::Bool(metadata.file_type().is_symlink()));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let number = match name {
            b"getperms" => metadata.mode() as i64,
            b"getinode" => metadata.ino() as i64,
            b"getsize" => metadata.size() as i64,
            b"getowner" => metadata.uid() as i64,
            b"getgroup" => metadata.gid() as i64,
            b"getatime" => metadata.atime(),
            b"getmtime" => metadata.mtime(),
            b"getctime" => metadata.ctime(),
            b"gettype" => {
                let kind = match metadata.mode() & 0o170000 {
                    0o010000 => "fifo",
                    0o020000 => "char",
                    0o040000 => "dir",
                    0o060000 => "block",
                    0o100000 => "file",
                    0o120000 => "link",
                    0o140000 => "socket",
                    _ => "unknown",
                };
                return Ok(Value::String(kind.as_bytes().to_vec().into()));
            }
            _ => {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Unsupported filesystem metadata operation",
                    line,
                ));
            }
        };
        Ok(Value::Int(number))
    }
    #[cfg(not(unix))]
    {
        Err(Error::new(
            ErrorKind::Error,
            "Filesystem metadata is not supported on this platform",
            line,
        ))
    }
}
