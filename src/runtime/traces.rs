use super::functions::CallTarget;
use super::*;
use crate::ast::Parameter;

/// Active frames keep local parameter names, so capture sees reassignment and
/// unset. Captured traces own PHP array snapshots; object arguments stay handles.
pub(super) struct TraceFrame {
    fields: PhpArray,
    object: Option<Rc<crate::PhpObject>>,
    sensitive: Vec<ArrayKey>,
    pub(super) internal: bool,
    arguments: Option<PhpArray>,
    fixed: Vec<Vec<u8>>,
    pub(super) bound: usize,
    scope_depth: Option<usize>,
}

fn put(array: &mut PhpArray, key: &[u8], value: Value) {
    array.insert(ArrayKey::String(key.to_vec()), value);
}

impl TraceFrame {
    pub(super) fn call_site(&self) -> Option<(Vec<u8>, usize)> {
        let file = self.fields.get(&ArrayKey::String(b"file".to_vec()))?;
        let Value::Int(line) = self.fields.get(&ArrayKey::String(b"line".to_vec()))? else {
            return None;
        };
        Some((file.to_bytes(), *line as usize))
    }

    pub(super) fn call(
        runtime: &Runtime,
        target: &CallTarget,
        parameters: &[Parameter],
        fixed: &[Option<Value>],
        references: &[Option<variables::Cell>],
        extra: &PhpArray,
        site: Option<usize>,
    ) -> Self {
        let name = target.function.as_ref().map_or(&target.name, |f| &f.name);
        let mut frame = Self::unit(runtime, name, site, Some(PhpArray::default()));
        frame.object = target.this.clone();
        frame.internal = target.builtin.is_some()
            || target.function.as_ref().is_some_and(|f| f.native.is_some());
        if let Some(context) = &target.context {
            let function = if name.starts_with(b"{closure:") {
                name.as_slice()
            } else {
                name.rsplit(|b| *b == b':').next().unwrap()
            };
            put(
                &mut frame.fields,
                b"function",
                Value::String(function.to_vec().into()),
            );
            put(
                &mut frame.fields,
                b"class",
                Value::String(context.scope.name.clone().into()),
            );
            put(
                &mut frame.fields,
                b"type",
                Value::String(
                    if target.this.is_some() { b"->" } else { b"::" }
                        .to_vec()
                        .into(),
                ),
            );
        }
        let supplied = if extra
            .entries()
            .iter()
            .any(|(k, _)| matches!(k, ArrayKey::Int(_)))
        {
            parameters.iter().take_while(|p| !p.variadic).count()
        } else {
            fixed.iter().rposition(Option::is_some).map_or(0, |i| i + 1)
        };
        let arguments = frame.arguments.as_mut().unwrap();
        for (index, value) in fixed.iter().take(supplied).enumerate() {
            arguments.insert(
                ArrayKey::Int(index as i64),
                // Active by-reference arguments refer to the caller's cell.
                // Keeping an array snapshot here would cause artificial COW
                // and reset exhausted cursors during native argument parsing.
                references[index].as_ref().map_or_else(
                    || value.clone().unwrap_or(Value::Null),
                    |reference| Value::Reference(reference.clone()),
                ),
            );
            frame.fixed.push(parameters[index].name.clone());
        }
        for (key, value) in extra.entries() {
            match key {
                ArrayKey::Int(_) => {
                    arguments.append(value.clone(), 0).expect("trace size");
                }
                ArrayKey::String(_) => {
                    arguments.insert(key.clone(), value.clone());
                }
            }
        }
        for (key, _) in arguments.entries() {
            // Zend appends extra named variadics after positional redaction.
            let ArrayKey::Int(index) = key else { continue };
            let parameter = parameters
                .get(*index as usize)
                .or_else(|| parameters.last().filter(|parameter| parameter.variadic));
            if parameter.is_some_and(|parameter| {
                parameter
                    .attributes
                    .iter()
                    .any(|attribute| attribute.name.eq_ignore_ascii_case(b"SensitiveParameter"))
            }) {
                frame.sensitive.push(key.clone());
            }
        }
        frame
    }

