use super::*;
use crate::{
    ast::TypeDecl,
    object::{Class, ClassContext},
    runtime::functions::CallTarget,
};

impl Runtime {
    pub(super) fn contextual_class_name(&self, name: &[u8], line: usize) -> Result<Vec<u8>, Error> {
        let keyword = name.to_ascii_lowercase();
        if !matches!(keyword.as_slice(), b"self" | b"parent" | b"static") {
            return Ok(name.strip_prefix(b"\\").unwrap_or(name).to_vec());
        }
        let context = self.class_context.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot access {} when no class scope is active",
                    String::from_utf8_lossy(name)
                ),
                line,
            )
        })?;
        let class = match keyword.as_slice() {
            b"self" => &context.scope,
            b"static" => &context.called,
            _ => context.scope.parent.as_ref().ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    "Cannot access parent when current class scope has no parent",
                    line,
                )
            })?,
        };
        Ok(class.name.clone())
    }

    pub(super) fn class_name_expression(
        &mut self,
        class: &Expr,
        line: usize,
    ) -> Result<Value, Error> {
        let name = if let ExprKind::Constant(name) = &class.kind {
            self.contextual_class_name(name, line)?
        } else {
            match self.eval(class)? {
                Value::Object(object) => object.class.name.clone(),
                Value::Closure(_) => b"Closure".to_vec(),
                value => {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "Cannot use ::class on value of type {}",
                            builtins::debug_type(&value)
                        ),
                        line,
                    ));
                }
            }
        };
        Ok(Value::String(name))
    }

    pub(super) fn resolve_scoped_call(
        &mut self,
        class: &Expr,
        name: &Expr,
        line: usize,
    ) -> Result<CallTarget, Error> {
        let (class, forwarding) = if let ExprKind::Constant(name) = &class.kind {
            (
                self.contextual_class_name(name, line)?,
                matches!(
                    name.to_ascii_lowercase().as_slice(),
                    b"self" | b"parent" | b"static"
                ),
            )
        } else {
            let name = match self.eval(class)? {
                Value::String(name) => name,
                Value::Object(object) => object.class.name.clone(),
                Value::Closure(_) => b"Closure".to_vec(),
                _ => {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Class name must be a valid object or a string",
                        line,
                    ));
                }
            };
            (name, false)
        };
        let name = self.variable_name(name)?;
        self.resolve_class_method(&class, &name, forwarding, true, line)
    }

    pub(super) fn resolve_class_method(
        &self,
        class: &[u8],
        name: &[u8],
        forwarding: bool,
        allow_this: bool,
        line: usize,
    ) -> Result<CallTarget, Error> {
        let class = self.find_class(class).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!("Class \"{}\" not found", String::from_utf8_lossy(class)),
                line,
            )
        })?;
        let function = objects::method(&class, name).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!(
                    "Call to undefined method {}::{}()",
                    String::from_utf8_lossy(&class.name),
                    String::from_utf8_lossy(name)
                ),
                line,
            )
        })?;
        let this = if allow_this && !function.is_static {
            match self.lookup(b"this") {
                Some(Value::Object(object)) if object.class.is_a(&class.name) => Some(object),
                _ => None,
            }
        } else {
            None
        };
        let called = if let Some(object) = &this {
            object.class.clone()
        } else if forwarding {
            self.class_context
                .as_ref()
                .map_or(class.clone(), |c| c.called.clone())
        } else {
            class
        };
        self.method_target(function, called, this, line)
    }

    pub(super) fn method_target(
        &self,
        function: Rc<FunctionDecl>,
        called: Rc<Class>,
        this: Option<Rc<crate::PhpObject>>,
        line: usize,
    ) -> Result<CallTarget, Error> {
        if !function.is_static && this.is_none() {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Non-static method {}() cannot be called statically",
                    String::from_utf8_lossy(&function.name)
                ),
                line,
            ));
        }
        let declaring = function
            .name
            .windows(2)
            .position(|part| part == b"::")
            .expect("method names are qualified");
        let scope = self
            .find_class(&function.name[..declaring])
            .expect("method declaring class is linked");
        let this = if function.is_static { None } else { this };
        Ok(CallTarget {
            name: function.name.clone(),
            function: Some(function),
            builtin: None,
            closure: None,
            this,
            context: Some(ClassContext { scope, called }),
        })
    }

    pub(super) fn resolve_context_type(&self, ty: &TypeDecl) -> Option<TypeDecl> {
        let mut ty = ty.clone();
        for member in &mut ty.members {
            if matches!(member.as_str(), "self" | "parent" | "static") {
                *member = String::from_utf8_lossy(
                    &self.contextual_class_name(member.as_bytes(), 0).ok()?,
                )
                .into_owned();
            }
        }
        Some(ty)
    }
}

