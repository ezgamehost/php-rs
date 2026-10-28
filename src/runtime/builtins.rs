use super::*;
use crate::ast::Parameter;

#[derive(Debug, Clone, Copy)]
pub(super) enum Builtin {
    Exit,
    Clone,
    Strlen,
    StrRepeat,
    StrRev,
    Chr,
    Ord,
    Count,
    VarDump,
    GetType,
    GetDebugType,
    GetClass,
    GetParentClass,
    GetCalledClass,
    ClassExists,
    ClassAlias,
    IsA,
    IsSubclassOf,
    InterfaceExists,
    TraitExists,
    MethodExists,
    PropertyExists,
    GetObjectVars,
    Is(&'static str),
    ArrayKeys,
    ArrayValues,
    ArrayKeyExists,
    ArrayMap,
    ArrayFilter,
    ArrayReduce,
    CallUserFunc,
    CallUserFuncArray,
    FunctionExists,
    Define,
    Defined,
    Constant,
    GetIncludedFiles,
    FuncNumArgs,
    FuncGetArg,
    FuncGetArgs,
    ErrorReporting,
    AutoloadRegister,
    AutoloadUnregister,
    AutoloadFunctions,
    AutoloadCall,
    Autoload,
    AutoloadExtensions,
}
impl Builtin {
    pub(super) fn lookup(name: &[u8]) -> Option<Self> {
        Some(match name {
            b"exit" | b"die" => Self::Exit,
            b"clone" => Self::Clone,
            b"strlen" => Self::Strlen,
            b"str_repeat" => Self::StrRepeat,
            b"strrev" => Self::StrRev,
            b"chr" => Self::Chr,
            b"ord" => Self::Ord,
            b"count" | b"sizeof" => Self::Count,
            b"var_dump" => Self::VarDump,
            b"gettype" => Self::GetType,
            b"get_debug_type" => Self::GetDebugType,
            b"get_class" => Self::GetClass,
            b"get_parent_class" => Self::GetParentClass,
            b"get_called_class" => Self::GetCalledClass,
            b"class_exists" => Self::ClassExists,
            b"class_alias" => Self::ClassAlias,
            b"is_a" => Self::IsA,
            b"is_subclass_of" => Self::IsSubclassOf,
            b"interface_exists" => Self::InterfaceExists,
            b"trait_exists" => Self::TraitExists,
            b"method_exists" => Self::MethodExists,
            b"property_exists" => Self::PropertyExists,
            b"get_object_vars" => Self::GetObjectVars,
            b"is_int" | b"is_integer" | b"is_long" => Self::Is("int"),
            b"is_float" | b"is_double" => Self::Is("float"),
            b"is_bool" => Self::Is("bool"),
            b"is_null" => Self::Is("null"),
            b"is_string" => Self::Is("string"),
            b"is_object" => Self::Is("object"),
            b"is_array" => Self::Is("array"),
            b"array_keys" => Self::ArrayKeys,
            b"array_values" => Self::ArrayValues,
            b"array_map" => Self::ArrayMap,
            b"array_filter" => Self::ArrayFilter,
            b"array_reduce" => Self::ArrayReduce,
            b"call_user_func" => Self::CallUserFunc,
            b"call_user_func_array" => Self::CallUserFuncArray,
            b"array_key_exists" | b"key_exists" => Self::ArrayKeyExists,
            b"function_exists" => Self::FunctionExists,
            b"define" => Self::Define,
            b"defined" => Self::Defined,
            b"constant" => Self::Constant,
            b"get_included_files" | b"get_required_files" => Self::GetIncludedFiles,
            b"func_num_args" => Self::FuncNumArgs,
            b"func_get_arg" => Self::FuncGetArg,
            b"func_get_args" => Self::FuncGetArgs,
            b"error_reporting" => Self::ErrorReporting,
            b"spl_autoload_register" => Self::AutoloadRegister,
            b"spl_autoload_unregister" => Self::AutoloadUnregister,
            b"spl_autoload_functions" => Self::AutoloadFunctions,
            b"spl_autoload_call" => Self::AutoloadCall,
            b"spl_autoload" => Self::Autoload,
            b"spl_autoload_extensions" => Self::AutoloadExtensions,
            _ => return None,
        })
    }
    pub(super) fn parameters(self) -> Vec<Parameter> {
        let definitions: &[(&str, bool, bool)] = match self {
            Self::GetClass => &[("object", true, false)],
            Self::Clone => &[("object", false, false), ("withProperties", true, false)],
            Self::GetParentClass => &[("object_or_class", true, false)],
            Self::GetCalledClass => &[],
            Self::GetObjectVars => &[("object", false, false)],
            Self::ClassExists => &[("class", false, false), ("autoload", true, false)],
            Self::ClassAlias => &[
                ("class", false, false),
                ("alias", false, false),
                ("autoload", true, false),
            ],
            Self::IsA | Self::IsSubclassOf => &[
                ("object_or_class", false, false),
                ("class", false, false),
                ("allow_string", true, false),
            ],
            Self::InterfaceExists => &[("interface", false, false), ("autoload", true, false)],
            Self::TraitExists => &[("trait", false, false), ("autoload", true, false)],
            Self::MethodExists => &[("object_or_class", false, false), ("method", false, false)],
            Self::PropertyExists => &[
                ("object_or_class", false, false),
                ("property", false, false),
            ],
            Self::Exit => &[("status", true, false)],
            Self::GetIncludedFiles | Self::FuncNumArgs | Self::FuncGetArgs => &[],
            Self::FuncGetArg => &[("position", false, false)],
            Self::AutoloadRegister => &[
                ("callback", true, false),
                ("throw", true, false),
                ("prepend", true, false),
            ],
            Self::AutoloadUnregister => &[("callback", false, false)],
            Self::AutoloadFunctions => &[],
            Self::AutoloadCall => &[("class", false, false)],
            Self::Autoload => &[("class", false, false), ("file_extensions", true, false)],
            Self::AutoloadExtensions => &[("file_extensions", true, false)],
            Self::ErrorReporting => &[("error_level", true, false)],
            Self::Strlen | Self::StrRev => &[("string", false, false)],
            Self::StrRepeat => &[("string", false, false), ("times", false, false)],
            Self::Chr => &[("codepoint", false, false)],
            Self::Ord => &[("character", false, false)],
            Self::Count => &[("value", false, false), ("mode", true, false)],
            Self::VarDump => &[("value", false, false), ("values", false, true)],
            Self::GetType | Self::GetDebugType | Self::Is(_) => &[("value", false, false)],
            Self::ArrayKeys => &[
                ("array", false, false),
                ("filter_value", true, false),
                ("strict", true, false),
            ],
            Self::ArrayValues => &[("array", false, false)],
            Self::ArrayMap => &[
                ("callback", false, false),
                ("array", false, false),
                ("arrays", false, true),
            ],
            Self::ArrayFilter => &[
                ("array", false, false),
                ("callback", true, false),
                ("mode", true, false),
            ],
            Self::ArrayReduce => &[
                ("array", false, false),
                ("callback", false, false),
                ("initial", true, false),
            ],
            Self::ArrayKeyExists => &[("key", false, false), ("array", false, false)],
            Self::CallUserFunc => &[("callback", false, false), ("args", false, true)],
            Self::CallUserFuncArray => &[("callback", false, false), ("args", false, false)],
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
                by_reference: false,
                promotion: None,
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
        dynamic: bool,
    ) -> Result<Value, Error> {
        if matches!(
            builtin,
            Builtin::AutoloadRegister
                | Builtin::AutoloadUnregister
                | Builtin::AutoloadFunctions
                | Builtin::AutoloadCall
                | Builtin::Autoload
                | Builtin::AutoloadExtensions
        ) {
            return self.autoload_builtin(builtin, &args, line);
        }
        if matches!(builtin, Builtin::ErrorReporting) {
            let previous = self.diagnostics.reporting;
            if let Some(value) = args[0]
                .as_ref()
                .filter(|value| !matches!(value, Value::Null))
            {
                self.diagnostics.reporting = self.native_int(value, "error_reporting", 1, line).map_err(|error| {
                    if error.kind != ErrorKind::TypeError { return error; }
                    let given = match value {
                        Value::Bool(true) => "true",
                        Value::Bool(false) => "false",
                        _ => debug_type(value),
                    };
                    Error::new(ErrorKind::TypeError, format!("error_reporting(): Argument #1 ($error_level) must be of type ?int, {given} given"), line)
                })? as i32 as i64;
            }
            return Ok(Value::Int(previous));
        }
        if matches!(
            builtin,
            Builtin::GetClass | Builtin::GetParentClass | Builtin::GetCalledClass
        ) {
            return self.inspect_class(builtin, args.first().and_then(Option::as_ref), line);
        }
        if matches!(builtin, Builtin::Exit) {
            return self.terminate(args[0].as_ref().unwrap_or(&Value::Int(0)), line);
        }
        if matches!(
            builtin,
            Builtin::FuncNumArgs | Builtin::FuncGetArg | Builtin::FuncGetArgs
        ) {
            return self.inspect_arguments(
                builtin,
                args.first().and_then(Option::as_ref),
                dynamic,
                line,
            );
        }
        if matches!(builtin, Builtin::GetIncludedFiles) {
            let mut paths = PhpArray::default();
            for path in &self.included_files {
                paths.append(
                    Value::String(path.as_os_str().as_encoded_bytes().to_vec().into()),
                    line,
                )?;
            }
            return Ok(Value::Array(Rc::new(paths)));
        }
        let name = String::from_utf8_lossy(name);
        let first = args[0].as_ref().unwrap(); // Required arguments checked by common binder.
        match builtin {
            Builtin::ErrorReporting
            | Builtin::AutoloadRegister
            | Builtin::AutoloadUnregister
            | Builtin::AutoloadFunctions
            | Builtin::AutoloadCall
            | Builtin::Autoload
            | Builtin::AutoloadExtensions => unreachable!(),
            Builtin::Clone => self.clone_value(first, args[1].as_ref(), line),
            Builtin::StrRepeat | Builtin::StrRev | Builtin::Chr | Builtin::Ord => {
                self.string_builtin(builtin, &args, line)
            }
            Builtin::ClassAlias => self.class_alias(&args, line),
            Builtin::IsA | Builtin::IsSubclassOf => self.inspect_inheritance(builtin, &args, line),
            Builtin::GetClass | Builtin::GetParentClass | Builtin::GetCalledClass => unreachable!(),
            Builtin::GetObjectVars => match first {
                Value::Object(object) => Ok(Value::Array(Rc::new(self.object_variables(object)))),
                Value::Closure(_) => Ok(Value::Array(Rc::new(PhpArray::default()))),
                _ => Err(Error::new(
                    ErrorKind::TypeError,
                    "get_object_vars(): Argument #1 ($object) must be of type object",
                    line,
                )),
            },
            Builtin::ClassExists | Builtin::InterfaceExists | Builtin::TraitExists => {
                let class = self.native_string(first, &name, 1, line)?;
                let autoload = args[1]
                    .as_ref()
                    .map(|value| self.native_bool(value, &name, 2, line))
                    .transpose()?
                    .unwrap_or(true);
                let found = if autoload {
                    self.load_class(&class, line)?
                } else {
                    self.find_class(&class)
                };
                Ok(Value::Bool(
                    found.is_some_and(|c| {
                        c.declaration.is_interface == matches!(builtin, Builtin::InterfaceExists)
                            && c.declaration.is_trait == matches!(builtin, Builtin::TraitExists)
                    }) || matches!(builtin, Builtin::ClassExists)
                        && class
                            .strip_prefix(b"\\")
                            .unwrap_or(&class)
                            .eq_ignore_ascii_case(b"Closure"),
                ))
            }
            Builtin::MethodExists | Builtin::PropertyExists => {
                let member = self.native_string(args[1].as_ref().unwrap(), &name, 2, line)?;
                let class = match first {
                    Value::Object(object) => Some(object.class.clone()),
                    Value::String(name) => self.load_class(&name.bytes(), line)?,
                    Value::Closure(_) => None,
                    _ => {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            format!(
                                "{name}(): Argument #1 ($object_or_class) must be of type object|string"
                            ),
                            line,
                        ));
                    }
                };
                let exists = if matches!(builtin, Builtin::MethodExists) {
                    class.is_some_and(|c| {
                        objects::method(&c, &member).is_some_and(|m| {
                            matches!(first, Value::Object(_))
                                || m.visibility != crate::ast::Visibility::Private
                                || method_visibility::declaring_name(&m)
                                    .eq_ignore_ascii_case(&c.name)
                        })
                    })
                } else {
                    class.is_some_and(|c| {
                        c.property_owner(&member).is_some()
                            || c.static_owner(&member).is_some_and(|owner| {
                                owner.name == c.name
                                    || owner.declaration.static_properties.iter().any(|p| {
                                        p.name == member
                                            && p.visibility != crate::ast::Visibility::Private
                                    })
                            })
                    }) || matches!(first, Value::Object(o) if self.check_instance_property_access(o, &member, true, line)? && o.properties.borrow().get(&self.instance_property_key(o, &member)).is_some())
                };
                Ok(Value::Bool(exists))
            }
            Builtin::ArrayMap => self.array_map(&args, &extra, line),
            Builtin::CallUserFunc | Builtin::CallUserFuncArray => {
                self.call_user_function(builtin, &args, &extra, line)
            }
            Builtin::ArrayFilter => self.array_filter(&args, line),
            Builtin::ArrayReduce => self.array_reduce(&args, line),
            Builtin::Exit
            | Builtin::GetIncludedFiles
            | Builtin::FuncNumArgs
            | Builtin::FuncGetArg
            | Builtin::FuncGetArgs => unreachable!(),
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
                    self.diagnostics.push(format!("define(): Argument #3 ($case_insensitive) is ignored since declaration of case-insensitive constants is no longer supported on line {line}"));
                }
                Ok(Value::Bool(self.define_constant(
                    constant,
                    args[1].as_ref().unwrap().clone(),
                    line,
                )))
            }
            Builtin::Defined | Builtin::Constant => {
                let constant = self.native_string(first, &name, 1, line)?;
                if constant.windows(2).any(|p| p == b"::") {
                    self.named_class_constant(&constant, matches!(builtin, Builtin::Defined), line)
                } else if matches!(builtin, Builtin::Defined) {
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
                    Value::Reference(_) | Value::Indirect(_) => {
                        unreachable!("arguments are dereferenced")
                    }
                    Value::Null => "NULL",
                    Value::Bool(_) => "boolean",
                    Value::Int(_) => "integer",
                    Value::Float(_) => "double",
                    Value::String(_) => "string",
                    Value::Array(_) => "array",
                    Value::Closure(_) | Value::Object(_) => "object",
                }
                .as_bytes()
                .to_vec()
                .into(),
            )),
            Builtin::GetDebugType => {
                Ok(Value::String(debug_type(first).as_bytes().to_vec().into()))
            }
            Builtin::Is(kind) => Ok(Value::Bool(
                debug_type(first) == kind
                    || (kind == "object" && matches!(first, Value::Closure(_) | Value::Object(_))),
            )),
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
                dump(first, 0, &mut self.output, line, &mut self.diagnostics);
                for (_, value) in extra.entries() {
                    dump(value, 0, &mut self.output, line, &mut self.diagnostics);
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
                Ok(Value::Int(count(
                    array,
                    mode == 1,
                    &mut HashSet::new(),
                    line,
                    &mut self.diagnostics,
                )))
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
                    let selected = match &args[1] {
                        None => true,
                        Some(filter) if strict => value.identical(filter, line)?,
                        Some(filter) => self.compare(value, filter, line)? == Some(Ordering::Equal),
                    };
                    if selected {
                        result.append(key.to_value(), line)?;
                    }
                }
                Ok(Value::Array(Rc::new(result)))
            }
            Builtin::ArrayKeyExists => {
                let array = require_array(args[1].as_ref().unwrap(), &name, 2, line)?;
                let key = ArrayKey::from_value(first, line, &mut self.diagnostics)?;
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
    pub(super) fn native_string(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        if !self.strict_types && self.can_stringify(value) {
            return self.string_bytes(value, line);
        }
        if matches!(
            value,
            Value::Array(_) | Value::Closure(_) | Value::Object(_)
        ) || (self.strict_types && !matches!(value, Value::String(_)))
        {
            return Err(type_error(value, function, position, "string", line));
        }
        self.warn_null(value, function, position, "string", line);
        value.warn_nan("string", line, &mut self.diagnostics);
        Ok(value.to_bytes())
    }
    pub(super) fn native_int(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<i64, Error> {
        if (self.strict_types && !matches!(value, Value::Int(_)))
            || matches!(
                value,
                Value::Array(_) | Value::Closure(_) | Value::Object(_)
            )
            || matches!(value, Value::String(s) if crate::value::numeric_string(&s.bytes()).is_none())
        {
            return Err(type_error(value, function, position, "int", line));
        }
        let number = match value {
            Value::String(s) => crate::value::numeric_string(&s.bytes()).unwrap(),
            _ => value.clone(),
        };
        if let Value::Float(n) = number
            && (!n.is_finite() || !(-9223372036854775808.0..9223372036854775808.0).contains(&n))
        {
            return Err(type_error(value, function, position, "int", line));
        }
        self.warn_null(value, function, position, "int", line);
        value.integer(line, &mut self.diagnostics)
    }
    pub(super) fn native_bool(
        &mut self,
        value: &Value,
        function: &str,
        position: usize,
        line: usize,
    ) -> Result<bool, Error> {
        if matches!(
            value,
            Value::Array(_) | Value::Closure(_) | Value::Object(_)
        ) || (self.strict_types && !matches!(value, Value::Bool(_)))
        {
            return Err(type_error(value, function, position, "bool", line));
        }
        self.warn_null(value, function, position, "bool", line);
        Ok(value.to_bool(line, &mut self.diagnostics))
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
            self.diagnostics.push(format!("Deprecated: {function}(): Passing null to parameter #{position}{} of type {kind} is deprecated on line {line}", parameter_label(function, position)));
        }
    }
}
fn parameter_label(function: &str, position: usize) -> String {
    Builtin::lookup(function.to_ascii_lowercase().as_bytes())
        .and_then(|builtin| builtin.parameters().get(position - 1).cloned())
        .map_or_else(String::new, |p| {
            format!(" (${})", String::from_utf8_lossy(&p.name))
        })
}
fn type_error(value: &Value, function: &str, position: usize, kind: &str, line: usize) -> Error {
    let given = match value {
        Value::Bool(true) => "true",
        Value::Bool(false) => "false",
        _ => debug_type(value),
    };
    Error::new(
        ErrorKind::TypeError,
        format!(
            "{function}(): Argument #{position}{} must be of type {kind}, {given} given",
            parameter_label(function, position)
        ),
        line,
    )
}
pub(super) fn require_array<'a>(
    value: &'a Value,
    function: &str,
    position: usize,
    line: usize,
) -> Result<&'a PhpArray, Error> {
    match value {
        Value::Array(array) => Ok(array),
        _ => Err(type_error(value, function, position, "array", line)),
    }
}
pub(super) fn debug_type(value: &Value) -> &str {
    match value {
        Value::Reference(_) | Value::Indirect(_) => {
            unreachable!("values are dereferenced before type checks")
        }
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Closure(_) => "Closure",
        Value::Object(object) => object.class_name.split('\0').next().unwrap(),
    }
}
fn count(
    array: &PhpArray,
    recursive: bool,
    seen: &mut HashSet<usize>,
    line: usize,
    warnings: &mut dyn WarningSink,
) -> i64 {
    if !recursive {
        return array.len() as i64;
    }
    let identity = array as *const PhpArray as usize;
    if !seen.insert(identity) {
        warnings.push(format!("count(): Recursion detected on line {line}"));
        return 0;
    }
    let mut result = array.len() as i64;
    for (_, value) in array.entries() {
        if let Value::Array(array) = value.dereferenced() {
            result += count(&array, true, seen, line, warnings);
        }
    }
    seen.remove(&identity);
    result
}
fn dump(
    value: &Value,
    depth: usize,
    out: &mut Vec<u8>,
    line: usize,
    warnings: &mut dyn WarningSink,
) {
    dump_inner(value, depth, out, &mut HashSet::new(), line, warnings);
}
fn dump_inner(
    value: &Value,
    depth: usize,
    out: &mut Vec<u8>,
    seen: &mut HashSet<usize>,
    line: usize,
    warnings: &mut dyn WarningSink,
) {
    match value {
        Value::Indirect(slot) => dump_inner(&slot.borrow(), depth, out, seen, line, warnings),
        Value::Reference(reference) => {
            let value = reference.borrow();
            let recursive = match &*value {
                Value::Array(array) => seen.contains(&(Rc::as_ptr(array) as usize)),
                Value::Object(object) => seen.contains(&object.object_id),
                _ => false,
            };
            if Rc::strong_count(reference) > 1 && !recursive {
                out.push(b'&');
            }
            dump_inner(&value, depth, out, seen, line, warnings);
        }
        Value::Object(object) => {
            if !seen.insert(object.object_id) {
                out.extend_from_slice(b"*RECURSION*\n");
                return;
            }
            let properties = object.snapshot();
            out.extend_from_slice(
                format!(
                    "object({})#{} ({}) {{\n",
                    object.class_name.split('\0').next().unwrap(),
                    object.object_id,
                    properties.len()
                )
                .as_bytes(),
            );
            let mut keys: Vec<ArrayKey> = object
                .class
                .properties
                .iter()
                .filter(|p| {
                    p.ty.is_some()
                        || properties
                            .get(&ArrayKey::String(p.storage_name()))
                            .is_some()
                })
                .map(|p| ArrayKey::String(p.storage_name()))
                .collect();
            for (key, _) in properties.entries() {
                if !keys.contains(key) {
                    keys.push(key.clone());
                }
            }
            for key in &keys {
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                out.extend_from_slice(b"[\"");
                let raw = key.to_value().to_bytes();
                let decoded = crate::object::property_name(&raw).unwrap_or_else(|message| {
                    warnings.push(format!("Notice: {message} on line {line}"));
                    crate::object::PropertyName {
                        scope: None,
                        name: &raw,
                    }
                });
                if let Some(scope) = decoded.scope {
                    let name = decoded.name.split(|b| *b == 0).next().unwrap();
                    out.extend_from_slice(name);
                    if scope.first() == Some(&b'*') {
                        out.extend_from_slice(b"\":protected]=>\n");
                    } else {
                        out.extend_from_slice(b"\":\"");
                        out.extend_from_slice(scope);
                        out.extend_from_slice(b"\":private]=>\n");
                    }
                } else {
                    out.extend_from_slice(decoded.name);
                    out.extend_from_slice(b"\"]=>\n");
                }
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                if let Some(value) = properties.get(key) {
                    dump_inner(value, depth + 2, out, seen, line, warnings);
                } else {
                    let property = object
                        .class
                        .properties
                        .iter()
                        .find(|p| ArrayKey::String(p.storage_name()) == *key)
                        .expect("typed property slot");
                    let owner = object.class.property_slot_owner(property);
                    let ty = property.ty.as_ref().expect("uninitialized typed property");
                    out.extend(
                        format!(
                            "uninitialized({})\n",
                            typed_properties::type_label(ty, &owner)
                        )
                        .as_bytes(),
                    );
                }
            }
            out.extend(std::iter::repeat_n(b' ', depth));
            out.extend_from_slice(b"}\n");
            seen.remove(&object.object_id);
        }
        Value::Closure(closure) => {
            let parameters = if closure.magic_name.is_some() {
                vec![functions::magic_parameter()]
            } else {
                closure
                    .function
                    .as_ref()
                    .map(|f| f.parameters.clone())
                    .unwrap_or_else(|| Builtin::lookup(&closure.name).unwrap().parameters())
            };
            let mut properties = PhpArray::default();
            let name = if closure.magic_name.is_some() {
                &closure.name
            } else {
                closure.function.as_ref().map_or(&closure.name, |f| &f.name)
            };
            properties.insert(
                ArrayKey::String(b"function".to_vec()),
                Value::String(name.clone().into()),
            );
            if let Some(object) = &closure.this {
                properties.insert(
                    ArrayKey::String(b"this".to_vec()),
                    Value::Object(object.clone()),
                );
            }
            if !parameters.is_empty() {
                let mut entries = PhpArray::default();
                for param in parameters {
                    let mut key = if param.by_reference {
                        b"&$".to_vec()
                    } else {
                        b"$".to_vec()
                    };
                    key.extend(param.name);
                    entries.insert(
                        ArrayKey::String(key),
                        Value::String(if param.default.is_some() || param.variadic {
                            b"<optional>".to_vec().into()
                        } else {
                            b"<required>".to_vec().into()
                        }),
                    );
                }
                properties.insert(
                    ArrayKey::String(b"parameter".to_vec()),
                    Value::Array(Rc::new(entries)),
                );
            }
            let mut rendered = Vec::new();
            dump_inner(
                &Value::Array(Rc::new(properties)),
                depth,
                &mut rendered,
                seen,
                line,
                warnings,
            );
            let start = rendered.iter().position(|b| *b == b'(').unwrap();
            out.extend_from_slice(format!("object(Closure)#{} ", closure.object_id).as_bytes());
            out.extend_from_slice(&rendered[start..]);
        }
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
            out.extend_from_slice(&s.bytes());
            out.extend_from_slice(b"\"\n");
        }
        Value::Array(array) => {
            let identity = Rc::as_ptr(array) as usize;
            if !seen.insert(identity) {
                out.extend_from_slice(b"*RECURSION*\n");
                return;
            }
            out.extend_from_slice(format!("array({}) {{\n", array.len()).as_bytes());
            for (key, value) in array.entries() {
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                match key {
                    ArrayKey::Int(n) => out.extend_from_slice(format!("[{n}]=>\n").as_bytes()),
                    ArrayKey::String(s) => {
                        out.extend_from_slice(b"[\"");
                        out.extend_from_slice(s);
                        out.extend_from_slice(b"\"]=>\n");
                    }
                }
                out.extend(std::iter::repeat_n(b' ', depth + 2));
                dump_inner(value, depth + 2, out, seen, line, warnings);
            }
            out.extend(std::iter::repeat_n(b' ', depth));
            out.extend_from_slice(b"}\n");
            seen.remove(&identity);
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
