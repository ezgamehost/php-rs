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
    Context(Box<crate::runtime::stream_contexts::Context>),
    Closed,
}

#[derive(Debug)]
pub(crate) struct Stream {
    pub(crate) backend: Backend,
    pub(crate) writable: bool,
    pub(crate) eof: bool,
    pub(crate) read_buffer: ReadBuffer,
    pub(crate) position: u64,
    pub(crate) mode: Vec<u8>,
    pub(crate) context: Option<Rc<PhpResource>>,
    pub(crate) uri: Option<Vec<u8>>,
    pub(crate) remove_on_close: Option<std::path::PathBuf>,
}

#[derive(Debug, Default)]
pub(crate) struct ReadBuffer {
    bytes: Vec<u8>,
    position: usize,
    allocated: usize,
}

impl ReadBuffer {
    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes[self.position..]
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes.len() - self.position
    }

    pub(crate) fn consume(&mut self, count: usize) {
        self.position += count;
    }

    pub(crate) fn clear(&mut self) {
        self.bytes.clear();
        self.position = 0;
    }

    pub(crate) fn fill_space(&mut self) -> io::Result<usize> {
        if self.allocated - self.bytes.len() < 8192 && self.position != 0 {
            self.bytes.drain(..self.position);
            self.position = 0;
        }
        if self.allocated - self.bytes.len() < 8192 {
            self.allocated = self
                .allocated
                .checked_add(8192)
                .ok_or_else(|| io::Error::from(io::ErrorKind::OutOfMemory))?;
        }
        Ok(self.allocated - self.bytes.len())
    }

    pub(crate) fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.bytes
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
}

impl Stream {
    pub(crate) fn supports_lock(&self) -> bool {
        match &self.backend {
            Backend::File(_) | Backend::StandardInput(_) | Backend::StandardOutput(_) => true,
            Backend::Memory(_) | Backend::Directory(_) => false,
            Backend::Temporary(resource, _) => {
                matches!(&*resource.kind.borrow(), Kind::Stream(inner) if inner.supports_lock())
            }
        }
    }

    pub(crate) fn at_eof(&self) -> bool {
        self.eof && self.read_buffer.len() == 0
    }

    pub(crate) fn read_uses_stdio(&self) -> bool {
        self.read_buffer.len() == 0
            && match &self.backend {
                Backend::File(_) | Backend::StandardOutput(_) => true,
                Backend::StandardInput(reader) => reader.buffer().is_empty(),
                _ => false,
            }
    }

    pub(crate) fn read_bytes(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        // Plain files retain buffered read behavior once a record read has
        // allocated their buffer. Memory and temporary streams use raw reads
        // after draining it, as PHP's NO_BUFFER flag requires.
        if self.read_buffer.len() == 0
            && self.read_buffer.allocated != 0
            && matches!(self.backend, Backend::File(_))
            && !bytes.is_empty()
        {
            let capacity = self.read_buffer.fill_space()?;
            let mut chunk = Vec::new();
            chunk
                .try_reserve_exact(capacity)
                .map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
            chunk.resize(capacity, 0);
            let count = self.backend.read(&mut chunk)?;
            if count == 0 {
                self.eof = true;
                return Ok(0);
            }
            self.read_buffer.append(&chunk[..count])?;
        }
        let buffered = self.read_buffer.len().min(bytes.len());
        if buffered != 0 {
            bytes[..buffered].copy_from_slice(&self.read_buffer.bytes()[..buffered]);
            self.read_buffer.consume(buffered);
            return Ok(buffered);
        }
        let count = self.backend.read(bytes)?;
        match &self.backend {
            Backend::Memory(_) if count == 0 && !bytes.is_empty() => self.eof = true,
            Backend::Temporary(resource, _) => {
                if let Kind::Stream(inner) = &*resource.kind.borrow() {
                    self.eof = inner.at_eof();
                }
            }
            _ => {}
        }
        Ok(count)
    }

