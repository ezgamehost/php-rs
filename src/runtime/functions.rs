use super::builtins::Builtin;
use super::function_arguments::FunctionArguments;
use super::*;
use crate::ast::{Argument, Parameter};
use std::collections::HashSet;

impl Runtime {
    pub(super) fn prepare_functions(&mut self, stmts: &mut [Stmt]) -> Result<(), Error> {
        diagnose_functions(stmts, &mut self.warnings);
        self.prepare_classes(stmts)?;
        let mut declarations = Vec::new();
        take_hoisted(stmts, &mut declarations);
        let mut names = HashSet::new();
        for decl in &declarations {
            let name = decl.name.to_ascii_lowercase();
            if self.functions.contains_key(&name)
                || Builtin::lookup(&name).is_some()
                || !names.insert(name)
            {
                return Err(redeclared(decl));
            }
        }
        for decl in declarations {
            self.register_function(decl)?;
        }
        Ok(())
    }
    pub(super) fn register_function(&mut self, decl: Rc<FunctionDecl>) -> Result<(), Error> {
        let name = decl.name.to_ascii_lowercase();
        if self.functions.contains_key(&name) || Builtin::lookup(&name).is_some() {
            return Err(redeclared(&decl));
        }
        self.functions.insert(name, decl);
        Ok(())
    }
    pub(super) fn call(
        &mut self,
        callee: &Expr,
        args: &[Argument],
        line: usize,
    ) -> Result<Value, Error> {
        let target = self.resolve_callee(callee, line)?;
        self.invoke(
            target,
            args,
            line,
            !matches!(callee.kind, ExprKind::Constant(_)),
        )
    }

    pub(super) fn acquire_callable(&mut self, callee: &Expr, line: usize) -> Result<Value, Error> {
        let target = self.resolve_callee(callee, line)?;
        if let Some(closure) = target.closure {
            return Ok(Value::Closure(closure));
        }
        self.next_object_id += 1;
        Ok(Value::Closure(Rc::new(crate::Closure {
            name: if matches!(target.builtin, Some(Builtin::Exit)) {
                b"exit".to_vec()
            } else {
                target.name
            },
            this: target.this,
            context: target.context,
            function: target.function,
            object_id: self.next_object_id,
            environment: None,
        })))
    }

    pub(super) fn pipe_call(
        &mut self,
        callable: Value,
        value: Value,
        line: usize,
    ) -> Result<Value, Error> {
        let target = self.resolve_callable(callable, line)?;
        self.invoke(
            target,
            &[Argument::Positional(Expr {
                kind: ExprKind::Literal(value),
                line,
            })],
            line,
            true,
        )
    }

    fn resolve_callee(&mut self, callee: &Expr, line: usize) -> Result<CallTarget, Error> {
        match &callee.kind {
            ExprKind::ScopedMember(class, name) => self.resolve_scoped_call(class, name, line),
            ExprKind::Property(base, name) => {
                let value = self.eval(base)?;
                let name = self.variable_name(name)?;
                self.resolve_method(value, &name, line)
            }
            ExprKind::Constant(name) => self.resolve_name(name.clone(), line),
            _ => {
                let value = self.eval(callee)?;
                self.resolve_callable(value, line)
            }
        }
    }

    pub(super) fn native_callback(
        &self,
        value: &Value,
        function: &str,
        position: usize,
        nullable: bool,
        line: usize,
    ) -> Result<Option<CallTarget>, Error> {
        if nullable && matches!(value, Value::Null) {
            return Ok(None);
        }
        self.resolve_native_callback(value, line).map(Some).map_err(|error| {
            Error::new(ErrorKind::TypeError, format!(
                "{function}(): Argument #{position} ($callback) must be a valid callback{}, {}",
                if nullable { " or null" } else { "" }, error.message,
            ), line)
        })
    }

    pub(super) fn call_callback(
        &mut self,
        target: &CallTarget,
        values: &[Value],
        line: usize,
    ) -> Result<Value, Error> {
        let arguments: Vec<_> = values
            .iter()
            .map(|value| {
                Argument::Positional(Expr {
                    kind: ExprKind::Literal(value.clone()),
                    line,
                })
            })
            .collect();
        // Internal callbacks use weak argument coercion. The common invocation
        // path still installs the function's own strictness for its body/return.
        let caller_strict = std::mem::replace(&mut self.strict_types, false);
        let result = self.invoke(target.clone(), &arguments, line, true);
        self.strict_types = caller_strict;
        result
    }

