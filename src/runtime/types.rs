use super::*;
use crate::{ast::TypeDecl, value::numeric_string};

impl Runtime {
    pub(crate) fn constant_default(expr: &Expr) -> Option<Value> {
        Self {
            folding_constants: true,
            ..Self::default()
        }
        .eval(expr)
        .ok()
    }

    pub(crate) fn constant_without_diagnostics(expr: &Expr) -> Option<Value> {
        let mut runtime = Self {
            folding_constants: true,
            ..Self::default()
        };
        let value = runtime.eval(expr).ok()?;
        runtime.diagnostics.is_empty().then_some(value)
    }

    pub(super) fn coerce_type(
        &mut self,
        value: Value,
        ty: &TypeDecl,
        strict: bool,
        line: usize,
    ) -> Result<Option<Value>, Error> {
        let Some(resolved) = self.resolve_context_type(ty) else {
            return Ok(None);
        };
        let ty = &resolved;
        if ty.accepts(&value) || (ty.contains("callable") && self.is_callable(&value)) {
            return Ok(Some(value));
        }
        if !strict && ty.contains("string") && self.can_stringify(&value) {
            return self
                .string_bytes(&value, line)
                .map(|s| Some(Value::String(s.into())));
        }
        Ok(self.coerce_scalar_type(value, ty, strict, line))
    }

    fn coerce_scalar_type(
        &mut self,
        value: Value,
        ty: &TypeDecl,
        strict: bool,
        line: usize,
    ) -> Option<Value> {
        if ty.contains("float")
            && let Value::Int(n) = value
        {
            return Some(Value::Float(n as f64));
        }
        if strict
            || matches!(
                value,
                Value::Null | Value::Array(_) | Value::Closure(_) | Value::Object(_)
            )
        {
            return None;
        }
        // A numeric string chooses int or float by its representation when both
        // are present; other weak scalar unions prefer int, float, string, bool.
        let number = match &value {
            Value::String(bytes) => numeric_string(&bytes.bytes()),
            Value::Bool(b) => Some(Value::Int(i64::from(*b))),
            Value::Int(_) | Value::Float(_) => Some(value.clone()),
            _ => None,
        };
        if ty.contains("int")
            && let Some(number) = &number
        {
            let prefer_float = matches!(value, Value::String(_))
                && matches!(number, Value::Float(_))
                && ty.contains("float");
            let in_range = match number {
                Value::Int(_) => true,
                Value::Float(n) => (-9223372036854775808.0..9223372036854775808.0).contains(n),
                _ => false,
            };
            if !prefer_float && in_range {
                return Some(Value::Int(
                    number.integer(line, &mut self.diagnostics).ok()?,
                ));
            }
        }
        if ty.contains("float")
            && let Some(number) = number
        {
            return Some(Value::Float(number.as_float()));
        }
        if ty.contains("string") {
            value.warn_nan("string", line, &mut self.diagnostics);
            return Some(Value::String(value.to_bytes().into()));
        }
        if ty.contains("bool") {
            return Some(Value::Bool(value.to_bool(line, &mut self.diagnostics)));
        }
        None
    }

    fn is_callable(&self, value: &Value) -> bool {
        self.resolve_callable(value.clone(), 0).is_ok()
    }

    pub(super) fn check_argument(
        &mut self,
        value: Value,
        param: &crate::ast::Parameter,
        function: &FunctionDecl,
        position: usize,
        strict: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let Some(ty) = &param.ty else {
            return Ok(value);
        };
        if matches!(
            function.native,
            Some(
                crate::ast::NativeMethod::ThrowableConstruct
                    | crate::ast::NativeMethod::ErrorExceptionConstruct
            )
        ) && !strict
            && value == Value::Null
            && !ty.contains("null")
            && (ty.contains("string") || ty.contains("int"))
        {
            self.diagnostics.push(format!("Deprecated: {}(): Passing null to parameter #{position} (${}) of type {ty} is deprecated on line {line}",String::from_utf8_lossy(&function.name),String::from_utf8_lossy(&param.name)));
            return Ok(if ty.contains("string") {
                Value::String(Vec::new().into())
            } else {
                Value::Int(0)
            });
        }
        let given = match (&value, function.native.is_some()) {
            (Value::Bool(true), true) => "true",
            (Value::Bool(false), true) => "false",
            _ => builtins::debug_type(&value),
        }
        .to_owned();
        let source = function
            .native
            .is_none()
            .then(|| std::mem::replace(&mut self.source, function.source.clone()));
        let conversion_line = if function.native.is_none() {
            function.line
        } else {
            line
        };
        let converted = self.coerce_type(value, ty, strict, conversion_line);
        if let Some(source) = source {
            self.source = source;
        }
        converted?.ok_or_else(|| {
            Error::new(
                ErrorKind::TypeError,
                format!(
                    "{}(): Argument #{position} (${}) must be of type {ty}, {given} given",
                    String::from_utf8_lossy(&function.name),
                    String::from_utf8_lossy(&param.name)
                ),
                line,
            )
        })
    }

    pub(super) fn check_return(
        &mut self,
        flow: Flow,
        function: &FunctionDecl,
    ) -> Result<Value, Error> {
        let Some(ty) = &function.return_type else {
            return Ok(match flow {
                Flow::Return(v, _) => v,
                _ => Value::Null,
            });
        };
        if ty.contains("void") {
            return Ok(Value::Null);
        }
        let name = String::from_utf8_lossy(&function.name);
        if ty.contains("never") {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!("{name}(): never-returning function must not implicitly return"),
                function.line,
            ));
        }
        let (value, given, line) = match flow {
            Flow::Return(value, line) => {
                let given = builtins::debug_type(&value).to_owned();
                (Some(value), given, line)
            }
            _ => (None, "none".to_owned(), function.line),
        };
        let error = || {
            Error::new(
                ErrorKind::TypeError,
                format!("{name}(): Return value must be of type {ty}, {given} returned"),
                line,
            )
        };
        let converted = match value {
            Some(value) => self.coerce_type(value, ty, function.strict_types, line),
            None => Ok(None),
        };
        match converted {
            Ok(value) => value.ok_or_else(error),
            Err(previous) if previous.kind == ErrorKind::UnwindExit => Err(previous),
            Err(previous) => {
                // PHP's return verifier raises its own TypeError and retains
                // a failed __toString conversion as the previous throwable.
                let error = self.materialize_error(error());
                error
                    .throwable
                    .as_ref()
                    .unwrap()
                    .throwable
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .previous = previous.throwable;
                Err(error)
            }
        }
    }
}
