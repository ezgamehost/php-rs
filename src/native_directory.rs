use std::{io, path::Path};

#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", target_pointer_width = "64")
))]
mod posix {
    use std::{
        ffi::{CStr, CString, c_char, c_int, c_void},
        io,
        os::unix::ffi::OsStrExt,
        path::Path,
        ptr::NonNull,
    };

    // Prefix of the 64-bit native dirent. Only the terminating-NUL name and
    // d_type are read; the allocation remains owned by the directory handle.
    #[repr(C)]
    struct Dirent {
        inode: u64,
        offset: u64,
        record_length: u16,
        #[cfg(target_os = "macos")]
        name_length: u16,
        entry_type: u8,
        name: [c_char; 1],
    }

    unsafe extern "C" {
        #[cfg_attr(
            all(target_os = "macos", target_arch = "x86_64"),
            link_name = "opendir$INODE64"
        )]
        fn opendir(path: *const c_char) -> *mut c_void;
        #[cfg_attr(
            all(target_os = "macos", target_arch = "x86_64"),
            link_name = "readdir$INODE64"
        )]
        fn readdir(directory: *mut c_void) -> *const Dirent;
        #[cfg_attr(
            all(target_os = "macos", target_arch = "x86_64"),
            link_name = "rewinddir$INODE64"
        )]
        fn rewinddir(directory: *mut c_void);
        fn closedir(directory: *mut c_void) -> c_int;
    }

    #[derive(Debug)]
    pub(super) struct Handle(NonNull<c_void>);

    impl Handle {
        pub(super) fn open(path: &Path) -> io::Result<Self> {
            let path = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
            // SAFETY: CString provides a live, NUL-terminated path. A non-null
            // result is exclusively owned here and closed once by Drop.
            let handle = unsafe { opendir(path.as_ptr()) };
            NonNull::new(handle)
                .map(Self)
                .ok_or_else(io::Error::last_os_error)
        }

        pub(super) fn read(&mut self) -> Option<(Vec<u8>, u8)> {
            // SAFETY: the owned handle remains open. readdir's entry is valid
            // until the next operation on this handle, and its name has a NUL.
            // Copy it before releasing the exclusive borrow of the handle.
            unsafe {
                let entry = readdir(self.0.as_ptr()).as_ref()?;
                let name = CStr::from_ptr(entry.name.as_ptr()).to_bytes().to_vec();
                // PHP copies d_type only when libc defines
                // _DIRENT_HAVE_D_TYPE. Darwin does not define that macro.
                let entry_type = if cfg!(target_os = "macos") {
                    0
                } else {
                    entry.entry_type
                };
                Some((name, entry_type))
            }
        }

        pub(super) fn rewind(&mut self) {
            // SAFETY: exclusive access to the live, owned directory handle.
            unsafe { rewinddir(self.0.as_ptr()) };
        }
    }

    impl Drop for Handle {
        fn drop(&mut self) {
            // SAFETY: this is the unique owner; no borrowed entries escape read.
            unsafe { closedir(self.0.as_ptr()) };
        }
    }
}

#[derive(Debug)]
pub(crate) struct Directory {
    #[cfg(any(
        target_os = "macos",
        all(target_os = "linux", target_pointer_width = "64")
    ))]
    handle: Option<posix::Handle>,
    glob: Option<crate::native_glob::Glob>,
}

impl Directory {
    pub(crate) fn open_stream(path: &[u8], cwd: &Path) -> io::Result<Self> {
        if let Some(pattern) = path.strip_prefix(b"glob://") {
            Ok(Self {
                #[cfg(any(
                    target_os = "macos",
                    all(target_os = "linux", target_pointer_width = "64")
                ))]
                handle: None,
                glob: Some(crate::native_glob::Glob::open(pattern, cwd)?),
            })
        } else {
            Self::open(&cwd.join(crate::source::path_from_bytes(
                path.strip_prefix(b"file://").unwrap_or(path),
            )))
        }
    }

    pub(crate) fn glob(&self) -> Option<&crate::native_glob::Glob> {
        self.glob.as_ref()
    }

    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        #[cfg(any(
            target_os = "macos",
            all(target_os = "linux", target_pointer_width = "64")
        ))]
        {
            Ok(Self {
                handle: Some(posix::Handle::open(path)?),
                glob: None,
            })
        }
        #[cfg(not(any(
            target_os = "macos",
            all(target_os = "linux", target_pointer_width = "64")
        )))]
        {
            let _ = path;
            Err(io::Error::from(io::ErrorKind::Unsupported))
        }
    }

    pub(crate) fn read(&mut self) -> Option<(Vec<u8>, u8)> {
        #[cfg(any(
            target_os = "macos",
            all(target_os = "linux", target_pointer_width = "64")
        ))]
        {
            if let Some(glob) = &mut self.glob {
                glob.read()
            } else {
                self.handle.as_mut().unwrap().read()
            }
        }
        #[cfg(not(any(
            target_os = "macos",
            all(target_os = "linux", target_pointer_width = "64")
        )))]
        {
            None
        }
    }

    pub(crate) fn rewind(&mut self) {
        #[cfg(any(
            target_os = "macos",
            all(target_os = "linux", target_pointer_width = "64")
        ))]
        {
            if let Some(glob) = &mut self.glob {
                glob.rewind();
            } else {
                self.handle.as_mut().unwrap().rewind();
            }
        }
    }
}

pub(crate) const RECORD_SIZE: usize = if cfg!(target_os = "macos") {
    1025
} else {
    4097
};
