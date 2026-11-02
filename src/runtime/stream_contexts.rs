// Altered Rust adaptation of PHP 8.5.0 ext/standard/streamsfuncs.c,
// main/streams/streams.c and Zend/zend_API.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md for its license.
use super::functions::CallTarget;
use super::*;
use crate::resource::{Kind, PhpResource};

#[derive(Clone, Copy, Debug)]
pub(super) enum Operation {
    Create,
    GetDefault,
    SetDefault,
    GetOptions,
    GetParams,
    SetOption,
    SetOptions,
    SetParams,
}

#[derive(Default)]
pub(crate) struct Context {
    options: Rc<PhpArray>,
    notification: Option<(CallTarget, Value)>,
}

impl std::fmt::Debug for Context {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Context").finish_non_exhaustive()
    }
}

pub(super) fn parameters(operation: Operation) -> &'static [(&'static str, bool, bool)] {
    match operation {
        Operation::Create => &[("options", true, false), ("params", true, false)],
        Operation::GetDefault => &[("options", true, false)],
        Operation::SetDefault => &[("options", false, false)],
        Operation::GetOptions => &[("stream_or_context", false, false)],
        Operation::GetParams => &[("context", false, false)],
        Operation::SetOptions => &[("context", false, false), ("options", false, false)],
        Operation::SetParams => &[("context", false, false), ("params", false, false)],
        Operation::SetOption => &[
            ("context", false, false),
            ("wrapper_or_options", false, false),
            ("option_name", true, false),
            ("value", true, false),
        ],
    }
}

impl Runtime {
    pub(super) fn stream_context_builtin(
        &mut self,
        operation: Operation,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(
            operation,
            Operation::Create | Operation::GetDefault | Operation::SetDefault
        ) {
            let options = context_array(
                args[0].as_ref(),
                name,
                1,
                !matches!(operation, Operation::SetDefault),
                line,
            )?;
            let params = if matches!(operation, Operation::Create) {
                context_array(args[1].as_ref(), name, 2, true, line)?
            } else {
                None
            };
            let context = if matches!(operation, Operation::Create) {
                self.resources.allocate(Kind::Context(Box::default()))
            } else {
                self.resources.default_context()
            };
            if let Some(options) = options {
                self.context_options(&context, options, line)?;
            }
            if let Some(params) = params {
                self.context_params(&context, params, name, line)?;
            }
            return Ok(Value::Resource(context));
        }
        if matches!(operation, Operation::SetOption) && args[2].is_none() && args[3].is_none() {
            self.warn(format!("Deprecated: Calling stream_context_set_option() with 2 arguments is deprecated, use stream_context_set_options() instead on line {line}"))?;
        }
        let value = args[0].as_ref().unwrap();
        if !matches!(value, Value::Resource(_)) {
            return Err(builtins::type_error(value, name, 1, "resource", line));
        }
        // PHP parses all argument types before resolving a stream's context.
        let array = if matches!(operation, Operation::SetOptions | Operation::SetParams) {
            context_array(args[1].as_ref(), name, 2, false, line)?
        } else {
            None
        };
        let wrapper = if matches!(operation, Operation::SetOption) {
            Some(self.native_array_or_string(args[1].as_ref().unwrap(), name, 2, line)?)
        } else {
            None
        };
        let option = if matches!(operation, Operation::SetOption) {
            args[2]
                .as_ref()
                .filter(|v| !matches!(v, Value::Null))
                .map(|v| self.native_string_type(v, name, 3, line, "?string"))
                .transpose()?
        } else {
            None
        };
        let context = self.decode_stream_context(value, name, line)?;
        match operation {
            Operation::GetOptions | Operation::GetParams => {
                let kind = context.kind.borrow();
                let Kind::Context(context) = &*kind else {
                    unreachable!()
                };
                if matches!(operation, Operation::GetOptions) {
                    return Ok(Value::Array(context.options.clone()));
                }
                let mut params = PhpArray::default();
                if let Some((_, notification)) = &context.notification {
                    params.insert(
                        ArrayKey::String(b"notification".to_vec()),
                        notification.clone(),
                    );
                }
                params.insert(
                    ArrayKey::String(b"options".to_vec()),
                    Value::Array(context.options.clone()),
                );
                Ok(Value::Array(Rc::new(params)))
            }
            Operation::SetOptions => {
                self.context_options(&context, array.unwrap(), line)?;
                Ok(Value::Bool(true))
            }
            Operation::SetParams => {
                self.context_params(&context, array.unwrap(), name, line)?;
                Ok(Value::Bool(true))
            }
            Operation::SetOption => {
                let wrapper = wrapper.unwrap();
                match wrapper {
                    Value::Array(options) => {
                        if option.is_some() {
                            return Err(context_argument_error(
                                name,
                                3,
                                "option_name",
                                "must be null when argument #2 ($wrapper_or_options) is an array",
                                line,
                            ));
                        }
                        if args[3].is_some() {
                            return Err(context_argument_error(
                                name,
                                4,
                                "value",
                                "cannot be provided when argument #2 ($wrapper_or_options) is an array",
                                line,
                            ));
                        }
                        self.context_options(&context, &options, line)?;
                    }
                    Value::String(wrapper) => {
                        let Some(option) = option else {
                            return Err(context_argument_error(
                                name,
                                3,
                                "option_name",
                                "cannot be null when argument #2 ($wrapper_or_options) is a string",
                                line,
                            ));
                        };
                        let Some(value) = &args[3] else {
                            return Err(context_argument_error(
                                name,
                                4,
                                "value",
                                "must be provided when argument #2 ($wrapper_or_options) is a string",
                                line,
                            ));
                        };
                        context_option(&context, &wrapper.bytes(), &option, value);
                    }
                    _ => unreachable!(),
                }
                Ok(Value::Bool(true))
            }
            _ => unreachable!(),
        }
    }

