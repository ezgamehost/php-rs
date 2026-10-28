use super::*;

impl Runtime {
    pub(super) fn foreach_reference(
        &mut self,
        expression: &Expr,
        key_target: Option<&Expr>,
        value_target: &Expr,
        body: &Stmt,
    ) -> Result<Flow, Error> {
        let mut expression = expression;
        while let ExprKind::Group(inner) = &expression.kind {
            expression = inner;
        }
        let reference_source = match &expression.kind {
            ExprKind::Assign(target, op, source) if op == "=&" => {
                Some(self.reference_assignment(target, source, expression.line)?)
            }
            _ => None,
        };
        let mut location = if expression.is_writable() && !expression.is_globals() {
            Some(self.location(expression)?)
        } else {
            None
        };
        let value = if let Some(reference) = &reference_source {
            reference.borrow().clone()
        } else {
            match &mut location {
                Some(location) => self.read_location(location, expression.line, false, None)?,
                None => self.eval(expression)?,
            }
        };
        if matches!(value, Value::Closure(_)) {
            return Ok(Flow::Next);
        }
        if matches!(value, Value::Object(_)) {
            return Err(Error::new(
                ErrorKind::Error,
                "Foreach by reference over objects is not supported yet",
                expression.line,
            ));
        }
        if !matches!(value, Value::Array(_)) {
            self.diagnostics.push(format!(
                "foreach() argument must be of type array|object on line {}",
                expression.line
            ));
            return Ok(Flow::Next);
        }
        let source = if let Some(reference) = reference_source {
            drop(value);
            reference
        } else if let Some(location) = &mut location {
            if location.object.is_some() || location.static_class.is_some() {
                return Err(Error::new(
                    ErrorKind::Error,
                    "References to properties are not supported yet",
                    expression.line,
                ));
            }
            drop(value);
            self.location_reference(location, expression.line)?
        } else {
            cell(value)
        };
        let iterator = Rc::new(());
        loop {
            let next = {
                let mut value = source.borrow_mut();
                match &mut *value {
                    Value::Array(array) => {
                        if !array.has_iterator(&iterator) {
                            Rc::make_mut(array).start_reference_iteration(&iterator);
                        }
                        array.next_reference(&iterator)
                    }
                    _ => None,
                }
            };
            let Some((key, reference)) = next else {
                break;
            };
            let mut destination = self.location(value_target)?;
            self.bind_reference_location(&mut destination, reference, value_target.line)?;
            if let Some(target) = key_target {
                self.assign_target(target, key.to_value())?;
            }
            match self.block(std::slice::from_ref(body))? {
                Flow::Next | Flow::Continue(1) => {}
                Flow::Break(1) => break,
                flow => return Ok(flow.leave_level()),
            }
        }
        Ok(Flow::Next)
    }
}
