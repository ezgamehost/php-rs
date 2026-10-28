use super::*;

impl Runtime {
    pub(super) fn reflection_parameter_constructor(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let value = self.lookup(b"function").unwrap();
        let function = match value {
            Value::Array(array) => {
                let (Some(class), Some(method)) = (array.get(&ArrayKey::Int(0)), array.get(&ArrayKey::Int(1))) else {
                    return Err(self.reflection_error("Expected array($object, $method) or array($classname, $method)", line));
                };
                let class = self.reflected_class(class.dereferenced(), false, line)?;
                let name = self.string_bytes(&method.dereferenced(), line)?;
                self.reflected_method(&class, &name, line)?
            }
            Value::Object(object) => self.reflected_method(&object.class, b"__invoke", line)?,
            Value::Closure(_) => FunctionReflection { target: self.resolve_callable(value, line)?, is_method: false },
            Value::String(name) => {
                if !self.functions.contains_key(&name.to_vec().to_ascii_lowercase()) && builtins::Builtin::lookup(&name.to_vec().to_ascii_lowercase()).is_none() {
                    return Err(self.reflection_error(format!("Function {}() does not exist", String::from_utf8_lossy(&name.bytes())), line));
                }
                FunctionReflection { target: self.resolve_callable(Value::String(name), line)?, is_method: false }
            }
            _ => return Err(self.reflection_error(format!("ReflectionParameter::__construct(): Argument #1 ($function) must be a string, an array(class, method), or a callable object, {} given", builtins::debug_type(&value)), line)),
        };
        let parameters = function.parameters();
        let selector = self.lookup(b"param").unwrap();
        let index = match selector {
            Value::Int(index) => usize::try_from(index)
                .ok()
                .filter(|index| *index < parameters.len())
                .ok_or_else(|| {
                    self.reflection_error(
                        "The parameter specified by its offset could not be found",
                        line,
                    )
                })?,
            Value::String(name) => parameters
                .iter()
                .position(|p| p.name == name.to_vec())
                .ok_or_else(|| {
                    self.reflection_error(
                        "The parameter specified by its name could not be found",
                        line,
                    )
                })?,
            _ => unreachable!("bound parameter selector"),
        };
        object.properties.borrow_mut().insert(
            ArrayKey::String(b"name".to_vec()),
            string(&parameters[index].name),
        );
        *object.reflection.borrow_mut() = Some(ReflectionData::Parameter { function, index });
        Ok(Value::Null)
    }

    pub(super) fn reflect_parameter_method(
        &mut self,
        function: FunctionReflection,
        index: usize,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let parameters = function.parameters();
        let parameter = &parameters[index];
        let last_required = parameters
            .iter()
            .rposition(|p| !p.variadic && p.default.is_none());
        let optional = last_required.is_none_or(|last| index > last);
        let default = parameter
            .default
            .as_ref()
            .filter(|_| optional && !parameter.variadic);
        Ok(match name {
            b"getname" => string(&parameter.name),
            b"getposition" => Value::Int(index as i64),
            b"ispassedbyreference" => Value::Bool(parameter.by_reference),
            b"canbepassedbyvalue" => Value::Bool(!parameter.by_reference),
            b"isoptional" => Value::Bool(optional),
            b"isvariadic" => Value::Bool(parameter.variadic),
            b"ispromoted" => Value::Bool(parameter.promotion.is_some()),
            b"hastype" => Value::Bool(parameter.ty.is_some()),
            b"gettype" => parameter
                .ty
                .clone()
                .map_or(Value::Null, |ty| self.reflection_type(ty)),
            b"allowsnull" => Value::Bool(
                parameter
                    .ty
                    .as_ref()
                    .is_none_or(|ty| ty.contains("null") || ty.contains("mixed")),
            ),
            b"getdeclaringclass" => {
                function
                    .target
                    .context
                    .as_ref()
                    .map_or(Value::Null, |context| {
                        self.reflection_object(
                            b"ReflectionClass",
                            ReflectionData::Class(context.scope.clone()),
                        )
                    })
            }
            b"getdeclaringfunction" => {
                let kind = if function.is_method {
                    b"ReflectionMethod".as_slice()
                } else {
                    b"ReflectionFunction".as_slice()
                };
                self.reflection_object(kind, ReflectionData::Function(function))
            }
            b"isdefaultvalueavailable" => Value::Bool(default.is_some()),
            b"getdefaultvalue" | b"isdefaultvalueconstant" | b"getdefaultvalueconstantname" => {
                let default = default.ok_or_else(|| {
                    self.reflection_error(
                        "Internal error: Failed to retrieve the default value",
                        line,
                    )
                })?;
                if name == b"getdefaultvalue" {
                    let context = function.attribute_context(self);
                    // Reuse declaration-context restoration for deferred constant expressions.
                    let values = self.attribute_arguments(
                        &context,
                        &Attribute {
                            name: Vec::new(),
                            line: default.line,
                            arguments: vec![Argument::Positional(default.clone())],
                        },
                    )?;
                    values.get(&ArrayKey::Int(0)).unwrap().clone()
                } else {
                    let constant = default_constant_name(default);
                    if name == b"isdefaultvalueconstant" {
                        Value::Bool(constant.is_some())
                    } else {
                        constant.map_or(Value::Null, string)
                    }
                }
            }
            b"getattributes" => {
                let mut context = function.attribute_context(self);
                context.target = 32;
                return self.reflected_attributes(
                    &parameter.attributes,
                    &context,
                    "ReflectionParameter",
                    line,
                );
            }
            _ => unreachable!("unhandled parameter reflection method"),
        })
    }
}

fn default_constant_name(expr: &Expr) -> Option<Vec<u8>> {
    match &expr.kind {
        ExprKind::Group(inner) => default_constant_name(inner),
        ExprKind::Constant(name) => Some(name.clone()),
        ExprKind::NamespacedName(name) => Some(name.constant.clone()),
        ExprKind::ScopedMember(class, name) => {
            let mut qualified = class.static_class_name()?.to_vec();
            let ExprKind::Literal(Value::String(name)) = &name.kind else {
                return None;
            };
            qualified.extend(b"::");
            qualified.extend(name.bytes());
            Some(qualified)
        }
        _ => None,
    }
}
