use super::functions::CallTarget;
use super::*;
use crate::ast::Parameter;

/// Active frames keep local parameter names, so capture sees reassignment and
/// unset. Captured traces own PHP array snapshots; object arguments stay handles.
pub(super) struct TraceFrame {
    fields: PhpArray,
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
    pub(super) fn call(
        runtime: &Runtime,
        target: &CallTarget,
        parameters: &[Parameter],
        fixed: &[Option<Value>],
        extra: &PhpArray,
        site: Option<usize>,
    ) -> Self {
        let name = target.function.as_ref().map_or(&target.name, |f| &f.name);
        let mut frame = Self::unit(runtime, name, site, Some(PhpArray::default()));
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
                value.clone().unwrap_or(Value::Null),
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

    fn capture(&self, runtime: &Runtime) -> Value {
        let mut fields = self.fields.clone();
        if let Some(mut arguments) = self.arguments.clone() {
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
    pub(super) fn capture_trace(&self) -> Rc<PhpArray> {
        let mut trace = PhpArray::default();
        for frame in self.trace_frames.iter().rev() {
            trace.append(frame.capture(self), 0).expect("trace size");
        }
        Rc::new(trace)
    }
}

pub(super) fn trace_string(trace: &PhpArray) -> Vec<u8> {
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
    output.extend_from_slice(format!("#{} {{main}}", trace.len()).as_bytes());
    output
}

fn format_argument(output: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Reference(reference) => format_argument(output, &reference.borrow()),
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
        Value::Int(_) => output.extend_from_slice(&value.to_bytes()),
    }
}