    pub(super) fn resolve_callable(&self, value: Value, line: usize) -> Result<CallTarget, Error> {
        match value {
            Value::Object(object) => {
                if let Some(function) = objects::method(&object.class, b"__invoke") {
                    self.unchecked_method_target(function, object.class.clone(), Some(object), line)
                } else {
                    self.resolve_method(Value::Object(object), b"__invoke", line)
                }
            }
            Value::Array(array) if array.len() == 2 => {
                let base = array.get(&ArrayKey::Int(0));
                let method = array.get(&ArrayKey::Int(1));
                if let (Some(Value::Object(object)), Some(Value::String(name))) = (base, method) {
                    self.resolve_method(Value::Object(object.clone()), name, line)
                } else if let (Some(Value::String(class)), Some(Value::String(name))) =
                    (base, method)
                {
                    self.resolve_class_method(class, name, false, false, line)
                } else {
                    Err(Error::new(
                        ErrorKind::Error,
                        "Invalid method callback",
                        line,
                    ))
                }
            }
            Value::String(name) => self.resolve_name(name, line),
            Value::Closure(closure) => Ok(CallTarget {
                this: closure.this.clone(),
                context: closure.context.clone(),
                name: closure.name.clone(),
                function: closure.function.clone(),
                builtin: if closure.function.is_none() {
                    Builtin::lookup(&closure.name)
                } else {
                    None
                },
                closure: Some(closure),
            }),
            value => Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Value of type {} is not callable",
                    builtins::debug_type(&value)
                ),
                line,
            )),
        }
    }

    fn resolve_name(&self, name: Vec<u8>, line: usize) -> Result<CallTarget, Error> {
        if let Some(separator) = name.windows(2).position(|part| part == b"::") {
            return self.resolve_class_method(
                &name[..separator],
                &name[separator + 2..],
                false,
                false,
                line,
            );
        }
        let normalized_name = name
            .strip_prefix(b"\\")
            .unwrap_or(&name)
            .to_ascii_lowercase();
        let function = self.functions.get(&normalized_name).cloned();
        let builtin = Builtin::lookup(&normalized_name);
        if function.is_none() && builtin.is_none() {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Call to undefined function {}()",
                    String::from_utf8_lossy(&name)
                ),
                line,
            ));
        }
        Ok(CallTarget {
            this: None,
            context: None,
            name: normalized_name,
            function,
            builtin,
            closure: None,
        })
    }

    pub(super) fn invoke(
        &mut self,
        target: CallTarget,
        args: &[Argument],
        line: usize,
        dynamic: bool,
    ) -> Result<Value, Error> {
        let CallTarget {
            name,
            function,
            builtin,
            closure,
            this,
            context,
        } = target;
        let parameters = if let Some(function) = &function {
            function.parameters.clone()
        } else {
            builtin.unwrap().parameters()
        };
        let mut bound = BoundArguments::new(&parameters);
        for arg in args {
            match arg {
                Argument::Positional(expr) => {
                    let value = self.eval(expr)?;
                    bound.push(None, value, &parameters, line)?;
                }
                Argument::Named(name, expr) => {
                    let value = self.eval(expr)?;
                    bound.push(Some(name.clone()), value, &parameters, line)?;
                }
                Argument::Unpack(expr) => {
                    let Value::Array(array) = self.eval(expr)? else {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            "Only arrays and Traversables can be unpacked",
                            line,
                        ));
                    };
                    for (key, value) in array.entries() {
                        let name = match key {
                            ArrayKey::Int(_) => None,
                            ArrayKey::String(name) => Some(name.clone()),
                        };
                        bound.push(name, value.clone(), &parameters, line)?;
                    }
                }
            }
        }
        if let Some(builtin) = builtin {
            for (index, param) in parameters.iter().enumerate() {
                if !param.variadic && param.default.is_none() && bound.fixed[index].is_none() {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        format!(
                            "{}(): Argument #{} (${}) not passed",
                            String::from_utf8_lossy(&name),
                            index + 1,
                            String::from_utf8_lossy(&param.name)
                        ),
                        line,
                    ));
                }
            }
            if !bound.variadic && !bound.extra.is_empty() {
                return Err(Error::new(
                    ErrorKind::ArgumentCountError,
                    format!(
                        "{}() expects at most {} arguments",
                        String::from_utf8_lossy(&name),
                        parameters.len()
                    ),
                    line,
                ));
            }
            return self.call_builtin(builtin, &name, bound.fixed, bound.extra, line, dynamic);
        }
        let function = function.unwrap();
        if function.native.is_some() && !bound.extra.is_empty() {
            return Err(Error::new(
                ErrorKind::ArgumentCountError,
                "Too many arguments for internal method",
                line,
            ));
        }
        let extra: Vec<_> = bound
            .extra
            .entries()
            .iter()
            .filter(|(key, _)| matches!(key, ArrayKey::Int(_)))
            .map(|(_, value)| value.clone())
            .collect();
        let supplied_fixed = if extra.is_empty() {
            bound
                .fixed
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |i| i + 1)
        } else {
            function
                .parameters
                .iter()
                .take_while(|p| !p.variadic)
                .count()
        };
        let mut arguments = FunctionArguments {
            fixed: function.parameters[..supplied_fixed]
                .iter()
                .map(|p| p.name.clone())
                .collect(),
            extra,
        };
        self.callers.push(std::mem::take(&mut self.scope));
        let caller_context = std::mem::replace(&mut self.class_context, context);
        let caller_arguments = self.current_arguments.take();
        let caller_closure = std::mem::replace(&mut self.current_closure, closure.clone());
        let caller_function = self
            .current_function
            .replace(function.name.to_ascii_lowercase());
        let caller_source = self.source.clone();
        let caller_strict = self.strict_types;
        let result = (|| {
            let last_required = function
                .parameters
                .iter()
                .rposition(|p| p.default.is_none() && !p.variadic);
            for (index, param) in function.parameters.iter().enumerate() {
                let value = if param.variadic {
                    let mut values = PhpArray::default();
                    arguments.extra.clear();
                    for (offset, (key, value)) in bound.extra.entries().iter().enumerate() {
                        let value = self.check_argument(
                            value.clone(),
                            param,
                            &function,
                            index + offset + 1,
                            caller_strict,
                            line,
                        )?;
                        if matches!(key, ArrayKey::Int(_)) {
                            arguments.extra.push(value.clone());
                        }
                        values.insert(key.clone(), value);
                    }
                    Value::Array(Rc::new(values))
                } else if let Some(value) = bound.fixed[index].take() {
                    value
                } else if last_required.is_none_or(|last| index > last)
                    && let Some(default) = &param.default
                {
                    self.eval_default(default)?
                } else {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        format!(
                            "{}(): Argument #{} (${}) not passed",
                            String::from_utf8_lossy(&function.name),
                            index + 1,
                            String::from_utf8_lossy(&param.name)
                        ),
                        line,
                    ));
                };
                let value = if param.variadic {
                    value
                } else {
                    self.check_argument(value, param, &function, index + 1, caller_strict, line)?
                };
                self.scope.insert(param.name.clone(), cell(value));
            }
            if let Some(native) = function.native {
                if let Some(object) = this {
                    self.scope
                        .insert(b"this".to_vec(), cell(Value::Object(object)));
                }
                return self.native_throwable_method(native, line);
            }
            self.strict_types = function.strict_types;
            self.source = function.source.clone();
            if let Some(closure) = &closure
                && let Some(environment) = &closure.environment
            {
                for (name, capture) in &environment.captures {
                    let value = match capture {
                        crate::closure::CapturedValue::Value(value) => cell(value.clone()),
                        crate::closure::CapturedValue::Reference(value) => value.clone(),
                    };
                    self.scope.insert(name.clone(), value);
                }
            }
            if let Some(object) = this {
                self.scope
                    .insert(b"this".to_vec(), cell(Value::Object(object)));
            }
            self.current_arguments = Some(arguments);
            let flow = self.block(&function.body)?;
            self.check_return(flow, &function)
        })();
        let result = result.map_err(|error| self.materialize_error(error));
        self.scope = self.callers.pop().unwrap();
        self.class_context = caller_context;
        self.current_function = caller_function;
        self.current_arguments = caller_arguments;
        self.current_closure = caller_closure;
        self.strict_types = caller_strict;
        self.source = caller_source;
        result
    }
}
fn take_hoisted(stmts: &mut [Stmt], declarations: &mut Vec<Rc<FunctionDecl>>) {
    for stmt in stmts {
        match stmt {
            Stmt::Function(decl) => {
                declarations.push(decl.clone());
                *stmt = Stmt::Block(Vec::new());
            }
            Stmt::Block(stmts) => take_hoisted(stmts, declarations),
            _ => {}
        }
    }
}
fn redeclared(decl: &FunctionDecl) -> Error {
    Error::new(
        ErrorKind::Error,
        format!(
            "Cannot redeclare function {}()",
            String::from_utf8_lossy(&decl.name)
        ),
        decl.line,
    )
}
struct BoundArguments {
    fixed: Vec<Option<Value>>,
    extra: PhpArray,
    positional: usize,
    saw_named: bool,
    variadic: bool,
}
impl BoundArguments {
    fn new(parameters: &[Parameter]) -> Self {
        Self {
            fixed: vec![None; parameters.len()],
            extra: PhpArray::default(),
            positional: 0,
            saw_named: false,
            variadic: parameters.last().is_some_and(|p| p.variadic),
        }
    }
    fn push(
        &mut self,
        name: Option<Vec<u8>>,
        value: Value,
        parameters: &[Parameter],
        line: usize,
    ) -> Result<(), Error> {
        if let Some(name) = name {
            self.saw_named = true;
            if let Some(index) = parameters
                .iter()
                .position(|p| p.name == name && !p.variadic)
            {
                if self.fixed[index].is_some() {
                    return Err(overwritten(&name, line));
                }
                self.fixed[index] = Some(value);
            } else if self.variadic {
                let key = ArrayKey::String(name.clone());
                if self.extra.get(&key).is_some() {
                    return Err(overwritten(&name, line));
                }
                self.extra.insert(key, value);
            } else {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Unknown named parameter ${}",
                        String::from_utf8_lossy(&name)
                    ),
                    line,
                ));
            }
        } else {
            if self.saw_named {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot use positional argument after named argument during unpacking",
                    line,
                ));
            }
            let index = self.positional;
            self.positional += 1;
            if index < parameters.len() && !parameters[index].variadic {
                self.fixed[index] = Some(value);
            } else {
                self.extra.append(value, line)?;
            }
        }
        Ok(())
    }
}
fn overwritten(name: &[u8], line: usize) -> Error {
    Error::new(
        ErrorKind::Error,
        format!(
            "Named parameter ${} overwrites previous argument",
            String::from_utf8_lossy(name)
        ),
        line,
    )
}

