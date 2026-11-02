// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_iterators.c and .stub.php.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    PhpObject,
    ast::{Argument, NativeMethod},
    object::{Class, ObjectHandle},
};

pub(crate) struct DualIteratorData {
    current: Option<Value>,
    inner_current: Option<Value>,
    key: Option<Value>,
    inner: Rc<PhpObject>,
    iterator: Rc<PhpObject>,
    callback: Option<functions::CallTarget>,
    callback_value: Option<Value>,
    position: i64,
    offset: i64,
    limit: i64,
    _wrapper: ObjectHandle,
}
impl DualIteratorData {
    pub(super) fn values(&self) -> Vec<Value> {
        let mut values = vec![
            Value::Object(self.inner.clone()),
            Value::Object(self.iterator.clone()),
        ];
        values.extend(self.current.iter().cloned());
        values.extend(self.inner_current.iter().cloned());
        values.extend(self.key.iter().cloned());
        values.extend(self.callback_value.iter().cloned());
        if let Some(target) = &self.callback {
            if let Some(object) = &target.this {
                values.push(Value::Object(object.clone()));
            }
            if let Some(closure) = &target.closure {
                values.push(Value::Closure(closure.clone()));
            }
        }
        values
    }
}
pub(super) fn native_iteration(object: &PhpObject) -> bool {
    object.class.is_a(b"IteratorIterator")
        && [b"rewind".as_slice(), b"valid", b"current", b"key", b"next"]
            .iter()
            .all(|name| {
                objects::method(&object.class, name)
                    .is_some_and(|m| matches!(m.native, Some(NativeMethod::DualIterator)))
            })
}
fn missing(line: usize) -> Error {
    Error::new(
        ErrorKind::Error,
        "The object is in an invalid state as the parent constructor was not called",
        line,
    )
}
fn literal(value: Value, line: usize) -> Argument {
    Argument::Positional(Expr {
        kind: ExprKind::Literal(value),
        line,
    })
}
impl Runtime {
    pub(super) fn builtin_dual_iterator_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, parent, extra, interface) = match name {
            b"iteratoriterator" => (
                "IteratorIterator",
                None,
                "public function __construct(Traversable $iterator, ?string $class = null) {} public function getInnerIterator(): ?Iterator {} public function rewind(): void {} public function valid(): bool {} public function key(): mixed {} public function current(): mixed {} public function next(): void {}",
                Some("OuterIterator"),
            ),
            b"filteriterator" => (
                "FilterIterator",
                Some("IteratorIterator"),
                "abstract public function accept(): bool; public function __construct(Iterator $iterator) {} public function rewind(): void {} public function next(): void {}",
                None,
            ),
            b"recursivefilteriterator" => (
                "RecursiveFilterIterator",
                Some("FilterIterator"),
                "public function __construct(RecursiveIterator $iterator) {} public function hasChildren(): bool {} public function getChildren(): ?RecursiveFilterIterator {}",
                Some("RecursiveIterator"),
            ),
            b"callbackfilteriterator" => (
                "CallbackFilterIterator",
                Some("FilterIterator"),
                "public function __construct(Iterator $iterator, callable $callback) {} public function accept(): bool {}",
                None,
            ),
            b"recursivecallbackfilteriterator" => (
                "RecursiveCallbackFilterIterator",
                Some("CallbackFilterIterator"),
                "public function __construct(RecursiveIterator $iterator, callable $callback) {} public function hasChildren(): bool {} public function getChildren(): RecursiveCallbackFilterIterator {}",
                Some("RecursiveIterator"),
            ),
            b"parentiterator" => (
                "ParentIterator",
                Some("RecursiveFilterIterator"),
                "public function __construct(RecursiveIterator $iterator) {} public function accept(): bool {}",
                None,
            ),
            b"limititerator" => (
                "LimitIterator",
                Some("IteratorIterator"),
                "public function __construct(Iterator $iterator, int $offset = 0, int $limit = -1) {} public function rewind(): void {} public function valid(): bool {} public function next(): void {} public function seek(int $offset): int {} public function getPosition(): int {}",
                None,
            ),
            b"norewinditerator" => (
                "NoRewindIterator",
                Some("IteratorIterator"),
                "public function __construct(Iterator $iterator) {} public function rewind(): void {} public function valid(): bool {} public function key(): mixed {} public function current(): mixed {} public function next(): void {}",
                None,
            ),
            b"infiniteiterator" => (
                "InfiniteIterator",
                Some("IteratorIterator"),
                "public function __construct(Iterator $iterator) {} public function next(): void {}",
                None,
            ),
            b"emptyiterator" => (
                "EmptyIterator",
                None,
                "public function current(): never {} public function next(): void {} public function key(): never {} public function valid(): false {} public function rewind(): void {}",
                Some("Iterator"),
            ),
            _ => return None,
        };
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let code = format!(
            "<?php {}class {canonical}{}{} {{ {extra} }}",
            if matches!(name, b"filteriterator" | b"recursivefilteriterator") {
                "abstract "
            } else {
                ""
            },
            parent.map_or(String::new(), |p| format!(" extends {p}")),
            interface.map_or(String::new(), |i| format!(" implements {i}"))
        );
        let (mut statements, _, _) = parser::parse(lexer::lex(code.as_bytes()).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            method.native = Some(NativeMethod::DualIterator);
            method.tentative_return_type = method.return_type.take();
        }
        let parent = parent.map(|p| self.find_class(p.as_bytes()).unwrap());
        let mut methods = declaration.methods.clone();
        if let Some(parent) = &parent {
            let overridden: Vec<_> = methods
                .iter()
                .map(|m| {
                    m.name
                        .rsplit(|b| *b == b':')
                        .next()
                        .unwrap()
                        .to_ascii_lowercase()
                })
                .collect();
            methods.extend(
                parent
                    .methods
                    .iter()
                    .filter(|m| {
                        !overridden.iter().any(|name| {
                            m.name
                                .rsplit(|b| *b == b':')
                                .next()
                                .unwrap()
                                .eq_ignore_ascii_case(name)
                        })
                    })
                    .cloned(),
            );
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods,
            declaration,
            parent,
            interfaces: interface
                .map(|i| self.find_class(i.as_bytes()).unwrap())
                .into_iter()
                .collect(),
            properties: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
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
    pub(super) fn dual_iterator_inner(&self, object: &PhpObject) -> Option<Rc<PhpObject>> {
        object
            .dual_iterator
            .borrow()
            .as_ref()
            .map(|d| d.inner.clone())
    }
    fn dual_call(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        args: &[Value],
        line: usize,
    ) -> Result<Value, Error> {
        let method = objects::method(&object.class, name).unwrap();
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        let internal = self.trace_frames.last().is_some_and(|f| f.internal);
        self.call_callback_at(&target, args, line, internal)
    }
    fn dual_clear(&mut self, object: &PhpObject, line: usize) -> Result<(), Error> {
        let old = {
            let mut data = object.dual_iterator.borrow_mut();
            let data = data.as_mut().ok_or_else(|| missing(line))?;
            (
                data.current.take(),
                data.key.take(),
                data.inner_current.take(),
            )
        };
        drop(old);
        self.drain_destructors(Ok(()), line)
    }
    fn dual_inner_step(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let iterator = object
            .dual_iterator
            .borrow()
            .as_ref()
            .ok_or_else(|| missing(line))?
            .iterator
            .clone();
        let cache = !array_iterator::native_iteration(&iterator)
            && !recursive_iterator::native_iteration(&iterator)
            && !native_iteration(&iterator)
            && !native_iterator::hidden_iterator(&iterator);
        if cache
            && name == b"current"
            && let Some(value) = object
                .dual_iterator
                .borrow()
                .as_ref()
                .unwrap()
                .inner_current
                .clone()
        {
            return Ok(value);
        }
        if matches!(name, b"rewind" | b"next") {
            let old = object
                .dual_iterator
                .borrow_mut()
                .as_mut()
                .unwrap()
                .inner_current
                .take();
            drop(old);
            self.drain_destructors(Ok(()), line)?;
        }
        let result = self.iterator_method(&iterator, name, line)?;
        if cache && name == b"current" {
            object
                .dual_iterator
                .borrow_mut()
                .as_mut()
                .unwrap()
                .inner_current = Some(result.clone());
        }
        Ok(result)
    }
    fn dual_fetch(&mut self, object: &PhpObject, check: bool, line: usize) -> Result<bool, Error> {
        self.dual_clear(object, line)?;
        if check && !self.dual_inner_step(object, b"valid", line)?.is_truthy() {
            return Ok(false);
        }
        let value = self.dual_inner_step(object, b"current", line)?;
        object.dual_iterator.borrow_mut().as_mut().unwrap().current = Some(value);
        let key = self.dual_inner_step(object, b"key", line)?;
        object.dual_iterator.borrow_mut().as_mut().unwrap().key = Some(key);
        Ok(true)
    }
    fn dual_rewind(&mut self, object: &PhpObject, line: usize) -> Result<(), Error> {
        self.dual_clear(object, line)?;
        object.dual_iterator.borrow_mut().as_mut().unwrap().position = 0;
        self.dual_inner_step(object, b"rewind", line).map(drop)
    }
    fn dual_next(&mut self, object: &PhpObject, line: usize) -> Result<(), Error> {
        self.dual_clear(object, line)?;
        self.dual_inner_step(object, b"next", line)?;
        let mut data = object.dual_iterator.borrow_mut();
        let data = data.as_mut().unwrap();
        data.position = data.position.wrapping_add(1);
        Ok(())
    }
    fn dual_filter_fetch(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<(), Error> {
        while self.dual_fetch(object, true, line)? {
            let accepted = self.dual_call(object, b"accept", &[], line)?;
            if accepted.is_truthy() {
                return Ok(());
            }
            drop(accepted);
            self.dual_inner_step(object, b"next", line)?;
        }
        self.dual_clear(object, line)
    }
    fn dual_limit_seek(
        &mut self,
        object: &Rc<PhpObject>,
        position: i64,
        line: usize,
    ) -> Result<Value, Error> {
        self.dual_clear(object, line)?;
        let (inner, current, offset, limit) = {
            let data = object.dual_iterator.borrow();
            let data = data.as_ref().unwrap();
            (data.inner.clone(), data.position, data.offset, data.limit)
        };
        if position < offset {
            return Err(self.array_iterator_exception(
                b"OutOfBoundsException",
                format!("Cannot seek to {position} which is below the offset {offset}"),
                line,
            ));
        }
        if limit != -1 && position.wrapping_sub(offset) >= limit {
            return Err(self.array_iterator_exception(
                b"OutOfBoundsException",
                format!(
                    "Cannot seek to {position} which is behind offset {offset} plus count {limit}"
                ),
                line,
            ));
        }
        if position != current && inner.class.is_a(b"SeekableIterator") {
            self.dual_call(&inner, b"seek", &[Value::Int(position)], line)?;
            object.dual_iterator.borrow_mut().as_mut().unwrap().position = position;
            if self.dual_inner_step(object, b"valid", line)?.is_truthy() {
                self.dual_fetch(object, false, line)?;
            }
        } else {
            if position < current {
                self.dual_rewind(object, line)?;
            }
            loop {
                let current = object.dual_iterator.borrow().as_ref().unwrap().position;
                if position <= current || !self.dual_inner_step(object, b"valid", line)?.is_truthy()
                {
                    break;
                }
                self.dual_next(object, line)?;
            }
            if self.dual_inner_step(object, b"valid", line)?.is_truthy() {
                self.dual_fetch(object, true, line)?;
            }
        }
        Ok(Value::Int(
            object.dual_iterator.borrow().as_ref().unwrap().position,
        ))
    }
    pub(super) fn dual_iterator_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        raw: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let method = objects::method(&object.class, name).unwrap();
        let owner = method
            .name
            .split(|b| *b == b':')
            .next()
            .unwrap()
            .to_ascii_lowercase();
        self.dual_iterator_step_owned(object, name, &owner, raw, line)
    }
    fn dual_iterator_step_owned(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        owner: &[u8],
        raw: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if owner == b"norewinditerator" && name == b"rewind" {
            return Ok(Value::Null);
        }
        if object.dual_iterator.borrow().is_none() {
            return Err(missing(line));
        }
        if owner == b"norewinditerator" {
            let value = self.dual_inner_step(object, name, line)?;
            return Ok(if name == b"valid" {
                Value::Bool(value.is_truthy())
            } else if raw {
                value
            } else {
                value.dereferenced()
            });
        }
        match name {
            b"current" | b"key" => {
                let data = object.dual_iterator.borrow();
                let data = data.as_ref().unwrap();
                let value = if name == b"current" {
                    &data.current
                } else {
                    &data.key
                };
                Ok(value.as_ref().map_or(Value::Null, |v| {
                    if raw { v.clone() } else { v.dereferenced() }
                }))
            }
            b"valid" => {
                let data = object.dual_iterator.borrow();
                let data = data.as_ref().unwrap();
                Ok(Value::Bool(
                    data.current.is_some()
                        && (owner != b"limititerator"
                            || data.limit == -1
                            || data.position.wrapping_sub(data.offset) < data.limit),
                ))
            }
            b"rewind" => {
                self.dual_rewind(object, line)?;
                if owner == b"filteriterator" {
                    self.dual_filter_fetch(object, line)?;
                } else if owner == b"limititerator" {
                    let offset = object.dual_iterator.borrow().as_ref().unwrap().offset;
                    self.dual_limit_seek(object, offset, line)?;
                } else {
                    self.dual_fetch(object, true, line)?;
                }
                Ok(Value::Null)
            }
            b"next" => {
                self.dual_next(object, line)?;
                if owner == b"filteriterator" {
                    self.dual_filter_fetch(object, line)?;
                } else if owner == b"infiniteiterator" {
                    if self.dual_inner_step(object, b"valid", line)?.is_truthy() {
                        self.dual_fetch(object, false, line)?;
                    } else {
                        self.dual_rewind(object, line)?;
                        if self.dual_inner_step(object, b"valid", line)?.is_truthy() {
                            self.dual_fetch(object, false, line)?;
                        }
                    }
                } else if owner == b"limititerator" {
                    let more = {
                        let data = object.dual_iterator.borrow();
                        let data = data.as_ref().unwrap();
                        data.limit == -1 || data.position.wrapping_sub(data.offset) < data.limit
                    };
                    if more {
                        self.dual_fetch(object, true, line)?;
                    }
                } else {
                    self.dual_fetch(object, true, line)?;
                }
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }
    pub(super) fn native_dual_iterator_method(
        &mut self,
        line: usize,
        prepared_callback: Option<functions::CallTarget>,
    ) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let full_name = self.current_function.as_ref().unwrap().clone();
        let name = full_name.rsplit(|b| *b == b':').next().unwrap();
        let owner = full_name.split(|b| *b == b':').next().unwrap();
        if owner == b"emptyiterator" {
            return match name {
                b"valid" => Ok(Value::Bool(false)),
                b"key" | b"current" => Err(self.array_iterator_exception(
                    b"BadMethodCallException",
                    format!(
                        "Accessing the {} of an EmptyIterator",
                        if name == b"key" { "key" } else { "value" }
                    ),
                    line,
                )),
                _ => Ok(Value::Null),
            };
        }
        if name == b"__construct" {
            if object.dual_iterator.borrow().is_some() {
                let canonical = self.find_class(owner).unwrap().name.clone();
                return Err(self.array_iterator_exception(
                    b"BadMethodCallException",
                    format!(
                        "{}::getIterator() must be called exactly once per instance",
                        String::from_utf8_lossy(&canonical)
                    ),
                    line,
                ));
            }
            let Value::Object(mut inner) = self.lookup(b"iterator").unwrap() else {
                unreachable!()
            };
            let offset = self
                .lookup(b"offset")
                .unwrap_or(Value::Int(0))
                .integer(line, self)?;
            let limit = self
                .lookup(b"limit")
                .unwrap_or(Value::Int(-1))
                .integer(line, self)?;
            if offset < 0 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "LimitIterator::__construct(): Argument #2 ($offset) must be greater than or equal to 0",
                    line,
                ));
            }
            if limit < -1 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "LimitIterator::__construct(): Argument #3 ($limit) must be greater than or equal to -1",
                    line,
                ));
            }
            if owner == b"iteratoriterator" && !inner.class.is_a(b"Iterator") {
                let cast = self.lookup(b"class").unwrap();
                let mut class = inner.class.clone();
                if let Value::String(name) = cast {
                    let resolved = self.load_class(&name.bytes(), line)?;
                    let Some(base) =
                        resolved.filter(|c| inner.class.is_a(&c.name) && c.is_a(b"Traversable"))
                    else {
                        return Err(self.array_iterator_exception(b"LogicException", "Class to downcast to not found or not base class or does not implement Traversable".into(), line));
                    };
                    class = base;
                }
                if class.is_a(b"IteratorAggregate") {
                    let method = objects::method(&class, b"getIterator").unwrap();
                    let target = self.unchecked_method_target(
                        method,
                        class.clone(),
                        Some(inner.clone()),
                        line,
                    )?;
                    let value = self.call_callback_at(&target, &[], line, true)?;
                    match value {
                        Value::Object(iterator) if iterator.class.is_a(b"Traversable") => inner = iterator,
                        _ => return Err(self.array_iterator_exception(b"LogicException", format!("{}::getIterator() must return an object that implements Traversable", class.display_name()), line)),
                    }
                }
            }
            let callback_value = self.lookup(b"callback");
            let callback = prepared_callback;
            let iterator = self.resolve_iterator(&inner, false, line)?;
            let wrapper = self.allocate_object_handle();
            *object.dual_iterator.borrow_mut() = Some(DualIteratorData {
                current: None,
                inner_current: None,
                key: None,
                inner,
                iterator,
                callback,
                callback_value,
                position: 0,
                offset,
                limit,
                _wrapper: wrapper,
            });
            return Ok(Value::Null);
        }
        if matches!(name, b"rewind" | b"next" | b"valid" | b"current" | b"key") {
            return self.dual_iterator_step_owned(&object, name, owner, false, line);
        }
        let inner = self
            .dual_iterator_inner(&object)
            .ok_or_else(|| missing(line))?;
        match name {
            b"getinneriterator" => Ok(Value::Object(inner)),
            b"haschildren" => self.dual_call(&inner, b"hasChildren", &[], line),
            b"getchildren" => {
                let child = self.dual_call(&inner, b"getChildren", &[], line)?;
                let mut args = vec![literal(child, line)];
                if owner == b"recursivecallbackfilteriterator" {
                    let callback = object
                        .dual_iterator
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .callback_value
                        .clone()
                        .unwrap();
                    args.push(literal(callback, line));
                }
                self.instantiate_class(object.class.clone(), &args, true, line)
            }
            b"accept" if owner == b"parentiterator" => {
                self.dual_call(&inner, b"hasChildren", &[], line)
            }
            b"accept" => {
                let (target, values) = {
                    let data = object.dual_iterator.borrow();
                    let data = data.as_ref().unwrap();
                    let (Some(value), Some(key)) = (&data.current, &data.key) else {
                        return Ok(Value::Bool(false));
                    };
                    (
                        data.callback.clone().unwrap(),
                        [value.clone(), key.clone(), Value::Object(inner)],
                    )
                };
                self.call_callback_at(&target, &values, line, true)
                    .map(|v| v.dereferenced())
            }
            b"seek" => {
                let Value::Int(offset) = self.lookup(b"offset").unwrap() else {
                    unreachable!()
                };
                self.dual_limit_seek(&object, offset, line)
            }
            b"getposition" => Ok(Value::Int(
                object.dual_iterator.borrow().as_ref().unwrap().position,
            )),
            _ => unreachable!(),
        }
    }
}
