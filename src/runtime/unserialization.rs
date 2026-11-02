// Altered Rust adaptation of PHP 8.5.0 ext/standard/var.c and var_unserializer.re.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, object::Class};
use variables::Cell;

#[derive(Clone)]
struct Options {
    allowed: Option<HashSet<Vec<u8>>>,
    max_depth: usize,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            allowed: None,
            max_depth: 4096,
        }
    }
}
struct Hook {
    object: Rc<PhpObject>,
    data: Option<Rc<PhpArray>>,
}
#[derive(Default)]
pub(super) struct UnserializeState {
    slots: Vec<Option<Cell>>,
    arrays: Vec<Rc<PhpArray>>,
    objects: Vec<Rc<PhpObject>>,
    properties: HashMap<usize, (Rc<PhpObject>, ArrayKey)>,
    hooks: Vec<Hook>,
    options: Options,
    depth: usize,
}

pub(super) enum Failure {
    Syntax,
    Runtime(Error),
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        Self::Runtime(error)
    }
}
pub(super) type Parsed<T> = Result<T, Failure>;

#[derive(Clone)]
enum Container {
    Array(Rc<PhpArray>),
    Object(Rc<PhpObject>),
}
enum Task {
    Value(Cell),
    Entries {
        container: Container,
        remaining: usize,
        hook: Option<Hook>,
        counted: bool,
    },
    CheckProperty {
        object: Rc<PhpObject>,
        key: ArrayKey,
        slot: Cell,
    },
}
pub(super) struct Decoder<'a> {
    bytes: &'a [u8],
    pub(super) pos: usize,
    function: String,
    state: Rc<RefCell<UnserializeState>>,
    line: usize,
}

fn valid_class_name(name: &[u8]) -> bool {
    name.iter()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'\\') || *b >= 128)
}
fn value_name(value: &Value) -> &str {
    match value {
        Value::Bool(true) => "true",
        Value::Bool(false) => "false",
        _ => builtins::debug_type(value),
    }
}
fn display_bytes(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    String::from_utf8_lossy(bytes.split(|b| *b == 0).next().unwrap())
}
fn native_serialization_forbidden(class: &Class) -> bool {
    class.name.contains(&0)
        || [
            b"Closure".as_slice(),
            b"InternalIterator",
            b"SplFileInfo",
            b"SensitiveParameterValue",
            b"ReflectionClass",
            b"ReflectionFunctionAbstract",
            b"ReflectionType",
            b"ReflectionParameter",
            b"ReflectionProperty",
            b"ReflectionClassConstant",
            b"ReflectionConstant",
            b"ReflectionAttribute",
            b"ReflectionExtension",
        ]
        .iter()
        .any(|name| class.is_a(name))
}

