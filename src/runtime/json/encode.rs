use super::*;

struct Encoder {
    flags: i64,
    max_depth: i32,
    depth: usize,
    error: i64,
    output: Vec<u8>,
}

impl Encoder {
    fn partial(&self) -> bool {
        self.flags & PARTIAL != 0
    }
    fn newline(&mut self) {
        if self.flags & 128 != 0 {
            self.output.push(b'\n');
            self.output
                .extend(std::iter::repeat_n(b' ', self.depth * 4));
        }
    }
    fn fail(&mut self, code: i64, replacement: &[u8]) -> bool {
        self.error = code;
        self.output.extend(replacement);
        false
    }
    fn number(&mut self, value: f64) {
        if !value.is_finite() {
            self.fail(7, b"0");
            return;
        }
        let mut text = crate::value::shortest_float(value).replace('E', "e");
        if self.flags & 1024 != 0 && !text.contains(['.', 'e']) {
            text.push_str(".0");
        }
        self.output.extend(text.bytes());
    }
    fn string(&mut self, bytes: &[u8], key: bool) -> bool {
        if !key && self.flags & 32 != 0 {
            match crate::value::numeric_string(bytes) {
                Some(Value::Int(n)) => {
                    self.output.extend(n.to_string().bytes());
                    return true;
                }
                Some(Value::Float(n)) if n.is_finite() => {
                    self.number(n);
                    return true;
                }
                _ => {}
            }
        }
        let start = self.output.len();
        self.output.push(b'"');
        let mut cursor = 0;
        while cursor < bytes.len() {
            let code = match utf8::next_char(bytes, &mut cursor) {
                Some(code) => code,
                None if self.flags & IGNORE_UTF8 != 0 => continue,
                None if self.flags & SUBSTITUTE_UTF8 != 0 => 0xfffd,
                None => {
                    self.output.truncate(start);
                    return self.fail(
                        5,
                        if key && self.partial() {
                            b"\"\""
                        } else {
                            b"null"
                        },
                    );
                }
            };
            match code {
                0x22 if self.flags & 8 != 0 => self.output.extend(b"\\u0022"),
                0x22 => self.output.extend(b"\\\""),
                0x5c => self.output.extend(b"\\\\"),
                0x2f if self.flags & 64 == 0 => self.output.extend(b"\\/"),
                8 => self.output.extend(b"\\b"),
                12 => self.output.extend(b"\\f"),
                10 => self.output.extend(b"\\n"),
                13 => self.output.extend(b"\\r"),
                9 => self.output.extend(b"\\t"),
                0x3c if self.flags & 1 != 0 => self.output.extend(b"\\u003C"),
                0x3e if self.flags & 1 != 0 => self.output.extend(b"\\u003E"),
                0x26 if self.flags & 2 != 0 => self.output.extend(b"\\u0026"),
                0x27 if self.flags & 4 != 0 => self.output.extend(b"\\u0027"),
                code if code < 32
                    || code >= 128
                        && (self.flags & 256 == 0
                            || matches!(code, 0x2028 | 0x2029) && self.flags & 2048 == 0) =>
                {
                    let mut units = [0; 2];
                    for unit in char::from_u32(code).unwrap().encode_utf16(&mut units) {
                        self.output.extend(format!("\\u{unit:04x}").bytes());
                    }
                }
                code => {
                    let mut encoded = [0; 4];
                    self.output.extend(
                        char::from_u32(code)
                            .unwrap()
                            .encode_utf8(&mut encoded)
                            .as_bytes(),
                    );
                }
            }
        }
        self.output.push(b'"');
        true
    }
}

impl Runtime {
    pub(super) fn json_encode(
        &mut self,
        value: &Value,
        flags: i64,
        max_depth: i32,
        line: usize,
    ) -> Result<Value, Error> {
        let mut encoder = Encoder {
            flags,
            max_depth,
            depth: 0,
            error: 0,
            output: Vec::new(),
        };
        let result = self.json_encode_value(value, &mut encoder, line);
        if flags & THROW == 0 || encoder.partial() {
            self.json_last_error = encoder.error;
        }
        result?;
        if encoder.error != 0 && !encoder.partial() {
            if flags & THROW != 0 {
                return Err(self.json_exception(encoder.error, line));
            }
            return Ok(Value::Bool(false));
        }
        self.drain_destructors(Ok(Value::String(encoder.output.into())), line)
    }

