// Altered Rust adaptation of PHP 8.5.0 ext/spl/spl_iterators.c.
// Copyright (c) The PHP Group; see THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{
    PhpObject,
    ast::{Argument, NativeMethod},
    object::{Class, ObjectHandle},
};

#[derive(Clone, Copy)]
enum State {
    Next,
    Test,
    SelfEntry,
    Child,
    Start,
}

pub(crate) struct Frame {
    // Release the cached value and inner object before the iterator handle.
    current: Option<Value>,
    object: Rc<PhpObject>,
    _wrapper: Option<ObjectHandle>,
    state: State,
}

#[derive(Default)]
pub(crate) struct RecursiveIteratorData {
    frames: Vec<Rc<RefCell<Frame>>>,
    mode: i64,
    flags: i64,
    max_depth: i64,
    in_iteration: bool,
    tree: Option<TreeData>,
}
struct TreeData {
    prefix: [Vec<u8>; 6],
    postfix: Vec<u8>,
}
impl Default for TreeData {
    fn default() -> Self {
        Self {
            prefix: [
                b"".to_vec(),
                b"| ".to_vec(),
                b"  ".to_vec(),
                b"|-".to_vec(),
                b"\\-".to_vec(),
                b"".to_vec(),
            ],
            postfix: Vec::new(),
        }
    }
}

impl RecursiveIteratorData {
    pub(super) fn new(class: &Class) -> Self {
        Self {
            tree: class.is_a(b"RecursiveTreeIterator").then(TreeData::default),
            frames: Vec::new(),
            mode: 0,
            flags: 0,
            max_depth: 0,
            in_iteration: false,
        }
    }
    pub(super) fn initialized(&self) -> bool {
        !self.frames.is_empty()
    }
    pub(super) fn values(&self) -> Vec<Value> {
        let mut values = Vec::new();
        for frame in &self.frames {
            let frame = frame.borrow();
            values.push(Value::Object(frame.object.clone()));
            values.extend(frame.current.iter().cloned());
        }
        values
    }
}

impl Drop for RecursiveIteratorData {
    fn drop(&mut self) {
        while self.frames.pop().is_some() {}
    }
}

fn frame(object: &PhpObject, line: usize) -> Result<Rc<RefCell<Frame>>, Error> {
    object
        .recursive_iterator
        .borrow()
        .as_ref()
        .and_then(|data| data.frames.last().cloned())
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                "The object is in an invalid state as the parent constructor was not called",
                line,
            )
        })
}

fn depth(object: &PhpObject) -> usize {
    object
        .recursive_iterator
        .borrow()
        .as_ref()
        .unwrap()
        .frames
        .len()
        .saturating_sub(1)
}

pub(super) fn native_iteration(object: &PhpObject) -> bool {
    object.recursive_iterator.borrow().is_some()
        && !object.class.is_a(b"RecursiveTreeIterator")
        && [b"rewind".as_slice(), b"valid", b"current", b"key", b"next"]
            .iter()
            .all(|name| {
                objects::method(&object.class, name).is_some_and(|method| {
                    matches!(method.native, Some(NativeMethod::RecursiveIterator))
                })
            })
}