impl Runtime {
    pub(super) fn inspect_class(
        &mut self,
        builtin: builtins::Builtin,
        value: Option<&Value>,
        line: usize,
    ) -> Result<Value, Error> {
        use builtins::Builtin;
        if matches!(builtin, Builtin::GetCalledClass) {
            return self
                .class_context
                .as_ref()
                .map(|c| Value::String(c.called.name.clone()))
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Error,
                        "get_called_class() must be called from within a class",
                        line,
                    )
                });
        }
        let parent = matches!(builtin, Builtin::GetParentClass);
        let name = if parent {
            "get_parent_class"
        } else {
            "get_class"
        };
        let class = if let Some(value) = value {
            match value {
                Value::Object(object) => Some(object.class.clone()),
                Value::Closure(_) => {
                    return Ok(if parent {
                        Value::Bool(false)
                    } else {
                        Value::String(b"Closure".to_vec())
                    });
                }
                Value::String(class) if parent => {
                    if class
                        .strip_prefix(b"\\")
                        .unwrap_or(class)
                        .eq_ignore_ascii_case(b"Closure")
                    {
                        return Ok(Value::Bool(false));
                    }
                    Some(self.find_class(class).ok_or_else(|| Error::new(ErrorKind::TypeError, "get_parent_class(): Argument #1 must be an object or a valid class name", line))?)
                }
                _ => {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "{name}(): Argument #1 must be of type {}",
                            if parent { "object|string" } else { "object" }
                        ),
                        line,
                    ));
                }
            }
        } else {
            if parent && self.class_context.is_none() {
                self.warnings.push(format!("Deprecated: Calling get_parent_class() without arguments is deprecated on line {line}"));
                return Ok(Value::Bool(false));
            }
            let class = self
                .class_context
                .as_ref()
                .map(|c| c.scope.clone())
                .ok_or_else(|| {
                    Error::new(
                        ErrorKind::Error,
                        format!("{name}() without arguments must be called from within a class"),
                        line,
                    )
                })?;
            self.warnings.push(format!(
                "Deprecated: Calling {name}() without arguments is deprecated on line {line}"
            ));
            Some(class)
        };
        let class = if parent {
            class.and_then(|c| c.parent.clone())
        } else {
            class
        };
        Ok(class.map_or(Value::Bool(false), |c| Value::String(c.name.clone())))
    }
}

impl Runtime {
    pub(super) fn eval_default(&mut self, expr: &Expr) -> Result<Value, Error> {
        let outer = self.class_context.clone();
        if let Some(context) = &mut self.class_context {
            context.called = context.scope.clone();
        }
        let constant_expression = std::mem::replace(&mut self.in_constant_expression, true);
        let result = self.eval(expr);
        self.in_constant_expression = constant_expression;
        self.class_context = outer;
        result
    }

    pub(super) fn property_defaults(&mut self, class: &Rc<Class>) -> Result<PhpArray, Error> {
        if let Some(defaults) = class.defaults.borrow().as_ref() {
            return Ok(defaults.clone());
        }
        let mut defaults = PhpArray::default();
        for property in &class.properties {
            let declaring = class
                .property_owner(&property.name)
                .expect("property owner");
            if let Some(value) = self.property_default(&declaring, property)? {
                defaults.insert(ArrayKey::String(property.name.clone()), value);
            }
        }
        *class.defaults.borrow_mut() = Some(defaults.clone());
        Ok(defaults)
    }
}
