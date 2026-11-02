// Directory-stream state follows PHP 8.5.0 main/streams/glob_wrapper.c.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use std::{io, path::Path};

// Pattern compilation and matching adapt PHP's main/php_glob.c (glob0,
// glob2/glob3 and match), derived from Berkeley glob by Guido van Rossum.
// Copyright (c) 1989, 1993 The Regents of the University of California.
// See the retained BSD license in THIRD_PARTY_NOTICES.md.
const PROTECT: u16 = 0x4000;
const META: u16 = 0x8000;
const ALL: u16 = META | b'*' as u16;
const ONE: u16 = META | b'?' as u16;
const SET: u16 = META | b'[' as u16;
const END: u16 = META | b']' as u16;
const NOT: u16 = META | b'!' as u16;
const RANGE: u16 = META | b'-' as u16;
const CLASS: u16 = META | b':' as u16;
const CLASSES: &[&[u8]] = &[
    b"alnum", b"alpha", b"blank", b"cntrl", b"digit", b"graph", b"lower", b"print", b"punct",
    b"space", b"upper", b"xdigit",
];
const PATH_MAX: usize = crate::native_directory::RECORD_SIZE - 1;

fn compile(pattern: &[u8]) -> Option<Vec<u16>> {
    let mut quoted = Vec::new();
    let mut bytes = pattern.iter().copied();
    while let Some(byte) = bytes.next() {
        quoted.push(if byte == b'\\' {
            u16::from(bytes.next().unwrap_or(b'\\')) | PROTECT
        } else {
            u16::from(byte)
        });
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < quoted.len() {
        let c = quoted[i];
        i += 1;
        match c {
            42 => {
                if out.last() != Some(&ALL) {
                    out.push(ALL);
                }
            }
            63 => out.push(ONE),
            91 => {
                let negate = quoted.get(i) == Some(&33);
                let start = i + usize::from(negate);
                if start + 1 >= quoted.len() || !quoted[start + 1..].contains(&93) {
                    out.push(91);
                    continue;
                }
                i = start;
                out.push(SET);
                if negate {
                    out.push(NOT);
                }
                loop {
                    let c = *quoted.get(i)?;
                    i += 1;
                    if c == 91
                        && quoted.get(i) == Some(&58)
                        && let Some(end) = quoted[i + 1..]
                            .iter()
                            .position(|c| *c == 58)
                            .map(|n| i + 1 + n)
                        && quoted.get(end + 1) == Some(&93)
                    {
                        let class = CLASSES.iter().position(|name| {
                            name.len() == end - i - 1
                                && name
                                    .iter()
                                    .zip(&quoted[i + 1..end])
                                    .all(|(a, b)| u16::from(*a) == *b)
                        })?;
                        out.extend_from_slice(&[CLASS, class as u16]);
                        i = end + 2;
                    } else {
                        out.push(c & 0xff);
                        if quoted.get(i) == Some(&45) && quoted.get(i + 1) != Some(&93) {
                            out.extend_from_slice(&[RANGE, *quoted.get(i + 1)? & 0xff]);
                            i += 2;
                        }
                    }
                    if quoted.get(i) == Some(&93) {
                        i += 1;
                        break;
                    }
                }
                out.push(END);
            }
            _ => out.push(c & 0xff),
        }
    }
    Some(out)
}

fn in_class(byte: u8, class: u16) -> bool {
    match class {
        0 => byte.is_ascii_alphanumeric(),
        1 => byte.is_ascii_alphabetic(),
        2 => matches!(byte, b' ' | b'\t'),
        3 => byte.is_ascii_control(),
        4 => byte.is_ascii_digit(),
        5 => byte.is_ascii_graphic(),
        6 => byte.is_ascii_lowercase(),
        7 => byte == b' ' || byte.is_ascii_graphic(),
        8 => byte.is_ascii_punctuation(),
        9 => matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 11 | 12),
        10 => byte.is_ascii_uppercase(),
        11 => byte.is_ascii_hexdigit(),
        _ => false,
    }
}