impl Runtime {
    pub(super) fn builtin_recursive_iterator_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        if name == b"recursivetreeiterator" {
            return Some(self.builtin_recursive_tree_iterator_class());
        }
        if name != b"recursiveiteratoriterator" {
            return None;
        }
        if let Some(class) = self.builtin_classes.borrow().get(name) {
            return Some(class.clone());
        }
        let code = b"<?php class RecursiveIteratorIterator implements OuterIterator {
            public const int LEAVES_ONLY = 0, SELF_FIRST = 1, CHILD_FIRST = 2, CATCH_GET_CHILD = 16;
            public function __construct(Traversable $iterator, int $mode = 0, int $flags = 0) {}
            public function rewind(): void {}
            public function valid(): bool {}
            public function key(): mixed {}
            public function current(): mixed {}
            public function next(): void {}
            public function getDepth(): int {}
            public function getSubIterator(?int $level = null): ?RecursiveIterator {}
            public function getInnerIterator(): RecursiveIterator {}
            public function beginIteration(): void {}
            public function endIteration(): void {}
            public function callHasChildren(): bool {}
            public function callGetChildren(): ?RecursiveIterator {}
            public function beginChildren(): void {}
            public function endChildren(): void {}
            public function nextElement(): void {}
            public function setMaxDepth(int $maxDepth = -1): void {}
            public function getMaxDepth(): int|false {}
        }";
        let (mut statements, _, _) = parser::parse(lexer::lex(code).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            // Install this after parsing to avoid recursively bootstrapping the
            // class while the parser folds its own native default constant.
            for parameter in &mut method.parameters {
                if parameter.name == b"mode" {
                    parameter.default = Some(Expr {
                        line: 0,
                        kind: ExprKind::ScopedMember(
                            Box::new(Expr {
                                line: 0,
                                kind: ExprKind::Constant(b"RecursiveIteratorIterator".to_vec()),
                            }),
                            Box::new(Expr {
                                line: 0,
                                kind: ExprKind::Literal(Value::String(
                                    b"LEAVES_ONLY".to_vec().into(),
                                )),
                            }),
                        ),
                    });
                }
            }
            method.native = Some(NativeMethod::RecursiveIterator);
            method.tentative_return_type = method.return_type.take();
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods: declaration.methods.clone(),
            declaration,
            parent: None,
            interfaces: vec![self.find_class(b"OuterIterator").unwrap()],
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

    pub(super) fn recursive_iterator_step_inner(
        &self,
        object: &PhpObject,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        Ok(native_iterator::source_object(
            &frame(object, line)?.borrow().object,
        ))
    }

    fn recursive_call(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let object = native_iterator::source_object(object);
        let Some(method) = objects::method(&object.class, name) else {
            let message = format!(
                "Couldn't find implementation for method {}::{}",
                object.class_name,
                String::from_utf8_lossy(&name.to_ascii_lowercase())
            );
            return Err(self.native_fatal_at(message.as_bytes(), b"Unknown", 0, 16, true));
        };
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        let internal = self.trace_frames.last().is_some_and(|frame| frame.internal);
        self.call_callback_at(&target, &[], line, internal)
    }

    fn recursive_hook(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<(), Error> {
        let method = objects::method(&object.class, name).unwrap();
        if matches!(method.native, Some(NativeMethod::RecursiveIterator)) {
            return Ok(());
        }
        drop(self.recursive_call(object, name, line)?);
        self.drain_destructors(Ok(()), line)
    }

    fn recursive_child_call(
        &mut self,
        object: &Rc<PhpObject>,
        children: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let name = if children {
            b"callGetChildren".as_slice()
        } else {
            b"callHasChildren".as_slice()
        };
        let method = objects::method(&object.class, name).unwrap();
        if !matches!(method.native, Some(NativeMethod::RecursiveIterator)) {
            return self.recursive_call(object, name, line);
        }
        let inner = frame(object, line)?.borrow().object.clone();
        self.recursive_call(
            &inner,
            if children {
                b"getChildren"
            } else {
                b"hasChildren"
            },
            line,
        )
    }

    fn recursive_catch(
        &mut self,
        object: &PhpObject,
        result: Result<(), Error>,
        line: usize,
    ) -> Result<(), Error> {
        if object.recursive_iterator.borrow().as_ref().unwrap().flags & 16 != 0
            && result
                .as_ref()
                .is_err_and(|error| error.kind != ErrorKind::UnwindExit)
        {
            drop(result);
            return self.drain_destructors(Ok(()), line);
        }
        result
    }

    fn recursive_frame(
        &mut self,
        object: Rc<PhpObject>,
        line: usize,
    ) -> Result<Rc<RefCell<Frame>>, Error> {
        let object = self.resolve_iterator(&object, false, line)?;
        let wrapper =
            (!native_iterator::hidden_iterator(&object)).then(|| self.allocate_object_handle());
        Ok(Rc::new(RefCell::new(Frame {
            current: None,
            object,
            _wrapper: wrapper,
            state: State::Start,
        })))
    }

    pub(super) fn recursive_iterator_release(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<(), Error> {
        if !native_iteration(object) {
            return Ok(());
        }
        while depth(object) > 0 {
            let removed = object
                .recursive_iterator
                .borrow_mut()
                .as_mut()
                .unwrap()
                .frames
                .pop();
            drop(removed);
            self.drain_destructors(Ok(()), line)?;
        }
        Ok(())
    }

    fn recursive_advance(&mut self, object: &Rc<PhpObject>, line: usize) -> Result<(), Error> {
        frame(object, line)?;
        loop {
            let active = frame(object, line)?;
            let (state, inner) = {
                let frame = active.borrow();
                (frame.state, frame.object.clone())
            };
            let (mode, maximum) = {
                let data = object.recursive_iterator.borrow();
                let data = data.as_ref().unwrap();
                (data.mode, data.max_depth)
            };
            match state {
                State::Next | State::Start => {
                    if matches!(state, State::Next) {
                        let old = active.borrow_mut().current.take();
                        drop(old);
                        self.drain_destructors(Ok(()), line)?;
                        let result = self.iterator_method(&inner, b"next", line).map(drop);
                        self.recursive_catch(object, result, line)?;
                    }
                    let valid = self
                        .iterator_method(&inner, b"valid", line)?
                        .to_bool(line, self)?;
                    if valid {
                        active.borrow_mut().state = State::Test;
                        continue;
                    }
                }
                State::Test => {
                    let result = self.recursive_child_call(object, false, line);
                    let has_children = match result {
                        Ok(value) => value.to_bool(line, self)?,
                        Err(error) => {
                            if let Err(error) = self.recursive_catch(object, Err(error), line) {
                                active.borrow_mut().state = State::Next;
                                return Err(error);
                            }
                            // With CATCH_GET_CHILD, nextElement still observes
                            // the Test state until its callback has returned.
                            false
                        }
                    };
                    if has_children {
                        if maximum == -1 || maximum > depth(object) as i64 {
                            match mode {
                                0 | 2 => {
                                    active.borrow_mut().state = State::Child;
                                    continue;
                                }
                                1 => {
                                    active.borrow_mut().state = State::SelfEntry;
                                    continue;
                                }
                                _ => {}
                            }
                        } else if mode == 0 {
                            active.borrow_mut().state = State::Next;
                            continue;
                        }
                    }
                    let result = self.recursive_hook(object, b"nextElement", line);
                    active.borrow_mut().state = State::Next;
                    self.recursive_catch(object, result, line)?;
                    return Ok(());
                }
                State::SelfEntry => {
                    let result = if mode == 1 || mode == 2 {
                        self.recursive_hook(object, b"nextElement", line)
                    } else {
                        Ok(())
                    };
                    active.borrow_mut().state = if mode == 1 { State::Child } else { State::Next };
                    return result;
                }
                State::Child => {
                    let child = match self.recursive_child_call(object, true, line) {
                        Ok(child) => child,
                        Err(error) => {
                            self.recursive_catch(object, Err(error), line)?;
                            active.borrow_mut().state = State::Next;
                            continue;
                        }
                    };
                    let Value::Object(child) = child else {
                        return Err(self.array_iterator_exception(b"UnexpectedValueException", "Objects returned by RecursiveIterator::getChildren() must implement RecursiveIterator".into(), line));
                    };
                    if !child.class.is_a(b"RecursiveIterator") {
                        return Err(self.array_iterator_exception(b"UnexpectedValueException", "Objects returned by RecursiveIterator::getChildren() must implement RecursiveIterator".into(), line));
                    }
                    active.borrow_mut().state = if mode == 2 {
                        State::SelfEntry
                    } else {
                        State::Next
                    };
                    let child_frame = self.recursive_frame(child.clone(), line)?;
                    object
                        .recursive_iterator
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .frames
                        .push(child_frame);
                    let result = self.iterator_method(&child, b"rewind", line).map(drop);
                    // A user beginChildren hook is entered even after a rewind
                    // exception. Only that hook path applies CATCH_GET_CHILD.
                    let overridden =
                        objects::method(&object.class, b"beginChildren").is_some_and(|method| {
                            !matches!(method.native, Some(NativeMethod::RecursiveIterator))
                        });
                    if overridden {
                        let hook = self.recursive_hook(object, b"beginChildren", line);
                        self.recursive_catch(object, result.and(hook), line)?;
                    } else {
                        result?;
                    }
                    continue;
                }
            }
            if depth(object) == 0 {
                return Ok(());
            }
            let result = self.recursive_hook(object, b"endChildren", line);
            self.recursive_catch(object, result, line)?;
            drop(inner);
            drop(active);
            if depth(object) > 0 {
                let removed = object
                    .recursive_iterator
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .frames
                    .pop();
                drop(removed);
                self.drain_destructors(Ok(()), line)?;
            }
        }
    }

    pub(super) fn recursive_iterator_step(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        raw: bool,
        line: usize,
    ) -> Result<Value, Error> {
        match name {
            b"rewind" => {
                frame(object, line)?;
                while depth(object) > 0 {
                    let removed = object
                        .recursive_iterator
                        .borrow_mut()
                        .as_mut()
                        .unwrap()
                        .frames
                        .pop();
                    drop(removed);
                    self.drain_destructors(Ok(()), line)?;
                    self.recursive_hook(object, b"endChildren", line)?;
                }
                let active = frame(object, line)?;
                active.borrow_mut().state = State::Start;
                let old = active.borrow_mut().current.take();
                drop(old);
                let inner = active.borrow().object.clone();
                self.iterator_method(&inner, b"rewind", line)?;
                let in_iteration = object
                    .recursive_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .in_iteration;
                let result = if !in_iteration {
                    self.recursive_hook(object, b"beginIteration", line)
                } else {
                    Ok(())
                };
                object
                    .recursive_iterator
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .in_iteration = true;
                result?;
                self.recursive_advance(object, line)?;
                Ok(Value::Null)
            }
            b"next" => {
                self.recursive_advance(object, line)?;
                Ok(Value::Null)
            }
            b"valid" => {
                let length = object
                    .recursive_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .frames
                    .len();
                for level in (0..length).rev() {
                    let inner = object
                        .recursive_iterator
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .frames
                        .get(level)
                        .map(|frame| frame.borrow().object.clone());
                    if let Some(inner) = inner
                        && self
                            .iterator_method(&inner, b"valid", line)?
                            .to_bool(line, self)?
                    {
                        return Ok(Value::Bool(true));
                    }
                }
                let active = object
                    .recursive_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .in_iteration;
                let result = if active {
                    self.recursive_hook(object, b"endIteration", line)
                } else {
                    Ok(())
                };
                object
                    .recursive_iterator
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .in_iteration = false;
                result?;
                Ok(Value::Bool(false))
            }
            b"current" | b"key" => {
                let active = frame(object, line)?;
                let inner = active.borrow().object.clone();
                if name == b"key" {
                    return self
                        .iterator_method(&inner, name, line)
                        .map(|v| v.dereferenced());
                }
                let cached = active.borrow().current.clone();
                let value = if let Some(value) = cached {
                    value
                } else {
                    let value = self.iterator_method(&inner, name, line)?;
                    if !array_iterator::native_iteration(&inner)
                        && !native_iteration(&inner)
                        && !native_iterator::hidden_iterator(&inner)
                    {
                        active.borrow_mut().current = Some(value.clone());
                    }
                    value
                };
                Ok(if raw { value } else { value.dereferenced() })
            }
            _ => unreachable!(),
        }
    }

    pub(super) fn native_recursive_iterator_method(&mut self, line: usize) -> Result<Value, Error> {
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let tree_method = self
            .current_function
            .as_ref()
            .unwrap()
            .starts_with(b"recursivetreeiterator::");
        let name = self
            .current_function
            .as_ref()
            .unwrap()
            .rsplit(|b| *b == b':')
            .next()
            .unwrap()
            .to_vec();
        if tree_method && name != b"__construct" {
            return self.recursive_tree_method(&object, &name, line);
        }
        match name.as_slice() {
            b"__construct" => {
                let mut value = self.lookup(b"iterator").unwrap();
                if let Value::Object(inner) = &value
                    && inner.class.is_a(b"IteratorAggregate")
                {
                    let returned = self.recursive_call(inner, b"getIterator", line)?;
                    match returned {
                        Value::Object(iterator) if iterator.class.is_a(b"Traversable") => value = Value::Object(iterator),
                        _ => return Err(self.array_iterator_exception(b"LogicException", format!("{}::getIterator() must return an object that implements Traversable", inner.class_name), line)),
                    }
                }
                if tree_method {
                    let class = self.find_class(b"RecursiveCachingIterator").unwrap();
                    let flags = self.lookup(b"cachingIteratorFlags").unwrap();
                    let args = [value, flags]
                        .into_iter()
                        .map(|value| {
                            Argument::Positional(Expr {
                                line,
                                kind: ExprKind::Literal(value),
                            })
                        })
                        .collect::<Vec<_>>();
                    value = self.instantiate_class(class, &args, true, line)?;
                }
                let Value::Object(inner) = value else {
                    return Err(self.array_iterator_exception(b"InvalidArgumentException", "An instance of RecursiveIterator or IteratorAggregate creating it is required".into(), line));
                };
                if !inner.class.is_a(b"RecursiveIterator") {
                    return Err(self.array_iterator_exception(b"InvalidArgumentException", "An instance of RecursiveIterator or IteratorAggregate creating it is required".into(), line));
                }
                let mode = self.lookup(b"mode").unwrap().integer(line, self)? as i32 as i64;
                let flags = self.lookup(b"flags").unwrap().integer(line, self)? as i32 as i64;
                let tree = object
                    .recursive_iterator
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .tree
                    .take();
                let old = object
                    .recursive_iterator
                    .borrow_mut()
                    .replace(RecursiveIteratorData {
                        tree,
                        frames: Vec::new(),
                        mode: 0,
                        flags: 0,
                        max_depth: 0,
                        in_iteration: false,
                    });
                drop(old);
                self.drain_destructors(Ok(()), line)?;
                let tree = object
                    .recursive_iterator
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .tree
                    .take();
                *object.recursive_iterator.borrow_mut() = Some(RecursiveIteratorData {
                    tree,
                    mode,
                    flags,
                    max_depth: -1,
                    frames: Vec::new(),
                    in_iteration: false,
                });
                let frame = self.recursive_frame(inner, line)?;
                object
                    .recursive_iterator
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .frames
                    .push(frame);
                Ok(Value::Null)
            }
            b"rewind" | b"next" | b"current" | b"key" | b"valid" => {
                self.recursive_iterator_step(&object, &name, false, line)
            }
            b"getdepth" => Ok(Value::Int(depth(&object) as i64)),
            b"getsubiterator" => {
                let level = self.lookup(b"level").unwrap();
                let level = if matches!(level, Value::Null) {
                    depth(&object) as i64
                } else {
                    level.integer(line, self)?
                };
                if level < 0 || level > depth(&object) as i64 {
                    return Ok(Value::Null);
                }
                frame(&object, line)?;
                Ok(Value::Object(native_iterator::source_object(
                    &object.recursive_iterator.borrow().as_ref().unwrap().frames[level as usize]
                        .borrow()
                        .object
                        .clone(),
                )))
            }
            b"getinneriterator" => Ok(Value::Object(native_iterator::source_object(
                &frame(&object, line)?.borrow().object,
            ))),
            b"callhaschildren" | b"callgetchildren" => {
                if name == b"callhaschildren"
                    && !object
                        .recursive_iterator
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .initialized()
                {
                    return Ok(Value::Bool(false));
                }
                let inner = frame(&object, line)?.borrow().object.clone();
                self.recursive_call(
                    &inner,
                    if name == b"callhaschildren" {
                        b"hasChildren"
                    } else {
                        b"getChildren"
                    },
                    line,
                )
            }
            b"setmaxdepth" => {
                let maximum = self.lookup(b"maxDepth").unwrap().integer(line, self)?;
                if maximum < -1 {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "RecursiveIteratorIterator::setMaxDepth(): Argument #1 ($maxDepth) must be greater than or equal to -1",
                        line,
                    ));
                }
                object
                    .recursive_iterator
                    .borrow_mut()
                    .as_mut()
                    .unwrap()
                    .max_depth = maximum.min(i32::MAX as i64);
                Ok(Value::Null)
            }
            b"getmaxdepth" => {
                let maximum = object
                    .recursive_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .max_depth;
                Ok(if maximum == -1 {
                    Value::Bool(false)
                } else {
                    Value::Int(maximum)
                })
            }
            b"beginiteration" | b"enditeration" | b"beginchildren" | b"endchildren"
            | b"nextelement" => Ok(Value::Null),
            _ => unreachable!(),
        }
    }
}