fn diagnose_functions(stmts: &[Stmt], warnings: &mut Vec<String>) {
    diagnose_statements(stmts, warnings, &mut Vec::new());
}

fn diagnose_statements(stmts: &[Stmt], warnings: &mut Vec<String>, switches: &mut Vec<bool>) {
    for stmt in stmts {
        match stmt {
            Stmt::Class(class) => {
                for method in &class.methods {
                    diagnose_functions(&[Stmt::Function(method.clone())], warnings);
                }
            }
            Stmt::Function(decl) => {
                if let Some(last) = decl
                    .parameters
                    .iter()
                    .rposition(|p| p.default.is_none() && !p.variadic)
                {
                    for param in &decl.parameters[..last] {
                        if param.default.is_some() && !param.implicit_nullable {
                            warnings.push(format!("Deprecated: Optional parameter ${} declared before required parameter is implicitly treated as required on line {}", String::from_utf8_lossy(&param.name), decl.line));
                        }
                    }
                }
                diagnose_functions(&decl.body, warnings);
            }
            Stmt::Block(body) => diagnose_statements(body, warnings, switches),
            Stmt::Try(body, catches, finally) => {
                diagnose_statements(body, warnings, switches);
                for catch in catches {
                    diagnose_statements(&catch.body, warnings, switches);
                }
                if let Some(finally) = finally {
                    diagnose_statements(finally, warnings, switches);
                }
            }
            Stmt::If(_, yes, no) => {
                diagnose_statements(std::slice::from_ref(yes), warnings, switches);
                if let Some(no) = no {
                    diagnose_statements(std::slice::from_ref(no), warnings, switches);
                }
            }
            Stmt::While(_, body)
            | Stmt::DoWhile(body, _)
            | Stmt::For(_, _, _, body)
            | Stmt::Foreach(_, _, _, body) => {
                switches.push(false);
                diagnose_statements(std::slice::from_ref(body), warnings, switches);
                switches.pop();
            }
            Stmt::Switch(_, cases) => {
                switches.push(true);
                for case in cases {
                    if let Some(line) = case.semicolon_line {
                        warnings.push(format!("Deprecated: Case statements followed by a semicolon (;) are deprecated, use a colon (:) instead on line {line}"));
                    }
                    diagnose_statements(&case.body, warnings, switches);
                }
                switches.pop();
            }
            Stmt::Continue(level, line) if switches[switches.len() - level] => {
                let suffix = if *level == 1 {
                    String::new()
                } else {
                    format!(" {level}")
                };
                warnings.push(format!("\"continue{suffix}\" targeting switch is equivalent to \"break{suffix}\" on line {line}"));
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
pub(super) struct CallTarget {
    pub(super) name: Vec<u8>,
    pub(super) function: Option<Rc<FunctionDecl>>,
    pub(super) builtin: Option<Builtin>,
    pub(super) closure: Option<Rc<crate::Closure>>,
    pub(super) this: Option<Rc<crate::PhpObject>>,
    pub(super) context: Option<crate::object::ClassContext>,
}
