// Altered Rust adaptation of PHP 8.5.0 Zend/zend_closures.c and zend_closures.stub.php.
// Copyright (c) Zend Technologies Ltd.; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    ast::{LambdaDecl, NativeMethod},
    closure::{CapturedValue, Environment},
    object::{Class, ClassContext},
};

impl Runtime {
    pub(super) fn acquire_lambda(&mut self, lambda: &LambdaDecl) -> Result<Value, Error> {
        let mut captures = Vec::new();
        for capture in &lambda.captures {
            if capture.name == b"this" {
                continue;
            }
            let value = if capture.by_reference {
                CapturedValue::Reference(self.scope.get_or_insert(&capture.name, Value::Null))
            } else if let Some(value) = self.lookup(&capture.name) {
                CapturedValue::Value(value)
            } else if lambda.arrow {
                continue;
            } else {
                self.warn(format!(
                    "Undefined variable ${} on line {}",
                    String::from_utf8_lossy(&capture.name),
                    lambda.function.line
                ))?;
                CapturedValue::Value(Value::Null)
            };
            captures.push((capture.name.clone(), value));
        }
        let handle = self.allocate_object_handle();
        Ok(Value::Closure(Rc::new(crate::Closure {
            native_receiver: None,
            magic_name: None,
            name: lambda.function.name.clone(),
            context: self.class_context.clone(),
            this: if !lambda.is_static {
                match self.lookup(b"this") {
                    Some(Value::Object(object)) => Some(object),
                    _ => None,
                }
            } else {
                None
            },
            function: Some(lambda.function.clone()),
            object_id: handle.id,
            _handle: handle,
            environment: Some(Environment {
                captures,
                statics: RefCell::new(HashMap::new()),
            }),
        })))
    }
}

