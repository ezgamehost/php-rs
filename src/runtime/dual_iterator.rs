// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_iterators.c and .stub.php.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    PhpObject,
    ast::{Argument, NativeMethod},
    object::{Class, ObjectHandle},
};

#[derive(Default)]
pub(crate) struct DualIteratorData {
    current: Option<Value>,
    inner_current: Option<Value>,
    key: Option<Value>,
    // The native iterator handle is released before its retained inner object.
    _wrapper: Option<ObjectHandle>,
    inner: Option<Rc<PhpObject>>,
    iterator: Option<Rc<PhpObject>>,
    callback: Option<functions::CallTarget>,
    callback_value: Option<Value>,
    position: i64,
    offset: i64,
    limit: i64,
    caching: Option<CachingData>,
    append: Option<AppendData>,
}
struct AppendData {
    _wrapper: ObjectHandle,
    array: Rc<PhpObject>,
}
struct CachingData {
    flags: i64,
    cache: Rc<PhpArray>,
    children: Option<Value>,
    string: Option<Value>,
    recursive: bool,
}
const CACHE_PUBLIC: i64 = 0xffff;
const CACHE_VALID: i64 = 0x10000;
const FULL_CACHE: i64 = 0x100;

fn caching_flags(flags: i64, function: &str, position: usize, line: usize) -> Result<(), Error> {
    if (flags & 15).count_ones() > 1 {
        return Err(Error::new(
            ErrorKind::ValueError,
            format!(
                "{function}(): Argument #{position} ($flags) must contain only one of CachingIterator::CALL_TOSTRING, CachingIterator::TOSTRING_USE_KEY, CachingIterator::TOSTRING_USE_CURRENT, or CachingIterator::TOSTRING_USE_INNER"
            ),
            line,
        ));
    }
    Ok(())
}
impl DualIteratorData {
    pub(super) fn values(&self) -> Vec<Value> {
        let mut values: Vec<_> = self
            .inner
            .iter()
            .chain(&self.iterator)
            .cloned()
            .map(Value::Object)
            .collect();
        if let Some(append) = &self.append {
            values.push(Value::Object(append.array.clone()));
        }
        if let Some(cache) = &self.caching {
            values.push(Value::Array(cache.cache.clone()));
            values.extend(cache.children.iter().cloned());
            values.extend(cache.string.iter().cloned());
        }
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
            b"cachingiterator" => (
                "CachingIterator",
                Some("IteratorIterator"),
                "public const int CALL_TOSTRING = 1, TOSTRING_USE_KEY = 2, TOSTRING_USE_CURRENT = 4, TOSTRING_USE_INNER = 8, CATCH_GET_CHILD = 16, FULL_CACHE = 256; public function __construct(Iterator $iterator, int $flags = 1) {} public function rewind(): void {} public function valid(): bool {} public function next(): void {} public function hasNext(): bool {} public function __toString(): string {} public function getFlags(): int {} public function setFlags(int $flags): void {} public function offsetGet($key): mixed {} public function offsetSet($key, mixed $value): void {} public function offsetUnset($key): void {} public function offsetExists($key): bool {} public function getCache(): array {} public function count(): int {}",
                Some("ArrayAccess, Countable, Stringable"),
            ),
            b"recursivecachingiterator" => (
                "RecursiveCachingIterator",
                Some("CachingIterator"),
                "public function __construct(Iterator $iterator, int $flags = 1) {} public function hasChildren(): bool {} public function getChildren(): ?RecursiveCachingIterator {}",
                Some("RecursiveIterator"),
            ),
            b"norewinditerator" => (
                "NoRewindIterator",
                Some("IteratorIterator"),
                "public function __construct(Iterator $iterator) {} public function rewind(): void {} public function valid(): bool {} public function key(): mixed {} public function current(): mixed {} public function next(): void {}",
                None,
            ),
            b"appenditerator" => (
                "AppendIterator",
                Some("IteratorIterator"),
                "public function __construct() {} public function append(Iterator $iterator): void {} public function rewind(): void {} public function valid(): bool {} public function current(): mixed {} public function next(): void {} public function getIteratorIndex(): ?int {} public function getArrayIterator(): ArrayIterator {}",
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
            // Install native constant defaults after parsing, so constant folding
            // does not recursively construct the same builtin class.
            if matches!(name, b"cachingiterator" | b"recursivecachingiterator") {
                for parameter in &mut method.parameters {
                    if parameter.name == b"flags" && parameter.default.is_some() {
                        parameter.default = Some(Expr {
                            line: 0,
                            kind: ExprKind::ScopedMember(
                                Box::new(Expr {
                                    line: 0,
                                    kind: ExprKind::Constant(canonical.as_bytes().to_vec()),
                                }),
                                Box::new(Expr {
                                    line: 0,
                                    kind: ExprKind::Literal(Value::String(
                                        b"CALL_TOSTRING".to_vec().into(),
                                    )),
                                }),
                            ),
                        });
                    }
                }
            }
            method.native = Some(NativeMethod::DualIterator);
            if !method.name.ends_with(b"::__toString") {
                method.tentative_return_type = method.return_type.take();
            }
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
                .into_iter()
                .flat_map(|i| i.split(", "))
                .map(|i| self.find_class(i.as_bytes()).unwrap())
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
            .and_then(|d| d.inner.clone())
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
                data.caching.as_mut().and_then(|c| c.children.take()),
                data.caching.as_mut().and_then(|c| c.string.take()),
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
        let Some(iterator) = iterator else {
            return Ok(if name == b"valid" {
                Value::Bool(false)
            } else {
                Value::Null
            });
        };
        let cache = !array_iterator::native_iteration(&iterator)
            && !recursive_iterator::native_iteration(&iterator)
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
            (
                data.inner.clone().ok_or_else(|| missing(line))?,
                data.position,
                data.offset,
                data.limit,
            )
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
        if owner == b"appenditerator" {
            return self.append_iterator_step(object, name, raw, line);
        }
        if owner == b"cachingiterator" {
            return self.caching_iterator_step(object, name, line);
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
            if owner == b"appenditerator" {
                let class = self.find_class(b"ArrayIterator").unwrap();
                let Value::Object(array) = self.instantiate_class(class, &[], true, line)? else {
                    unreachable!()
                };
                let wrapper = self.allocate_object_handle();
                *object.dual_iterator.borrow_mut() = Some(DualIteratorData {
                    append: Some(AppendData {
                        _wrapper: wrapper,
                        array,
                    }),
                    ..DualIteratorData::default()
                });
                return Ok(Value::Null);
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
            let caching = if matches!(owner, b"cachingiterator" | b"recursivecachingiterator") {
                let Value::Int(flags) = self.lookup(b"flags").unwrap() else {
                    unreachable!()
                };
                let constructor = if owner == b"cachingiterator" {
                    "CachingIterator::__construct"
                } else {
                    "RecursiveCachingIterator::__construct"
                };
                caching_flags(flags, constructor, 2, line)?;
                Some(CachingData {
                    flags: flags & CACHE_PUBLIC,
                    cache: Rc::new(PhpArray::default()),
                    children: None,
                    string: None,
                    recursive: owner == b"recursivecachingiterator",
                })
            } else {
                None
            };
            let callback_value = self.lookup(b"callback");
            let callback = prepared_callback;
            let iterator = self.resolve_iterator(&inner, false, line)?;
            let wrapper = self.allocate_object_handle();
            *object.dual_iterator.borrow_mut() = Some(DualIteratorData {
                current: None,
                inner_current: None,
                key: None,
                inner: Some(inner),
                iterator: Some(iterator),
                callback,
                callback_value,
                position: 0,
                offset,
                limit,
                caching,
                _wrapper: Some(wrapper),
                append: None,
            });
            return Ok(Value::Null);
        }
        if matches!(name, b"rewind" | b"next" | b"valid" | b"current" | b"key") {
            return self.dual_iterator_step_owned(&object, name, owner, false, line);
        }
        if matches!(owner, b"cachingiterator" | b"recursivecachingiterator") {
            return self.caching_iterator_method(&object, name, line);
        }
        if owner == b"appenditerator" {
            return self.append_iterator_method(&object, name, line);
        }
        if name == b"getinneriterator" {
            if object.dual_iterator.borrow().is_none() {
                return Err(missing(line));
            }
            return Ok(self
                .dual_iterator_inner(&object)
                .map_or(Value::Null, Value::Object));
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

impl Runtime {
    fn caching_iterator_next(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<(), Error> {
        let fetched = self.dual_fetch(object, true, line);
        if !matches!(fetched, Ok(true)) {
            object
                .dual_iterator
                .borrow_mut()
                .as_mut()
                .unwrap()
                .caching
                .as_mut()
                .unwrap()
                .flags &= !CACHE_VALID;
            return fetched.map(drop);
        }
        let (flags, recursive, inner, key, value) = {
            let mut data = object.dual_iterator.borrow_mut();
            let data = data.as_mut().unwrap();
            let cache = data.caching.as_mut().unwrap();
            cache.flags |= CACHE_VALID;
            (
                cache.flags,
                cache.recursive,
                data.inner.clone().ok_or_else(|| missing(line))?,
                data.key.clone().unwrap(),
                data.current.clone().unwrap(),
            )
        };
        if flags & FULL_CACHE != 0 {
            let key = ArrayKey::from_value(&key, line, self)?;
            {
                let mut data = object.dual_iterator.borrow_mut();
                let cache = &mut data.as_mut().unwrap().caching.as_mut().unwrap().cache;
                cache.mutate_shared(|cache| cache.insert(key, value.dereferenced()));
            }
            self.drain_destructors(Ok(()), line)?;
        }
        if recursive {
            let result = (|| {
                if self
                    .dual_call(&inner, b"hasChildren", &[], line)?
                    .is_truthy()
                {
                    let child = self.dual_call(&inner, b"getChildren", &[], line)?;
                    let class = self.find_class(b"RecursiveCachingIterator").unwrap();
                    let flags = object
                        .dual_iterator
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .caching
                        .as_ref()
                        .unwrap()
                        .flags;
                    let args = [
                        literal(child, line),
                        literal(Value::Int(flags & CACHE_PUBLIC), line),
                    ];
                    let child = self.instantiate_class(class, &args, true, line)?;
                    object
                        .dual_iterator
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .caching
                        .as_mut()
                        .unwrap()
                        .children = Some(child);
                }
                Ok(())
            })();
            let catch = object
                .dual_iterator
                .borrow()
                .as_ref()
                .unwrap()
                .caching
                .as_ref()
                .unwrap()
                .flags
                & 16
                != 0;
            if !catch {
                result?;
            }
        }
        // Read flags again: a child callback may have changed them.
        let flags = object
            .dual_iterator
            .borrow()
            .as_ref()
            .unwrap()
            .caching
            .as_ref()
            .unwrap()
            .flags;
        if flags & 9 != 0 {
            let source = if flags & 8 != 0 {
                Value::Object(inner)
            } else {
                value
            };
            let string = Value::String(self.string_bytes(&source, line)?.into());
            object
                .dual_iterator
                .borrow_mut()
                .as_mut()
                .unwrap()
                .caching
                .as_mut()
                .unwrap()
                .string = Some(string);
        }
        self.dual_inner_step(object, b"next", line)?;
        let mut data = object.dual_iterator.borrow_mut();
        let data = data.as_mut().unwrap();
        data.position = data.position.wrapping_add(1);
        Ok(())
    }
    fn caching_iterator_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        match name {
            b"valid" => Ok(Value::Bool(
                object
                    .dual_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .caching
                    .as_ref()
                    .is_some_and(|c| c.flags & CACHE_VALID != 0),
            )),
            b"rewind" => {
                self.dual_rewind(object, line)?;
                let old = {
                    let mut data = object.dual_iterator.borrow_mut();
                    data.as_mut()
                        .unwrap()
                        .caching
                        .as_mut()
                        .unwrap()
                        .cache
                        .mutate_shared(std::mem::take)
                };
                drop(old);
                self.drain_destructors(Ok(()), line)?;
                self.caching_iterator_next(object, line)?;
                Ok(Value::Null)
            }
            b"next" => {
                self.caching_iterator_next(object, line)?;
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }
    fn caching_iterator_method(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let flags = object
            .dual_iterator
            .borrow()
            .as_ref()
            .ok_or_else(|| missing(line))?
            .caching
            .as_ref()
            .ok_or_else(|| missing(line))?
            .flags;
        match name {
            b"hasnext" => Ok(Value::Bool(
                self.dual_inner_step(object, b"valid", line)?.is_truthy(),
            )),
            b"haschildren" => Ok(Value::Bool(
                object
                    .dual_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .caching
                    .as_ref()
                    .unwrap()
                    .children
                    .is_some(),
            )),
            b"getchildren" => Ok(object
                .dual_iterator
                .borrow()
                .as_ref()
                .unwrap()
                .caching
                .as_ref()
                .unwrap()
                .children
                .clone()
                .unwrap_or(Value::Null)),
            b"getflags" => Ok(Value::Int(flags)),
            b"setflags" => {
                let Value::Int(new_flags) = self.lookup(b"flags").unwrap() else {
                    unreachable!()
                };
                caching_flags(new_flags, "CachingIterator::setFlags", 1, line)?;
                for (flag, label) in [(1, "CALL_TO_STRING"), (8, "TOSTRING_USE_INNER")] {
                    if flags & flag != 0 && new_flags & flag == 0 {
                        return Err(self.array_iterator_exception(
                            b"InvalidArgumentException",
                            format!("Unsetting flag {label} is not possible"),
                            line,
                        ));
                    }
                }
                let old = {
                    let mut data = object.dual_iterator.borrow_mut();
                    let cache = data.as_mut().unwrap().caching.as_mut().unwrap();
                    let old = (new_flags & FULL_CACHE != 0 && flags & FULL_CACHE == 0)
                        .then(|| cache.cache.mutate_shared(std::mem::take));
                    cache.flags = (cache.flags & !CACHE_PUBLIC) | (new_flags & CACHE_PUBLIC);
                    old
                };
                drop(old);
                self.drain_destructors(Ok(Value::Null), line)
            }
            b"__tostring" => {
                if flags & 15 == 0 {
                    return Err(self.array_iterator_exception(
                        b"BadMethodCallException",
                        format!(
                            "{} does not fetch string value (see CachingIterator::__construct)",
                            object.class.display_name()
                        ),
                        line,
                    ));
                }
                let value = {
                    let data = object.dual_iterator.borrow();
                    let data = data.as_ref().unwrap();
                    if flags & 2 != 0 {
                        data.key.clone()
                    } else if flags & 4 != 0 {
                        data.current.clone()
                    } else {
                        data.caching.as_ref().unwrap().string.clone()
                    }
                }
                .unwrap_or(Value::Null);
                Ok(Value::String(self.string_bytes(&value, line)?.into()))
            }
            b"getcache" | b"count" | b"offsetget" | b"offsetset" | b"offsetexists"
            | b"offsetunset" => {
                if flags & FULL_CACHE == 0 {
                    return Err(self.array_iterator_exception(
                        b"BadMethodCallException",
                        format!(
                            "{} does not use a full cache (see CachingIterator::__construct)",
                            object.class.display_name()
                        ),
                        line,
                    ));
                }
                if name == b"getcache" {
                    return Ok(Value::Array(
                        object
                            .dual_iterator
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .caching
                            .as_ref()
                            .unwrap()
                            .cache
                            .clone(),
                    ));
                }
                if name == b"count" {
                    return Ok(Value::Int(
                        object
                            .dual_iterator
                            .borrow()
                            .as_ref()
                            .unwrap()
                            .caching
                            .as_ref()
                            .unwrap()
                            .cache
                            .len() as i64,
                    ));
                }
                let key_value = self.lookup(b"key").unwrap();
                let key = ArrayKey::from_value(&key_value, line, self)?;
                let value = {
                    let mut data = object.dual_iterator.borrow_mut();
                    let cache = &mut data.as_mut().unwrap().caching.as_mut().unwrap().cache;
                    match name {
                        b"offsetget" => cache.get(&key),
                        b"offsetexists" => Some(Value::Bool(cache.get(&key).is_some())),
                        b"offsetset" => {
                            cache.mutate_shared(|cache| {
                                cache.insert(key, self.lookup(b"value").unwrap())
                            });
                            Some(Value::Null)
                        }
                        b"offsetunset" => {
                            cache.mutate_shared(|cache| cache.remove(&key));
                            Some(Value::Null)
                        }
                        _ => unreachable!(),
                    }
                };
                if value.is_none() {
                    self.warn(format!(
                        "Undefined array key \"{}\" on line {line}",
                        String::from_utf8_lossy(&key_value.to_bytes())
                    ))?;
                }
                self.drain_destructors(Ok(value.unwrap_or(Value::Null).dereferenced()), line)
            }
            _ => unreachable!(),
        }
    }
}

impl Runtime {
    fn append_iterator_array(
        &self,
        object: &PhpObject,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        object
            .dual_iterator
            .borrow()
            .as_ref()
            .and_then(|data| data.append.as_ref())
            .map(|append| append.array.clone())
            .ok_or_else(|| missing(line))
    }
    fn append_iterator_select(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<bool, Error> {
        self.dual_clear(object, line)?;
        let (array, old) = {
            let mut data = object.dual_iterator.borrow_mut();
            let data = data.as_mut().unwrap();
            let array = data
                .append
                .as_ref()
                .ok_or_else(|| missing(line))?
                .array
                .clone();
            let old = (
                data.inner.take(),
                data.iterator.take(),
                data._wrapper.take(),
            );
            (array, old)
        };
        drop(old);
        self.drain_destructors(Ok(()), line)?;
        if !self.iterator_method(&array, b"valid", line)?.is_truthy() {
            return Ok(false);
        }
        let value = self
            .iterator_method(&array, b"current", line)?
            .dereferenced();
        let Value::Object(inner) = value else {
            return Err(Error::new(
                ErrorKind::TypeError,
                "AppendIterator's iterator array must contain objects implementing Iterator",
                line,
            ));
        };
        if !inner.class.is_a(b"Iterator") {
            return Err(Error::new(
                ErrorKind::TypeError,
                "AppendIterator's iterator array must contain objects implementing Iterator",
                line,
            ));
        }
        object.dual_iterator.borrow_mut().as_mut().unwrap().inner = Some(inner.clone());
        let iterator = self.resolve_iterator(&inner, false, line)?;
        let wrapper = self.allocate_object_handle();
        {
            let mut data = object.dual_iterator.borrow_mut();
            let data = data.as_mut().unwrap();
            data.iterator = Some(iterator);
            data._wrapper = Some(wrapper);
        }
        self.dual_rewind(object, line)?;
        Ok(true)
    }
    fn append_iterator_fetch(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<(), Error> {
        let array = self.append_iterator_array(object, line)?;
        while !self.dual_inner_step(object, b"valid", line)?.is_truthy() {
            self.iterator_method(&array, b"next", line)?;
            if !self.append_iterator_select(object, line)? {
                return Ok(());
            }
        }
        self.dual_fetch(object, false, line).map(drop)
    }
    fn append_iterator_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        raw: bool,
        line: usize,
    ) -> Result<Value, Error> {
        match name {
            b"rewind" => {
                let array = self.append_iterator_array(object, line)?;
                self.iterator_method(&array, b"rewind", line)?;
                if self.append_iterator_select(object, line)? {
                    self.append_iterator_fetch(object, line)?;
                }
                Ok(Value::Null)
            }
            b"valid" => Ok(Value::Bool(
                object
                    .dual_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .current
                    .is_some(),
            )),
            b"current" => {
                self.dual_fetch(object, true, line)?;
                let value = object
                    .dual_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .current
                    .clone()
                    .unwrap_or(Value::Null);
                Ok(if raw { value } else { value.dereferenced() })
            }
            b"next" => {
                if self.dual_inner_step(object, b"valid", line)?.is_truthy() {
                    self.dual_next(object, line)?;
                }
                self.append_iterator_fetch(object, line)?;
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }
    fn append_iterator_method(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let array = self.append_iterator_array(object, line)?;
        match name {
            b"getarrayiterator" => Ok(Value::Object(array)),
            b"getiteratorindex" => self.iterator_method(&array, b"key", line),
            b"append" => {
                let Value::Object(inner) = self.lookup(b"iterator").unwrap() else {
                    unreachable!()
                };
                let advance = self.iterator_method(&array, b"valid", line)?.is_truthy()
                    && !self.dual_inner_step(object, b"valid", line)?.is_truthy();
                self.dual_call(&array, b"append", &[Value::Object(inner.clone())], line)?;
                if advance {
                    self.iterator_method(&array, b"next", line)?;
                }
                let initialized = object
                    .dual_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .iterator
                    .is_some();
                if !initialized || !self.dual_inner_step(object, b"valid", line)?.is_truthy() {
                    if !self.iterator_method(&array, b"valid", line)?.is_truthy() {
                        self.iterator_method(&array, b"rewind", line)?;
                    }
                    loop {
                        if !self.append_iterator_select(object, line)? {
                            break;
                        }
                        let selected = self.dual_iterator_inner(object);
                        if selected.is_some_and(|selected| Rc::ptr_eq(&selected, &inner)) {
                            break;
                        }
                    }
                    self.append_iterator_fetch(object, line)?;
                }
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }
}
