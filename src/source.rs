use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub name: Vec<u8>,
    pub directory: PathBuf,
    pub path: Option<PathBuf>,
}

impl Default for Source {
    fn default() -> Self {
        Self::memory(b"Standard input code")
    }
}

impl Source {
    pub fn memory(name: &[u8]) -> Self {
        Self {
            name: name.to_vec(),
            directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            path: None,
        }
    }

    pub fn file(path: &Path) -> Self {
        Self {
            name: path.as_os_str().as_encoded_bytes().to_vec(),
            directory: path.parent().unwrap_or(Path::new(".")).to_owned(),
            path: Some(path.to_owned()),
        }
    }

    pub fn evaluated(&self, line: usize) -> Self {
        let mut name = self.name.clone();
        name.extend_from_slice(format!("({line}) : eval()'d code").as_bytes());
        Self {
            name,
            directory: self.directory.clone(),
            path: None,
        }
    }
}

pub(crate) fn path_from_bytes(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(bytes.to_vec()).into()
    }
    #[cfg(not(unix))]
    {
        String::from_utf8_lossy(bytes).into_owned().into()
    }
}
