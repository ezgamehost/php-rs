// Altered Rust adaptations of PHP 8.5.0 ext/standard/string.c (PHP Group)
// and Zend/zend_compile.c zend_dirname (Zend Technologies Ltd.).
// See THIRD_PARTY_NOTICES.md for their PHP and Zend licenses.
use super::{builtins::Builtin, *};

impl Runtime {
    pub(super) fn path_part(
        &mut self,
        builtin: Builtin,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let path = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        match builtin {
            Builtin::Basename => {
                let suffix = args[1]
                    .as_ref()
                    .map(|v| self.native_string(v, name, 2, line))
                    .transpose()?
                    .unwrap_or_default();
                let mut base = basename(&path);
                if suffix.len() < base.len() && base.ends_with(&suffix) {
                    base = &base[..base.len() - suffix.len()];
                }
                Ok(Value::String(base.to_vec().into()))
            }
            Builtin::Dirname => {
                let mut levels = args[1]
                    .as_ref()
                    .map(|v| self.native_int(v, name, 2, line))
                    .transpose()?
                    .unwrap_or(1);
                if levels < 1 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "dirname(): Argument #2 ($levels) must be greater than or equal to 1",
                        line,
                    ));
                }
                let mut directory = path;
                loop {
                    let previous_len = directory.len();
                    directory = dirname(&directory);
                    levels -= 1;
                    if levels == 0 || directory.len() >= previous_len {
                        break;
                    }
                }
                Ok(Value::String(directory.into()))
            }
            Builtin::Pathinfo => {
                let flags = args[1]
                    .as_ref()
                    .map(|v| self.native_int(v, name, 2, line))
                    .transpose()?
                    .unwrap_or(15);
                let mut parts = PhpArray::default();
                let mut add = |name: &[u8], bytes: &[u8]| {
                    parts.insert(
                        ArrayKey::String(name.to_vec()),
                        Value::String(bytes.to_vec().into()),
                    );
                };
                if flags & 1 != 0 {
                    let directory = dirname(&path);
                    // pathinfo inserts dirname through a C-string API, unlike
                    // its binary-safe basename/extension/filename fields.
                    let end = directory
                        .iter()
                        .position(|&b| b == 0)
                        .unwrap_or(directory.len());
                    if end != 0 {
                        add(b"dirname", &directory[..end]);
                    }
                }
                let base = basename(&path);
                let dot = base.iter().rposition(|&b| b == b'.');
                if flags & 2 != 0 {
                    add(b"basename", base);
                }
                if flags & 4 != 0
                    && let Some(dot) = dot
                {
                    add(b"extension", &base[dot + 1..]);
                }
                if flags & 8 != 0 {
                    add(b"filename", &base[..dot.unwrap_or(base.len())]);
                }
                if flags == 15 {
                    Ok(Value::Array(Rc::new(parts)))
                } else {
                    Ok(parts.entries().first().map_or_else(
                        || Value::String(Vec::new().into()),
                        |(_, value)| value.clone(),
                    ))
                }
            }
            _ => unreachable!(),
        }
    }
}

fn basename(path: &[u8]) -> &[u8] {
    let end = path.iter().rposition(|&b| b != b'/').map_or(0, |i| i + 1);
    let start = path[..end]
        .iter()
        .rposition(|&b| b == b'/')
        .map_or(0, |i| i + 1);
    &path[start..end]
}

fn dirname(path: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        return Vec::new();
    }
    let Some(end) = path.iter().rposition(|&b| b != b'/') else {
        return b"/".to_vec();
    };
    let Some(slash) = path[..=end].iter().rposition(|&b| b == b'/') else {
        return b".".to_vec();
    };
    let Some(end) = path[..slash].iter().rposition(|&b| b != b'/') else {
        return b"/".to_vec();
    };
    path[..=end].to_vec()
}
