use super::*;
use crate::ast::{Parameter, TypeDecl};
use crate::runtime::functions::CallTarget;

impl FunctionReflection {
    pub(super) fn name(&self) -> &[u8] {
        let name = self
            .target
            .function
            .as_ref()
            .map_or(self.target.name.as_slice(), |f| &f.name);
        if !name.starts_with(b"{closure:") && self.target.context.is_some() {
            name.rsplit(|b| *b == b':').next().unwrap()
        } else {
            name
        }
    }

    fn namespace(&self) -> &[u8] {
        if self.is_method || self.name().starts_with(b"{closure:") {
            return &[];
        }
        self.name()
            .iter()
            .rposition(|b| *b == b'\\')
            .map_or(&[], |i| &self.name()[..i])
    }

    pub(super) fn parameters(&self) -> Vec<Parameter> {
        self.target.function.as_ref().map_or_else(
            || {
                self.target
                    .builtin
                    .map_or_else(Vec::new, |b| b.parameters())
            },
            |f| f.parameters.clone(),
        )
    }

    fn internal(&self) -> bool {
        self.target.builtin.is_some()
            || self
                .target
                .function
                .as_ref()
                .is_some_and(|f| f.native.is_some())
    }

    pub(super) fn modifiers(&self) -> i64 {
        let function = self.target.function.as_ref().unwrap();
        let class = &self.target.context.as_ref().unwrap().scope;
        visibility_flags(function.visibility)
            | (i64::from(function.is_static) * 16)
            | (i64::from(function.is_abstract) * 64)
            | (i64::from(class.method_is_final(self.name())) * 32)
    }

    pub(super) fn attribute_context(&self, runtime: &Runtime) -> AttributeContext {
        AttributeContext {
            class: self.target.context.as_ref().map(|c| c.scope.clone()),
            source: self
                .target
                .function
                .as_ref()
                .map_or_else(|| runtime.source.clone(), |f| f.source.clone()),
            strict_types: self
                .target
                .function
                .as_ref()
                .is_some_and(|f| f.strict_types),
            target: if self.target.context.is_some() && !self.name().starts_with(b"{closure:") {
                4
            } else {
                2
            },
        }
    }
}

impl Runtime {
    pub(super) fn reflection_function_constructor(
        &mut self,
        object: &Rc<PhpObject>,
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let function = if self
            .current_function
            .as_ref()
            .unwrap()
            .starts_with(b"reflectionmethod::")
        {
            if supplied == 1 {
                self.diagnostics.push(format!("Deprecated: Calling ReflectionMethod::__construct() with 1 argument is deprecated, use ReflectionMethod::createFromMethodName() instead on line {line}"));
            }
            let subject = self.lookup(b"objectOrMethod").unwrap();
            let (subject, name) = match self.lookup(b"method").unwrap() {
                Value::String(name) => (subject, name.to_vec()),
                _ => {
                    if matches!(subject, Value::Object(_) | Value::Closure(_)) {
                        return Err(Error::new(
                            ErrorKind::ValueError,
                            "ReflectionMethod::__construct(): Argument #2 ($method) cannot be null when argument #1 ($objectOrMethod) is an object",
                            line,
                        ));
                    }
                    let name = subject.to_bytes();
                    let Some(split) = name.windows(2).position(|part| part == b"::") else {
                        return Err(self.reflection_error("ReflectionMethod::__construct(): Argument #1 ($objectOrMethod) must be a valid method name", line));
                    };
                    (string(&name[..split]), name[split + 2..].to_vec())
                }
            };
            let class = self.reflected_class(subject, false, line)?;
            self.reflected_method(&class, &name, line)?
        } else {
            let value = self.lookup(b"function").unwrap();
            if let Value::String(name) = &value {
                let normalized = name.to_vec();
                let normalized = normalized
                    .strip_prefix(b"\\")
                    .unwrap_or(&normalized)
                    .to_ascii_lowercase();
                if !self.functions.contains_key(&normalized)
                    && builtins::Builtin::lookup(&normalized).is_none()
                {
                    return Err(self.reflection_error(
                        format!(
                            "Function {}() does not exist",
                            String::from_utf8_lossy(&name.bytes())
                        ),
                        line,
                    ));
                }
            }
            FunctionReflection {
                target: self.resolve_callable(value, line)?,
                is_method: false,
            }
        };
        let mut properties = object.properties.borrow_mut();
        properties.insert(ArrayKey::String(b"name".to_vec()), string(function.name()));
        if function.is_method {
            properties.insert(
                ArrayKey::String(b"class".to_vec()),
                string(&function.target.context.as_ref().unwrap().scope.name),
            );
        }
        *object.reflection.borrow_mut() = Some(ReflectionData::Function(function));
        Ok(Value::Null)
    }