impl Runtime {
    fn builtin_recursive_tree_iterator_class(&self) -> Rc<Class> {
        let name = b"recursivetreeiterator";
        if let Some(class) = self.builtin_classes.borrow().get(name.as_slice()) {
            return class.clone();
        }
        let code = b"<?php class RecursiveTreeIterator extends RecursiveIteratorIterator {
            public const int BYPASS_CURRENT = 4, BYPASS_KEY = 8;
            public const int PREFIX_LEFT = 0, PREFIX_MID_HAS_NEXT = 1, PREFIX_MID_LAST = 2, PREFIX_END_HAS_NEXT = 3, PREFIX_END_LAST = 4, PREFIX_RIGHT = 5;
            public function __construct(RecursiveIterator|IteratorAggregate $iterator, int $flags = 8, int $cachingIteratorFlags = 16, int $mode = 1) {}
            public function key(): mixed {}
            public function current(): mixed {}
            public function getPrefix(): string {}
            public function setPostfix(string $postfix): void {}
            public function setPrefixPart(int $part, string $value): void {}
            public function getEntry(): string {}
            public function getPostfix(): string {}
        }";
        let (mut statements, _, _) = parser::parse(lexer::lex(code).unwrap()).unwrap();
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            let method = Rc::make_mut(method);
            // Avoid recursively bootstrapping native classes during parser folding.
            for parameter in &mut method.parameters {
                let constant = match parameter.name.as_slice() {
                    b"flags" => Some(("RecursiveTreeIterator", "BYPASS_KEY")),
                    b"cachingIteratorFlags" => Some(("CachingIterator", "CATCH_GET_CHILD")),
                    b"mode" => Some(("RecursiveTreeIterator", "SELF_FIRST")),
                    _ => None,
                };
                if let Some((class, constant)) = constant {
                    parameter.default = Some(Expr {
                        line: 0,
                        kind: ExprKind::ScopedMember(
                            Box::new(Expr {
                                line: 0,
                                kind: ExprKind::Constant(class.as_bytes().to_vec()),
                            }),
                            Box::new(Expr {
                                line: 0,
                                kind: ExprKind::Literal(Value::String(
                                    constant.as_bytes().to_vec().into(),
                                )),
                            }),
                        ),
                    });
                }
            }
            method.native = Some(NativeMethod::RecursiveIterator);
            method.tentative_return_type = method.return_type.take();
        }
        let parent = self.find_class(b"RecursiveIteratorIterator").unwrap();
        let mut methods = declaration.methods.clone();
        let own: Vec<_> = methods
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
                    !own.iter().any(|name| {
                        m.name
                            .rsplit(|b| *b == b':')
                            .next()
                            .unwrap()
                            .eq_ignore_ascii_case(name)
                    })
                })
                .cloned(),
        );
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: declaration.name.clone(),
            methods,
            declaration,
            parent: Some(parent),
            interfaces: Vec::new(),
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
        class
    }
    fn recursive_tree_prefix(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        frame(object, line)?;
        let mut result = object
            .recursive_iterator
            .borrow()
            .as_ref()
            .unwrap()
            .tree
            .as_ref()
            .unwrap()
            .prefix[0]
            .clone();
        let mut level = 0;
        loop {
            let (inner, last) = {
                let data = object.recursive_iterator.borrow();
                let data = data.as_ref().unwrap();
                let Some(frame) = data.frames.get(level) else {
                    break;
                };
                (
                    frame.borrow().object.clone(),
                    level + 1 == data.frames.len(),
                )
            };
            let has_next = self.recursive_call(&inner, b"hasNext", line)? == Value::Bool(true);
            let part = match (last, has_next) {
                (false, true) => 1,
                (false, false) => 2,
                (true, true) => 3,
                (true, false) => 4,
            };
            result.extend(
                &object
                    .recursive_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .tree
                    .as_ref()
                    .unwrap()
                    .prefix[part],
            );
            if last {
                break;
            }
            level += 1;
        }
        result.extend(
            &object
                .recursive_iterator
                .borrow()
                .as_ref()
                .unwrap()
                .tree
                .as_ref()
                .unwrap()
                .prefix[5],
        );
        Ok(result)
    }
    fn recursive_tree_entry(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        let value = self.recursive_iterator_step(object, b"current", false, line)?;
        if matches!(value, Value::Array(_)) {
            Ok(b"Array".to_vec())
        } else {
            self.string_bytes(&value, line)
        }
    }
    fn recursive_tree_method(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        if name == b"setpostfix" {
            let value = self.lookup(b"postfix").unwrap().to_bytes();
            object
                .recursive_iterator
                .borrow_mut()
                .as_mut()
                .unwrap()
                .tree
                .as_mut()
                .unwrap()
                .postfix = value;
            return Ok(Value::Null);
        }
        if name == b"setprefixpart" {
            let Value::Int(part) = self.lookup(b"part").unwrap() else {
                unreachable!()
            };
            if !(0..=5).contains(&part) {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "RecursiveTreeIterator::setPrefixPart(): Argument #1 ($part) must be a RecursiveTreeIterator::PREFIX_* constant",
                    line,
                ));
            }
            let value = self.lookup(b"value").unwrap().to_bytes();
            object
                .recursive_iterator
                .borrow_mut()
                .as_mut()
                .unwrap()
                .tree
                .as_mut()
                .unwrap()
                .prefix[part as usize] = value;
            return Ok(Value::Null);
        }
        frame(object, line)?;
        if name == b"getprefix" {
            return self
                .recursive_tree_prefix(object, line)
                .map(|v| Value::String(v.into()));
        }
        if name == b"getentry" {
            return self
                .recursive_tree_entry(object, line)
                .map(|v| Value::String(v.into()));
        }
        if name == b"getpostfix" {
            return Ok(Value::String(
                object
                    .recursive_iterator
                    .borrow()
                    .as_ref()
                    .unwrap()
                    .tree
                    .as_ref()
                    .unwrap()
                    .postfix
                    .clone()
                    .into(),
            ));
        }
        let flags = object.recursive_iterator.borrow().as_ref().unwrap().flags;
        let value = match name {
            b"current" if flags & 4 != 0 => {
                return self.recursive_iterator_step(object, name, false, line);
            }
            b"current" => self.recursive_tree_entry(object, line)?,
            b"key" => {
                let key = self.recursive_iterator_step(object, name, false, line)?;
                if flags & 8 != 0 {
                    return Ok(key);
                }
                self.string_bytes(&key, line)?
            }
            _ => unreachable!(),
        };
        let mut result = self.recursive_tree_prefix(object, line)?;
        result.extend(value);
        result.extend(
            &object
                .recursive_iterator
                .borrow()
                .as_ref()
                .unwrap()
                .tree
                .as_ref()
                .unwrap()
                .postfix,
        );
        Ok(Value::String(result.into()))
    }
}