    pub(super) fn unit(
        runtime: &Runtime,
        name: &[u8],
        site: Option<usize>,
        arguments: Option<PhpArray>,
    ) -> Self {
        let mut fields = PhpArray::default();
        if let Some(line) = site {
            put(
                &mut fields,
                b"file",
                Value::String(runtime.source.name.clone().into()),
            );
            put(&mut fields, b"line", Value::Int(line as i64));
        }
        put(
            &mut fields,
            b"function",
            Value::String(name.to_vec().into()),
        );
        Self {
            internal: false,
            object: None,
            sensitive: Vec::new(),
            fields,
            arguments,
            fixed: Vec::new(),
            bound: 0,
            scope_depth: None,
        }
    }

    pub(super) fn enter_scope(&mut self, depth: usize) {
        self.scope_depth = Some(depth);
    }

    pub(super) fn set_extra(&mut self, key: &ArrayKey, value: Value) {
        let key = match key {
            ArrayKey::Int(index) => ArrayKey::Int(self.fixed.len() as i64 + index),
            key => key.clone(),
        };
        self.arguments.as_mut().unwrap().insert(key, value);
    }

    fn capture(&self, runtime: &Runtime, options: i32) -> Value {
        let mut fields = PhpArray::default();
        for (key, value) in self.fields.entries() {
            if matches!(key, ArrayKey::String(name) if name == b"type")
                && options & 1 != 0
                && let Some(object) = &self.object
            {
                put(&mut fields, b"object", Value::Object(object.clone()));
            }
            fields.insert(key.clone(), value.clone());
        }
        if options & 2 == 0
            && let Some(stored) = &self.arguments
        {
            let mut arguments = PhpArray::default();
            for (key, value) in stored.entries() {
                arguments.insert(key.clone(), value.dereferenced());
            }
            if let Some(depth) = self.scope_depth {
                let scope = if depth == runtime.callers.len() {
                    &runtime.scope
                } else {
                    &runtime.callers[depth]
                };
                for (index, name) in self.fixed.iter().take(self.bound).enumerate() {
                    let value = scope.get(name).map_or(Value::Null, |c| c.borrow().clone());
                    arguments.insert(ArrayKey::Int(index as i64), value);
                }
            }
            put(&mut fields, b"args", Value::Array(Rc::new(arguments)));
        }
        Value::Array(Rc::new(fields))
    }
}

impl Runtime {
    pub(super) fn debug_backtrace(
        &mut self,
        builtin: builtins::Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let print = matches!(builtin, builtins::Builtin::DebugPrintBacktrace);
        let name = if print {
            "debug_print_backtrace"
        } else {
            "debug_backtrace"
        };
        let options = args[0]
            .as_ref()
            .map(|value| self.native_int(value, name, 1, line))
            .transpose()?
            .unwrap_or(i64::from(!print)) as i32;
        let limit = args[1]
            .as_ref()
            .map(|value| self.native_int(value, name, 2, line))
            .transpose()?
            .unwrap_or(0) as i32;
        // Retain the builtin frame for argument errors, but omit it from a
        // successful requested trace. Zero means no frame limit.
        let trace = self.capture_trace_frames(1, options, limit);
        if print {
            self.emit_output(&format_trace(&trace, false));
            Ok(Value::Null)
        } else {
            Ok(Value::Array(trace))
        }
    }

    pub(super) fn capture_trace(&mut self) -> Rc<PhpArray> {
        self.capture_trace_frames(0, 0, 0)
    }

