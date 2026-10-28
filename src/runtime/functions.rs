use super::builtins::Builtin;
use super::function_arguments::FunctionArguments;
use super::*;
use crate::ast::{Argument, Parameter};
use std::collections::HashSet;

impl Runtime {
    pub(super) fn prepare_functions(&mut self, stmts: &mut [Stmt]) -> Result<(), Error> {
        diagnose_functions(stmts, &mut self.diagnostics);
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
        if let Some(builtin @ (Builtin::CallUserFunc | Builtin::CallUserFuncArray)) = target.builtin
            && self.can_inline_callback(callee, args, builtin)
        {
            return self.inline_callback(args, builtin, line);
        }
        self.invoke(
            target,
            args,
            line,
            !matches!(
                callee.kind,
                ExprKind::Constant(_) | ExprKind::NamespacedName(_)
            ),
        )
    }

    pub(super) fn acquire_callable(&mut self, callee: &Expr, line: usize) -> Result<Value, Error> {
        let target = self.resolve_callee(callee, line)?;
        if self.in_constant_expression && target.magic_name.is_some() {
            return Err(Error::new(
                ErrorKind::Error,
                "Creating a callable for the magic __callStatic() method is not supported in constant expressions",
                line,
            ));
        }
        if let Some(closure) = target.closure {
            return Ok(Value::Closure(closure));
        }
        let handle = self.allocate_object_handle();
        Ok(Value::Closure(Rc::new(crate::Closure {
            magic_name: target.magic_name,
            name: if matches!(target.builtin, Some(Builtin::Exit)) {
                b"exit".to_vec()
            } else {
                target.name
            },
            this: target.this,
            context: target.context,
            function: target.function,
            object_id: handle.id,
            _handle: handle,
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
                let name = self.method_name(name)?;
                self.resolve_method(value, &name, line)
            }
            ExprKind::Constant(name) => self.resolve_name(name.clone(), line),
            ExprKind::NamespacedName(name) => {
                let key = name.function.to_ascii_lowercase();
                let resolved =
                    if self.functions.contains_key(&key) || Builtin::lookup(&key).is_some() {
                        &name.function
                    } else if let Some(fallback) = &name.function_fallback {
                        let key = fallback.to_ascii_lowercase();
                        if self.functions.contains_key(&key) || Builtin::lookup(&key).is_some() {
                            fallback
                        } else {
                            &name.function
                        }
                    } else {
                        &name.function
                    };
                self.resolve_name(resolved.clone(), line)
            }
            _ => {
                let value = self.eval(callee)?;
                self.resolve_callable(value, line)
            }
        }
    }

    pub(super) fn method_name(&mut self, name: &Expr) -> Result<Vec<u8>, Error> {
        match self.eval(name)? {
            Value::String(name) => Ok(name.to_vec()),
            _ => Err(Error::new(
                ErrorKind::Error,
                "Method name must be a string",
                name.line,
            )),
        }
    }

    pub(super) fn native_callback(
        &mut self,
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
            if error.throwable.is_some() || error.kind == ErrorKind::UnwindExit { return error; }
            Error::new(ErrorKind::TypeError, format!(
                "{function}(): Argument #{position} ($callback) must be a valid callback{}, {}",
                if nullable { " or null" } else { "" }, self.callback_error(value, &error),
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
        let mut target = target.clone();
        target.reference_values = true;
        let result = self.invoke_at(target, &arguments, line, true, true);
        self.strict_types = caller_strict;
        result
    }

    pub(super) fn resolve_callable(
        &mut self,
        value: Value,
        line: usize,
    ) -> Result<CallTarget, Error> {
        match value {
            Value::Object(object) => {
                if let Some(function) = objects::method(&object.class, b"__invoke") {
                    self.unchecked_method_target(function, object.class.clone(), Some(object), line)
                } else {
                    Err(Error::new(
                        ErrorKind::Error,
                        format!("Object of type {} is not callable", object.class_name),
                        line,
                    ))
                }
            }
            Value::Array(array) if array.len() == 2 => {
                let base = array.get(&ArrayKey::Int(0)).map(Value::dereferenced);
                let method = array.get(&ArrayKey::Int(1)).map(Value::dereferenced);
                if let (Some(Value::Object(object)), Some(Value::String(name))) =
                    (base.as_ref(), method.as_ref())
                {
                    self.resolve_method(Value::Object(object.clone()), &name.bytes(), line)
                } else if let (Some(Value::Closure(closure)), Some(Value::String(name))) =
                    (base.as_ref(), method.as_ref())
                {
                    self.resolve_method(Value::Closure(closure.clone()), &name.bytes(), line)
                } else if let (Some(Value::String(class)), Some(Value::String(name))) =
                    (base.as_ref(), method.as_ref())
                {
                    self.resolve_class_method(&class.bytes(), &name.bytes(), false, false, line)
                } else {
                    Err(Error::new(
                        ErrorKind::Error,
                        "Invalid method callback",
                        line,
                    ))
                }
            }
            Value::String(name) => self.resolve_name(name.to_vec(), line),
            Value::Closure(closure) => Ok(CallTarget {
                reference_values: false,
                magic_name: closure.magic_name.clone(),
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

    fn resolve_name(&mut self, name: Vec<u8>, line: usize) -> Result<CallTarget, Error> {
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
            reference_values: false,
            magic_name: None,
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
        self.invoke_at(target, args, line, dynamic, false)
    }

    pub(super) fn invoke_at(
        &mut self,
        mut target: CallTarget,
        args: &[Argument],
        line: usize,
        dynamic: bool,
        internal: bool,
    ) -> Result<Value, Error> {
        if let Some(name) = target.magic_name.take() {
            target.name = target.function.as_ref().unwrap().name.clone();
            let parameters = [magic_parameter()];
            let bound = self.bind_call_arguments(&parameters, args, &target, line)?;
            let literal = |value| {
                Argument::Positional(Expr {
                    kind: ExprKind::Literal(value),
                    line,
                })
            };
            return self.invoke_at(
                target,
                &[
                    literal(Value::String(name.into())),
                    literal(Value::Array(Rc::new(bound.extra))),
                ],
                line,
                dynamic,
                internal,
            );
        }
        let function = &target.function;
        let builtin = target.builtin;

        let parameters = if let Some(function) = &function {
            function.parameters.clone()
        } else {
            builtin.unwrap().parameters()
        };
        let bound = self.bind_call_arguments(&parameters, args, &target, line)?;
        let optimized = !dynamic
            && target.name != b"key_exists"
            && optimized_trace_call(builtin, args, self.current_function.is_some());
        if !optimized {
            self.trace_frames.push(traces::TraceFrame::call(
                self,
                &target,
                &parameters,
                &bound.fixed,
                &bound.extra,
                if internal { None } else { Some(line) },
            ));
        }
        let result = self.invoke_bound(target, parameters, bound, line, dynamic);
        let result = result.map_err(|error| self.materialize_error(error));
        if !optimized {
            self.trace_frames.pop();
        }
        result
    }

    fn bind_call_arguments(
        &mut self,
        parameters: &[Parameter],
        args: &[Argument],
        target: &CallTarget,
        line: usize,
    ) -> Result<BoundArguments, Error> {
        let mut bound = BoundArguments::new(parameters);
        for arg in args {
            match arg {
                Argument::Positional(expr) => {
                    let (value, reference) =
                        if let Some(index) = bound.parameter_index(parameters, None) {
                            self.eval_call_argument(
                                expr,
                                &parameters[index],
                                bound.positional,
                                target,
                                line,
                            )?
                        } else {
                            (self.eval(expr)?, None)
                        };
                    bound.push(None, value, reference, parameters, line)?;
                }
                Argument::Named(name, expr) => {
                    let (value, reference) =
                        if let Some(index) = bound.parameter_index(parameters, Some(name)) {
                            self.eval_call_argument(expr, &parameters[index], index, target, line)?
                        } else {
                            (self.eval(expr)?, None)
                        };
                    bound.push(Some(name.clone()), value, reference, parameters, line)?;
                }
                Argument::Unpack(expr) => {
                    self.bind_unpacked_arguments(expr, parameters, &mut bound, line)?;
                }
            }
        }
        for (value, reference) in bound.fixed.iter_mut().zip(&bound.references) {
            if let Some(reference) = reference {
                *value = Some(reference.borrow().clone());
            }
        }
        Ok(bound)
    }

    fn bind_unpacked_arguments(
        &mut self,
        expr: &Expr,
        parameters: &[Parameter],
        bound: &mut BoundArguments,
        line: usize,
    ) -> Result<(), Error> {
        let mut expr = expr;
        while let ExprKind::Group(inner) = &expr.kind {
            expr = inner;
        }
        let wants_references = parameters.iter().any(|parameter| parameter.by_reference);
        let mut reference = if let ExprKind::Assign(target, op, source) = &expr.kind
            && op == "=&"
            && wants_references
        {
            Some(self.reference_assignment(target, source, line)?)
        } else {
            None
        };
        let mut location = if wants_references && expr.is_writable() && !expr.is_globals() {
            Some(self.location(expr)?)
        } else {
            None
        };
        let value = if let Some(reference) = &reference {
            reference.borrow().clone()
        } else if let Some(location) = &mut location {
            self.read_location(location, line, false, None)?
        } else {
            self.eval_inner(expr)?
        };
        let value = if let Value::Reference(returned) = value {
            let value = returned.borrow().clone();
            reference = Some(returned);
            value
        } else {
            value
        };
        let Value::Array(array) = value else {
            return Err(Error::new(
                ErrorKind::TypeError,
                format!(
                    "Only arrays and Traversables can be unpacked, {} given",
                    builtins::debug_type(&value)
                ),
                line,
            ));
        };
        let mut positional = bound.positional;
        let requires_reference = array.entries().iter().any(|(key, _)| {
            let index = match key {
                ArrayKey::Int(_) => {
                    let index = positional;
                    positional += 1;
                    (index < parameters.len())
                        .then_some(index)
                        .or_else(|| bound.variadic.then(|| parameters.len() - 1))
                }
                ArrayKey::String(name) => bound.parameter_index(parameters, Some(name)),
            };
            index.is_some_and(|index| parameters[index].by_reference)
        });
        if !requires_reference {
            for (key, value) in array.entries() {
                let name = match key {
                    ArrayKey::Int(_) => None,
                    ArrayKey::String(name) => Some(name.clone()),
                };
                bound.push(name, value.dereferenced(), None, parameters, line)?;
            }
            return Ok(());
        }
        let keys: Vec<_> = array.entries().iter().map(|(key, _)| key.clone()).collect();
        let source = if let Some(reference) = reference {
            drop(array);
            reference
        } else if let Some(location) = &mut location {
            drop(array);
            self.location_reference(location, line)?
        } else {
            cell(Value::Array(array))
        };
        for key in keys {
            let name = match &key {
                ArrayKey::Int(_) => None,
                ArrayKey::String(name) => Some(name.clone()),
            };
            let by_reference = bound
                .parameter_index(parameters, name.as_deref())
                .is_some_and(|index| parameters[index].by_reference);
            let mut offset = Offset::new(Some(key.to_value()));
            let (value, reference) = if by_reference {
                let reference = reference_paths::acquire(
                    source.clone(),
                    std::slice::from_mut(&mut offset),
                    line,
                    self,
                )?;
                let value = reference.borrow().clone();
                (value, Some(reference))
            } else {
                (
                    read_offset(
                        source.borrow().clone(),
                        &mut offset,
                        line,
                        Access::Read,
                        &mut self.diagnostics,
                    )?,
                    None,
                )
            };
            bound.push(name, value, reference, parameters, line)?;
        }
        Ok(())
    }

    fn eval_call_argument(
        &mut self,
        expr: &Expr,
        parameter: &Parameter,
        index: usize,
        target: &CallTarget,
        line: usize,
    ) -> Result<(Value, Option<variables::Cell>), Error> {
        if !parameter.by_reference {
            return Ok((self.eval(expr)?, None));
        }
        let label = if parameter.variadic {
            String::new()
        } else {
            format!(" (${})", String::from_utf8_lossy(&parameter.name))
        };
        if target.reference_values {
            if let ExprKind::Literal(Value::Reference(reference)) = &expr.kind {
                return Ok((reference.borrow().clone(), Some(reference.clone())));
            }
            let value = self.eval(expr)?;
            self.diagnostics.push(format!(
                "{}(): Argument #{}{label} must be passed by reference, value given on line {line}",
                String::from_utf8_lossy(&target.name),
                index + 1
            ));
            return Ok((value, None));
        }
        let mut expr = expr;
        while let ExprKind::Group(inner) = &expr.kind {
            expr = inner;
        }
        let reference = if let ExprKind::Assign(left, op, right) = &expr.kind
            && op == "=&"
        {
            Some(self.reference_assignment(left, right, line)?)
        } else if matches!(&expr.kind, ExprKind::Variable(name) if name == b"this") {
            Some(cell(self.eval(expr)?))
        } else if expr.is_writable() && !expr.is_globals() {
            Some(self.argument_reference(expr, line)?)
        } else {
            None
        };
        if let Some(reference) = reference {
            let value = reference.borrow().clone();
            return Ok((value, Some(reference)));
        }
        let value = self.eval_inner(expr)?;
        if let Value::Reference(reference) = value {
            let value = reference.borrow().clone();
            return Ok((value, Some(reference)));
        }
        if matches!(expr.kind, ExprKind::Call(_, _) | ExprKind::New(_, _)) {
            self.diagnostics.push(format!(
                "Notice: Only variables should be passed by reference on line {line}"
            ));
            return Ok((value, None));
        }
        Err(Error::new(
            ErrorKind::Error,
            format!(
                "{}(): Argument #{}{label} could not be passed by reference",
                String::from_utf8_lossy(&target.name),
                index + 1
            ),
            line,
        ))
    }

    fn invoke_bound(
        &mut self,
        target: CallTarget,
        parameters: Vec<Parameter>,
        mut bound: BoundArguments,
        line: usize,
        dynamic: bool,
    ) -> Result<Value, Error> {
        let CallTarget {
            reference_values: _,
            magic_name: _,
            name,
            function,
            builtin,
            closure,
            this,
            context,
        } = target;
        if builtin.is_some() || function.as_ref().is_some_and(|f| f.native.is_some()) {
            let supplied_fixed = bound
                .fixed
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |i| i + 1);
            let supplied = supplied_fixed + bound.extra.len();
            let required = parameters
                .iter()
                .filter(|p| !p.variadic && p.default.is_none())
                .count();
            let maximum = parameters.iter().filter(|p| !p.variadic).count();
            let count_error = |count: usize, direction: &str| {
                Error::new(
                    ErrorKind::ArgumentCountError,
                    format!(
                        "{}() expects {} {} argument{}, {} given",
                        String::from_utf8_lossy(&name),
                        if !bound.variadic && required == maximum {
                            "exactly"
                        } else {
                            direction
                        },
                        count,
                        if count == 1 { "" } else { "s" },
                        supplied
                    ),
                    line,
                )
            };
            for (index, param) in parameters.iter().enumerate() {
                if !param.variadic && param.default.is_none() && bound.fixed[index].is_none() {
                    if index >= supplied_fixed && supplied < required {
                        return Err(count_error(required, "at least"));
                    }
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
                return Err(count_error(maximum, "at most"));
            }
            if let Some(builtin) = builtin {
                return self.call_builtin(builtin, &name, bound, line, dynamic);
            }
        }
        let function = function.unwrap();
        if function.is_abstract {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot call abstract method {}()",
                    String::from_utf8_lossy(&function.name)
                ),
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
        self.trace_frames
            .last_mut()
            .unwrap()
            .enter_scope(self.callers.len());
        let caller_context = std::mem::replace(&mut self.class_context, context);
        let caller_returns_reference = self.current_returns_reference;
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
                    for (offset, (key, stored)) in bound.extra.entries().iter().enumerate() {
                        let value = self.check_argument(
                            stored.dereferenced(),
                            param,
                            &function,
                            index + offset + 1,
                            caller_strict,
                            line,
                        )?;
                        let value = if let Value::Reference(reference) = stored {
                            self.write_reference_value(reference, value, true, line)?;
                            Value::Reference(reference.clone())
                        } else {
                            value
                        };
                        if matches!(key, ArrayKey::Int(_)) {
                            arguments.extra.push(value.clone());
                        }
                        self.trace_frames
                            .last_mut()
                            .unwrap()
                            .set_extra(key, value.clone());
                        values.insert(key.clone(), value);
                    }
                    Value::Array(Rc::new(values))
                } else if let Some(reference) = &bound.references[index] {
                    reference.borrow().clone()
                } else if let Some(value) = bound.fixed[index].take() {
                    value
                } else if last_required.is_none_or(|last| index > last)
                    && let Some(default) = &param.default
                {
                    self.eval_default(default)?
                } else {
                    let message = if index >= supplied_fixed {
                        let required = last_required.unwrap() + 1;
                        let maximum = function.parameters.iter().filter(|p| !p.variadic).count();
                        let site = self
                            .trace_frames
                            .last()
                            .and_then(traces::TraceFrame::call_site)
                            .map_or(String::new(), |(file, line)| {
                                format!(" in {} on line {line}", String::from_utf8_lossy(&file))
                            });
                        format!(
                            "Too few arguments to function {}(), {supplied_fixed} passed{site} and {} {required} expected",
                            String::from_utf8_lossy(&function.name),
                            if required == maximum {
                                "exactly"
                            } else {
                                "at least"
                            },
                        )
                    } else {
                        format!(
                            "{}(): Argument #{} (${}) not passed",
                            String::from_utf8_lossy(&function.name),
                            index + 1,
                            String::from_utf8_lossy(&param.name)
                        )
                    };
                    let error = Error::new(ErrorKind::ArgumentCountError, message, function.line);
                    return Err(self.materialize_error_from(error, &function.source.name));
                };
                let value = if param.variadic {
                    value
                } else {
                    self.check_argument(value, param, &function, index + 1, caller_strict, line)?
                };
                let reference = if let Some(reference) = bound.references[index].take() {
                    self.write_reference_value(&reference, value, true, line)?;
                    reference
                } else {
                    cell(value)
                };
                self.scope.insert(param.name.clone(), reference);
                self.trace_frames.last_mut().unwrap().bound = index + 1;
            }
            if let Some(native) = function.native {
                if let Some(object) = this {
                    self.scope
                        .insert(b"this".to_vec(), cell(Value::Object(object)));
                }
                return self.native_throwable_method(native, supplied_fixed, line);
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
            self.current_returns_reference = function.returns_reference;
            let flow = self.block(&function.body)?;
            self.check_return(flow, &function)
        })();
        let result = result.map_err(|error| self.materialize_error(error));
        self.scope = self.callers.pop().unwrap();
        self.class_context = caller_context;
        self.current_function = caller_function;
        self.current_arguments = caller_arguments;
        self.current_returns_reference = caller_returns_reference;
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
pub(super) struct BoundArguments {
    pub(super) fixed: Vec<Option<Value>>,
    pub(super) references: Vec<Option<variables::Cell>>,
    pub(super) extra: PhpArray,
    positional: usize,
    saw_named: bool,
    variadic: bool,
}
impl BoundArguments {
    fn new(parameters: &[Parameter]) -> Self {
        Self {
            fixed: vec![None; parameters.len()],
            references: vec![None; parameters.len()],
            extra: PhpArray::default(),
            positional: 0,
            saw_named: false,
            variadic: parameters.last().is_some_and(|p| p.variadic),
        }
    }
    fn parameter_index(&self, parameters: &[Parameter], name: Option<&[u8]>) -> Option<usize> {
        let index = if let Some(name) = name {
            parameters
                .iter()
                .position(|p| p.name == name && !p.variadic)
        } else {
            (self.positional < parameters.len()).then_some(self.positional)
        };
        index.or_else(|| self.variadic.then(|| parameters.len() - 1))
    }
    fn push(
        &mut self,
        name: Option<Vec<u8>>,
        value: Value,
        reference: Option<variables::Cell>,
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
                self.references[index] = reference;
            } else if self.variadic {
                let key = ArrayKey::String(name.clone());
                if self.extra.get(&key).is_some() {
                    return Err(overwritten(&name, line));
                }
                let value = if parameters.last().is_some_and(|p| p.by_reference) {
                    Value::Reference(reference.unwrap_or_else(|| cell(value)))
                } else {
                    value
                };
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
                self.references[index] = reference;
            } else {
                let value = if self.variadic && parameters.last().is_some_and(|p| p.by_reference) {
                    Value::Reference(reference.unwrap_or_else(|| cell(value)))
                } else {
                    value
                };
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

fn diagnose_functions(stmts: &[Stmt], warnings: &mut dyn WarningSink) {
    diagnose_statements(stmts, warnings, &mut Vec::new());
}

fn diagnose_statements(stmts: &[Stmt], warnings: &mut dyn WarningSink, switches: &mut Vec<bool>) {
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
            | Stmt::Foreach(_, _, _, _, body) => {
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
    pub(super) reference_values: bool,
    pub(super) magic_name: Option<Vec<u8>>,
    pub(super) name: Vec<u8>,
    pub(super) function: Option<Rc<FunctionDecl>>,
    pub(super) builtin: Option<Builtin>,
    pub(super) closure: Option<Rc<crate::Closure>>,
    pub(super) this: Option<Rc<crate::PhpObject>>,
    pub(super) context: Option<crate::object::ClassContext>,
}

// These direct calls compile to specialized PHP opcodes and have no call frame.
fn optimized_trace_call(builtin: Option<Builtin>, args: &[Argument], in_function: bool) -> bool {
    if args
        .iter()
        .any(|arg| !matches!(arg, Argument::Positional(_)))
    {
        return false;
    }
    match builtin {
        Some(
            Builtin::Strlen | Builtin::Count | Builtin::GetType | Builtin::Is(_) | Builtin::Clone,
        ) => args.len() == 1,
        Some(Builtin::GetClass) => args.len() <= 1,
        Some(Builtin::GetCalledClass) => args.is_empty(),
        Some(Builtin::FuncNumArgs | Builtin::FuncGetArgs) => in_function && args.is_empty(),
        Some(Builtin::ArrayKeyExists) => args.len() == 2,
        _ => false,
    }
}

pub(super) fn magic_parameter() -> Parameter {
    Parameter {
        attributes: Vec::new(),
        by_reference: false,
        name: b"arguments".to_vec(),
        ty: None,
        implicit_nullable: false,
        default: None,
        variadic: true,
        promotion: None,
    }
}
