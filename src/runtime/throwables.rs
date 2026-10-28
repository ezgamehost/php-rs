use super::*;
use crate::ast::{CatchClause, NativeMethod};
use crate::object::{Class, ThrowableData};

fn builtin_spec(name: &[u8]) -> Option<(&'static str, Option<&'static str>)> {
    Some(match name {
        b"stringable" => ("Stringable", None),
        b"throwable" => ("Throwable", None),
        b"exception" => ("Exception", None),
        b"error" => ("Error", None),
        b"typeerror" => ("TypeError", Some("Error")),
        b"argumentcounterror" => ("ArgumentCountError", Some("TypeError")),
        b"arithmeticerror" => ("ArithmeticError", Some("Error")),
        b"divisionbyzeroerror" => ("DivisionByZeroError", Some("ArithmeticError")),
        b"compileerror" => ("CompileError", Some("Error")),
        b"parseerror" => ("ParseError", Some("CompileError")),
        b"valueerror" => ("ValueError", Some("Error")),
        b"unhandledmatcherror" => ("UnhandledMatchError", Some("Error")),
        _ => return None,
    })
}

impl Runtime {
    pub(super) fn builtin_throwable_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, parent_name) = builtin_spec(name)?;
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        // Parse internal declarations with the same signature representation as
        // user methods; native bodies are dispatched explicitly, never as PHP.
        let code = if canonical == "Stringable" {
            "<?php interface Stringable { public function __toString():string; }".to_owned()
        } else if canonical == "Throwable" {
            "<?php interface Throwable extends Stringable {}".to_owned()
        } else if let Some(parent) = parent_name {
            format!("<?php class {canonical} extends {parent} {{}}")
        } else {
            format!("<?php class {canonical} implements Throwable {{
            public function __construct(string $message='', int $code=0, ?Throwable $previous=null) {{}}
            final public function getMessage():string {{}}
            final public function getCode() {{}}
            final public function getFile():string {{}}
            final public function getLine():int {{}}
            final public function getPrevious():?Throwable {{}}
            final public function getTrace():array {{}}
            final public function getTraceAsString():string {{}}
            public function __toString():string {{}}
        }}")
        };
        let (mut statements, _, _) =
            parser::parse(lexer::lex(code.as_bytes()).expect("native source"))
                .expect("native declarations");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for function in &mut Rc::make_mut(&mut declaration).methods {
            let function = Rc::make_mut(function);
            if function.is_abstract {
                continue;
            }
            function.native = Some(match function.name.rsplit(|b| *b == b':').next().unwrap() {
                b"__toString" => NativeMethod::ThrowableToString,
                b"__construct" => NativeMethod::ThrowableConstruct,
                b"getMessage" => NativeMethod::GetMessage,
                b"getCode" => NativeMethod::GetCode,
                b"getFile" => NativeMethod::GetFile,
                b"getLine" => NativeMethod::GetLine,
                b"getPrevious" => NativeMethod::GetPrevious,
                b"getTrace" => NativeMethod::GetTrace,
                b"getTraceAsString" => NativeMethod::GetTraceAsString,
                _ => unreachable!(),
            });
        }
        let parent = parent_name.and_then(|name| self.find_class(name.as_bytes()));
        let interfaces = if canonical == "Exception" || canonical == "Error" {
            vec![self.find_class(b"Throwable").expect("Throwable interface")]
        } else if canonical == "Throwable" {
            vec![
                self.find_class(b"Stringable")
                    .expect("Stringable interface"),
            ]
        } else {
            Vec::new()
        };
        let mut methods = parent.as_ref().map_or_else(Vec::new, |p| p.methods.clone());
        if canonical == "Throwable" {
            methods.extend(interfaces[0].methods.iter().cloned());
        }
        methods.extend(declaration.methods.iter().cloned());
        let class = Rc::new(Class {
            name: canonical.as_bytes().to_vec(),
            declaration,
            parent,
            interfaces,
            methods,
            properties: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
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

    pub(super) fn throwable_data(&self, class: &Class, line: usize) -> Option<ThrowableData> {
        class.is_a(b"Throwable").then(|| ThrowableData {
            trace: self.capture_trace(),
            message: Vec::new(),
            code: 0,
            file: self.source.name.clone(),
            line,
            previous: None,
        })
    }

    pub(super) fn native_throwable_method(
        &mut self,
        method: NativeMethod,
        line: usize,
    ) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            return Err(Error::new(
                ErrorKind::Error,
                "Non-static method requires an object",
                line,
            ));
        };
        if matches!(method, NativeMethod::ThrowableToString) {
            return Ok(Value::String(stringable::throwable_string(&object)));
        }
        let mut data = object.throwable.borrow_mut();
        let data = data.as_mut().expect("native throwable receiver");
        Ok(match method {
            NativeMethod::ThrowableToString => unreachable!(),
            NativeMethod::ThrowableConstruct => {
                let Value::String(message) = self.lookup(b"message").unwrap() else {
                    unreachable!()
                };
                let Value::Int(code) = self.lookup(b"code").unwrap() else {
                    unreachable!()
                };
                let previous = self.lookup(b"previous").unwrap();
                data.message = message;
                data.code = code;
                data.previous = if let Value::Object(previous) = previous {
                    Some(previous)
                } else {
                    None
                };
                Value::Null
            }
            NativeMethod::GetTrace => Value::Array(data.trace.clone()),
            NativeMethod::GetTraceAsString => Value::String(traces::trace_string(&data.trace)),
            NativeMethod::GetMessage => Value::String(data.message.clone()),
            NativeMethod::GetCode => Value::Int(data.code),
            NativeMethod::GetFile => Value::String(data.file.clone()),
            NativeMethod::GetLine => Value::Int(data.line as i64),
            NativeMethod::GetPrevious => data.previous.clone().map_or(Value::Null, Value::Object),
        })
    }

    pub(super) fn materialize_error(&mut self, error: Error) -> Error {
        self.materialize_error_from(error, &self.source.name.clone())
    }

    pub(super) fn materialize_error_from(&mut self, mut error: Error, file: &[u8]) -> Error {
        if error.kind == ErrorKind::UnwindExit || error.throwable.is_some() {
            return error;
        }
        let name = format!("{:?}", error.kind);
        let class = self.find_class(name.as_bytes()).expect("error class");
        self.next_object_id += 1;
        error.throwable = Some(Rc::new(crate::PhpObject {
            class_name: name,
            class,
            object_id: self.next_object_id,
            properties: RefCell::default(),
            reinitializable: RefCell::default(),
            throwable: RefCell::new(Some(ThrowableData {
                trace: self.capture_trace(),
                message: error.message.as_bytes().to_vec(),
                code: 0,
                file: file.to_vec(),
                line: error.line,
                previous: None,
            })),
        }));
        error
    }

    pub(super) fn throw_value(&self, value: Value, line: usize) -> Error {
        let Value::Object(object) = value else {
            return Error::new(ErrorKind::Error, "Can only throw objects", line);
        };
        if !object.class.is_a(b"Throwable") {
            return Error::new(
                ErrorKind::Error,
                "Cannot throw objects that do not implement Throwable",
                line,
            );
        }
        let data = object.throwable.borrow();
        let data = data.as_ref().expect("throwable object");
        let kind = [
            (b"Exception".as_slice(), ErrorKind::Exception),
            (b"ArgumentCountError", ErrorKind::ArgumentCountError),
            (b"TypeError", ErrorKind::TypeError),
            (b"DivisionByZeroError", ErrorKind::DivisionByZeroError),
            (b"ArithmeticError", ErrorKind::ArithmeticError),
            (b"ParseError", ErrorKind::ParseError),
            (b"CompileError", ErrorKind::CompileError),
            (b"ValueError", ErrorKind::ValueError),
            (b"UnhandledMatchError", ErrorKind::UnhandledMatchError),
        ]
        .into_iter()
        .find_map(|(class, kind)| object.class.is_a(class).then_some(kind))
        .unwrap_or(ErrorKind::Error);
        let mut error = Error::new(kind, String::from_utf8_lossy(&data.message), data.line);
        error.throwable = Some(object.clone());
        error
    }

    pub(super) fn try_block(
        &mut self,
        body: &[Stmt],
        catches: &[CatchClause],
        finally: Option<&[Stmt]>,
        entry: Option<&[u8]>,
    ) -> Result<Flow, Error> {
        let mut active_catch = entry.and_then(|name| {
            if super::jumps::label_index(body, name).is_some() {
                None
            } else {
                Some(
                    catches
                        .iter()
                        .position(|c| super::jumps::label_index(&c.body, name).is_some())
                        .expect("catch label"),
                )
            }
        });
        let mut entry = entry.map(<[u8]>::to_vec);
        loop {
            let region = active_catch.map_or(body, |index| catches[index].body.as_slice());
            let result = if let Some(entry) = entry.take() {
                self.enter_exception_body(region, &entry)
            } else {
                self.block(region)
            };
            if let Err(error) = &result {
                if error.kind == ErrorKind::UnwindExit {
                    return result;
                }
                if active_catch.is_none() {
                    let object = error
                        .throwable
                        .as_ref()
                        .expect("materialized exception")
                        .clone();
                    if let Some(index) = catches
                        .iter()
                        .position(|c| c.types.iter().any(|name| object.class.is_a(name)))
                    {
                        if let Some(name) = &catches[index].variable {
                            self.scope.insert(name.clone(), cell(Value::Object(object)));
                        }
                        active_catch = Some(index);
                        continue;
                    }
                }
            }
            if let Ok(Flow::Goto(name)) = &result {
                if super::jumps::label_index(body, name).is_some() {
                    active_catch = None;
                    entry = Some(name.clone());
                    continue;
                }
                if let Some(index) = catches
                    .iter()
                    .position(|c| super::jumps::label_index(&c.body, name).is_some())
                {
                    active_catch = Some(index);
                    entry = Some(name.clone());
                    continue;
                }
            }
            return self.finish_finally(result, finally);
        }
    }

    fn enter_exception_body(&mut self, body: &[Stmt], entry: &[u8]) -> Result<Flow, Error> {
        let index = super::jumps::label_index(body, entry).expect("exception label");
        self.block_from(body, index, Some(entry.to_vec()))
    }

    fn finish_finally(
        &mut self,
        pending: Result<Flow, Error>,
        finally: Option<&[Stmt]>,
    ) -> Result<Flow, Error> {
        if pending
            .as_ref()
            .is_err_and(|e| e.kind == ErrorKind::UnwindExit)
        {
            return pending;
        }
        let Some(finally) = finally else {
            return pending;
        };
        match self.block(finally) {
            Ok(Flow::Next) => pending,
            Err(error) => {
                if let Err(previous) = pending
                    && let (Some(current), Some(previous)) = (&error.throwable, previous.throwable)
                {
                    append_previous(current, &previous);
                }
                Err(error)
            }
            result => result,
        }
    }
}

fn append_previous(current: &Rc<crate::PhpObject>, previous: &Rc<crate::PhpObject>) {
    let mut seen = HashSet::new();
    let mut next = Some(previous.clone());
    while let Some(object) = next {
        if Rc::ptr_eq(&object, current) || !seen.insert(object.object_id) {
            return;
        }
        next = object
            .throwable
            .borrow()
            .as_ref()
            .and_then(|d| d.previous.clone());
    }
    let mut cursor = current.clone();
    seen.clear();
    loop {
        if Rc::ptr_eq(&cursor, previous) || !seen.insert(cursor.object_id) {
            return;
        }
        let next = cursor
            .throwable
            .borrow()
            .as_ref()
            .and_then(|d| d.previous.clone());
        if let Some(next) = next {
            cursor = next;
        } else {
            cursor.throwable.borrow_mut().as_mut().unwrap().previous = Some(previous.clone());
            return;
        }
    }
}