impl Runtime {
    pub(super) fn builtin_closure_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        if name != b"closure" {
            return None;
        }
        let source = b"<?php final class Closure {
            private function __construct() {}
            public static function bind(Closure $closure, ?object $newThis, object|string|null $newScope = 'static'): ?Closure {}
            public function bindTo(?object $newThis, object|string|null $newScope = 'static'): ?Closure {}
            public function call(object $newThis, mixed ...$args): mixed {}
            public static function fromCallable(callable $callback): Closure {}
            public static function getCurrent(): Closure {}
        }";
        let (mut statements, _, _) = parser::parse(lexer::lex(source).expect("closure source"))
            .expect("closure declaration");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        let decl = Rc::make_mut(&mut declaration);
        decl.line = 0;
        decl.end_line = 0;
        for method in &mut decl.methods {
            Rc::make_mut(method).native = Some(NativeMethod::Closure);
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            properties: Vec::new(),
            declaration,
            parent: None,
            interfaces: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic: true,
            defaults: RefCell::default(),
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        });
        self.builtin_classes
            .borrow_mut()
            .insert(name.to_vec(), class.clone());
        Some(class)
    }

    pub(super) fn native_closure_method(
        &mut self,
        receiver: Option<Rc<crate::Closure>>,
        caller: Option<Rc<crate::Closure>>,
        caller_context: Option<ClassContext>,
        line: usize,
    ) -> Result<Value, Error> {
        let name = self.current_function.as_ref().unwrap().clone();
        if name.ends_with(b"::__construct") {
            return Err(Error::new(
                ErrorKind::Error,
                "Instantiation of class Closure is not allowed",
                line,
            ));
        }
        if name.ends_with(b"::getcurrent") {
            return caller
                .filter(|closure| closure.environment.is_some())
                .map(Value::Closure)
                .ok_or_else(|| {
                    Error::new(ErrorKind::Error, "Current function is not a closure", line)
                });
        }
        if name.ends_with(b"::fromcallable") {
            let callback = self.lookup(b"callback").unwrap();
            let context = std::mem::replace(&mut self.class_context, caller_context);
            let caller_this = self
                .callers
                .last()
                .and_then(|scope| scope.get(b"this"))
                .cloned();
            if let Some(this) = caller_this {
                self.scope.insert(b"this".to_vec(), this);
            }
            let result = self
                .resolve_native_callback(&callback, line)
                .map_err(|error| {
                    if error.kind == ErrorKind::UnwindExit {
                        return error;
                    }
                    Error::new(
                        ErrorKind::TypeError,
                        format!(
                            "Failed to create closure from callable: {}",
                            self.callback_error(&callback, &error)
                        ),
                        line,
                    )
                });
            self.class_context = context;
            self.scope.remove(b"this");
            return Ok(self.closure_from_target(result?));
        }
        let closure = if name.ends_with(b"::bind") {
            let Value::Closure(closure) = self.lookup(b"closure").unwrap() else {
                unreachable!()
            };
            closure
        } else {
            receiver.ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    "Closure method requires a closure object",
                    line,
                )
            })?
        };
        let new_this = self.lookup(b"newThis").unwrap();
        let this_class = match &new_this {
            Value::Object(object) => Some(object.class.clone()),
            Value::Closure(_) => self.find_class(b"Closure"),
            _ => None,
        };
        let call = name.ends_with(b"::call");
        let scope = if call {
            this_class.clone()
        } else {
            match self.lookup(b"newScope").unwrap() {
                Value::Null => None,
                Value::Object(object) => Some(object.class.clone()),
                Value::Closure(_) => self.find_class(b"Closure"),
                Value::String(name) if name.bytes() == b"static" => closure
                    .context
                    .as_ref()
                    .map(|context| context.scope.clone()),
                value => {
                    let name = value.to_bytes();
                    let class = self.load_class(&name, line)?;
                    if class.is_none() {
                        self.warn(format!(
                            "Class \"{}\" not found on line {line}",
                            String::from_utf8_lossy(&name)
                        ))?;
                        return Ok(Value::Null);
                    }
                    class
                }
            }
        };
        if let Some(message) =
            self.invalid_closure_binding(&closure, this_class.as_ref(), scope.as_ref())
        {
            self.warn(format!(
                "{message}, this will be an error in PHP 9 on line {line}"
            ))?;
            return Ok(Value::Null);
        }
        let this = match new_this {
            Value::Object(object) => Some(object),
            Value::Null => None,
            _ => {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Binding a closure receiver is not supported",
                    line,
                ));
            }
        };
        let context = scope
            .or_else(|| this.as_ref().and_then(|_| self.find_class(b"Closure")))
            .map(|scope| ClassContext {
                called: this_class.unwrap_or_else(|| scope.clone()),
                scope,
            });
        if call {
            let Value::Array(args) = self.lookup(b"args").unwrap() else {
                unreachable!()
            };
            let args = callbacks::callback_arguments(&args, line)?;
            let mut target = self.resolve_callable(Value::Closure(closure), line)?;
            target.context = context;
            target.this = this;
            target.reference_values = true;
            let strict = std::mem::replace(&mut self.strict_types, false);
            let result = self.invoke_at(target, &args, line, true, true);
            self.strict_types = strict;
            return result.map(|value| value.dereferenced());
        }
        let Value::Closure(mut bound) = self.clone_value(&Value::Closure(closure), None, line)?
        else {
            unreachable!()
        };
        let result = Rc::get_mut(&mut bound).unwrap();
        result.this = this;
        result.context = context;
        Ok(Value::Closure(bound))
    }

    fn invalid_closure_binding(
        &self,
        closure: &crate::Closure,
        this: Option<&Rc<Class>>,
        scope: Option<&Rc<Class>>,
    ) -> Option<String> {
        let fake = closure.environment.is_none();
        let original = closure.context.as_ref().map(|context| &context.scope);
        let is_static = closure
            .function
            .as_ref()
            .is_some_and(|function| function.is_static);
        if let Some(this) = this {
            if is_static {
                return Some("Cannot bind an instance to a static closure".into());
            }
            if fake
                && let Some(original) = original
                && !this.is_a(&original.name)
            {
                let method = closure.name.rsplit(|byte| *byte == b':').next().unwrap();
                return Some(format!(
                    "Cannot bind method {}::{}() to object of class {}",
                    original.display_name(),
                    String::from_utf8_lossy(method),
                    this.display_name()
                ));
            }
        } else if fake && original.is_some() && !is_static {
            return Some("Cannot unbind $this of method".into());
        } else if !fake
            && closure.this.is_some()
            && closure.function.as_ref().is_some_and(|function| {
                let mut variables = Scope::default();
                variables.reserve_variables(&function.body);
                variables.has_slot(b"this")
            })
        {
            return Some("Cannot unbind $this of closure using $this".into());
        }
        let same_scope = match (original, scope) {
            (Some(a), Some(b)) => Rc::ptr_eq(a, b),
            (None, None) => true,
            _ => false,
        };
        if !same_scope {
            if let Some(scope) = scope
                && self
                    .builtin_classes
                    .borrow()
                    .values()
                    .any(|class| Rc::ptr_eq(class, scope))
            {
                return Some(format!(
                    "Cannot bind closure to scope of internal class {}",
                    scope.display_name()
                ));
            }
            if fake {
                return Some(format!(
                    "Cannot rebind scope of closure created from {}",
                    if original.is_some() {
                        "method"
                    } else {
                        "function"
                    }
                ));
            }
        }
        None
    }
}