    fn json_encode_value(
        &mut self,
        value: &Value,
        encoder: &mut Encoder,
        line: usize,
    ) -> Result<bool, Error> {
        match value {
            Value::Reference(reference) | Value::Indirect(reference) => {
                let value = reference.borrow().clone();
                self.json_encode_value(&value, encoder, line)
            }
            Value::Null => {
                encoder.output.extend(b"null");
                Ok(true)
            }
            Value::Bool(value) => {
                encoder
                    .output
                    .extend(if *value { b"true".as_slice() } else { b"false" });
                Ok(true)
            }
            Value::Int(value) => {
                encoder.output.extend(value.to_string().bytes());
                Ok(true)
            }
            Value::Float(value) => {
                encoder.number(*value);
                Ok(true)
            }
            Value::String(value) => Ok(encoder.string(&value.bytes(), false)),
            Value::Resource(_) => Ok(encoder.fail(8, b"null")),
            Value::Array(array) => {
                let identity = (0, Rc::as_ptr(array) as usize);
                if !self.json_guards.insert(identity) {
                    return Ok(encoder.fail(6, b"null"));
                }
                let entries = array.entries().to_vec();
                let object = encoder.flags & 16 != 0
                    || entries
                        .iter()
                        .enumerate()
                        .any(|(i, (key, _))| *key != ArrayKey::Int(i as i64));
                let result = self.json_encode_entries(entries, object, None, encoder, line);
                self.json_guards.remove(&identity);
                result
            }
            Value::Closure(_) => self.json_encode_entries(Vec::new(), true, None, encoder, line),
            Value::Object(object) => {
                if object.class.is_a(b"JsonSerializable") {
                    let identity = (2, object.object_id);
                    if !self.json_guards.insert(identity) {
                        return Ok(encoder.fail(6, b"null"));
                    }
                    let method = objects::method(&object.class, b"jsonSerialize").unwrap();
                    let result = (|| {
                        let target = self.unchecked_method_target(
                            method,
                            object.class.clone(),
                            Some(object.clone()),
                            line,
                        )?;
                        let returned = self.invoke_at(target, &[], line, false, true)?;
                        let result = if matches!(&returned, Value::Object(other) if Rc::ptr_eq(other, object))
                        {
                            self.json_guards.remove(&identity);
                            self.json_encode_object(object, encoder, line)
                        } else {
                            self.json_encode_value(&returned, encoder, line)
                        };
                        drop(returned);
                        self.drain_destructors(result, line)
                    })();
                    self.json_guards.remove(&identity);
                    result
                } else if object.class.declaration.is_enum {
                    let value = object
                        .properties
                        .borrow()
                        .get(&ArrayKey::String(b"value".to_vec()));
                    match value {
                        Some(value) => self.json_encode_value(&value, encoder, line),
                        None => Ok(encoder.fail(11, b"0")),
                    }
                } else {
                    self.json_encode_object(object, encoder, line)
                }
            }
        }
    }

    fn json_encode_object(
        &mut self,
        object: &Rc<crate::PhpObject>,
        encoder: &mut Encoder,
        line: usize,
    ) -> Result<bool, Error> {
        let object = self.initialize_lazy_object(object, line)?;
        let identity = (1, object.object_id);
        if !self.json_guards.insert(identity) {
            return Ok(encoder.fail(6, b"null"));
        }
        let entries = object
            .export_properties()
            .entries()
            .iter()
            .filter(|(key, _)| !matches!(key, ArrayKey::String(name) if name.first() == Some(&0)))
            .cloned()
            .collect();
        let live = object.array_iterator.borrow().is_none().then_some(&object);
        let result = self.json_encode_entries(entries, true, live, encoder, line);
        self.json_guards.remove(&identity);
        result
    }

    fn json_encode_entries(
        &mut self,
        entries: Vec<(ArrayKey, Value)>,
        object: bool,
        live: Option<&Rc<crate::PhpObject>>,
        encoder: &mut Encoder,
        line: usize,
    ) -> Result<bool, Error> {
        // Bound native recursion independently of the user-selected JSON depth.
        if encoder.depth >= 1024 {
            return Ok(encoder.fail(1, b"null"));
        }
        encoder.output.push(if object { b'{' } else { b'[' });
        encoder.depth += 1;
        let mut count = 0;
        for (key, value) in entries {
            // Declared slots remain indirect in PHP's property table. Dynamic
            // entries instead retain the snapshot taken before user callbacks.
            let value = if let Some(object) = live
                && object
                    .class
                    .properties
                    .iter()
                    .any(|p| key == ArrayKey::String(p.storage_name()))
            {
                let Some(value) = object.properties.borrow().get(&key) else {
                    continue;
                };
                value
            } else {
                value
            };
            if count != 0 {
                encoder.output.push(b',');
            }
            count += 1;
            encoder.newline();
            if object {
                let bytes = match key {
                    ArrayKey::Int(n) => n.to_string().into_bytes(),
                    ArrayKey::String(s) => s,
                };
                if !encoder.string(&bytes, true) && !encoder.partial() {
                    return Ok(false);
                }
                encoder.output.push(b':');
                if encoder.flags & 128 != 0 {
                    encoder.output.push(b' ');
                }
            }
            if !self.json_encode_value(&value, encoder, line)? && !encoder.partial() {
                return Ok(false);
            }
        }
        if encoder.depth as i64 > i64::from(encoder.max_depth) {
            encoder.error = 1;
            if !encoder.partial() {
                return Ok(false);
            }
        }
        encoder.depth -= 1;
        if count != 0 {
            encoder.newline();
        }
        encoder.output.push(if object { b'}' } else { b']' });
        Ok(true)
    }
}
