// Altered Rust adaptation of PHP 8.5.0 dump traversal. See THIRD_PARTY_NOTICES.md.
use super::*;

impl Runtime {
    pub(super) fn defer_warning(
        &mut self,
        message: String,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        if pending.is_some()
            && self.error_handler.is_some()
            && self.error_handler_reporting & self.diagnostics.level(&message) != 0
        {
            // Zend skips entry into PHP while an exception is pending, without
            // requesting fallback reporting from that handler.
            return Ok(());
        }
        if let Err(error) = self.warn(message) {
            if error.kind == ErrorKind::UnwindExit {
                return Err(error);
            }
            *pending = Some(error);
        }
        Ok(())
    }
    pub(super) fn dump_value(
        &mut self,
        value: &Value,
        depth: usize,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        self.dump_inner(value, depth, false, line, pending)
    }

    fn dump_inner(
        &mut self,
        value: &Value,
        depth: usize,
        mut reference: bool,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        // No reference or property borrow survives a user __debugInfo call.
        match value {
            Value::Indirect(slot) => {
                let value = slot.borrow().copy_array_entry();
                return self.dump_inner(&value, depth, reference, line, pending);
            }
            Value::Reference(slot) => {
                reference |= Rc::strong_count(slot) > 1;
                let value = slot.borrow().copy_array_entry();
                return self.dump_inner(&value, depth, reference, line, pending);
            }
            _ => {}
        }
        let identity = match value {
            Value::Object(object) if !object.class.declaration.is_enum => {
                Some((true, object.object_id))
            }
            Value::Closure(closure) => Some((true, closure.object_id)),
            Value::Array(array) => Some((false, Rc::as_ptr(array) as usize)),
            _ => None,
        };
        if let Some(identity) = identity
            && !self.dump_guards.insert(identity)
        {
            self.emit_output(b"*RECURSION*\n")?;
            return Ok(());
        }
        let result = self.dump_body(value, depth, reference, line, pending);
        // A fatal bailout leaves PHP's recursion guard set through shutdown.
        if !result
            .as_ref()
            .is_err_and(|e| e.kind == ErrorKind::UnwindExit)
            && let Some(identity) = identity
        {
            self.dump_guards.remove(&identity);
        }
        result
    }