    pub(crate) fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if let SeekFrom::Start(position) = position
            && matches!(self.backend, Backend::File(_) | Backend::StandardInput(_))
            && position > self.position
            && position - self.position <= self.read_buffer.len() as u64
        {
            self.read_buffer
                .consume((position - self.position) as usize);
            return Ok(position);
        }
        let result = self.backend.seek(position);
        if result.is_ok() || matches!(self.backend, Backend::File(_) | Backend::Memory(_)) {
            self.read_buffer.clear();
        }
        result
    }

    // PHP's truncate callbacks leave the logical position, EOF and stat cache
    // unchanged. Memory streams clamp only their internal cursor on shrink.
    pub(crate) fn truncate(&mut self, size: u64) -> Option<io::Result<()>> {
        Some(match &mut self.backend {
            Backend::Directory(_) => return None,
            Backend::File(file) => file.set_len(size),
            Backend::StandardInput(reader) => reader.get_ref().set_len(size),
            Backend::StandardOutput(stderr) => {
                output_file(*stderr).and_then(|file| file.set_len(size))
            }
            Backend::Temporary(resource, _) => {
                let mut kind = resource.kind.borrow_mut();
                let Kind::Stream(inner) = &mut *kind else {
                    return Some(Err(io::Error::from_raw_os_error(9)));
                };
                return inner.truncate(size);
            }
            Backend::Memory(buffer) => {
                if !self.writable {
                    return Some(Err(io::Error::from(io::ErrorKind::PermissionDenied)));
                }
                let Ok(size) = usize::try_from(size) else {
                    return Some(Err(io::Error::from(io::ErrorKind::OutOfMemory)));
                };
                let old_size = buffer.get_ref().len();
                if size > old_size && buffer.get_mut().try_reserve(size - old_size).is_err() {
                    return Some(Err(io::Error::from(io::ErrorKind::OutOfMemory)));
                }
                buffer.get_mut().resize(size, 0);
                if size <= old_size && buffer.position() > size as u64 {
                    buffer.set_position(size as u64);
                }
                Ok(())
            }
        })
    }

    pub(crate) fn needs_temporary_spill(&self, length: usize) -> bool {
        let Backend::Temporary(resource, limit) = &self.backend else {
            return false;
        };
        let kind = resource.kind.borrow();
        let Kind::Stream(inner) = &*kind else {
            return false;
        };
        let Backend::Memory(buffer) = &inner.backend else {
            return false;
        };
        let position = if self.read_buffer.len() != 0 {
            self.position
        } else {
            buffer.position()
        };
        position.saturating_add(length as u64) >= *limit as u64
    }

    pub(crate) fn write_bytes(
        &mut self,
        bytes: &[u8],
        temporary_directory: Option<&std::path::Path>,
    ) -> io::Result<usize> {
        if self.read_buffer.len() != 0 && self.backend.seek(SeekFrom::Start(self.position)).is_ok()
        {
            self.read_buffer.clear();
            self.eof = false;
        }
        if self.mode.starts_with(b"a")
            && let Backend::Memory(buffer) = &mut self.backend
        {
            buffer.set_position(buffer.get_ref().len() as u64);
        }
        if let (Some(directory), Backend::Temporary(resource, limit)) =
            (temporary_directory, &mut self.backend)
        {
            resource.with_stream(|stream| {
                if let Backend::Memory(buffer) = &stream.backend
                    && buffer.position().saturating_add(bytes.len() as u64) >= *limit as u64
                {
                    let (mut file, path) =
                        crate::runtime::temporary_files::create_temporary(directory, b"php")?;
                    let copied = file
                        .write_all(buffer.get_ref())
                        .and_then(|()| file.seek(SeekFrom::Start(buffer.position())));
                    if let Err(error) = copied {
                        let _ = std::fs::remove_file(path);
                        return Err(error);
                    }
                    stream.backend = Backend::File(file);
                    stream.remove_on_close = Some(path);
                }
                Ok(())
            })?;
        }
        self.backend.write(bytes)
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        if let Some(path) = &self.remove_on_close {
            let _ = std::fs::remove_file(path);
        }
    }
}

