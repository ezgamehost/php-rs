use super::*;

impl Runtime {
    pub(super) fn string_replace(
        &mut self,
        insensitive: bool,
        args: &[Option<Value>],
        references: &[Option<variables::Cell>],
        line: usize,
    ) -> Result<Value, Error> {
        let name = if insensitive {
            "str_ireplace"
        } else {
            "str_replace"
        };
        let search = self.native_array_or_string(args[0].as_ref().unwrap(), name, 1, line)?;
        let replace = self.native_array_or_string(args[1].as_ref().unwrap(), name, 2, line)?;
        let subject = self.native_array_or_string(args[2].as_ref().unwrap(), name, 3, line)?;
        if matches!(search, Value::String(_)) && matches!(replace, Value::Array(_)) {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "{name}(): Argument #2 ($replace) must be of type string when argument #1 ($search) is a string"
                ),
                line,
            ));
        }
        let mut count = 0;
        let mut replace_subject = |runtime: &mut Self, value: &Value| {
            let bytes = runtime.string_bytes(&value.dereferenced(), line)?;
            let (bytes, replaced) =
                runtime.replace_in_subject(bytes, &search, &replace, insensitive, line)?;
            count += replaced;
            Ok::<_, Error>(Value::String(bytes.into()))
        };
        let result = if let Value::Array(subjects) = &subject {
            let mut result = PhpArray::default();
            for (key, value) in subjects.entries() {
                result.insert(key.clone(), replace_subject(self, value)?);
            }
            Value::Array(Rc::new(result))
        } else {
            replace_subject(self, &subject)?
        };
        if let Some(reference) = &references[3] {
            self.write_reference_value(reference, Value::Int(count), self.strict_types, line)?;
        }
        Ok(result)
    }

    fn replace_in_subject(
        &mut self,
        mut subject: Vec<u8>,
        search: &Value,
        replace: &Value,
        insensitive: bool,
        line: usize,
    ) -> Result<(Vec<u8>, i64), Error> {
        let mut count = 0;
        if subject.is_empty() {
            return Ok((subject, count));
        }
        if let Value::Array(searches) = search {
            for (index, (_, needle)) in searches.entries().iter().enumerate() {
                // Convert each element when PHP reaches it: Stringable callbacks may
                // mutate referenced values used by a later search or replacement.
                let needle = self.string_bytes(&needle.dereferenced(), line)?;
                let replacement = if let Value::Array(replacements) = replace {
                    match replacements.entries().get(index) {
                        Some((_, value)) => self.string_bytes(&value.dereferenced(), line)?,
                        None => Vec::new(),
                    }
                } else {
                    let Value::String(bytes) = replace else {
                        unreachable!()
                    };
                    bytes.to_vec()
                };
                let (result, replaced) =
                    replace_bytes(subject, &needle, &replacement, insensitive, line)?;
                subject = result;
                count += replaced;
                if subject.is_empty() {
                    break;
                }
            }
        } else {
            let (Value::String(needle), Value::String(replacement)) = (search, replace) else {
                unreachable!()
            };
            (subject, count) = replace_bytes(
                subject,
                &needle.bytes(),
                &replacement.bytes(),
                insensitive,
                line,
            )?;
        }
        Ok((subject, count))
    }
}

fn replace_bytes(
    subject: Vec<u8>,
    needle: &[u8],
    replacement: &[u8],
    insensitive: bool,
    line: usize,
) -> Result<(Vec<u8>, i64), Error> {
    if needle.is_empty() || needle.len() > subject.len() {
        return Ok((subject, 0));
    }
    let mut result = Vec::new();
    let mut cursor = 0;
    let mut count = 0;
    while let Some(offset) = subject[cursor..].windows(needle.len()).position(|window| {
        if insensitive {
            window.eq_ignore_ascii_case(needle)
        } else {
            window == needle
        }
    }) {
        let extra = offset
            .checked_add(replacement.len())
            .ok_or_else(|| Error::new(ErrorKind::Error, "String size overflow", line))?;
        result
            .try_reserve(extra)
            .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
        result.extend_from_slice(&subject[cursor..cursor + offset]);
        result.extend_from_slice(replacement);
        cursor += offset + needle.len();
        count += 1;
    }
    if count == 0 {
        return Ok((subject, count));
    }
    result
        .try_reserve(subject.len() - cursor)
        .map_err(|_| Error::new(ErrorKind::Error, "Unable to allocate string storage", line))?;
    result.extend_from_slice(&subject[cursor..]);
    Ok((result, count))
}
