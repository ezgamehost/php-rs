use super::property_export::ExportProperty;
use super::*;

impl Runtime {
    pub(super) fn var_export(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let return_output = args[1]
            .as_ref()
            .map(|v| self.native_bool(v, "var_export", 2, line))
            .transpose()?
            .unwrap_or(false);
        let mut output = Vec::new();
        self.export_value(args[0].as_ref().unwrap(), 1, &mut output, line)?;
        if return_output {
            Ok(Value::String(output.into()))
        } else {
            self.emit_output(&output)?;
            Ok(Value::Null)
        }
    }

    fn export_value(
        &mut self,
        value: &Value,
        level: usize,
        output: &mut Vec<u8>,
        line: usize,
    ) -> Result<(), Error> {
        let value = value.dereferenced();
        let (table, identity, object) = match &value {
            Value::Array(array) => (array.clone(), (false, Rc::as_ptr(array) as usize), false),
            Value::Object(object) => (Rc::new(PhpArray::default()), (true, object.object_id), true),
            Value::Closure(closure) => (
                Rc::new(PhpArray::default()),
                (true, closure.object_id),
                true,
            ),
            Value::Null | Value::Resource(_) => {
                output.extend(b"NULL");
                return Ok(());
            }
            Value::Bool(b) => {
                output.extend(if *b { b"true".as_slice() } else { b"false" });
                return Ok(());
            }
            Value::Int(n) => {
                if *n == i64::MIN {
                    output.extend(b"-9223372036854775807-1");
                } else {
                    output.extend(n.to_string().bytes());
                }
                return Ok(());
            }
            Value::Float(n) => {
                output.extend(export_float(*n).bytes());
                return Ok(());
            }
            Value::String(s) => {
                export_string(&s.bytes(), true, output);
                return Ok(());
            }
            Value::Reference(_) | Value::Indirect(_) => unreachable!(),
        };
        if !self.export_guards.insert(identity) {
            output.extend(b"NULL");
            self.warn(format!(
                "Warning: var_export does not handle circular references on line {line}"
            ))?;
            return Ok(());
        }
        let result = (|| {
            let initialized = if let Value::Object(object) = &value {
                Some(self.initialize_lazy_object(object, line)?)
            } else {
                None
            };
            let entries: Vec<_> = if let Some(object) = &initialized {
                if self.has_property_hooks(object) {
                    self.property_export_entries(object)
                } else {
                    object
                        .export_properties()
                        .iter_copied()
                        .map(|(key, value)| ExportProperty::snapshot(key, value))
                        .collect()
                }
            } else {
                table
                    .iter_copied()
                    .map(|(key, value)| ExportProperty::snapshot(key, value))
                    .collect()
            };
            if level > 1 {
                output.push(b'\n');
                output.extend(std::iter::repeat_n(b' ', level - 1));
            }
            let std_class = matches!(&value, Value::Object(o) if o.class.name == b"stdClass");
            if object {
                if std_class {
                    output.extend(b"(object) array(\n");
                } else {
                    output.push(b'\\');
                    if let Value::Object(object) = &value {
                        output.extend(&object.class.name);
                        if object.class.declaration.is_enum {
                            output.extend(b"::");
                            output.extend(
                                object
                                    .properties
                                    .borrow()
                                    .get(&ArrayKey::String(b"name".to_vec()))
                                    .unwrap()
                                    .to_bytes(),
                            );
                            return Ok(());
                        }
                    } else {
                        output.extend(b"Closure");
                    }
                    output.extend(b"::__set_state(array(\n");
                }
            } else {
                output.extend(b"array (\n");
            }
            for entry in entries {
                let value =
                    if let (Some(object), Some(property)) = (&initialized, &entry.declaration) {
                        let Some(value) = self.read_export_property(object, property, line)? else {
                            continue;
                        };
                        value
                    } else {
                        entry.value
                    };
                let key = &entry.key;
                output.extend(std::iter::repeat_n(b' ', level + 1 + usize::from(object)));
                match key {
                    ArrayKey::Int(n) => output.extend(n.to_string().bytes()),
                    ArrayKey::String(name) => {
                        let name = if object {
                            match crate::object::property_name(name) {
                                Ok(name) => name.name,
                                Err(message) => {
                                    self.warn(format!("Notice: {message} on line {line}"))?;
                                    name
                                }
                            }
                        } else {
                            name
                        };
                        export_string(name, !object, output);
                    }
                }
                output.extend(b" => ");
                self.export_value(&value, level + 2, output, line)?;
                output.extend(b",\n");
            }
            if level > 1 {
                output.extend(std::iter::repeat_n(b' ', level - 1));
            }
            output.push(b')');
            if object && !std_class {
                output.push(b')');
            }
            Ok(())
        })();
        self.export_guards.remove(&identity);
        result
    }
}

fn export_string(bytes: &[u8], escape_nul: bool, output: &mut Vec<u8>) {
    output.push(b'\'');
    for byte in bytes {
        match byte {
            b'\'' | b'\\' => output.extend([b'\\', *byte]),
            0 if escape_nul => output.extend(b"' . \"\\0\" . '"),
            _ => output.push(*byte),
        }
    }
    output.push(b'\'');
}

fn export_float(number: f64) -> String {
    if !number.is_finite() {
        return if number.is_nan() {
            "NAN"
        } else if number.is_sign_negative() {
            "-INF"
        } else {
            "INF"
        }
        .to_owned();
    }
    // PHP's default serialize_precision=-1 selects the shortest round-tripping
    // decimal, with scientific notation outside exponents -4 through 16.
    let scientific = format!("{number:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    if !(-4..17).contains(&exponent) {
        let fraction = if mantissa.contains('.') { "" } else { ".0" };
        format!("{mantissa}{fraction}E{exponent:+}")
    } else {
        let mut result = number.to_string();
        if !result.contains('.') {
            result.push_str(".0");
        }
        result
    }
}
