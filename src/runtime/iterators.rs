// Altered Rust adaptation of PHP 8.5.0 Zend/zend_interfaces.c and the
// object foreach handlers in Zend/zend_vm_def.h. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::PhpObject;
use crate::object::Class;

impl Runtime {
    pub(super) fn validate_iterator_interfaces(&mut self, class: &Class) -> Result<(), Error> {
        if class.declaration.is_interface {
            return Ok(());
        }
        let iterator = class.is_a(b"Iterator");
        let aggregate = class.is_a(b"IteratorAggregate");
        let message = if iterator && aggregate {
            Some(format!(
                "Class {} cannot implement both Iterator and IteratorAggregate at the same time",
                String::from_utf8_lossy(&class.name)
            ))
        } else if class.is_a(b"Traversable")
            && !class.declaration.is_abstract
            && !iterator
            && !aggregate
        {
            Some(format!(
                "{} {} must implement interface Traversable as part of either Iterator or IteratorAggregate",
                if class.declaration.is_enum {
                    "Enum"
                } else {
                    "Class"
                },
                String::from_utf8_lossy(&class.name)
            ))
        } else {
            None
        };
        if let Some(message) = message {
            // The Traversable implementation hook raises E_CORE_ERROR, which
            // PHP attributes to Unknown:0. The dual-interface error is E_ERROR
            // and retains the declaration site.
            if !iterator && !aggregate {
                self.diagnostics.last_fatal_trace = None;
                self.diagnostics.output_position = self.output.len();
                self.diagnostics
                    .display(16, "Fatal error", message.as_bytes(), b"Unknown", 0);
                self.exit_status = Some(255);
                return Err(Error::new(ErrorKind::UnwindExit, "", 0));
            }
            return Err(self.compile_fatal_at(
                &message,
                &class.declaration.source,
                class.declaration.line,
            ));
        }
        Ok(())
    }

    pub(super) fn iterator_method(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        if array_iterator::native_iteration(object) {
            return self.array_iterator_step(object, name, true, line);
        }
        let method = objects::method(&object.class, name).expect("Iterator interface method");
        let target =
            self.unchecked_method_target(method, object.class.clone(), Some(object.clone()), line)?;
        self.invoke_at(target, &[], line, false, true)
    }

    pub(super) fn resolve_iterator(
        &mut self,
        object: &Rc<PhpObject>,
        by_reference: bool,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        if object.class.is_a(b"IteratorAggregate") {
            // Unlike current(), the aggregate hook inspects the raw return
            // zval: a reference return is not accepted as an iterator object.
            let returned = self.iterator_method(object, b"getIterator", line)?;
            let result = match &returned {
                Value::Object(inner)
                    if inner.class.is_a(b"Traversable") && !Rc::ptr_eq(object, inner) =>
                {
                    self.resolve_iterator(inner, by_reference, line)
                }
                _ => Err(Error::new(
                    ErrorKind::Exception,
                    format!(
                        "Objects returned by {}::getIterator() must be traversable or implement interface Iterator",
                        object.class_name
                    ),
                    line,
                )),
            };
            drop(returned);
            return self.drain_destructors(result, line);
        }
        if by_reference && !array_iterator::native_iteration(object) {
            return Err(Error::new(
                ErrorKind::Error,
                "An iterator cannot be used with foreach by reference",
                line,
            ));
        }
        Ok(object.clone())
    }

    pub(super) fn foreach_iterator(
        &mut self,
        object: Rc<PhpObject>,
        key_target: Option<&Expr>,
        value_target: &Expr,
        by_reference: bool,
        body: &Stmt,
        line: usize,
    ) -> Result<Flow, Error> {
        let iterator = self.resolve_iterator(&object, by_reference, line)?;
        // Zend iterator wrappers occupy an object-store handle even though
        // they are not exposed as PHP values. Release it after the iterator.
        let wrapper = self.allocate_object_handle();
        let mut source = Some(object);
        let result = (|| {
            drop(self.iterator_method(&iterator, b"rewind", line)?);
            self.drain_destructors(Ok(()), line)?;
            loop {
                let valid = self.iterator_method(&iterator, b"valid", line)?;
                let more = valid.to_bool(line, self);
                drop(valid);
                let more = self.drain_destructors(more, line)?;
                // FE_RESET releases a temporary aggregate after rewind and the
                // first valid call; only its resolved iterator survives the loop.
                drop(source.take());
                self.drain_destructors(Ok(()), line)?;
                if !more {
                    break;
                }
                // Keep the raw current result alive through the body, including
                // references, as PHP's iterator cache does. Fetch key before
                // assigning either target, and only if the loop requests it.
                let current = if by_reference {
                    let key = self
                        .array_iterator_key(&iterator)
                        .expect("valid native iterator");
                    Value::Reference(self.array_iterator_reference(&iterator, &key, line)?)
                } else {
                    self.iterator_method(&iterator, b"current", line)?
                };
                let key = key_target
                    .map(|_| {
                        self.iterator_method(&iterator, b"key", line)
                            .map(|key| key.dereferenced())
                    })
                    .transpose()?;
                if by_reference {
                    let Value::Reference(reference) = &current else {
                        unreachable!()
                    };
                    if matches!(value_target.kind, ExprKind::Destructure(_)) {
                        self.destructure_reference(value_target, reference.clone())?;
                    } else {
                        let mut target = self.location(value_target)?;
                        self.bind_reference_location(
                            &mut target,
                            reference.clone(),
                            value_target.line,
                        )?;
                    }
                } else {
                    self.assign_target(value_target, current.dereferenced())?;
                }
                if let (Some(target), Some(key)) = (key_target, key) {
                    self.assign_target(target, key.dereferenced())?;
                }
                // Native array iteration points into storage, without the
                // persistent current-result cache used by user Iterator hooks.
                let current = if array_iterator::native_iteration(&iterator) {
                    drop(current);
                    self.drain_destructors(Ok(()), line)?;
                    None
                } else {
                    Some(current)
                };
                let flow = self.block(std::slice::from_ref(body));
                drop(current);
                match self.drain_destructors(flow, line)? {
                    Flow::Next | Flow::Continue(1) => {}
                    Flow::Break(1) => break,
                    flow => return Ok(flow.leave_level()),
                }
                drop(self.iterator_method(&iterator, b"next", line)?);
                self.drain_destructors(Ok(()), line)?;
            }
            Ok(Flow::Next)
        })();
        drop(iterator);
        let result = self.drain_destructors(result, line);
        drop(wrapper);
        drop(source);
        self.drain_destructors(result, line)
    }
}