    fn dump_body(
        &mut self,
        value: &Value,
        depth: usize,
        reference: bool,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        if let Value::Object(object) = value
            && !object.class.declaration.is_enum
        {
            let custom = objects::method(&object.class, b"__debugInfo").is_some();
            let properties = if custom {
                self.debug_properties(object, line, pending)?
            } else {
                object.export_properties()
            };
            if reference {
                self.emit_output(b"&")?;
            }
            self.emit_output(
                format!(
                    "object({})#{} ({}) {{\n",
                    object.class_name.split('\0').next().unwrap(),
                    object.object_id,
                    properties.len()
                )
                .as_bytes(),
            )?;
            let mut keys = Vec::new();
            if !custom
                && !object.properties_reordered.get()
                && object.fixed_array.borrow().is_none()
            {
                keys.extend(
                    object
                        .class
                        .properties
                        .iter()
                        .filter(|p| {
                            !object.hides_properties()
                                && (p.ty.is_some()
                                    || properties
                                        .get(&ArrayKey::String(p.storage_name()))
                                        .is_some())
                        })
                        .map(|p| ArrayKey::String(p.storage_name())),
                );
            }
            for (key, _) in properties.entries().iter() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
            for key in &keys {
                self.dump_key(key, depth, true, line, pending)?;
                self.dump_indent(depth + 2)?;
                if let Some(value) =
                    properties.with_value(key, |value| value.map(Value::copy_array_entry))
                {
                    self.dump_inner(&value, depth + 2, false, line, pending)?;
                } else {
                    let property = object
                        .class
                        .properties
                        .iter()
                        .find(|p| ArrayKey::String(p.storage_name()) == *key)
                        .expect("typed property slot");
                    let owner = object.class.property_slot_owner(property);
                    let ty = property.ty.as_ref().expect("uninitialized typed property");
                    self.emit_output(
                        format!(
                            "uninitialized({})\n",
                            typed_properties::type_label(ty, &owner)
                        )
                        .as_bytes(),
                    )?;
                }
            }
            self.dump_indent(depth)?;
            return self.emit_output(b"}\n");
        }
        if reference {
            self.emit_output(b"&")?;
        }
        match value {
            Value::Null => self.emit_output(b"NULL\n"),
            Value::Bool(value) => self.emit_output(if *value {
                b"bool(true)\n"
            } else {
                b"bool(false)\n"
            }),
            Value::Int(value) => self.emit_output(format!("int({value})\n").as_bytes()),
            Value::Float(value) => self.emit_output(
                format!("float({})\n", crate::value::shortest_float(*value)).as_bytes(),
            ),
            Value::String(value) => {
                self.emit_output(format!("string({}) \"", value.len()).as_bytes())?;
                self.emit_output(&value.bytes())?;
                self.emit_output(b"\"\n")
            }
            Value::Resource(value) => self.emit_output(
                format!("resource({}) of type ({})\n", value.id, value.type_name()).as_bytes(),
            ),
            Value::Object(object) => {
                let name = object
                    .properties
                    .borrow()
                    .get(&ArrayKey::String(b"name".to_vec()))
                    .unwrap()
                    .to_bytes();
                self.emit_output(b"enum(")?;
                self.emit_output(&object.class.name)?;
                self.emit_output(b"::")?;
                self.emit_output(&name)?;
                self.emit_output(b")\n")
            }
            Value::Array(array) => {
                self.emit_output(format!("array({}) {{\n", array.len()).as_bytes())?;
                self.dump_table(array, depth, line, pending)
            }
            Value::Closure(closure) => {
                let properties = builtins::closure_debug_properties(closure);
                self.emit_output(
                    format!(
                        "object(Closure)#{} ({}) {{\n",
                        closure.object_id,
                        properties.len()
                    )
                    .as_bytes(),
                )?;
                self.dump_table(&properties, depth, line, pending)
            }
            Value::Reference(_) | Value::Indirect(_) => unreachable!(),
        }
    }

    fn dump_table(
        &mut self,
        table: &PhpArray,
        depth: usize,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        for (ref key, ref value) in table.iter_copied() {
            self.dump_key(key, depth, false, line, pending)?;
            self.dump_indent(depth + 2)?;
            self.dump_inner(value, depth + 2, false, line, pending)?;
        }
        self.dump_indent(depth)?;
        self.emit_output(b"}\n")
    }

    fn dump_indent(&mut self, depth: usize) -> Result<(), Error> {
        self.emit_output(&vec![b' '; depth])
    }

    fn dump_key(
        &mut self,
        key: &ArrayKey,
        depth: usize,
        object: bool,
        line: usize,
        pending: &mut Option<Error>,
    ) -> Result<(), Error> {
        let ArrayKey::String(raw) = key else {
            self.dump_indent(depth + 2)?;
            let ArrayKey::Int(n) = key else {
                unreachable!()
            };
            return self.emit_output(format!("[{n}]=>\n").as_bytes());
        };
        let decoded = if object {
            match crate::object::property_name(raw) {
                Ok(decoded) => Some(decoded),
                Err(message) => {
                    self.defer_warning(format!("Notice: {message} on line {line}"), pending)?;
                    None
                }
            }
        } else {
            None
        };
        self.dump_indent(depth + 2)?;
        self.emit_output(b"[\"")?;
        if let Some(decoded) = decoded
            && let Some(scope) = decoded.scope
        {
            self.emit_output(decoded.name.split(|b| *b == 0).next().unwrap())?;
            if scope.first() == Some(&b'*') {
                return self.emit_output(b"\":protected]=>\n");
            }
            self.emit_output(b"\":\"")?;
            self.emit_output(scope)?;
            self.emit_output(b"\":private]=>\n")
        } else {
            self.emit_output(raw)?;
            self.emit_output(b"\"]=>\n")
        }
    }
}
