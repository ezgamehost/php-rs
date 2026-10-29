use std::{
    cell::RefCell,
    fs::File,
    io::{self, Cursor, Read, Seek, SeekFrom, Write},
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
    Memory(Cursor<Vec<u8>>),
    Temporary(Rc<PhpResource>),
}

impl Read for Backend {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::File(file) => file.read(bytes),
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
            Self::Memory(buffer) => buffer.flush(),
            Self::Temporary(resource) => resource.with_stream(|stream| stream.backend.flush()),
        }
    }
}
impl Seek for Backend {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match self {
            Self::File(file) => file.seek(position),
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
}
impl Default for Store {
    fn default() -> Self {
        // PHP CLI reserves resource handles 1..=3 for its standard streams.
        Self {
            next_id: 4,
            default_context: None,
        }
    }
}
impl Store {
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
