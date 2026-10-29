use std::{
    cell::RefCell,
    fs::File,
    io::{self, BufReader, Cursor, Read, Seek, SeekFrom, Write},
    rc::Rc,
};

#[derive(Debug)]
pub struct PhpResource {
    pub(crate) id: i64,
    pub(crate) kind: RefCell<Kind>,
}

#[derive(Debug)]
pub(crate) enum Kind {
    Stream(Stream),
    Context,
    Closed,
}

#[derive(Debug)]
pub(crate) struct Stream {
    pub(crate) backend: Backend,
    pub(crate) writable: bool,
    pub(crate) eof: bool,
    pub(crate) position: u64,
}

#[derive(Debug)]
pub(crate) enum Backend {
    File(File),
    StandardInput(BufReader<File>),
    StandardOutput(bool),
    Memory(Cursor<Vec<u8>>),
    Temporary(Rc<PhpResource>),
}

impl Read for Backend {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => file.read(bytes),
            Self::StandardInput(file) => file.read(bytes),
            Self::StandardOutput(_) => Err(io::Error::from_raw_os_error(9)),
            Self::Memory(buffer) => buffer.read(bytes),
            Self::Temporary(resource) => resource.with_stream(|stream| {
                let count = stream.backend.read(bytes)?;
                stream.position += count as u64;
                stream.eof = count == 0;
                Ok(count)
            }),
        }
    }
}
impl Write for Backend {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => file.write(bytes),
            Self::StandardInput(file) => file.get_mut().write(bytes),
            // Captured standard output is emitted by the runtime, which also
            // preserves its order relative to diagnostics and ordinary echo.
            Self::StandardOutput(_) => Err(io::Error::from(io::ErrorKind::Unsupported)),
            Self::Memory(buffer) => {
                let end = usize::try_from(buffer.position())
                    .ok()
                    .and_then(|position| position.checked_add(bytes.len()))
                    .ok_or_else(|| io::Error::from(io::ErrorKind::OutOfMemory))?;
                if end > buffer.get_ref().len() {
                    let additional = end - buffer.get_ref().len();
                    buffer
                        .get_mut()
                        .try_reserve(additional)
                        .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
                }
                buffer.write(bytes)
            }
            Self::Temporary(resource) => resource.with_stream(|stream| {
                let count = stream.backend.write(bytes)?;
                stream.position += count as u64;
                Ok(count)
            }),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::File(file) => file.flush(),
            Self::StandardInput(file) => file.get_mut().flush(),
            Self::StandardOutput(_) => Ok(()),
            Self::Memory(buffer) => buffer.flush(),
            Self::Temporary(resource) => resource.with_stream(|stream| stream.backend.flush()),
        }
    }
}
impl Seek for Backend {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match self {
            Self::File(file) => file.seek(position),
            Self::StandardInput(file) => file.seek(position),
            Self::StandardOutput(_) => Err(io::Error::from_raw_os_error(29)),
            Self::Memory(buffer) => buffer.seek(position),
            Self::Temporary(resource) => resource.with_stream(|stream| {
                let position = stream.backend.seek(position)?;
                stream.position = position;
                stream.eof = false;
                Ok(position)
            }),
        }
    }
}

impl PhpResource {
    fn with_stream<T>(&self, action: impl FnOnce(&mut Stream) -> io::Result<T>) -> io::Result<T> {
        let mut kind = self.kind.borrow_mut();
        match &mut *kind {
            Kind::Stream(stream) => action(stream),
            _ => Err(io::Error::from_raw_os_error(9)),
        }
    }
    pub(crate) fn is_open(&self) -> bool {
        !matches!(*self.kind.borrow(), Kind::Closed)
    }
    pub(crate) fn type_name(&self) -> &'static str {
        match &*self.kind.borrow() {
            Kind::Stream(_) => "stream",
            Kind::Context => "stream-context",
            Kind::Closed => "Unknown",
        }
    }
    pub(crate) fn debug_type(&self) -> &'static str {
        match &*self.kind.borrow() {
            Kind::Stream(_) => "resource (stream)",
            Kind::Context => "resource (stream-context)",
            Kind::Closed => "resource (closed)",
        }
    }
}

pub(crate) struct Store {
    next_id: i64,
    default_context: Option<Rc<PhpResource>>,
    standard: Option<[Rc<PhpResource>; 3]>,
}
impl Default for Store {
    fn default() -> Self {
        let standard = (|| {
            let mut streams = Vec::new();
            for index in 0..3 {
                streams.push(standard_stream(index).ok()?);
            }
            Some(std::array::from_fn(|index| {
                Rc::new(PhpResource {
                    id: index as i64 + 1,
                    kind: RefCell::new(Kind::Stream(streams.remove(0))),
                })
            }))
        })();
        Self {
            next_id: if standard.is_some() { 4 } else { 1 },
            default_context: None,
            standard,
        }
    }
}
impl Store {
    pub(crate) fn standard(&self, name: &[u8]) -> Option<Rc<PhpResource>> {
        let index = match name {
            b"STDIN" => 0,
            b"STDOUT" => 1,
            b"STDERR" => 2,
            _ => return None,
        };
        self.standard.as_ref().map(|streams| streams[index].clone())
    }

    pub(crate) fn allocate(&mut self, kind: Kind) -> Rc<PhpResource> {
        let id = self.next_id;
        self.next_id += 1;
        Rc::new(PhpResource {
            id,
            kind: RefCell::new(kind),
        })
    }
    pub(crate) fn default_context(&mut self) -> Rc<PhpResource> {
        if self.default_context.is_none() {
            self.default_context = Some(self.allocate(Kind::Context));
        }
        self.default_context.as_ref().unwrap().clone()
    }
}

pub(crate) fn standard_stream(index: usize) -> io::Result<Stream> {
    // Duplicate host handles: a PHP request must not close the embedding
    // application's descriptors. Runtime state models explicit PHP closes.
    #[cfg(unix)]
    let mut file = {
        use std::os::fd::AsFd;
        File::from(match index {
            0 => io::stdin().as_fd().try_clone_to_owned()?,
            1 => io::stdout().as_fd().try_clone_to_owned()?,
            _ => io::stderr().as_fd().try_clone_to_owned()?,
        })
    };
    #[cfg(windows)]
    let mut file = {
        use std::os::windows::io::AsHandle;
        File::from(match index {
            0 => io::stdin().as_handle().try_clone_to_owned()?,
            1 => io::stdout().as_handle().try_clone_to_owned()?,
            _ => io::stderr().as_handle().try_clone_to_owned()?,
        })
    };
    let position = file.stream_position().unwrap_or(u64::MAX);
    Ok(Stream {
        backend: if index == 0 {
            Backend::StandardInput(BufReader::with_capacity(8192, file))
        } else {
            Backend::StandardOutput(index == 2)
        },
        writable: index != 0,
        eof: false,
        position,
    })
}
