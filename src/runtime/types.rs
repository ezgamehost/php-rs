use super::*;
use crate::{ast::TypeDecl, value::numeric_string};

impl Runtime {
    pub(crate) fn constant_default(expr: &Expr) -> Option<Value> {
        Self::default().eval(expr).ok()
    }

    pub(super) fn coerce_type(
        &mut self,
        value: Value,
        ty: &TypeDecl,
        strict: bool,
        line: usize,
    ) -> Option<Value> {
        if ty.accepts(&value) || (ty.contains("callable") && self.is_callable(&value)) {
            return Some(value);
        }
        if ty.contains("float")
            && let Value::Int(n) = value
        {
            return Some(Value::Float(n as f64));
        }
        if strict || matches!(value, Value::Null | Value::Array(_)) {
            return None;
        }
        // A numeric string chooses int or float by its representation when both
        // are present; other weak scalar unions prefer int, float, string, bool.
        let number = match &value {
            Value::String(bytes) => numeric_string(bytes),
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
                return Some(Value::Int(number.integer(line, &mut self.warnings).ok()?));
            }
        }
        if ty.contains("float")
            && let Some(number) = number
        {
            return Some(Value::Float(number.as_float()));
        }
        if ty.contains("string") {
            return Some(Value::String(value.to_bytes()));
        }
        if ty.contains("bool") {
            return Some(Value::Bool(value.is_truthy()));
        }
        None
    }

    fn is_callable(&self, value: &Value) -> bool {
        let Value::String(name) = value else {
            return false;
        };
        let name = name
            .strip_prefix(b"\\")
            .unwrap_or(name)
            .to_ascii_lowercase();
        self.functions.contains_key(&name) || builtins::Builtin::lookup(&name).is_some()
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
        let given = builtins::debug_type(&value);
        self.coerce_type(value, ty, strict, line).ok_or_else(|| {
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
                Flow::Return(v) => v,
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
        let (value, given) = match flow {
            Flow::Return(value) => {
                let given = builtins::debug_type(&value);
                (Some(value), given)
            }
            _ => (None, "none"),
        };
        value
            .and_then(|v| self.coerce_type(v, ty, function.strict_types, function.line))
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::TypeError,
                    format!("{name}(): Return value must be of type {ty}, {given} returned"),
                    function.line,
                )
            })
    }
}