    pub(super) fn reflection_method_factory(&mut self, line: usize) -> Result<Value, Error> {
        let name = self.lookup(b"method").unwrap().to_bytes();
        let Some(split) = name.windows(2).position(|part| part == b"::") else {
            return Err(self.reflection_error("ReflectionMethod::createFromMethodName(): Argument #1 ($method) must be a valid method name", line));
        };
        let class = self.reflected_class(string(&name[..split]), false, line)?;
        let function = self.reflected_method(&class, &name[split + 2..], line)?;
        let reflection_class = self.class_context.as_ref().unwrap().called.clone();
        let object = self.instantiate_class(reflection_class, &[], false, line)?;
        let Value::Object(receiver) = &object else {
            unreachable!()
        };
        receiver
            .properties
            .borrow_mut()
            .insert(ArrayKey::String(b"name".to_vec()), string(function.name()));
        receiver.properties.borrow_mut().insert(
            ArrayKey::String(b"class".to_vec()),
            string(&function.target.context.as_ref().unwrap().scope.name),
        );
        *receiver.reflection.borrow_mut() = Some(ReflectionData::Function(function));
        Ok(object)
    }

    pub(super) fn reflected_method(
        &mut self,
        class: &Rc<Class>,
        name: &[u8],
        line: usize,
    ) -> Result<FunctionReflection, Error> {
        let function = objects::method(class, name).ok_or_else(|| {
            self.reflection_error(
                format!(
                    "Method {}::{}() does not exist",
                    class.display_name(),
                    String::from_utf8_lossy(name)
                ),
                line,
            )
        })?;
        let scope = self
            .find_class(method_visibility::declaring_name(&function))
            .expect("method declaring class");
        Ok(FunctionReflection {
            target: CallTarget {
                result_used: true,
                reference_values: false,
                magic_name: None,
                name: function.name.clone(),
                function: Some(function),
                builtin: None,
                closure: None,
                this: None,
                context: Some(ClassContext {
                    scope,
                    called: class.clone(),
                }),
            },
            is_method: true,
        })
    }

