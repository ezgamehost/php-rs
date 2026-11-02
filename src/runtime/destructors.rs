// Altered Rust adaptation of PHP 8.5.0's Zend object destruction and request
// shutdown rules. See THIRD_PARTY_NOTICES.md.
use super::*;
use crate::{PhpObject, ast::Visibility, object::Destruction};

impl Runtime {
    pub(super) fn create_object(&self, object: PhpObject) -> Rc<PhpObject> {
        let object = Rc::new(object);
        self.object_store
            .borrow_mut()
            .objects
            .insert(object.object_id, Rc::downgrade(&object));
        object
    }

    pub(super) fn drain_destructors<T>(
        &mut self,
        pending: Result<T, Error>,
        line: usize,
    ) -> Result<T, Error> {
        if pending
            .as_ref()
            .is_err_and(|e| e.kind == ErrorKind::UnwindExit)
        {
            return pending;
        }
        let mut queue = std::mem::take(&mut self.object_store.borrow_mut().pending);
        if queue.is_empty() {
            return pending;
        }
        let mut pending = pending.map_err(|error| self.materialize_error(error));
        while let Some(event) = queue.pop_front() {
            match event {
                Destruction::Free(id) => self.object_store.borrow_mut().free.push(id),
                Destruction::Object(object) => {
                    // The queue owns a replacement allocation. Refresh weak
                    // holders before aliases are used from the destructor.
                    self.inherit_reference_sources(&object);
                    let result = self.invoke_destructor(&object, line);
                    drop(object);
                    if let Err(error) = result {
                        let error = self.materialize_error(error);
                        if let Err(previous) = &pending
                            && let (Some(current), Some(previous)) =
                                (&error.throwable, &previous.throwable)
                        {
                            throwables::append_previous(current, previous);
                        }
                        pending = Err(error);
                    }
                    // Finish child properties before releasing the next local
                    // or parent handle. PHP calls within a destructor may also
                    // drain this queue, without seeing its later siblings.
                    pending = self.drain_destructors(pending, line);
                    if pending
                        .as_ref()
                        .is_err_and(|e| e.kind == ErrorKind::UnwindExit)
                    {
                        self.object_store.borrow_mut().pending.extend(queue);
                        return pending;
                    }
                }
            }
        }
        pending
    }

    pub(super) fn invoke_destructor(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<(), Error> {
        let Some(method) = objects::method(&object.class, b"__destruct") else {
            return Ok(());
        };
        object._handle.destructor_called.set(true);
        let shutdown = self.shutdown_started && self.trace_frames.is_empty();
        if self.check_method_access(&method, line).is_err() {
            let visibility = if method.visibility == Visibility::Private {
                "private"
            } else {
                "protected"
            };
            let context = self.class_context.as_ref().map_or_else(
                || "global scope".to_owned(),
                |context| format!("scope {}", context.scope.display_name()),
            );
            let message = format!(
                "Call to {visibility} {}::__destruct() from {context}",
                object.class.display_name(),
            );
            if shutdown {
                return self.warn(format!("{message} during shutdown ignored on line 0"));
            }
            return Err(Error::new(ErrorKind::Error, message, line));
        }
        let target =
            self.method_target(method, object.class.clone(), Some(object.clone()), line)?;
        let internal = shutdown
            || self.collecting_cycles
            || self.trace_frames.last().is_some_and(|frame| frame.internal);
        self.invoke_at(target, &[], line, false, internal).map(drop)
    }

    pub(super) fn shutdown_destructors(&mut self) -> Result<(), Error> {
        // The public embedding snapshot must not act as another PHP reference.
        self.variables.clear();
        let result = self.shutdown_destructors_inner();
        self.object_store.borrow_mut().destructors_finished = true;
        // Drop outside the store's RefCell borrow; handles access the store.
        let queue = std::mem::take(&mut self.object_store.borrow_mut().pending);
        drop(queue);
        result
    }

    fn shutdown_destructors_inner(&mut self) -> Result<(), Error> {
        let result = self.drain_destructors(Ok(()), 0);
        if let Err(error) = result {
            self.dispatch_uncaught(error)?;
        }
        loop {
            let names: Vec<_> = self.scope.iter().map(|(name, _)| name.clone()).collect();
            let before = names.len();
            for name in names.into_iter().rev() {
                let unique = !self.scope.is_reference(&name) && self.scope.get(&name).is_some_and(|cell| {
                    Rc::strong_count(cell) == 1
                        && matches!(&*cell.borrow(), Value::Object(object) if Rc::strong_count(object) == 1)
                });
                if unique {
                    self.scope.remove(&name);
                    let result = self.drain_destructors(Ok(()), 0);
                    if let Err(error) = result {
                        self.dispatch_uncaught(error)?;
                    }
                }
            }
            if before == self.scope.iter().count() {
                break;
            }
        }
        // Shared objects, static variables and cycles remain alive. PHP visits
        // these in handle order and disables reuse during this final sweep.
        self.object_store.borrow_mut().no_reuse = true;
        let mut previous = 0;
        loop {
            let next = self
                .object_store
                .borrow()
                .objects
                .range((previous + 1)..)
                .next()
                .map(|(id, object)| (*id, object.upgrade()));
            let Some((id, object)) = next else { break };
            previous = id;
            if let Some(object) = object
                && !object._handle.destructor_called.get()
            {
                let result = self.invoke_destructor(&object, 0);
                drop(object);
                let result = self.drain_destructors(result, 0);
                if let Err(error) = result {
                    self.dispatch_uncaught(error)?;
                }
            }
        }
        Ok(())
    }
}
