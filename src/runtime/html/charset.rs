// Altered Rust adaptation of PHP 8.5.0 ext/standard/html.c charset handling.
// Copyright (c) The PHP Group. See THIRD_PARTY_NOTICES.md.
use super::tables;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub(super) enum Charset {
    Utf8,
    Latin1,
    Windows1252,
    Latin9,
    Windows1251,
    Iso88595,
    Cp866,
    MacRoman,
    Koi8r,
    Big5,
    Gb2312,
    Big5Hkscs,
    Sjis,
    EucJp,
}

impl Charset {
    pub(super) fn named(name: &[u8]) -> Option<Self> {
        const ALL: [Charset; 14] = [
            Charset::Utf8,
            Charset::Latin1,
            Charset::Windows1252,
            Charset::Latin9,
            Charset::Windows1251,
            Charset::Iso88595,
            Charset::Cp866,
            Charset::MacRoman,
            Charset::Koi8r,
            Charset::Big5,
            Charset::Gb2312,
            Charset::Big5Hkscs,
            Charset::Sjis,
            Charset::EucJp,
        ];
        tables::ALIASES
            .iter()
            .find(|(alias, _)| name.eq_ignore_ascii_case(alias))
            .map(|(_, index)| ALL[*index])
    }

    pub(super) fn partial(self) -> bool {
        self as usize >= Self::Big5 as usize
    }
    pub(super) fn unicode_compatible(self) -> bool {
        matches!(self, Self::Utf8 | Self::Latin1)
    }

    pub(super) fn to_unicode(self, code: u32) -> u32 {
        if self.unicode_compatible() || self.partial() {
            code
        } else {
            u32::from(tables::SINGLE_BYTE[self as usize - 1][code as usize])
        }
    }

    pub(super) fn map_unicode(self, code: u32) -> Option<u32> {
        if self == Self::Utf8 {
            return Some(code);
        }
        if self.partial() {
            return ((0x20..=0x7d).contains(&code)
                && !(matches!(self, Self::Sjis | Self::EucJp) && code == 0x5c))
                .then_some(code);
        }
        // Native reverse mapping treats ASCII as identity, even where a forward
        // table (notably MacRoman line endings) maps the byte differently.
        if code <= 0x7f {
            return (!(self == Self::MacRoman && code == 0x7f)).then_some(code);
        }
        if code == 0xffff {
            return None;
        }
        tables::SINGLE_BYTE[self as usize - 1]
            .iter()
            .position(|&cp| u32::from(cp) == code)
            .map(|byte| byte as u32)
    }

    pub(super) fn write(self, code: u32, output: &mut Vec<u8>) {
        if self == Self::Utf8 {
            let mut bytes = [0; 4];
            output.extend_from_slice(
                char::from_u32(code)
                    .expect("validated Unicode character")
                    .encode_utf8(&mut bytes)
                    .as_bytes(),
            );
        } else {
            output.push(code as u8);
        }
    }

    pub(super) fn next(self, input: &[u8], cursor: &mut usize) -> Option<u32> {
        if self == Self::Utf8 {
            return super::super::utf8::next_char(input, cursor);
        }
        let bytes = &input[*cursor..];
        let first = bytes[0];
        let next = bytes.get(1).copied();
        let mut width = 1;
        let mut valid = true;
        match self {
            Self::Big5 | Self::Big5Hkscs if (0x81..=0xfe).contains(&first) => {
                if let Some(next) = next {
                    if (0x40..=0x7e).contains(&next) || (0xa1..=0xfe).contains(&next) {
                        width = 2;
                    } else {
                        valid = false;
                        width = if self == Self::Big5Hkscs && matches!(next, 0x80 | 0xff) {
                            2
                        } else {
                            1
                        };
                    }
                } else {
                    valid = false;
                }
            }
            Self::Gb2312 => {
                if (0xa1..=0xfe).contains(&first) {
                    if let Some(next) = next {
                        if (0xa1..=0xfe).contains(&next) {
                            width = 2;
                        } else {
                            valid = false;
                            width = if matches!(next, 0x8e | 0x8f | 0xa0 | 0xff) {
                                2
                            } else {
                                1
                            };
                        }
                    } else {
                        valid = false;
                    }
                } else {
                    valid = !matches!(first, 0x8e | 0x8f | 0xa0 | 0xff);
                }
            }
            Self::Sjis => {
                if (0x81..=0x9f).contains(&first) || (0xe0..=0xfc).contains(&first) {
                    if let Some(next) = next {
                        if (0x40..=0xfc).contains(&next) && next != 0x7f {
                            width = 2;
                        } else {
                            valid = false;
                            width = if next >= 0xfd || matches!(next, 0x80 | 0xa0) {
                                2
                            } else {
                                1
                            };
                        }
                    } else {
                        valid = false;
                    }
                } else {
                    valid = first < 0x80 || (0xa1..=0xdf).contains(&first);
                }
            }
            Self::EucJp => {
                if (0xa1..=0xfe).contains(&first) || first == 0x8e {
                    if let Some(next) = next {
                        if (0xa1..=if first == 0x8e { 0xdf } else { 0xfe }).contains(&next) {
                            width = 2;
                        } else {
                            valid = false;
                            width = if matches!(next, 0xa0 | 0xff) { 2 } else { 1 };
                        }
                    } else {
                        valid = false;
                    }
                } else if first == 0x8f {
                    if bytes.len() >= 3 && bytes[1..3].iter().all(|b| (0xa1..=0xfe).contains(b)) {
                        width = 3;
                    } else {
                        valid = false;
                        width = if next.is_none_or(|b| !matches!(b, 0xa0 | 0xff)) {
                            1
                        } else if bytes.get(2).is_none_or(|b| !matches!(b, 0xa0 | 0xff)) {
                            2
                        } else {
                            3
                        };
                    }
                } else {
                    valid = !matches!(first, 0xa0 | 0xff);
                }
            }
            _ => {}
        }
        *cursor += width;
        valid.then(|| {
            bytes[..width]
                .iter()
                .fold(0, |code, &byte| (code << 8) | u32::from(byte))
        })
    }
}