    pub(super) fn reflect_function_method(
        &mut self,
        reflection: FunctionReflection,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let target = &reflection.target;
        let function = target.function.as_ref();
        let internal = reflection.internal();
        Ok(match name {
            b"getname" => string(reflection.name()),
            b"getshortname" => string(if reflection.namespace().is_empty() {
                reflection.name()
            } else {
                reflection.name().rsplit(|b| *b == b'\\').next().unwrap()
            }),
            b"getnamespacename" => string(reflection.namespace()),
            b"innamespace" => Value::Bool(!reflection.namespace().is_empty()),
            b"isclosure" => Value::Bool(target.closure.is_some()),
            b"isanonymous" => Value::Bool(
                target
                    .closure
                    .as_ref()
                    .is_some_and(|c| c.environment.is_some()),
            ),
            b"isinternal" => Value::Bool(internal),
            b"isuserdefined" => Value::Bool(!internal),
            b"isdeprecated" => Value::Bool(function.is_some_and(|f| {
                f.attributes
                    .iter()
                    .any(|a| a.name.eq_ignore_ascii_case(b"Deprecated"))
            })),
            b"isgenerator" => Value::Bool(false),
            b"isstatic" => Value::Bool(function.is_some_and(|f| f.is_static)),
            b"isvariadic" => Value::Bool(reflection.parameters().iter().any(|p| p.variadic)),
            b"getparameters" => {
                let mut result = PhpArray::default();
                for index in 0..reflection.parameters().len() {
                    result.append(
                        self.reflection_object(
                            b"ReflectionParameter",
                            ReflectionData::Parameter {
                                function: reflection.clone(),
                                index,
                            },
                        ),
                        line,
                    )?;
                }
                Value::Array(Rc::new(result))
            }
            b"getnumberofparameters" => Value::Int(reflection.parameters().len() as i64),
            b"getnumberofrequiredparameters" => Value::Int(
                reflection
                    .parameters()
                    .iter()
                    .rposition(|p| !p.variadic && p.default.is_none())
                    .map_or(0, |i| i + 1) as i64,
            ),
            b"getfilename" | b"getstartline" | b"getendline" => {
                if internal {
                    Value::Bool(false)
                } else {
                    let function = function.unwrap();
                    match name {
                        b"getfilename" => string(&function.source.name),
                        b"getstartline" => Value::Int(function.line as i64),
                        _ => Value::Int(function.end_line as i64),
                    }
                }
            }
            b"returnsreference" => Value::Bool(function.is_some_and(|f| f.returns_reference)),
            b"hasreturntype" => Value::Bool(function.is_some_and(|f| f.return_type.is_some())),
            b"getreturntype" => match function.and_then(|f| f.return_type.clone()) {
                Some(ty) => self.reflection_type(ty),
                None => Value::Null,
            },
            b"hastentativereturntype" => {
                Value::Bool(function.is_some_and(|f| f.tentative_return_type.is_some()))
            }
            b"gettentativereturntype" => {
                match function.and_then(|f| f.tentative_return_type.clone()) {
                    Some(ty) => self.reflection_type(ty),
                    None => Value::Null,
                }
            }
            b"getattributes" => {
                return self.reflected_attributes(
                    function.map_or(&[], |f| &f.attributes),
                    &reflection.attribute_context(self),
                    "ReflectionFunctionAbstract",
                    line,
                );
            }
            b"getclosurethis" => target
                .closure
                .as_ref()
                .and_then(|c| c.this.clone())
                .map_or(Value::Null, Value::Object),
            b"getclosurescopeclass" | b"getclosurecalledclass" => {
                match target.closure.as_ref().and_then(|c| c.context.as_ref()) {
                    Some(context) => self.reflection_object(
                        b"ReflectionClass",
                        ReflectionData::Class(if name == b"getclosurescopeclass" {
                            context.scope.clone()
                        } else {
                            context.called.clone()
                        }),
                    ),
                    None => Value::Null,
                }
            }
            b"getclosureusedvariables" => {
                let mut result = PhpArray::default();
                if let Some(environment) =
                    target.closure.as_ref().and_then(|c| c.environment.as_ref())
                {
                    for (name, capture) in &environment.captures {
                        let value = match capture {
                            crate::closure::CapturedValue::Value(value) => value.clone(),
                            crate::closure::CapturedValue::Reference(value) => {
                                Value::Reference(value.clone())
                            }
                        };
                        result.insert(ArrayKey::String(name.clone()), value);
                    }
                }
                Value::Array(Rc::new(result))
            }
            b"ispublic" | b"isprotected" | b"isprivate" => Value::Bool(
                function.unwrap().visibility
                    == match name {
                        b"ispublic" => Visibility::Public,
                        b"isprotected" => Visibility::Protected,
                        _ => Visibility::Private,
                    },
            ),
            b"isabstract" => Value::Bool(function.unwrap().is_abstract),
            b"isfinal" => Value::Bool(reflection.modifiers() & 32 != 0),
            b"getmodifiers" => Value::Int(reflection.modifiers()),
            b"isconstructor" => Value::Bool(reflection.name().eq_ignore_ascii_case(b"__construct")),
            b"isdestructor" => Value::Bool(reflection.name().eq_ignore_ascii_case(b"__destruct")),
            b"getdeclaringclass" => self.reflection_object(
                b"ReflectionClass",
                ReflectionData::Class(target.context.as_ref().unwrap().scope.clone()),
            ),
            b"setaccessible" => {
                self.diagnostics.push(format!("Deprecated: Method ReflectionMethod::setAccessible() is deprecated since 8.5, as it has no effect on line {line}"));
                Value::Null
            }
            b"getclosure" | b"invoke" | b"invokeargs" => {
                let mut target = target.clone();
                if reflection.is_method {
                    let function = target.function.as_ref().unwrap();
                    if name != b"getclosure" && function.is_abstract {
                        return Err(self.reflection_error(
                            format!(
                                "Trying to invoke abstract method {}()",
                                String::from_utf8_lossy(&function.name)
                            ),
                            line,
                        ));
                    }
                    if name == b"getclosure" && function.is_static {
                        let context = target.context.as_mut().unwrap();
                        context.called = context.scope.clone();
                    }
                    let supplied = self.lookup(b"object").unwrap();
                    if !function.is_static {
                        let Value::Object(object) = supplied else {
                            if name == b"getclosure" {
                                return Err(Error::new(
                                    ErrorKind::ValueError,
                                    "ReflectionMethod::getClosure(): Argument #1 ($object) cannot be null for non-static methods",
                                    line,
                                ));
                            }
                            return Err(self.reflection_error(
                                format!(
                                    "Trying to invoke non static method {}() without an object",
                                    String::from_utf8_lossy(&function.name)
                                ),
                                line,
                            ));
                        };
                        if !object
                            .class
                            .is_a(&target.context.as_ref().unwrap().scope.name)
                        {
                            return Err(self.reflection_error("Given object is not an instance of the class this method was declared in", line));
                        }
                        target.context.as_mut().unwrap().called = object.class.clone();
                        target.this = Some(object);
                    }
                }
                if name == b"getclosure" {
                    if let Some(closure) = target.closure {
                        return Ok(Value::Closure(closure));
                    }
                    let handle = self.allocate_object_handle();
                    return Ok(Value::Closure(Rc::new(crate::Closure {
                        magic_name: target.magic_name,
                        name: target.name,
                        this: target.this,
                        context: target.context,
                        function: target.function,
                        environment: None,
                        object_id: handle.id,
                        _handle: handle,
                    })));
                }
                let Some(Value::Array(args)) = self.lookup(b"args") else {
                    unreachable!()
                };
                let args = callbacks::callback_arguments(&args, line)?;
                target.reference_values = true;
                let strict = std::mem::replace(&mut self.strict_types, false);
                let result = self.invoke_at(target, &args, line, true, true);
                self.strict_types = strict;
                return result.map(|value| value.dereferenced());
            }
            _ => unreachable!("unhandled function reflection method"),
        })
    }

