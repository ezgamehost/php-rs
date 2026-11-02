// Altered Rust adaptation of PHP 8.5.0 main/php_variables.c and
// ext/standard/string.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md for attribution and license.
use super::*;

impl Runtime {
    pub(super) fn parse_query_string(
        &mut self,
        mut args: Vec<Option<Value>>,
        references: &[Option<variables::Cell>],
        line: usize,
    ) -> Result<Value, Error> {
        let old = args[1].take().unwrap();
        let reference = references[1].clone().unwrap_or_else(|| cell(old.clone()));
        drop(old);
        let input = self.native_string(args[0].as_ref().unwrap(), "parse_str", 1, line)?;
        let empty = Value::Array(Rc::new(PhpArray::default()));
        let mut pending = None;
        if self.reference_is_typed(&reference) {
            self.write_reference_value(&reference, empty, self.strict_types, line)?;
            pending = self.drain_destructors(Ok(()), line).err();
        } else {
            self.assign_native_reference(&reference, empty, &mut pending, line)?;
        }
        if pending
            .as_ref()
            .is_some_and(|error| error.kind == ErrorKind::UnwindExit)
        {
            return Err(pending.unwrap());
        }
        // A typed property's old destructor can replace the installed array.
        // SAPI copies the resulting zval and writes through its table directly.
        let result = reference.borrow().dereferenced();
        let input = input.split(|b| *b == 0).next().unwrap();
        for (count, field) in input
            .split(|b| *b == b'&')
            .filter(|s| !s.is_empty())
            .enumerate()
        {
            if count == 1000 {
                // PHP suppresses a matching user handler while an exception
                // from the replaced value's destructor is already pending.
                if pending.is_none()
                    || self.error_handler.is_none()
                    || self.error_handler_reporting & 2 == 0
                {
                    let warning = self.warn(format!("parse_str(): Input variables exceeded 1000. To increase the limit change max_input_vars in php.ini. on line {line}"));
                    if let Err(error) = warning {
                        self.set_error(error, &mut pending);
                    }
                }
                break;
            }
            let mut parts = field.splitn(2, |b| *b == b'=');
            let name = urls::decode(parts.next().unwrap(), false);
            let value = Value::String(urls::decode(parts.next().unwrap_or_default(), false).into());
            if let Value::Array(array) = &result {
                self.register_query_variable(array, &name, value, line);
            }
        }
        pending.map_or(Ok(Value::Null), Err)
    }

    fn query_key(&mut self, name: Vec<u8>, line: usize) -> ArrayKey {
        ArrayKey::from_value(&Value::String(name.into()), line, self).unwrap()
    }

    fn register_query_variable(
        &mut self,
        root: &Rc<PhpArray>,
        name: &[u8],
        value: Value,
        line: usize,
    ) {
        let name = name.split(|b| *b == 0).next().unwrap();
        let name = &name[name.iter().position(|b| *b != b' ').unwrap_or(name.len())..];
        let bracket = name.iter().position(|b| *b == b'[');
        let base = normalize(&name[..bracket.unwrap_or(name.len())]);
        if base.is_empty() {
            return;
        }
        let mut index = Some(base.clone());
        let mut current = root.clone();
        let mut bracket = bracket;
        let mut depth = 0;
        while let Some(open) = bracket {
            depth += 1;
            if depth > 64 {
                let key = self.query_key(base, line);
                root.mutate_shared(|table| table.remove(&key));
                return;
            }
            let start = open + 1;
            let skip = usize::from(
                name.get(start)
                    .is_some_and(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r' | 11 | 12)),
            );
            let (close, next) = if name.get(start + skip) == Some(&b']') {
                (start + skip, None)
            } else if let Some(end) = name[start..].iter().position(|b| *b == b']') {
                (start + end, Some(name[start..start + end].to_vec()))
            } else {
                // Only the first malformed bracket extends the top-level name.
                // At later levels the preceding closing bracket already ended it.
                if depth == 1 {
                    index = Some(normalize(name));
                }
                break;
            };
            let key = if let Some(index) = index {
                if forbidden(&index, name) {
                    return;
                }
                self.query_key(index, line)
            } else {
                let Ok(key) = current.append_key(line) else {
                    return;
                };
                key
            };
            let child = match current.get(&key) {
                Some(Value::Array(array)) => Rc::new((*array).clone()),
                _ => Rc::new(PhpArray::default()),
            };
            current.mutate_shared(|table| table.insert(key, Value::Array(child.clone())));
            current = child;
            index = next;
            bracket = (name.get(close + 1) == Some(&b'[')).then_some(close + 1);
        }
        if let Some(index) = index {
            if forbidden(&index, name) {
                return;
            }
            let key = self.query_key(index, line);
            current.mutate_shared(|table| table.insert(key, value));
        } else {
            // PHP silently discards appends after the maximum integer index.
            let _ = current.mutate_shared(|table| table.append(value, line));
        }
    }
}

fn normalize(name: &[u8]) -> Vec<u8> {
    name.iter()
        .map(|b| {
            if matches!(b, b' ' | b'.' | b'[') {
                b'_'
            } else {
                *b
            }
        })
        .collect()
}

fn forbidden(name: &[u8], original: &[u8]) -> bool {
    [b"__Host-".as_slice(), b"__Secure-"]
        .iter()
        .any(|prefix| name.starts_with(prefix) && !original.starts_with(prefix))
}