#[derive(Debug)]
pub(crate) enum Backend {
    Directory(crate::native_directory::Directory),
    File(File),
    StandardInput(BufReader<File>),
    StandardOutput(bool),
    Memory(Cursor<Vec<u8>>),
    Temporary(Rc<PhpResource>, usize),
}

impl Read for Backend {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Directory(directory) => {
                if bytes.len() != crate::native_directory::RECORD_SIZE {
                    return Err(io::Error::from(io::ErrorKind::InvalidInput));
                }
                let Some((name, kind)) = directory.read() else {
                    return Ok(0);
                };
                if name.len() >= bytes.len() - 1 {
                    return Err(io::Error::from(io::ErrorKind::InvalidData));
                }
                // Native unused record bytes are unspecified. Initialize them
                // so PHP code cannot observe uninitialized host memory.
                bytes.fill(0);
                bytes[..name.len()].copy_from_slice(&name);
                let last = bytes.len() - 1;
                bytes[last] = kind;
                Ok(bytes.len())
            }
            Self::File(file) => file.read(bytes),
            Self::StandardInput(file) => file.read(bytes),
            Self::StandardOutput(_) => Err(io::Error::from_raw_os_error(9)),
            Self::Memory(buffer) => buffer.read(bytes),
            Self::Temporary(resource, _) => resource.with_stream(|stream| {
                let mut total = 0;
                while total < bytes.len() {
                    let count = stream.read_bytes(&mut bytes[total..])?;
                    if count == 0 {
                        stream.eof = true;
                        break;
                    }
                    total += count;
                }
                stream.position += total as u64;
                Ok(total)
            }),
        }
    }
}
impl Write for Backend {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Directory(_) => Err(io::Error::from(io::ErrorKind::Unsupported)),
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
            Self::Temporary(resource, _) => resource.with_stream(|stream| {
                let count = stream.write_bytes(bytes, None)?;
                stream.position += count as u64;
                Ok(count)
            }),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Directory(_) => Ok(()),
            Self::File(file) => file.flush(),
            Self::StandardInput(file) => file.get_mut().flush(),
            Self::StandardOutput(_) => Ok(()),
            Self::Memory(buffer) => buffer.flush(),
            Self::Temporary(resource, _) => resource.with_stream(|stream| stream.backend.flush()),
        }
    }
}
impl Seek for Backend {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Directory(directory) => {
                directory.rewind();
                Ok(0)
            }
            Self::File(file) => file.seek(position),
            Self::StandardInput(file) => file.seek(position),
            Self::StandardOutput(_) => Err(io::Error::from_raw_os_error(29)),
            Self::Memory(buffer) => buffer.seek(position),
            Self::Temporary(resource, _) => resource.with_stream(|stream| {
                let position = stream.seek(position)?;
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
            Kind::Context(_) => "stream-context",
            Kind::Closed => "Unknown",
        }
    }
    pub(crate) fn debug_type(&self) -> &'static str {
        match &*self.kind.borrow() {
            Kind::Stream(_) => "resource (stream)",
            Kind::Context(_) => "resource (stream-context)",
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
            self.default_context = Some(self.allocate(Kind::Context(Box::default())));
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
        read_buffer: Default::default(),
        position,
        mode: if index == 0 {
            b"rb".to_vec()
        } else {
            b"wb".to_vec()
        },
        uri: Some(match index {
            0 => b"php://stdin".to_vec(),
            1 => b"php://stdout".to_vec(),
            _ => b"php://stderr".to_vec(),
        }),
        remove_on_close: None,
        context: None,
    })
}

pub(crate) fn output_file(stderr: bool) -> io::Result<File> {
    #[cfg(unix)]
    {
        use std::os::fd::AsFd;
        Ok(File::from(if stderr {
            io::stderr().as_fd().try_clone_to_owned()?
        } else {
            io::stdout().as_fd().try_clone_to_owned()?
        }))
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsHandle;
        Ok(File::from(if stderr {
            io::stderr().as_handle().try_clone_to_owned()?
        } else {
            io::stdout().as_handle().try_clone_to_owned()?
        }))
    }
}