    pub(super) fn reflection_type(&mut self, mut ty: TypeDecl) -> Value {
        ty.members = ty
            .members
            .into_iter()
            .flat_map(|m| {
                if m == "iterable" {
                    vec!["Traversable".to_owned(), "array".to_owned()]
                } else {
                    vec![m]
                }
            })
            .collect();
        ty.members.sort_by_key(|m| type_order(m));
        ty.members.dedup();
        let nullable_named = ty.members.len() == 2 && ty.contains("null");
        let kind = if ty.members.len() == 1 || nullable_named {
            b"ReflectionNamedType".as_slice()
        } else {
            b"ReflectionUnionType".as_slice()
        };
        self.reflection_object(kind, ReflectionData::Type(ty))
    }

    pub(super) fn reflect_type_method(
        &mut self,
        ty: TypeDecl,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let nonnull = ty
            .members
            .iter()
            .find(|m| *m != "null")
            .unwrap_or(&ty.members[0]);
        Ok(match name {
            b"allowsnull" => Value::Bool(ty.contains("null") || ty.contains("mixed")),
            b"getname" => string(nonnull),
            b"isbuiltin" => Value::Bool(type_order(nonnull) > 0 && nonnull != "static"),
            b"__tostring" => string(ty.to_string()),
            b"gettypes" => {
                let mut result = PhpArray::default();
                for member in ty.members {
                    result.append(
                        self.reflection_type(TypeDecl {
                            members: vec![member],
                        }),
                        line,
                    )?;
                }
                Value::Array(Rc::new(result))
            }
            _ => unreachable!("unhandled type reflection method"),
        })
    }
}

fn type_order(name: &str) -> u8 {
    match name {
        "static" => 1,
        "callable" => 2,
        "object" => 3,
        "array" => 4,
        "string" => 5,
        "int" => 6,
        "float" => 7,
        "bool" | "true" | "false" => 8,
        "null" => 9,
        "mixed" | "void" | "never" => 10,
        _ => 0,
    }
}
