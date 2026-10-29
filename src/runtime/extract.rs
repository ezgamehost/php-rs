use super::*;

impl Runtime {
    pub(super) fn extract_variables(
        &mut self,
        args: Vec<Option<Value>>,
        references: Vec<Option<variables::Cell>>,
        dynamic: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let mut args = args.into_iter();
        let first = args.next().unwrap().unwrap();
        builtins::require_array(&first, "extract", 1, line)?;
        let source = if let Some(reference) = &references[0] {
            drop(first);
            reference.clone()
        } else {
            cell(first)
        };
        let flags = match args.next().flatten() {
            Some(value) => self.native_int(&value, "extract", 2, line)?,
            None => 0,
        };
        let prefix = args
            .next()
            .flatten()
            .map(|value| self.native_string(&value, "extract", 3, line))
            .transpose()?;
        let by_reference = flags & 256 != 0;
        // PHP separates reference extraction before checking the flag range,
        // prefix identifier, or dynamic-call restriction.
        if by_reference {
            let mut input = source.borrow_mut();
            let Value::Array(array) = &mut *input else {
                return Err(builtins::type_error(&input, "extract", 1, "array", line));
            };
            Rc::make_mut(array);
        }
        let mode = flags & 255;
        if mode > 6 {
            return Err(Error::new(
                ErrorKind::ValueError,
                "extract(): Argument #2 ($flags) must be a valid extract type",
                line,
            ));
        }
        if (2..=5).contains(&mode) && prefix.is_none() {
            return Err(Error::new(
                ErrorKind::ValueError,
                "extract(): Argument #3 ($prefix) is required when using this extract type",
                line,
            ));
        }
        let prefix = prefix.unwrap_or_default();
        if !prefix.is_empty() && !valid_name(&prefix) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "extract(): Argument #3 ($prefix) must be a valid identifier",
                line,
            ));
        }
        if dynamic {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot call extract() dynamically",
                line,
            ));
        }
        let (keys, snapshot) = {
            let input = source.borrow();
            let array = builtins::require_array(&input, "extract", 1, line)?;
            let keys: Vec<_> = array.entries().iter().map(|(key, _)| key.clone()).collect();
            // Value extraction must survive overwriting its own source variable.
            // Reference extraction instead retains the native argument's cell.
            let snapshot = if by_reference {
                None
            } else {
                Some(input.clone())
            };
            (keys, snapshot)
        };
        let mut count = 0;
        for key in keys {
            let Some(name) = self.extracted_name(&key, mode, &prefix) else {
                continue;
            };
            if name == b"this" {
                return Err(Error::new(ErrorKind::Error, "Cannot re-assign $this", line));
            }
            if by_reference {
                let mut offset = Offset::new(Some(key.to_value()));
                let reference = reference_paths::acquire(
                    source.clone(),
                    std::slice::from_mut(&mut offset),
                    line,
                    self,
                )?;
                // EXTR_REFS rebinds the local, detaching any previous reference
                // rather than assigning through that reference's type constraint.
                self.scope.insert(name, reference);
            } else {
                let Value::Array(array) = snapshot.as_ref().unwrap() else {
                    unreachable!()
                };
                let value = array.get(&key).unwrap().dereferenced();
                if let Some(reference) = self.scope.get(&name).cloned() {
                    // PHP uses weak assignment to a typed reference even when
                    // extract was invoked from a strict-types file.
                    self.write_reference_value(&reference, value, false, line)?;
                } else {
                    self.scope.insert(name, cell(value));
                }
            }
            count += 1;
        }
        Ok(Value::Int(count))
    }

    fn extracted_name(&self, key: &ArrayKey, mode: i64, prefix: &[u8]) -> Option<Vec<u8>> {
        let numeric = matches!(key, ArrayKey::Int(_));
        if numeric && !matches!(mode, 3 | 4) {
            return None;
        }
        let name = match key {
            ArrayKey::String(name) => name.clone(),
            ArrayKey::Int(number) => number.to_string().into_bytes(),
        };
        let exists = self.scope.get(&name).is_some();
        // $this uses a dedicated opcode rather than an undefined compiled slot.
        let undefined_slot = !exists && name != b"this" && self.scope.has_slot(&name);
        let prefixed = || {
            let mut result = prefix.to_vec();
            result.push(b'_');
            result.extend_from_slice(&name);
            result
        };
        let result = match mode {
            0 => {
                if name == b"GLOBALS" && exists {
                    return None;
                }
                name
            }
            1 => {
                if exists || name == b"this" {
                    return None;
                }
                name
            }
            2 => {
                if name.is_empty() {
                    return None;
                }
                if exists || name == b"this" {
                    prefixed()
                } else {
                    name
                }
            }
            3 => {
                if name.is_empty() {
                    return None;
                }
                prefixed()
            }
            4 => {
                if numeric || !valid_name(&name) || name == b"this" {
                    prefixed()
                } else {
                    name
                }
            }
            5 => {
                if exists {
                    prefixed()
                } else if undefined_slot {
                    name
                } else {
                    return None;
                }
            }
            6 => {
                if !exists || name == b"GLOBALS" {
                    return None;
                }
                name
            }
            _ => unreachable!(),
        };
        valid_name(&result).then_some(result)
    }
}

fn valid_name(name: &[u8]) -> bool {
    let initial = |byte: u8| byte.is_ascii_alphabetic() || byte == b'_' || byte >= 128;
    name.first().is_some_and(|&byte| initial(byte))
        && name[1..]
            .iter()
            .all(|&byte| initial(byte) || byte.is_ascii_digit())
}