impl Runtime {
    pub(super) fn unserialize_value(
        &mut self,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        let bytes = self.native_string(args[0].as_ref().unwrap(), "unserialize", 1, line)?;
        let options = args
            .get(1)
            .and_then(Option::as_ref)
            .map(|v| builtins::require_array(v, "unserialize", 2, line))
            .transpose()?;
        if bytes.is_empty() {
            return Ok(Value::Bool(false));
        }
        let previous = self.unserialization.clone();
        let state = previous.clone().unwrap_or_default();
        let old_options = state.borrow().options.clone();
        let old_depth = state.borrow().depth;
        let mut next_options = old_options.clone();
        if let Some(options) = options {
            next_options.allowed = None;
            if let Some(allowed) = options.get(&ArrayKey::String(b"allowed_classes".to_vec())) {
                let allowed = allowed.dereferenced();
                match allowed {
                    Value::Bool(true) => (),
                    Value::Bool(false) => next_options.allowed = Some(HashSet::new()),
                    Value::Array(classes) => {
                        let mut names = HashSet::new();
                        for (_, entry) in classes.iter() {
                            let entry = entry.dereferenced();
                            if !matches!(
                                entry,
                                Value::String(_) | Value::Object(_) | Value::Closure(_)
                            ) {
                                return Err(Error::new(
                                    ErrorKind::TypeError,
                                    format!(
                                        "unserialize(): Option \"allowed_classes\" must be an array of class names, {} given",
                                        value_name(&entry)
                                    ),
                                    line,
                                ));
                            }
                            let name = self.string_bytes(&entry, line)?;
                            if !valid_class_name(&name) {
                                return Err(Error::new(
                                    ErrorKind::ValueError,
                                    format!(
                                        "unserialize(): Option \"allowed_classes\" must be an array of class names, \"{}\" given",
                                        display_bytes(&name)
                                    ),
                                    line,
                                ));
                            }
                            names.insert(name.to_ascii_lowercase());
                        }
                        next_options.allowed = Some(names);
                    }
                    _ => {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            format!(
                                "unserialize(): Option \"allowed_classes\" must be of type array|bool, {} given",
                                builtins::debug_type(&allowed)
                            ),
                            line,
                        ));
                    }
                }
            }
            if let Some(max_depth) = options.get(&ArrayKey::String(b"max_depth".to_vec())) {
                let max_depth = max_depth.dereferenced();
                let Value::Int(depth) = max_depth else {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "unserialize(): Option \"max_depth\" must be of type int, {} given",
                            builtins::debug_type(&max_depth)
                        ),
                        line,
                    ));
                };
                if depth < 0 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "unserialize(): Option \"max_depth\" must be greater than or equal to 0",
                        line,
                    ));
                }
                next_options.max_depth = depth as usize;
                state.borrow_mut().depth = 0;
            }
        }
        state.borrow_mut().options = next_options;
        self.unserialization = Some(state.clone());
        let root = cell(Value::Null);
        let first_slot = state.borrow().slots.len();
        let mut decoder = Decoder {
            bytes: &bytes,
            function: "unserialize".into(),
            pos: 0,
            state: state.clone(),
            line,
        };
        let parsed = decoder.parse(self, root.clone());
        let mut result = match parsed {
            Ok(()) => {
                if decoder.pos < bytes.len() {
                    self.warn(format!("Warning: unserialize(): Extra data starting at offset {} of {} bytes on line {line}", decoder.pos, bytes.len()))
                } else {
                    Ok(())
                }
            }
            Err(failure) => {
                for slot in &mut state.borrow_mut().slots[first_slot..] {
                    *slot = None;
                }
                *root.borrow_mut() = Value::Bool(false);
                match failure {
                    Failure::Syntax => self.warn(format!(
                        "Warning: unserialize(): Error at offset {} of {} bytes on line {line}",
                        decoder.pos,
                        bytes.len()
                    )),
                    Failure::Runtime(error) => Err(error),
                }
            }
        };
        drop(decoder);
        state.borrow_mut().options = old_options;
        state.borrow_mut().depth = old_depth;
        self.unserialization = previous.clone();
        if previous.is_none() {
            result = self.finish_unserialization(&state, result, line);
        }

        drop(state);
        let value = root.borrow().dereferenced();
        drop(root);
        self.drain_destructors(result.map(|()| value), line)
    }

    pub(super) fn unserialize_native(
        &mut self,
        bytes: &[u8],
        function: &str,
        line: usize,
        action: impl FnOnce(&mut Self, &mut Decoder<'_>) -> Parsed<()>,
    ) -> Result<Value, Error> {
        let previous = self.unserialization.clone();
        let state = previous.clone().unwrap_or_default();
        self.unserialization = Some(state.clone());
        let depth = state.borrow().depth;
        let mut decoder = Decoder {
            bytes,
            function: function.into(),
            pos: 0,
            state: state.clone(),
            line,
        };
        let parsed = action(self, &mut decoder);
        let offset_error = matches!(parsed, Err(Failure::Syntax)).then_some(decoder.pos);
        let mut result = match parsed {
            Ok(()) | Err(Failure::Syntax) => Ok(()),
            Err(Failure::Runtime(error)) => Err(error),
        };
        drop(decoder);
        state.borrow_mut().depth = depth;
        self.unserialization = previous.clone();
        if previous.is_none() {
            result = self.finish_unserialization(&state, result, line);
        }
        drop(state);
        if let Some(offset) = offset_error {
            result = Err(self.array_iterator_exception(
                b"UnexpectedValueException",
                format!("Error at offset {offset} of {} bytes", bytes.len()),
                line,
            ));
        }
        self.drain_destructors(result.map(|()| Value::Null), line)
    }

    fn finish_unserialization(
        &mut self,
        state: &Rc<RefCell<UnserializeState>>,
        mut result: Result<(), Error>,
        line: usize,
    ) -> Result<(), Error> {
        // Resolve parser slot indirections before user callbacks can mutate the graph.
        for array in &state.borrow().arrays {
            let entries: Vec<_> = array.iter().collect();
            array.mutate_shared(|table| {
                for (key, value) in entries {
                    table.insert(key, value);
                }
            });
        }
        for object in &state.borrow().objects {
            if let Some(data) = object.object_storage.borrow_mut().as_mut() {
                data.finish_unserialization();
            }
            if let Some(data) = object.linked_list.borrow_mut().as_mut() {
                data.finish_unserialization();
            }
            let entries: Vec<_> = object.properties.borrow().iter().collect();
            let mut props = object.properties.borrow_mut();
            for (key, value) in entries {
                props.insert(key, value);
            }
        }
        let hooks = std::mem::take(&mut state.borrow_mut().hooks);
        for hook in hooks {
            if result.is_err() {
                hook.object._handle.destructor_called.set(true);
                continue;
            }
            let name = if hook.data.is_some() {
                b"__unserialize".as_slice()
            } else {
                b"__wakeup"
            };
            let values = hook.data.into_iter().map(Value::Array).collect::<Vec<_>>();
            result = self.unserialize_hook(&hook.object, name, &values, true, line);
            if result.is_err() {
                hook.object._handle.destructor_called.set(true);
            }
        }
        result
    }

    fn unserialize_hook(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        values: &[Value],
        isolated: bool,
        line: usize,
    ) -> Result<(), Error> {
        let method = objects::method(&object.class, name).unwrap();
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        let previous = if isolated {
            self.unserialization.take()
        } else {
            None
        };
        let serialization = if isolated {
            self.serialization.take()
        } else {
            None
        };
        let result = self.call_callback_at(&target, values, line, true).map(drop);
        if isolated {
            self.unserialization = previous;
            self.serialization = serialization;
        }
        result
    }
}

