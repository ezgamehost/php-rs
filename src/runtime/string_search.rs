use super::*;

#[derive(Debug, Clone, Copy)]
pub(super) enum SearchKind {
    Position,
    PositionInsensitive,
    LastPosition,
    LastPositionInsensitive,
    Suffix,
    SuffixAlias,
    SuffixInsensitive,
    LastCharacter,
    Contains,
    StartsWith,
    EndsWith,
}

impl SearchKind {
    fn name(self) -> &'static str {
        match self {
            Self::Position => "strpos",
            Self::PositionInsensitive => "stripos",
            Self::LastPosition => "strrpos",
            Self::LastPositionInsensitive => "strripos",
            Self::Suffix => "strstr",
            Self::SuffixAlias => "strchr",
            Self::SuffixInsensitive => "stristr",
            Self::LastCharacter => "strrchr",
            Self::Contains => "str_contains",
            Self::StartsWith => "str_starts_with",
            Self::EndsWith => "str_ends_with",
        }
    }

    pub(super) fn parameters(self) -> &'static [(&'static str, bool, bool)] {
        match self {
            Self::Position
            | Self::PositionInsensitive
            | Self::LastPosition
            | Self::LastPositionInsensitive => &[
                ("haystack", false, false),
                ("needle", false, false),
                ("offset", true, false),
            ],
            Self::Suffix | Self::SuffixAlias | Self::SuffixInsensitive | Self::LastCharacter => &[
                ("haystack", false, false),
                ("needle", false, false),
                ("before_needle", true, false),
            ],
            _ => &[("haystack", false, false), ("needle", false, false)],
        }
    }
}

impl Runtime {
    pub(super) fn string_search(
        &mut self,
        kind: SearchKind,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = kind.name();
        let haystack = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let needle = self.native_string(args[1].as_ref().unwrap(), name, 2, line)?;
        let insensitive = matches!(
            kind,
            SearchKind::PositionInsensitive
                | SearchKind::LastPositionInsensitive
                | SearchKind::SuffixInsensitive
        );
        match kind {
            SearchKind::Contains => Ok(Value::Bool(
                find(&haystack, &needle, false, false).is_some(),
            )),
            SearchKind::StartsWith => Ok(Value::Bool(haystack.starts_with(&needle))),
            SearchKind::EndsWith => Ok(Value::Bool(haystack.ends_with(&needle))),
            SearchKind::Position
            | SearchKind::PositionInsensitive
            | SearchKind::LastPosition
            | SearchKind::LastPositionInsensitive => {
                let offset = args[2]
                    .as_ref()
                    .map(|value| self.native_int(value, name, 3, line))
                    .transpose()?
                    .unwrap_or(0);
                if offset.unsigned_abs() > haystack.len() as u64 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        format!(
                            "{name}(): Argument #3 ($offset) must be contained in argument #1 ($haystack)"
                        ),
                        line,
                    ));
                }
                let reverse = matches!(
                    kind,
                    SearchKind::LastPosition | SearchKind::LastPositionInsensitive
                );
                let (start, end) = if offset < 0 {
                    let index = haystack.len() - offset.unsigned_abs() as usize;
                    if reverse {
                        // A negative reverse offset bounds the match's starting position,
                        // so a multi-byte needle may extend beyond that position.
                        (0, index.saturating_add(needle.len()).min(haystack.len()))
                    } else {
                        (index, haystack.len())
                    }
                } else {
                    (offset as usize, haystack.len())
                };
                Ok(find(&haystack[start..end], &needle, insensitive, reverse)
                    .map_or(Value::Bool(false), |index| {
                        Value::Int((start + index) as i64)
                    }))
            }
            _ => {
                let before = args[2]
                    .as_ref()
                    .map(|value| self.native_bool(value, name, 3, line))
                    .transpose()?
                    .unwrap_or(false);
                let index = if matches!(kind, SearchKind::LastCharacter) {
                    let byte = needle.first().copied().unwrap_or(0);
                    haystack.iter().rposition(|value| *value == byte)
                } else {
                    find(&haystack, &needle, insensitive, false)
                };
                Ok(index.map_or(Value::Bool(false), |index| {
                    Value::String(
                        if before {
                            haystack[..index].to_vec()
                        } else {
                            haystack[index..].to_vec()
                        }
                        .into(),
                    )
                }))
            }
        }
    }
}

fn find(haystack: &[u8], needle: &[u8], insensitive: bool, reverse: bool) -> Option<usize> {
    if needle.is_empty() {
        return Some(if reverse { haystack.len() } else { 0 });
    }
    let mut windows = haystack.windows(needle.len());
    let matches = |window: &[u8]| {
        if insensitive {
            window.eq_ignore_ascii_case(needle)
        } else {
            window == needle
        }
    };
    if reverse {
        windows.rposition(matches)
    } else {
        windows.position(matches)
    }
}