    fn capture_trace_frames(&mut self, skip: usize, options: i32, limit: i32) -> Rc<PhpArray> {
        let limit = if limit == 0 {
            usize::MAX
        } else {
            limit.max(0) as usize
        };
        // Snapshot before allocating wrappers so no active-frame borrow spans
        // allocation. Captured values have PHP array copy-on-write semantics.
        let frames: Vec<_> = self
            .trace_frames
            .iter()
            .rev()
            .skip(skip)
            .take(limit)
            .map(|frame| (frame.capture(self, options), frame.sensitive.clone()))
            .collect();
        let mut trace = PhpArray::default();
        for (mut frame, sensitive) in frames {
            let Value::Array(fields) = &mut frame else {
                unreachable!()
            };
            if let Some(Value::Array(arguments)) =
                Rc::make_mut(fields).get_mut(&ArrayKey::String(b"args".to_vec()))
            {
                let arguments = Rc::make_mut(arguments);
                for key in sensitive {
                    if let Some(value) = arguments.get_mut(&key) {
                        *value = self.sensitive_parameter_value(value.dereferenced());
                    }
                }
            }
            trace.append(frame, 0).expect("trace size");
        }
        Rc::new(trace)
    }
}

pub(super) fn trace_string(trace: &PhpArray) -> Vec<u8> {
    format_trace(trace, true)
}

fn format_trace(trace: &PhpArray, include_main: bool) -> Vec<u8> {
    let mut output = Vec::new();
    for (index, (_, value)) in trace.entries().iter().enumerate() {
        let Value::Array(frame) = value else {
            unreachable!()
        };
        output.extend_from_slice(format!("#{index} ").as_bytes());
        let get = |key: &[u8]| frame.get(&ArrayKey::String(key.to_vec()));
        if let Some(Value::String(file)) = get(b"file") {
            output.extend_from_slice(&file.bytes());
            output.push(b'(');
            output.extend_from_slice(&get(b"line").unwrap().to_bytes());
            output.extend_from_slice(b"): ");
        } else {
            output.extend_from_slice(b"[internal function]: ");
        }
        for key in [b"class".as_slice(), b"type", b"function"] {
            if let Some(Value::String(value)) = get(key) {
                output.extend_from_slice(&value.bytes());
            }
        }
        output.push(b'(');
        if let Some(Value::Array(args)) = get(b"args") {
            for (index, (key, value)) in args.entries().iter().enumerate() {
                if index != 0 {
                    output.extend_from_slice(b", ");
                }
                if let ArrayKey::String(name) = key {
                    output.extend_from_slice(name);
                    output.extend_from_slice(b": ");
                }
                format_argument(&mut output, value);
            }
        }
        output.extend_from_slice(b")\n");
    }
    if include_main {
        output.extend_from_slice(format!("#{} {{main}}", trace.len()).as_bytes());
    }
    output
}

fn format_argument(output: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Reference(reference) | Value::Indirect(reference) => {
            format_argument(output, &reference.borrow())
        }
        Value::Null => output.extend_from_slice(b"NULL"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Array(_) => output.extend_from_slice(b"Array"),
        Value::Object(object) => {
            output.extend_from_slice(format!("Object({})", object.class_name).as_bytes())
        }
        Value::Closure(_) => output.extend_from_slice(b"Object(Closure)"),
        Value::String(value) => {
            output.push(b'\'');
            for byte in value.bytes().iter().take(15) {
                let escape: Option<&[u8]> = match byte {
                    b'\n' => Some(b"\\n"),
                    b'\r' => Some(b"\\r"),
                    b'\t' => Some(b"\\t"),
                    12 => Some(b"\\f"),
                    11 => Some(b"\\v"),
                    27 => Some(b"\\e"),
                    b'\\' => Some(b"\\\\"),
                    _ => None,
                };
                if let Some(escape) = escape {
                    output.extend_from_slice(escape);
                } else if !(32..=126).contains(byte) {
                    output.extend_from_slice(format!("\\x{byte:02X}").as_bytes());
                } else {
                    output.push(*byte);
                }
            }
            if value.len() > 15 {
                output.extend_from_slice(b"...");
            }
            output.push(b'\'');
        }
        Value::Float(number) => {
            let bytes = value.to_bytes();
            output.extend_from_slice(&bytes);
            if number.is_finite() && !bytes.contains(&b'.') && !bytes.contains(&b'E') {
                output.extend_from_slice(b".0");
            }
        }
        Value::Int(_) | Value::Resource(_) => output.extend_from_slice(&value.to_bytes()),
    }
}