impl Decoder<'_> {
    pub(super) fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }
    pub(super) fn value(&mut self, runtime: &mut Runtime) -> Parsed<Cell> {
        let slot = cell(Value::Null);
        self.parse(runtime, slot.clone())?;
        Ok(slot)
    }
    pub(super) fn track_native_object(&self, object: &Rc<PhpObject>) {
        self.state.borrow_mut().objects.push(object.clone());
    }
    pub(super) fn load_native_members(
        &self,
        runtime: &mut Runtime,
        object: &Rc<PhpObject>,
        members: &PhpArray,
    ) -> Parsed<()> {
        // Keep parser buckets live until the outermost decode completes: a later R
        // can turn an already loaded member into a shared reference.
        let entries: Vec<_> = members
            .entries()
            .iter()
            .map(|(key, value)| {
                let value = match value {
                    Value::Indirect(slot) => Value::Indirect(slot.clone()),
                    value => value.clone(),
                };
                (key.clone(), value)
            })
            .collect();
        for (key, value) in entries {
            runtime.load_native_object_member(object, key, value, self.line)?;
        }
        Ok(())
    }
    fn warning(&self, runtime: &mut Runtime, message: impl AsRef<str>) -> Parsed<()> {
        runtime.warn(format!(
            "Warning: {}(): {} on line {}",
            self.function,
            message.as_ref(),
            self.line
        ))?;
        Ok(())
    }
    pub(super) fn expect(&mut self, byte: u8) -> Parsed<()> {
        if self.bytes.get(self.pos) != Some(&byte) {
            return Err(Failure::Syntax);
        }
        self.pos += 1;
        Ok(())
    }
    fn header(&mut self, start: usize, suffix: &[u8]) -> Parsed<usize> {
        let mut p = start + 2;
        if self.bytes.get(start + 1) != Some(&b':') {
            return Err(Failure::Syntax);
        }
        let first = p;
        let mut value = 0usize;
        while let Some(b'0'..=b'9') = self.bytes.get(p) {
            value = value
                .wrapping_mul(10)
                .wrapping_add((self.bytes[p] - b'0') as usize);
            p += 1;
        }
        if p == first || self.bytes.get(p..p + suffix.len()) != Some(suffix) {
            return Err(Failure::Syntax);
        }
        self.pos = p + suffix.len();
        Ok(value)
    }
    fn integer(&mut self, runtime: &mut Runtime, start: usize) -> Parsed<i64> {
        let mut p = start;
        let negative = self.bytes.get(p) == Some(&b'-');
        if self.bytes.get(p).is_some_and(|b| matches!(b, b'+' | b'-')) {
            p += 1;
        }
        while self.bytes.get(p) == Some(&b'0') {
            p += 1;
        }
        let digits = p;
        let mut number = 0u64;
        let mut overflow = false;
        while let Some(b'0'..=b'9') = self.bytes.get(p) {
            match number
                .checked_mul(10)
                .and_then(|n| n.checked_add((self.bytes[p] - b'0') as u64))
            {
                Some(value) => number = value,
                None => overflow = true,
            }
            p += 1;
        }
        self.pos = p;
        if overflow || p - digits > 19 || number > i64::MAX as u64 + u64::from(negative) {
            self.warning(runtime, "Numerical result out of range")?;
            return Ok(if negative { i64::MIN } else { i64::MAX });
        }
        Ok(if negative {
            number.wrapping_neg() as i64
        } else {
            number as i64
        })
    }
    fn string(
        &mut self,
        runtime: &mut Runtime,
        start: usize,
        escaped: bool,
        terminator: u8,
    ) -> Parsed<Vec<u8>> {
        let len = self.header(start, b":\"")?;
        if len > self.bytes.len() - self.pos {
            self.pos = start + 2;
            return Err(Failure::Syntax);
        }
        let mut output = Vec::new();
        if escaped {
            for _ in 0..len {
                let byte = *self.bytes.get(self.pos).ok_or(Failure::Syntax)?;
                self.pos += 1;
                if byte == b'\\' {
                    let a = self
                        .bytes
                        .get(self.pos)
                        .and_then(|b| (*b as char).to_digit(16));
                    let b = self
                        .bytes
                        .get(self.pos + 1)
                        .and_then(|b| (*b as char).to_digit(16));
                    let (Some(a), Some(b)) = (a, b) else {
                        self.pos = start;
                        return Err(Failure::Syntax);
                    };
                    output.push((a * 16 + b) as u8);
                    self.pos += 2;
                } else {
                    output.push(byte);
                }
            }
        } else {
            output.extend_from_slice(&self.bytes[self.pos..self.pos + len]);
            self.pos += len;
        }
        self.expect(b'"')?;
        self.expect(terminator)?;
        if escaped {
            runtime.warn(format!(
                "Deprecated: {}(): Unserializing the 'S' format is deprecated on line {}",
                self.function, self.line
            ))?;
        }
        Ok(output)
    }
    fn scalar(&mut self, runtime: &mut Runtime, slot: &Cell, track: bool) -> Parsed<bool> {
        let start = self.pos;
        let tag = *self.bytes.get(start).ok_or(Failure::Syntax)?;
        if track && tag != b'R' {
            self.state.borrow_mut().slots.push(Some(slot.clone()));
        }
        let value = match tag {
            b'N' if self.bytes.get(start..start + 2) == Some(b"N;") => {
                self.pos += 2;
                Value::Null
            }
            b'b' if self.bytes.get(start..start + 4) == Some(b"b:0;") => {
                self.pos += 4;
                Value::Bool(false)
            }
            b'b' if self.bytes.get(start..start + 4) == Some(b"b:1;") => {
                self.pos += 4;
                Value::Bool(true)
            }
            b'i' if self.bytes.get(start + 1) == Some(&b':') => {
                // The lexer accepts the entire scalar token before parsing its value.
                let end = self.bytes[start + 2..]
                    .iter()
                    .position(|b| *b == b';')
                    .map(|n| start + 2 + n)
                    .ok_or(Failure::Syntax)?;
                let token = &self.bytes[start + 2..end];
                let digits = token
                    .strip_prefix(b"-")
                    .or_else(|| token.strip_prefix(b"+"))
                    .unwrap_or(token);
                if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
                    return Err(Failure::Syntax);
                }
                let number = self.integer(runtime, start + 2)?;
                self.pos = end + 1;
                Value::Int(number)
            }
            b'd' if self.bytes.get(start + 1) == Some(&b':') => {
                let end = self.bytes[start + 2..]
                    .iter()
                    .position(|b| *b == b';')
                    .map(|n| start + 2 + n)
                    .ok_or(Failure::Syntax)?;
                let token = &self.bytes[start + 2..end];
                let number = match token {
                    b"NAN" => f64::NAN,
                    b"INF" => f64::INFINITY,
                    b"-INF" => f64::NEG_INFINITY,
                    _ => {
                        if !valid_float(token) {
                            return Err(Failure::Syntax);
                        }
                        std::str::from_utf8(token)
                            .ok()
                            .and_then(|s| s.parse::<f64>().ok())
                            .ok_or(Failure::Syntax)?
                    }
                };
                self.pos = end + 1;
                Value::Float(number)
            }
            b's' | b'S' => Value::String(self.string(runtime, start, tag == b'S', b';')?.into()),
            b'R' | b'r' => {
                let index = self.header(start, b";")?;
                if !track {
                    return Err(Failure::Syntax);
                }
                let target = index
                    .checked_sub(1)
                    .and_then(|i| self.state.borrow().slots.get(i).cloned())
                    .flatten()
                    .ok_or(Failure::Syntax)?;
                if Rc::ptr_eq(&target, slot) {
                    return Err(Failure::Syntax);
                }
                if tag == b'r' {
                    let value = target.borrow().dereferenced();
                    if !matches!(value, Value::Object(_)) {
                        return Err(Failure::Syntax);
                    }
                    value
                } else {
                    let reference = {
                        let mut value = target.borrow_mut();
                        if let Value::Reference(reference) = &*value {
                            reference.clone()
                        } else {
                            let reference = cell(std::mem::replace(&mut *value, Value::Null));
                            *value = Value::Reference(reference.clone());
                            reference
                        }
                    };
                    if Rc::ptr_eq(&reference, slot) {
                        return Err(Failure::Syntax);
                    }
                    if let Some((object, key)) = self
                        .state
                        .borrow()
                        .properties
                        .get(&(Rc::as_ptr(&target) as usize))
                    {
                        runtime.register_unserialized_reference(object, key, &reference);
                    }
                    Value::Reference(reference)
                }
            }
            b'a' if !track => {
                self.header(start, b":{")?;
                return Err(Failure::Syntax);
            }
            b'a' | b'O' | b'C' | b'E' if track => return Ok(false),
            b'}' => {
                self.warning(runtime, "Unexpected end of serialized data")?;
                return Err(Failure::Syntax);
            }
            _ => return Err(Failure::Syntax),
        };
        *slot.borrow_mut() = value;
        Ok(true)
    }
    fn enter(&self, runtime: &mut Runtime) -> Parsed<()> {
        let state = self.state.borrow();
        if state.options.max_depth != 0 && state.depth >= state.options.max_depth {
            let max_depth = state.options.max_depth;
            drop(state);
            self.warning(runtime, format!("Maximum depth of {max_depth} exceeded. The depth limit can be changed using the max_depth unserialize() option or the unserialize_max_depth ini setting"))?;
            return Err(Failure::Syntax);
        }
        drop(state);
        self.state.borrow_mut().depth += 1;
        Ok(())
    }
    fn array(&self) -> Rc<PhpArray> {
        let array = Rc::new(PhpArray::default());
        self.state.borrow_mut().arrays.push(array.clone());
        array
    }
    fn parse(&mut self, runtime: &mut Runtime, root: Cell) -> Parsed<()> {
        let mut tasks = vec![Task::Value(root)];
        let result = (|| {
            while let Some(task) = tasks.pop() {
                match task {
                    Task::Value(slot) => {
                        let start = self.pos;
                        if self.scalar(runtime, &slot, true)? {
                            continue;
                        }
                        match self.bytes[start] {
                            b'a' => {
                                self.header(start, b":{")?;
                                let end = self.pos;
                                let count = self.integer(runtime, start + 2)? as usize;
                                self.pos = end;
                                if count >= 1 << 30 || count > (self.bytes.len() - self.pos) / 2 {
                                    return Err(Failure::Syntax);
                                }
                                let array = self.array();
                                *slot.borrow_mut() = Value::Array(array.clone());
                                if count != 0 {
                                    self.enter(runtime)?;
                                }
                                tasks.push(Task::Entries {
                                    container: Container::Array(array),
                                    remaining: count,
                                    hook: None,
                                    counted: count != 0,
                                });
                            }
                            b'O' | b'C' => self.object(runtime, &slot, &mut tasks, start)?,
                            b'E' => self.enumeration(runtime, &slot, start)?,
                            _ => unreachable!(),
                        }
                    }
                    Task::Entries {
                        container,
                        remaining,
                        hook,
                        counted,
                    } => {
                        if remaining == 0 {
                            if counted {
                                self.state.borrow_mut().depth -= 1;
                            }
                            if let Some(hook) = hook {
                                self.state.borrow_mut().hooks.push(hook);
                            }
                            self.expect(b'}')?;
                            continue;
                        }
                        // Keep the continuation live on failure to suppress destructors
                        // for an object whose magic restoration has not been scheduled.
                        tasks.push(Task::Entries {
                            container: container.clone(),
                            remaining: remaining - 1,
                            hook,
                            counted,
                        });
                        let key = cell(Value::Null);
                        self.scalar(runtime, &key, false)?;
                        let key = key.borrow().clone();
                        let key = match key {
                            Value::Int(_) | Value::String(_) => {
                                ArrayKey::from_value(&key, self.line, runtime)?
                            }
                            _ => return Err(Failure::Syntax),
                        };
                        let slot = match container {
                            Container::Array(array) => {
                                let slot = array
                                    .with_value(&key, |value| match value {
                                        Some(Value::Indirect(slot)) => Some(slot.clone()),
                                        _ => None,
                                    })
                                    .unwrap_or_else(|| cell(Value::Null));
                                *slot.borrow_mut() = Value::Null;
                                array.mutate_shared(|table| {
                                    table.insert(key, Value::Indirect(slot.clone()))
                                });
                                slot
                            }
                            Container::Object(object) => {
                                let key = self.property_key(runtime, &object, key)?;
                                let slot = object
                                    .properties
                                    .borrow()
                                    .with_value(&key, |value| match value {
                                        Some(Value::Indirect(slot)) => Some(slot.clone()),
                                        _ => None,
                                    })
                                    .unwrap_or_else(|| cell(Value::Null));
                                *slot.borrow_mut() = Value::Null;
                                object
                                    .properties
                                    .borrow_mut()
                                    .insert(key.clone(), Value::Indirect(slot.clone()));
                                self.state.borrow_mut().properties.insert(
                                    Rc::as_ptr(&slot) as usize,
                                    (object.clone(), key.clone()),
                                );
                                tasks.push(Task::CheckProperty {
                                    object,
                                    key,
                                    slot: slot.clone(),
                                });
                                slot
                            }
                        };
                        tasks.push(Task::Value(slot));
                    }
                    Task::CheckProperty { object, key, slot } => {
                        if let Some(property) = object
                            .class
                            .properties
                            .iter()
                            .find(|p| ArrayKey::String(p.storage_name()) == key)
                        {
                            let reference = match &*slot.borrow() {
                                Value::Reference(reference) => Some(reference.clone()),
                                _ => None,
                            };
                            let result = if let Some(reference) = reference {
                                runtime.check_unserialized_reference(
                                    &object, &key, &reference, self.line,
                                )
                            } else {
                                let owner = object.class.property_slot_owner(property);
                                let value = slot.borrow().dereferenced();
                                runtime
                                    .check_property_value(&owner, property, value, true, self.line)
                                    .map(|value| *slot.borrow_mut() = value)
                            };
                            if let Err(error) = result {
                                object.properties.borrow_mut().remove(&key);
                                return Err(error.into());
                            }
                        }
                    }
                }
            }
            Ok(())
        })();
        if result.is_err() {
            for task in tasks {
                if let Task::Entries {
                    hook: Some(hook), ..
                } = task
                {
                    hook.object._handle.destructor_called.set(true);
                }
            }
        }
        result
    }

    fn property_key(
        &self,
        runtime: &mut Runtime,
        object: &Rc<PhpObject>,
        key: ArrayKey,
    ) -> Parsed<ArrayKey> {
        let raw = match key {
            ArrayKey::String(bytes) => bytes,
            ArrayKey::Int(n) => n.to_string().into_bytes(),
        };
        let key = ArrayKey::String(raw.clone());
        if object
            .class
            .properties
            .iter()
            .any(|p| p.storage_name() == raw && !p.hooks.as_ref().is_some_and(|h| h.is_virtual))
        {
            return Ok(key);
        }
        let decoded = crate::object::property_name(&raw).map_err(|_| Failure::Syntax)?;
        if (decoded.scope.is_none()
            || decoded.scope == Some(b"*")
            || decoded
                .scope
                .is_some_and(|scope| scope.eq_ignore_ascii_case(&object.class.name)))
            && let Some(property) = object
                .class
                .properties
                .iter()
                .rev()
                .find(|p| p.name == decoded.name)
        {
            if property.hooks.as_ref().is_some_and(|h| h.is_virtual) {
                let name = raw.split(|b| *b == 0).next().unwrap();
                self.warning(
                    runtime,
                    format!(
                        "Cannot unserialize value for virtual property {}::${}",
                        String::from_utf8_lossy(&property.declaring_class),
                        String::from_utf8_lossy(name)
                    ),
                )?;
                return Err(Failure::Syntax);
            }
            return Ok(ArrayKey::String(property.storage_name()));
        }
        if object.properties.borrow().get(&key).is_none() {
            if object.class.forbid_dynamic || object.class.declaration.is_readonly {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Cannot create dynamic property {}::${}",
                        object.class.display_name(),
                        String::from_utf8_lossy(decoded.name)
                    ),
                    self.line,
                )
                .into());
            }
            if !object.class.allow_dynamic {
                runtime.warn(format!(
                    "Deprecated: Creation of dynamic property {}::${} is deprecated on line {}",
                    object.class.display_name(),
                    String::from_utf8_lossy(decoded.name),
                    self.line
                ))?;
            }
        }
        Ok(key)
    }
    fn object(
        &mut self,
        runtime: &mut Runtime,
        slot: &Cell,
        tasks: &mut Vec<Task>,
        start: usize,
    ) -> Parsed<()> {
        let custom = self.bytes[start] == b'C';
        let name = self.string(runtime, start, false, b':')?;
        if name.is_empty() {
            self.pos = start + 2;
            return Err(Failure::Syntax);
        }
        // PHP validates the name before committing its header cursor.
        if name.first() == Some(&b'\\') || !valid_class_name(&name) {
            self.pos = start;
            return Err(Failure::Syntax);
        }
        let allowed = self
            .state
            .borrow()
            .options
            .allowed
            .as_ref()
            .is_none_or(|classes| classes.contains(&name.to_ascii_lowercase()));
        let class = if allowed {
            let previous = runtime.unserialization.take();
            let serialization = runtime.serialization.take();
            let result = runtime.load_class(&name, self.line);
            runtime.unserialization = previous;
            runtime.serialization = serialization;
            result?
        } else {
            None
        };
        let incomplete = class.is_none();
        let class = class.unwrap_or_else(|| runtime.find_class(b"__PHP_Incomplete_Class").unwrap());
        if native_serialization_forbidden(&class) {
            return Err(Error::new(
                ErrorKind::Exception,
                format!(
                    "Unserialization of '{}' is not allowed",
                    class.display_name()
                ),
                self.line,
            )
            .into());
        }
        if !custom && self.bytes.len() - (self.pos - 2) <= 2 {
            self.pos -= 2;
            runtime.warn(format!(
                "Warning: Bad unserialize data on line {}",
                self.line
            ))?;
            return Err(Failure::Syntax);
        }
        let header_end = self.pos - 2;
        let count = self.integer(runtime, self.pos)?;
        if count < 0 || (!custom && count as usize > (self.bytes.len() - header_end) / 2) {
            return Err(Failure::Syntax);
        }
        let count = count as usize;
        self.expect(b':')?;
        self.expect(b'{')?;
        let has_hook = !incomplete && objects::method(&class, b"__unserialize").is_some();
        if !custom && class.is_a(b"Serializable") && !has_hook {
            runtime.warn(format!(
                "Warning: Erroneous data format for unserializing '{}' on line {}",
                class.display_name(),
                self.line
            ))?;
            return Err(Failure::Syntax);
        }
        if !custom && count >= 1 << 30 {
            return Err(Failure::Syntax);
        }
        if custom {
            if count >= self.bytes.len() - self.pos {
                runtime.warn(format!("Warning: Insufficient data for unserializing - {count} required, {} present on line {}", self.bytes.len() - self.pos, self.line))?;
                return Err(Failure::Syntax);
            }
            if self.bytes[self.pos + count] != b'}' {
                self.pos += count;
                return Err(Failure::Syntax);
            }
        }
        let Value::Object(object) = runtime.instantiate_class(class, &[], false, self.line)? else {
            unreachable!()
        };
        *slot.borrow_mut() = Value::Object(object.clone());
        self.state.borrow_mut().objects.push(object.clone());
        if incomplete {
            object.properties.borrow_mut().insert(
                ArrayKey::String(b"__PHP_Incomplete_Class_Name".to_vec()),
                Value::String(name.into()),
            );
        }
        if custom {
            let data = self.bytes[self.pos..self.pos + count].to_vec();
            if object.class.is_a(b"Serializable") {
                runtime.unserialize_hook(
                    &object,
                    b"unserialize",
                    &[Value::String(data.into())],
                    false,
                    self.line,
                )?;
            } else {
                runtime.warn(format!(
                    "Warning: Class {} has no unserializer on line {}",
                    object.class.display_name(),
                    self.line
                ))?;
            }
            self.pos += count + 1;
            return Ok(());
        }
        let hook = if has_hook {
            Some(Hook {
                object: object.clone(),
                data: Some(self.array()),
            })
        } else if !incomplete && objects::method(&object.class, b"__wakeup").is_some() {
            Some(Hook {
                object: object.clone(),
                data: None,
            })
        } else {
            None
        };
        let container = hook
            .as_ref()
            .and_then(|hook| hook.data.clone())
            .map_or_else(|| Container::Object(object.clone()), Container::Array);
        if let Err(error) = self.enter(runtime) {
            if hook.is_some() {
                object._handle.destructor_called.set(true);
            }
            return Err(error);
        }
        tasks.push(Task::Entries {
            container,
            remaining: count,
            hook,
            counted: true,
        });
        Ok(())
    }
    fn enumeration(&mut self, runtime: &mut Runtime, slot: &Cell, start: usize) -> Parsed<()> {
        let name = self.string(runtime, start, false, b';')?;
        let end = self.pos;
        self.pos = start;
        let Some(colon) = name.iter().position(|b| *b == b':') else {
            self.warning(
                runtime,
                format!(
                    "Invalid enum name '{}' (missing colon)",
                    String::from_utf8_lossy(&name)
                ),
            )?;
            return Err(Failure::Syntax);
        };
        let enum_name = &name[..colon];
        let case_name = &name[colon + 1..];
        if !valid_class_name(enum_name) {
            return Err(Failure::Syntax);
        }
        let Some(class) = runtime.load_class(enum_name, self.line)? else {
            self.warning(
                runtime,
                format!("Class '{}' not found", String::from_utf8_lossy(enum_name)),
            )?;
            return Err(Failure::Syntax);
        };
        if !class.declaration.is_enum {
            self.warning(
                runtime,
                format!(
                    "Class '{}' is not an enum",
                    String::from_utf8_lossy(enum_name)
                ),
            )?;
            return Err(Failure::Syntax);
        }
        self.pos = end;
        let Some(constant) = class
            .declaration
            .constants
            .iter()
            .find(|c| c.name == case_name)
        else {
            self.warning(
                runtime,
                format!(
                    "Undefined constant {}::{}",
                    String::from_utf8_lossy(enum_name),
                    String::from_utf8_lossy(case_name)
                ),
            )?;
            return Err(Failure::Syntax);
        };
        if !constant.is_enum_case {
            self.warning(
                runtime,
                format!(
                    "{}::{} is not an enum case",
                    String::from_utf8_lossy(enum_name),
                    String::from_utf8_lossy(case_name)
                ),
            )?;
            return Err(Failure::Syntax);
        }
        *slot.borrow_mut() = runtime.evaluate_class_constant(&class, case_name, self.line)?;
        Ok(())
    }
}

fn valid_float(bytes: &[u8]) -> bool {
    let bytes = bytes
        .strip_prefix(b"+")
        .or_else(|| bytes.strip_prefix(b"-"))
        .unwrap_or(bytes);
    let mut p = 0;
    while bytes.get(p).is_some_and(u8::is_ascii_digit) {
        p += 1;
    }
    let mut digits = p;
    if bytes.get(p) == Some(&b'.') {
        p += 1;
        while bytes.get(p).is_some_and(u8::is_ascii_digit) {
            p += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return false;
    }
    if bytes.get(p).is_some_and(|b| matches!(b, b'e' | b'E')) {
        p += 1;
        if bytes.get(p).is_some_and(|b| matches!(b, b'+' | b'-')) {
            p += 1;
        }
        let start = p;
        while bytes.get(p).is_some_and(u8::is_ascii_digit) {
            p += 1;
        }
        if start == p {
            return false;
        }
    }
    p == bytes.len()
}
