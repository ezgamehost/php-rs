use super::*;
use crate::ast::{CatchClause, NativeMethod};
use crate::object::Class;

fn builtin_spec(name: &[u8]) -> Option<(&'static str, Option<&'static str>)> {
    Some(match name {
        b"arrayaccess" => ("ArrayAccess", None),
        b"serializable" => ("Serializable", None),
        b"__php_incomplete_class" => ("__PHP_Incomplete_Class", None),
        b"jsonserializable" => ("JsonSerializable", None),
        b"countable" => ("Countable", None),
        b"traversable" => ("Traversable", None),
        b"iterator" => ("Iterator", None),
        b"iteratoraggregate" => ("IteratorAggregate", None),
        b"seekableiterator" => ("SeekableIterator", None),
        b"recursiveiterator" => ("RecursiveIterator", None),
        b"outeriterator" => ("OuterIterator", None),
        b"unitenum" => ("UnitEnum", None),
        b"backedenum" => ("BackedEnum", None),
        b"stringable" => ("Stringable", None),
        b"throwable" => ("Throwable", None),
        b"reflectionexception" => ("ReflectionException", Some("Exception")),
        b"random\\randomexception" => ("Random\\RandomException", Some("Exception")),
        b"random\\randomerror" => ("Random\\RandomError", Some("Error")),
        b"random\\brokenrandomengineerror" => (
            "Random\\BrokenRandomEngineError",
            Some("Random\\RandomError"),
        ),
        b"exception" => ("Exception", None),
        b"errorexception" => ("ErrorException", Some("Exception")),
        b"logicexception" => ("LogicException", Some("Exception")),
        b"badfunctioncallexception" => ("BadFunctionCallException", Some("LogicException")),
        b"badmethodcallexception" => ("BadMethodCallException", Some("BadFunctionCallException")),
        b"domainexception" => ("DomainException", Some("LogicException")),
        b"invalidargumentexception" => ("InvalidArgumentException", Some("LogicException")),
        b"lengthexception" => ("LengthException", Some("LogicException")),
        b"outofrangeexception" => ("OutOfRangeException", Some("LogicException")),
        b"runtimeexception" => ("RuntimeException", Some("Exception")),
        b"outofboundsexception" => ("OutOfBoundsException", Some("RuntimeException")),
        b"overflowexception" => ("OverflowException", Some("RuntimeException")),
        b"rangeexception" => ("RangeException", Some("RuntimeException")),
        b"underflowexception" => ("UnderflowException", Some("RuntimeException")),
        b"unexpectedvalueexception" => ("UnexpectedValueException", Some("RuntimeException")),
        b"error" => ("Error", None),
        b"typeerror" => ("TypeError", Some("Error")),
        b"argumentcounterror" => ("ArgumentCountError", Some("TypeError")),
        b"arithmeticerror" => ("ArithmeticError", Some("Error")),
        b"divisionbyzeroerror" => ("DivisionByZeroError", Some("ArithmeticError")),
        b"compileerror" => ("CompileError", Some("Error")),
        b"parseerror" => ("ParseError", Some("CompileError")),
        b"assertionerror" => ("AssertionError", Some("Error")),
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
        let code = if canonical == "ArrayAccess" {
            "<?php interface ArrayAccess {
                public function offsetExists(mixed $offset): bool;
                public function offsetGet(mixed $offset): mixed;
                public function offsetSet(mixed $offset, mixed $value): void;
                public function offsetUnset(mixed $offset): void;
            }"
            .to_owned()
        } else if canonical == "__PHP_Incomplete_Class" {
            "<?php final class __PHP_Incomplete_Class {}".to_owned()
        } else if canonical == "Serializable" {
            "<?php interface Serializable { public function serialize(); public function unserialize(string $data); }".to_owned()
        } else if canonical == "JsonSerializable" {
            "<?php interface JsonSerializable { public function jsonSerialize(): mixed; }"
                .to_owned()
        } else if canonical == "Countable" {
            "<?php interface Countable { public function count(): int; }".to_owned()
        } else if canonical == "Traversable" {
            "<?php interface Traversable {}".to_owned()
        } else if canonical == "IteratorAggregate" {
            "<?php interface IteratorAggregate extends Traversable { public function getIterator(): Traversable; }".to_owned()
        } else if canonical == "SeekableIterator" {
            "<?php interface SeekableIterator extends Iterator { public function seek(int $offset): void; }".to_owned()
        } else if canonical == "RecursiveIterator" {
            "<?php interface RecursiveIterator extends Iterator { public function hasChildren(): bool; public function getChildren(): ?RecursiveIterator; }".to_owned()
        } else if canonical == "OuterIterator" {
            "<?php interface OuterIterator extends Iterator { public function getInnerIterator(): ?Iterator; }".to_owned()
        } else if canonical == "Iterator" {
            "<?php interface Iterator extends Traversable {
                public function current(): mixed;
                public function next(): void;
                public function key(): mixed;
                public function valid(): bool;
                public function rewind(): void;
            }"
            .to_owned()
        } else if canonical == "UnitEnum" {
            "<?php interface UnitEnum { public static function cases(): array; }".to_owned()
        } else if canonical == "BackedEnum" {
            "<?php interface BackedEnum extends UnitEnum { public static function from(int|string $value): static; public static function tryFrom(int|string $value): ?static; }".to_owned()
        } else if canonical == "Stringable" {
            "<?php interface Stringable { public function __toString():string; }".to_owned()
        } else if canonical == "Throwable" {
            "<?php interface Throwable extends Stringable {
                public function getMessage(): string;
                public function getCode();
                public function getFile(): string;
                public function getLine(): int;
                public function getTrace(): array;
                public function getPrevious(): ?Throwable;
                public function getTraceAsString(): string;
            }"
            .to_owned()
        } else if canonical == "ErrorException" {
            "<?php class ErrorException extends Exception {
                protected int $severity = 1;
                public function __construct(string $message='', int $code=0,
                    int $severity=1, ?string $filename=null, ?int $line=null,
                    ?Throwable $previous=null) {}
                final public function getSeverity():int {}
            }"
            .to_owned()
        } else if let Some(parent) = parent_name {
            if let Some((namespace, name)) = canonical.rsplit_once('\\') {
                format!("<?php namespace {namespace}; class {name} extends \\{parent} {{}}")
            } else {
                format!("<?php class {canonical} extends {parent} {{}}")
            }
        } else {
            format!("<?php class {canonical} implements Throwable {{
            protected $message = \"\";
            private string $string = \"\";
            protected $code = 0;
            protected string $file = \"\";
            protected int $line{};
            private array $trace = [];
            private ?Throwable $previous = null;
            private function __clone(): void {{}}
            public function __wakeup(): void {{}}
            public function __construct(string $message='', int $code=0, ?Throwable $previous=null) {{}}
            final public function getMessage():string {{}}
            final public function getCode() {{}}
            final public function getFile():string {{}}
            final public function getLine():int {{}}
            final public function getPrevious():?Throwable {{}}
            final public function getTrace():array {{}}
            final public function getTraceAsString():string {{}}
            public function __toString():string {{}}
        }}", if canonical == "Exception" { " = 0" } else { "" })
        };
        let (mut statements, _, _) =
            parser::parse(lexer::lex(code.as_bytes()).expect("native source"))
                .expect("native declarations");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for property in &mut Rc::make_mut(&mut declaration).properties {
            property.declaring_class = canonical.as_bytes().to_vec();
        }
        for function in &mut Rc::make_mut(&mut declaration).methods {
            let function = Rc::make_mut(function);
            if function.is_abstract {
                function.native = Some(NativeMethod::Abstract);
                if matches!(
                    canonical,
                    "ArrayAccess"
                        | "Countable"
                        | "JsonSerializable"
                        | "Iterator"
                        | "IteratorAggregate"
                        | "SeekableIterator"
                        | "RecursiveIterator"
                        | "OuterIterator"
                ) {
                    function.tentative_return_type = function.return_type.take();
                }
                continue;
            }
            if function.name.ends_with(b"::__wakeup") {
                function.tentative_return_type = function.return_type.take();
            }
            function.native = Some(match function.name.rsplit(|b| *b == b':').next().unwrap() {
                b"__toString" => NativeMethod::ThrowableToString,
                b"__wakeup" => NativeMethod::ThrowableWakeup,
                b"__clone" => NativeMethod::ThrowableClone,
                b"__construct" if canonical == "ErrorException" => {
                    NativeMethod::ErrorExceptionConstruct
                }
                b"__construct" => NativeMethod::ThrowableConstruct,
                b"getMessage" => NativeMethod::GetMessage,
                b"getCode" => NativeMethod::GetCode,
                b"getFile" => NativeMethod::GetFile,
                b"getLine" => NativeMethod::GetLine,
                b"getPrevious" => NativeMethod::GetPrevious,
                b"getTrace" => NativeMethod::GetTrace,
                b"getTraceAsString" => NativeMethod::GetTraceAsString,
                b"getSeverity" => NativeMethod::GetSeverity,
                _ => unreachable!(),
            });
        }
        let parent = parent_name.and_then(|name| self.find_class(name.as_bytes()));
        let interfaces = if matches!(
            canonical,
            "SeekableIterator" | "RecursiveIterator" | "OuterIterator"
        ) {
            vec![self.find_class(b"Iterator").unwrap()]
        } else if matches!(canonical, "Iterator" | "IteratorAggregate") {
            vec![self.find_class(b"Traversable").unwrap()]
        } else if canonical == "BackedEnum" {
            vec![self.find_class(b"UnitEnum").unwrap()]
        } else if canonical == "Exception" || canonical == "Error" {
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
        if matches!(
            canonical,
            "Throwable" | "BackedEnum" | "SeekableIterator" | "RecursiveIterator" | "OuterIterator"
        ) {
            methods.extend(interfaces[0].methods.iter().cloned());
        }
        for own in &declaration.methods {
            let name = own.name.rsplit(|b| *b == b':').next().unwrap();
            methods.retain(|inherited| {
                !inherited
                    .name
                    .rsplit(|b| *b == b':')
                    .next()
                    .unwrap()
                    .eq_ignore_ascii_case(name)
            });
            methods.push(own.clone());
        }
        let mut properties = parent
            .as_ref()
            .map_or_else(Vec::new, |p| p.properties.clone());
        properties.extend(declaration.properties.iter().cloned());
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: canonical.as_bytes().to_vec(),
            declaration,
            parent,
            interfaces,
            methods,
            properties,
            static_properties: Vec::new(),
            allow_dynamic: canonical == "__PHP_Incomplete_Class",
            forbid_dynamic: false,
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

    pub(super) fn native_throwable_method(
        &mut self,
        method: NativeMethod,
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        if matches!(method, NativeMethod::ObjectStorage) {
            return self.native_object_storage_method(line);
        }
        if matches!(method, NativeMethod::LinkedList) {
            return self.native_linked_list_method(line);
        }
        if matches!(method, NativeMethod::FixedArray) {
            return self.native_fixed_array_method(line);
        }
        if matches!(method, NativeMethod::RecursiveIterator) {
            return self.native_recursive_iterator_method(line);
        }
        if matches!(method, NativeMethod::ArrayIterator) {
            return self.native_array_iterator_method(supplied, line);
        }
        if matches!(method, NativeMethod::Enum) {
            return self.native_enum_method(line);
        }
        if matches!(method, NativeMethod::Reflection) {
            return self.native_reflection_method(supplied, line);
        }
        if matches!(method, NativeMethod::SensitiveValue) {
            return self.native_sensitive_value(line);
        }
        if matches!(method, NativeMethod::AttributeConstruct) {
            return self.native_attribute_constructor(line);
        }
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            return Err(Error::new(
                ErrorKind::Error,
                "Non-static method requires an object",
                line,
            ));
        };
        self.throwable_method(&object, method, supplied, line)
    }

    pub(super) fn materialize_error(&mut self, error: Error) -> Error {
        self.materialize_error_from(error, &self.source.name.clone())
    }

    pub(super) fn materialize_error_from(&mut self, mut error: Error, file: &[u8]) -> Error {
        if error.kind == ErrorKind::UnwindExit || error.throwable.is_some() {
            return error;
        }
        let name = match error.kind {
            ErrorKind::RandomException => "Random\\RandomException".to_owned(),
            ErrorKind::BrokenRandomEngineError => "Random\\BrokenRandomEngineError".to_owned(),
            _ => format!("{:?}", error.kind),
        };
        let class = self.find_class(name.as_bytes()).expect("error class");
        let handle = self.allocate_object_handle();
        let mut properties = self
            .property_defaults(&class)
            .expect("native error defaults");
        for (name, value) in [
            (b"trace".as_slice(), Value::Array(self.capture_trace())),
            (
                b"message",
                Value::String(error.message.as_bytes().to_vec().into()),
            ),
            (b"file", Value::String(file.to_vec().into())),
            (b"line", Value::Int(error.line as i64)),
        ] {
            properties.insert(
                super::throwable_properties::property_key(&class, name),
                value,
            );
        }
        error.throwable = Some(self.create_object(crate::PhpObject {
            class_name: name,
            class,
            object_id: handle.id,
            _handle: handle,
            properties: RefCell::new(properties),
            linked_list: RefCell::default(),
            object_storage: RefCell::default(),
            fixed_array: RefCell::default(),
            internal_iterator: RefCell::default(),
            recursive_iterator: RefCell::default(),
            array_iterator: RefCell::default(),
            properties_reordered: std::cell::Cell::new(false),
            property_cursor: RefCell::default(),
            property_iterators: RefCell::default(),
            unset_properties: RefCell::default(),
            reinitializable: RefCell::default(),
            reflection: RefCell::default(),
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
        let data = object.throwable_snapshot();
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
            (b"AssertionError", ErrorKind::AssertionError),
        ]
        .into_iter()
        .find_map(|(class, kind)| object.class.is_a(class).then_some(kind))
        .unwrap_or(ErrorKind::Error);
        let mut error = Error::new(
            kind,
            String::from_utf8_lossy(&data.message),
            usize::try_from(data.line).unwrap_or(line),
        );
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
                            let reference = self.scope.get_or_insert(name, Value::Null);
                            let assigned = self.write_reference_value(
                                &reference,
                                Value::Object(object),
                                true,
                                catches[index].line,
                            );
                            if let Err(error) =
                                self.drain_destructors(assigned, catches[index].line)
                            {
                                let error = self.materialize_error(error);
                                return self.finish_finally(Err(error), finally);
                            }
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

pub(super) fn append_previous(current: &Rc<crate::PhpObject>, previous: &Rc<crate::PhpObject>) {
    // A shared ancestor would create a cycle even if neither root is the other.
    let mut ancestors = HashSet::new();
    let mut next = Some(previous.clone());
    while let Some(object) = next {
        if !ancestors.insert(object.object_id) {
            return;
        }
        next = object.throwable_previous();
    }
    let mut cursor = current.clone();
    let mut seen = HashSet::new();
    loop {
        if ancestors.contains(&cursor.object_id) || !seen.insert(cursor.object_id) {
            return;
        }
        if let Some(next) = cursor.throwable_previous() {
            cursor = next;
        } else {
            cursor.set_throwable_property(b"previous", Value::Object(previous.clone()));
            return;
        }
    }
}
