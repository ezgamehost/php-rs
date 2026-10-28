use super::*;

impl Runtime {
    pub(super) fn assign_destructure_references(
        &mut self,
        target: &Expr,
        source: &Expr,
    ) -> Result<Value, Error> {
        let mut source = source;
        while let ExprKind::Group(inner) = &source.kind {
            source = inner;
        }
        let referenceable = source.is_writable() && !source.is_globals();
        let reference = if referenceable {
            self.argument_reference(source, source.line)?
        } else {
            let value = self.eval(source)?;
            cell(value)
        };
        self.destructure_reference_inner(target, reference.clone(), referenceable)?;
        let value = reference.borrow().clone();
        Ok(value)
    }

    pub(super) fn destructure_reference(
        &mut self,
        target: &Expr,
        source: variables::Cell,
    ) -> Result<(), Error> {
        self.destructure_reference_inner(target, source, true)
    }

    fn destructure_reference_inner(
        &mut self,
        target: &Expr,
        source: variables::Cell,
        referenceable: bool,
    ) -> Result<(), Error> {
        let ExprKind::Destructure(elements) = &target.kind else {
            unreachable!()
        };
        for (index, element) in elements.iter().enumerate() {
            let (key, destination, by_reference) = match element {
                ArrayElement::Reference(key, destination) => (key, destination, true),
                ArrayElement::Entry(key, destination) => {
                    (key, destination, destination.has_destructure_references())
                }
                _ => continue,
            };
            let key = match key {
                Some(key) => self.eval(key)?,
                None => Value::Int(index as i64),
            };
            let mut offset = Offset::new(Some(key));
            if by_reference {
                let reference = if referenceable {
                    reference_paths::acquire(
                        source.clone(),
                        std::slice::from_mut(&mut offset),
                        destination.line,
                        &mut self.diagnostics,
                    )?
                } else {
                    self.diagnostics.push(format!(
                        "Notice: Attempting to set reference to non referenceable value on line {}",
                        destination.line
                    ));
                    let value = source.borrow().clone();
                    cell(self.destructure_read(value, &mut offset, destination.line)?)
                };
                if matches!(destination.kind, ExprKind::Destructure(_)) {
                    self.destructure_reference_inner(destination, reference, referenceable)?;
                } else {
                    let mut target = self.location(destination)?;
                    self.bind_reference_location(&mut target, reference, destination.line)?;
                }
            } else {
                let value = source.borrow().clone();
                let fetched = self.destructure_read(value, &mut offset, destination.line)?;
                self.assign_target(destination, fetched)?;
            }
        }
        Ok(())
    }

    fn destructure_read(
        &mut self,
        value: Value,
        offset: &mut Offset,
        line: usize,
    ) -> Result<Value, Error> {
        match value {
            Value::Array(_) | Value::Object(_) | Value::Closure(_) => {
                read_offset(value, offset, line, Access::Read, &mut self.diagnostics)
            }
            Value::Null => Ok(Value::Null),
            other => {
                self.diagnostics.push(format!(
                    "Cannot use {} as array on line {line}",
                    builtins::debug_type(&other)
                ));
                Ok(Value::Null)
            }
        }
    }

    pub(super) fn assign_target(&mut self, target: &Expr, value: Value) -> Result<Value, Error> {
        let ExprKind::Destructure(elements) = &target.kind else {
            let mut location = self.location(target)?;
            return self.write_location(&mut location, value, target.line);
        };
        for (index, element) in elements.iter().enumerate() {
            let ArrayElement::Entry(key, destination) = element else {
                continue;
            };
            let key = match key {
                Some(key) => self.eval(key)?,
                None => Value::Int(index as i64),
            };
            let fetched = self.destructure_read(
                value.clone(),
                &mut Offset::new(Some(key)),
                destination.line,
            )?;
            self.assign_target(destination, fetched)?;
        }
        Ok(value)
    }
}