    fn decode_stream_context(
        &mut self,
        value: &Value,
        name: &str,
        line: usize,
    ) -> Result<Rc<PhpResource>, Error> {
        let Value::Resource(resource) = value else {
            unreachable!()
        };
        let mut kind = resource.kind.borrow_mut();
        match &mut *kind {
            Kind::Context(_) => Ok(resource.clone()),
            Kind::Stream(stream) => {
                if stream.context.is_none() {
                    stream.context = Some(self.resources.allocate(Kind::Context(Box::default())));
                }
                Ok(stream.context.as_ref().unwrap().clone())
            }
            Kind::Closed => Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "{name}(): Argument #1 (${}) must be a valid stream/context",
                    if name == "stream_context_get_options" {
                        "stream_or_context"
                    } else {
                        "context"
                    }
                ),
                line,
            )),
        }
    }

    fn context_options(
        &mut self,
        context: &Rc<PhpResource>,
        options: &PhpArray,
        line: usize,
    ) -> Result<(), Error> {
        for (wrapper, values) in options.iter_copied() {
            let values = values.dereferenced();
            let (ArrayKey::String(wrapper), Value::Array(values)) = (wrapper, values) else {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "Options should have the form [\"wrappername\"][\"optionname\"] = $value",
                    line,
                ));
            };
            for (key, value) in values.iter_copied() {
                if let ArrayKey::String(key) = key {
                    context_option(context, &wrapper, &key, &value);
                }
            }
        }
        Ok(())
    }

    fn context_params(
        &mut self,
        context: &Rc<PhpResource>,
        params: &PhpArray,
        name: &str,
        line: usize,
    ) -> Result<(), Error> {
        if let Some(notification) = params.get(&ArrayKey::String(b"notification".to_vec())) {
            // Replacement clears the old callback even when validation fails.
            let previous = {
                let mut kind = context.kind.borrow_mut();
                let Kind::Context(context) = &mut *kind else {
                    unreachable!()
                };
                context.notification.take()
            };
            drop(previous);
            let notification = notification.dereferenced();
            let target = self.resolve_native_callback(&notification, line).map_err(|error| {
                if error.throwable.is_some() || error.kind == ErrorKind::UnwindExit { return error; }
                Error::new(ErrorKind::TypeError, format!("{name}(): Argument #1 (${}) must be an array with valid callbacks as values, {}", if name == "stream_context_create" { "options" } else { "context" }, self.callback_error(&notification, &error)), line)
            })?;
            let value = context_callback_value(&notification, &target, line)?;
            let mut kind = context.kind.borrow_mut();
            let Kind::Context(context) = &mut *kind else {
                unreachable!()
            };
            context.notification = Some((target, value));
        }
        if let Some(options) = params.get(&ArrayKey::String(b"options".to_vec())) {
            let Value::Array(options) = options else {
                return Err(Error::new(
                    ErrorKind::TypeError,
                    "Invalid stream/context parameter",
                    line,
                ));
            };
            self.context_options(context, &options, line)?;
        }
        Ok(())
    }
}

fn context_array<'a>(
    value: Option<&'a Value>,
    name: &str,
    position: usize,
    nullable: bool,
    line: usize,
) -> Result<Option<&'a PhpArray>, Error> {
    match value {
        None => Ok(None),
        Some(Value::Null) if nullable => Ok(None),
        Some(Value::Array(array)) => Ok(Some(array)),
        Some(value) => Err(builtins::type_error(
            value,
            name,
            position,
            if nullable { "?array" } else { "array" },
            line,
        )),
    }
}

fn context_option(resource: &Rc<PhpResource>, wrapper: &[u8], option: &[u8], value: &Value) {
    let wrapper = ArrayKey::String(wrapper.split(|&b| b == 0).next().unwrap().to_vec());
    let option = ArrayKey::String(option.split(|&b| b == 0).next().unwrap().to_vec());
    let mut kind = resource.kind.borrow_mut();
    let Kind::Context(context) = &mut *kind else {
        unreachable!()
    };
    let options = Rc::make_mut(&mut context.options);
    let mut group = match options.get(&wrapper) {
        Some(Value::Array(group)) => group,
        _ => Rc::new(PhpArray::default()),
    };
    Rc::make_mut(&mut group).insert(option, value.dereferenced());
    options.insert(wrapper, Value::Array(group));
}

fn context_argument_error(
    name: &str,
    position: usize,
    parameter: &str,
    message: &str,
    line: usize,
) -> Error {
    Error::new(
        ErrorKind::ValueError,
        format!("{name}(): Argument #{position} (${parameter}) {message}"),
        line,
    )
}

fn context_callback_value(value: &Value, target: &CallTarget, line: usize) -> Result<Value, Error> {
    if let Some(closure) = &target.closure {
        return Ok(Value::Closure(closure.clone()));
    }
    if matches!(value, Value::Object(_)) {
        return Ok(value.clone());
    }
    let name = target.magic_name.as_deref().unwrap_or_else(|| {
        target
            .function
            .as_ref()
            .map_or(target.name.as_slice(), |f| f.name.as_slice())
    });
    if let Some(context) = &target.context {
        let mut array = PhpArray::default();
        array.append(
            target
                .this
                .clone()
                .map(Value::Object)
                .unwrap_or_else(|| Value::String(context.called.name.clone().into())),
            line,
        )?;
        array.append(
            Value::String(name.rsplit(|&b| b == b':').next().unwrap().to_vec().into()),
            line,
        )?;
        Ok(Value::Array(Rc::new(array)))
    } else {
        Ok(Value::String(name.to_vec().into()))
    }
}