fn matches(name: &[u8], pattern: &[u16]) -> bool {
    let (mut n, mut p) = (0, 0);
    let mut retry = None;
    loop {
        if p == pattern.len() && n == name.len() {
            return true;
        }
        if let Some(&c) = pattern.get(p) {
            p += 1;
            if c == ALL {
                if p == pattern.len() {
                    return true;
                }
                if n == name.len() {
                    return false;
                }
                retry = Some((n + 1, p - 1));
                continue;
            }
            if let Some(&byte) = name.get(n) {
                let found = if c == SET {
                    let negate = pattern.get(p) == Some(&NOT);
                    p += usize::from(negate);
                    let mut found = false;
                    while let Some(&c) = pattern.get(p) {
                        p += 1;
                        if c == END {
                            break;
                        }
                        if c == CLASS {
                            if let Some(&class) = pattern.get(p) {
                                found |= in_class(byte, class);
                            }
                            p += 1;
                        } else if pattern.get(p) == Some(&RANGE) {
                            if let Some(&end) = pattern.get(p + 1) {
                                found |= c <= u16::from(byte) && u16::from(byte) <= end;
                            }
                            p += 2;
                        } else {
                            found |= c == u16::from(byte);
                        }
                    }
                    found != negate
                } else {
                    c == ONE || c == u16::from(byte)
                };
                if found {
                    n += 1;
                    continue;
                }
            }
        }
        if let Some((next_n, next_p)) = retry {
            n = next_n;
            p = next_p;
        } else {
            return false;
        }
    }
}

fn expand(pattern: &[u8], cwd: &Path) -> io::Result<Vec<Vec<u8>>> {
    if pattern.is_empty() || pattern.len() >= PATH_MAX {
        return Ok(Vec::new());
    }
    let Some(pattern) = compile(pattern) else {
        return Ok(Vec::new());
    };
    let mut pending = vec![(Vec::new(), 0)];
    let mut results = Vec::new();
    // An explicit stack avoids recursive host calls for deep wildcard paths.
    while let Some((mut path, mut start)) = pending.pop() {
        loop {
            if start == pattern.len() {
                if std::fs::symlink_metadata(cwd.join(crate::source::path_from_bytes(&path)))
                    .is_ok()
                {
                    results.push(path);
                }
                break;
            }
            let end = pattern[start..]
                .iter()
                .position(|c| *c == 47)
                .map_or(pattern.len(), |n| start + n);
            let segment = &pattern[start..end];
            if segment.iter().any(|c| *c & META != 0) {
                let resolved = cwd.join(crate::source::path_from_bytes(&path));
                let Ok(mut directory) = crate::native_directory::Directory::open(&resolved) else {
                    break;
                };
                while let Some((name, _)) = directory.read() {
                    if name.starts_with(b".") && segment.first() != Some(&46) {
                        continue;
                    }
                    if path.len() + name.len() >= PATH_MAX - 1 {
                        return Err(io::Error::from(io::ErrorKind::InvalidInput));
                    }
                    if matches(&name, segment) {
                        let mut matched = path.clone();
                        matched.extend_from_slice(&name);
                        pending.push((matched, end));
                    }
                }
                break;
            }
            path.extend(segment.iter().map(|c| *c as u8));
            start = end;
            while pattern.get(start) == Some(&47) {
                path.push(b'/');
                start += 1;
            }
            if path.len() >= PATH_MAX {
                return Err(io::Error::from(io::ErrorKind::InvalidInput));
            }
        }
    }
    results.sort();
    Ok(results)
}

#[derive(Debug)]
pub(crate) struct Glob {
    matches: Vec<Vec<u8>>,
    index: usize,
    path: Vec<u8>,
}

impl Glob {
    pub(crate) fn open(pattern: &[u8], cwd: &Path) -> io::Result<Self> {
        let matches = expand(pattern, cwd)?;
        Ok(Self {
            matches,
            index: 0,
            path: Vec::new(),
        })
    }

    pub(crate) fn count(&self) -> usize {
        self.matches.len()
    }

    pub(crate) fn path(&self) -> &[u8] {
        &self.path
    }

    pub(crate) fn read(&mut self) -> Option<(Vec<u8>, u8)> {
        self.path.clear();
        let name = self.matches.get(self.index)?;
        self.index += 1;
        let start = name.iter().rposition(|b| *b == b'/').map_or(0, |i| i + 1);
        let end = if start > 1 { start - 1 } else { start };
        self.path.extend_from_slice(&name[..end]);
        Some((name[start..].to_vec(), 0))
    }

    pub(crate) fn rewind(&mut self) {
        self.index = 0;
        self.path.clear();
    }
}
