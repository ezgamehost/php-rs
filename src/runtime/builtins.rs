use super::*;
use crate::ast::Parameter;

#[derive(Debug, Clone, Copy)]
pub(super) enum Builtin {
    Strlen,
    Count,
    VarDump,
    GetType,
    GetDebugType,
    Is(&'static str),
    ArrayKeys,
    ArrayValues,
    ArrayKeyExists,
    FunctionExists,
    Define,
    Defined,
    Constant,
}
impl Builtin {
    pub(super) fn lookup(name: &[u8]) -> Option<Self> {
        Some(match name {
            b"strlen" => Self::Strlen,
            b"count" | b"sizeof" => Self::Count,
            b"var_dump" => Self::VarDump,
            b"gettype" => Self::GetType,
            b"get_debug_type" => Self::GetDebugType,
            b"is_int" | b"is_integer" | b"is_long" => Self::Is("int"),
            b"is_float" | b"is_double" => Self::Is("float"),
            b"is_bool" => Self::Is("bool"),
            b"is_null" => Self::Is("null"),
            b"is_string" => Self::Is("string"),
            b"is_array" => Self::Is("array"),
            b"array_keys" => Self::ArrayKeys,
            b"array_values" => Self::ArrayValues,
            b"array_key_exists" | b"key_exists" => Self::ArrayKeyExists,
            b"function_exists" => Self::FunctionExists,
            b"define" => Self::Define,
            b"defined" => Self::Defined,
            b"constant" => Self::Constant,
            _ => return None,
        })
    }
    pub(super) fn parameters(self) -> Vec<Parameter> {
        let definitions: &[(&str, bool, bool)] = match self {
            Self::Strlen => &[("string", false, false)],
            Self::Count => &[("value", false, false), ("mode", true, false)],
            Self::VarDump => &[("value", false, false), ("values", false, true)],
            Self::GetType | Self::GetDebugType | Self::Is(_) => &[("value", false, false)],
            Self::ArrayKeys => &[
                ("array", false, false),
                ("filter_value", true, false),
                ("strict", true, false),
            ],
            Self::ArrayValues => &[("array", false, false)],
            Self::ArrayKeyExists => &[("key", false, false), ("array", false, false)],
            Self::FunctionExists => &[("function", false, false)],
            Self::Define => &[
                ("constant_name", false, false),
                ("value", false, false),
                ("case_insensitive", true, false),
            ],
            Self::Defined | Self::Constant => &[("constant_name", false, false)],
        };
        definitions
            .iter()
            .map(|(name, optional, variadic)| Parameter {
                name: name.as_bytes().to_vec(),
                ty: None,
                implicit_nullable: false,
                default: optional.then_some(Expr {
                    kind: ExprKind::Literal(Value::Null),
                    line: 0,
                }),
                variadic: *variadic,
            })
            .collect()
    }
}
impl Runtime {
    pub(super) fn call_builtin(
        &mut self,
        builtin: Builtin,
        name: &[u8],
        args: Vec<Option<Value>>,
        extra: PhpArray,
        line: usize,
    ) -> Result<Value, Error> {
        let name = String::from_utf8_lossy(name);
        let first = args[0].as_ref().unwrap(); // Required arguments checked by common binder.
        match builtin {
            Builtin::Define => {
                let constant = self.native_string(first, &name, 1, line)?;
                if constant.windows(2).any(|part| part == b"::") {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "define(): Argument #1 ($constant_name) cannot be a class constant",
                        line,
                    ));
                }
                if let Some(flag) = &args[2]
                    && self.native_bool(flag, &name, 3, line)?
                {
                    self.warnings.push(format!("define(): Argument #3 ($case_insensitive) is ignored since declaration of case-insensitive constants is no longer supported on line {line}"));
                }
                Ok(Value::Bool(self.define_constant(
                    constant,
                    args[1].as_ref().unwrap().clone(),
                    line,
                )))
            }
            Builtin::Defined | Builtin::Constant => {
                let constant = self.native_string(first, &name, 1, line)?;
                if matches!(builtin, Builtin::Defined) {
                    Ok(Value::Bool(self.lookup_constant(&constant).is_some()))
                } else {
                    self.read_constant(&constant, line)
                }
            }
            Builtin::Strlen => Ok(Value::Int(
                self.native_string(first, &name, 1, line)?.len() as i64
            )),
            Builtin::GetType => Ok(Value::String(
                match first {
                    Value::Null => "NULL",
                    Value::Bool(_) => "boolean",
                    Value::Int(_) => "integer",
                    Value::Float(_) => "double",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                }
                .as_bytes()
                .to_vec(),
            )),
            Builtin::GetDebugType => Ok(Value::String(debug_type(first).as_bytes().to_vec())),
            Builtin::Is(kind) => Ok(Value::Bool(debug_type(first) == kind)),
            Builtin::VarDump => {
                if extra
                    .entries()
                    .iter()
                    .any(|(k, _)| matches!(k, ArrayKey::String(_)))
                {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        "var_dump() does not accept unknown named parameters",
                        line,
                    ));
                }
                dump(first, 0, &mut self.output);
                for (_, value) in extra.entries() {
                    dump(value, 0, &mut self.output);
                }
                Ok(Value::Null)
            }
            Builtin::Count => {
                let array = require_array(first, &name, 1, line)?;
                let mode = match &args[1] {
                    None => 0,
                    Some(v) => self.native_int(v, &name, 2, line)?,
                };
                if mode != 0 && mode != 1 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        format!(
                            "{name}(): Argument #2 ($mode) must be either COUNT_NORMAL or COUNT_RECURSIVE"
                        ),
                        line,
                    ));
                }
                Ok(Value::Int(count(array, mode == 1)))
            }
            Builtin::ArrayValues => {
                let array = require_array(first, &name, 1, line)?;
                let mut result = PhpArray::default();
                for (_, v) in array.entries() {
                    result.append(v.clone(), line)?;
                }
                Ok(Value::Array(Rc::new(result)))
            }
            Builtin::ArrayKeys => {
                if args[1].is_none() && args[2].is_some() {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        "array_keys(): Argument #2 ($filter_value) must be passed explicitly, because the default value is not known",
                        line,
                    ));
                }
                let array = require_array(first, &name, 1, line)?;
                let strict = match &args[2] {
                    None => false,
                    Some(v) => self.native_bool(v, &name, 3, line)?,
                };
                let mut result = PhpArray::default();
                for (key, value) in array.entries() {
                    if args[1].as_ref().is_none_or(|filter| {
                        if strict {
                            value == filter
                        } else {
                            value.compare(filter) == Some(Ordering::Equal)
                        }
                    }) {
                        result.append(key.to_value(), line)?;
                    }
                }
                Ok(Value::Array(Rc::new(result)))
            }
            Builtin::ArrayKeyExists => {
                let array = require_array(args[1].as_ref().unwrap(), &name, 2, line)?;
                let key = ArrayKey::from_value(first, line, &mut self.warnings)?;
                Ok(Value::Bool(array.get(&key).is_some()))
            }
            Builtin::FunctionExists => {
                let bytes = self.native_string(first, &name, 1, line)?;
                let name = bytes
                    .strip_prefix(b"\\")
                    .unwrap_or(&bytes)
                    .to_ascii_lowercase();
                Ok(Value::Bool(
                    self.functions.contains_key(&name) || Builtin::lookup(&name).is_some(),
                ))
            }
        }
    }
    fn native_string(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        if matches!(value, Value::Array(_))
            || (self.strict_types && !matches!(value, Value::String(_)))
        {
            return Err(type_error(function, position, "string", line));
        }
        self.warn_null(value, function, position, "string", line);
        Ok(value.to_bytes())
    }
    fn native_int(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<i64, Error> {
        if (self.strict_types && !matches!(value, Value::Int(_)))
            || matches!(value, Value::Array(_))
            || matches!(value, Value::String(s) if crate::value::numeric_string(s).is_none())
        {
            return Err(type_error(function, position, "int", line));
        }
        let number = match value {
            Value::String(s) => crate::value::numeric_string(s).unwrap(),
            _ => value.clone(),
        };
        if let Value::Float(n) = number
            && (!n.is_finite() || !(-9223372036854775808.0..9223372036854775808.0).contains(&n))
        {
            return Err(type_error(function, position, "int", line));
        }
        self.warn_null(value, function, position, "int", line);
        value.integer(line, &mut self.warnings)
    }
    fn native_bool(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<bool, Error> {
        if matches!(value, Value::Array(_))
            || (self.strict_types && !matches!(value, Value::Bool(_)))
        {
            return Err(type_error(function, position, "bool", line));
        }
        self.warn_null(value, function, position, "bool", line);
        Ok(value.is_truthy())
    }
    fn warn_null(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        kind: &str,
        line: usize,
    ) {
        if matches!(value, Value::Null) {
            self.warnings.push(format!("Deprecated: {function}(): Passing null to parameter #{position} of type {kind} is deprecated on line {line}"));
        }
    }
}
fn type_error(function: &str, position: usize, kind: &str, line: usize) -> Error {
    Error::new(
        ErrorKind::TypeError,
        format!("{function}(): Argument #{position} must be of type {kind}"),
        line,
    )
}
fn require_array<'a>(
    value: &'a Value,
    function: &str,
    position: usize,
    line: usize,
) -> Result<&'a PhpArray, Error> {
    match value {
        Value::Array(array) => Ok(array),
        _ => Err(type_error(function, position, "array", line)),
    }
}
pub(super) fn debug_type(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Array(_) => "array",
    }
}
fn count(array: &PhpArray, recursive: bool) -> i64 {
    array.len() as i64
        + if recursive {
            array
                .entries()
                .iter()
                .map(|(_, value)| match value {
                    Value::Array(a) => count(a, true),
                    _ => 0,
                })
                .sum::<i64>()
        } else {
            0
        }
}
fn dump(value: &Value, depth: usize, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.extend_from_slice(b"NULL\n"),
        Value::Bool(b) => out.extend_from_slice(if *b {
            b"bool(true)\n"
        } else {
            b"bool(false)\n"
        }),
        Value::Int(n) => out.extend_from_slice(format!("int({n})\n").as_bytes()),
        Value::Float(n) => out.extend_from_slice(format!("float({})\n", dump_float(*n)).as_bytes()),
        Value::String(s) => {
            out.extend_from_slice(format!("string({}) \"", s.len()).as_bytes());
            out.extend(s);
            out.extend_from_slice(b"\"\n");
        }
        Value::Array(array) => {
            out.extend_from_slice(format!("array({}) {{\n", array.len()).as_bytes());
            for (key, value) in array.entries() {
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                match key {
                    ArrayKey::Int(n) => out.extend_from_slice(format!("[{n}]=>\n").as_bytes()),
                    ArrayKey::String(s) => {
                        out.extend_from_slice(b"[\"");
                        out.extend(s);
                        out.extend_from_slice(b"\"]=>\n");
                    }
                }
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                dump(value, depth + 2, out);
            }
            out.extend(std::iter::repeat_n(b' ', depth));
            out.extend_from_slice(b"}\n");
        }
    }
}
fn dump_float(n: f64) -> String {
    if n.is_nan() {
        return "NAN".into();
    }
    if n.is_infinite() {
        return if n.is_sign_negative() { "-INF" } else { "INF" }.into();
    }
    if n == 0.0 {
        return if n.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let scientific = format!("{n:e}");
    let (mantissa, exponent) = scientific.split_once('e').unwrap();
    let exponent: i32 = exponent.parse().unwrap();
    if !(-4..17).contains(&exponent) {
        format!(
            "{}{}E{exponent:+}",
            mantissa,
            if mantissa.contains('.') { "" } else { ".0" }
        )
    } else {
        n.to_string()
    }
}
