use super::*;

pub(super) type Cell = Rc<RefCell<Value>>;
#[derive(Default)]
pub(super) struct Scope {
    values: HashMap<Vec<u8>, Cell>,
    order: Vec<Vec<u8>>,
    slots: HashSet<Vec<u8>>,
}

impl Scope {
    pub(super) fn reserve(&mut self, name: &[u8]) {
        if self.slots.insert(name.to_vec()) && !self.values.contains_key(name) {
            self.order.push(name.to_vec());
        }
    }
    pub(super) fn get(&self, name: &[u8]) -> Option<&Cell> {
        self.values.get(name)
    }

    pub(super) fn insert(&mut self, name: Vec<u8>, value: Cell) {
        if !self.values.contains_key(&name) && !self.slots.contains(&name) {
            self.order.push(name.clone());
        }
        self.values.insert(name, value);
    }

    pub(super) fn remove(&mut self, name: &[u8]) {
        if self.values.remove(name).is_some() && !self.slots.contains(name) {
            self.order.retain(|key| key != name);
        }
    }

    pub(super) fn get_or_insert(&mut self, name: &[u8], value: Value) -> Cell {
        if let Some(value) = self.get(name) {
            return value.clone();
        }
        let value = cell(value);
        self.insert(name.to_vec(), value.clone());
        value
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&Vec<u8>, &Cell)> {
        self.order
            .iter()
            .filter_map(|name| self.values.get(name).map(|value| (name, value)))
    }

    pub(super) fn reconcile(&mut self, inputs: HashMap<Vec<u8>, Value>) {
        self.values.retain(|name, _| inputs.contains_key(name));
        self.order
            .retain(|name| inputs.contains_key(name) || self.slots.contains(name));
        // Host inputs have no insertion order. Sort new names for repeatability;
        // existing PHP symbols retain both their order and alias identity.
        let mut inputs: Vec<_> = inputs.into_iter().collect();
        inputs.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, value) in inputs {
            if let Some(stored) = self.get(&name) {
                *stored.borrow_mut() = value;
            } else {
                self.insert(name, cell(value));
            }
        }
    }
}

pub(super) fn cell(value: Value) -> Cell {
    Rc::new(RefCell::new(value))
}

impl Runtime {
    pub(super) fn eval_reference_return(&mut self, expression: &Expr) -> Result<Value, Error> {
        let mut expression = expression;
        while let ExprKind::Group(inner) = &expression.kind {
            expression = inner;
        }
        if expression.is_globals()
            || matches!(&expression.kind, ExprKind::Variable(name) if name == b"this")
        {
            return Ok(Value::Reference(cell(self.eval(expression)?)));
        }
        if let ExprKind::Assign(target, op, source) = &expression.kind
            && op == "=&"
        {
            return self
                .reference_assignment(target, source, expression.line)
                .map(Value::Reference);
        }
        if expression.is_writable() {
            return self
                .argument_reference(expression, expression.line)
                .map(Value::Reference);
        }
        self.eval_inner(expression)
    }

    fn reference_call(&mut self, expression: &Expr, line: usize) -> Result<Cell, Error> {
        match self.eval_inner(expression)? {
            Value::Reference(reference) => Ok(reference),
            value => {
                self.diagnostics.push(format!(
                    "Notice: Only variables should be assigned by reference on line {line}"
                ));
                Ok(cell(value))
            }
        }
    }

    pub(super) fn assign_reference(
        &mut self,
        target: &Expr,
        source: &Expr,
        line: usize,
    ) -> Result<Value, Error> {
        let reference = self.reference_assignment(target, source, line)?;
        let value = reference.borrow().clone();
        Ok(value)
    }

    pub(super) fn argument_reference(&mut self, source: &Expr, line: usize) -> Result<Cell, Error> {
        if matches!(source.kind, ExprKind::Call(_, _)) {
            return self.reference_call(source, line);
        }
        let mut source = self.location(source)?;
        if source.object.is_some() || source.static_class.is_some() {
            return self.location_reference(&mut source, line);
        }
        if source.name == b"this" {
            return Err(Error::new(ErrorKind::Error, "Cannot re-assign $this", line));
        }
        if source.name == b"GLOBALS" && !source.global {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot acquire reference to $GLOBALS",
                line,
            ));
        }
        self.location_reference(&mut source, line)
    }

    pub(super) fn reference_assignment(
        &mut self,
        target: &Expr,
        source: &Expr,
        line: usize,
    ) -> Result<Cell, Error> {
        let mut target = self.location(target)?;
        if matches!(source.kind, ExprKind::Call(_, _)) {
            let reference = self.reference_call(source, line)?;
            self.bind_reference_location(&mut target, reference.clone(), line)?;
            return Ok(reference);
        }
        let mut source = self.location(source)?;
        if target.name == b"GLOBALS"
            && !target.global
            && target.object.is_none()
            && target.static_class.is_none()
        {
            return Err(Error::new(
                ErrorKind::Error,
                "$GLOBALS can only be modified using the $GLOBALS[$name] = $value syntax",
                line,
            ));
        }
        if source.name == b"GLOBALS"
            && !source.global
            && source.object.is_none()
            && source.static_class.is_none()
        {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot acquire reference to $GLOBALS",
                line,
            ));
        }
        for location in [&target, &source] {
            if location.object.is_none()
                && location.static_class.is_none()
                && location.name == b"this"
            {
                return Err(Error::new(ErrorKind::Error, "Cannot re-assign $this", line));
            }
        }
        let reference = self.location_reference(&mut source, line)?;
        self.bind_reference_location(&mut target, reference.clone(), line)?;
        Ok(reference)
    }

    pub(super) fn bind_reference_location(
        &mut self,
        target: &mut Location,
        reference: Cell,
        line: usize,
    ) -> Result<(), Error> {
        if target.object.is_some() || target.static_class.is_some() {
            return self.bind_property_reference(target, reference, line);
        }
        if target.name == b"this" {
            return Err(Error::new(ErrorKind::Error, "Cannot re-assign $this", line));
        }
        if target.name == b"GLOBALS" && !target.global {
            return Err(Error::new(
                ErrorKind::Error,
                "$GLOBALS can only be modified using the $GLOBALS[$name] = $value syntax",
                line,
            ));
        }
        if target.keys.is_empty() {
            self.location_scope(target)
                .insert(target.name.clone(), reference);
        } else {
            let root = self
                .location_scope(target)
                .get_or_insert(&target.name, Value::Null);
            reference_paths::bind(
                root,
                &mut target.keys,
                reference,
                line,
                &mut self.diagnostics,
            )?;
        }
        Ok(())
    }

    pub(super) fn location_reference(
        &mut self,
        location: &mut Location,
        line: usize,
    ) -> Result<Cell, Error> {
        if location.object.is_some() || location.static_class.is_some() {
            return self.property_reference(location, line);
        }
        let root = self
            .location_scope(location)
            .get_or_insert(&location.name, Value::Null);
        reference_paths::acquire(root, &mut location.keys, line, &mut self.diagnostics)
    }

    pub(super) fn globals(&self) -> &Scope {
        self.callers.first().unwrap_or(&self.scope)
    }

    pub(super) fn globals_snapshot(&self) -> Value {
        let mut array = PhpArray::default();
        for (name, value) in self.globals().iter() {
            let key = ArrayKey::from_value(&Value::String(name.clone().into()), 0, &mut Vec::new())
                .expect("string keys are valid");
            let value = if Rc::strong_count(value) > 1 {
                Value::Reference(value.clone())
            } else {
                value.borrow().clone()
            };
            array.insert(key, value);
        }
        Value::Array(Rc::new(array))
    }

    pub(super) fn location_scope(&mut self, location: &Location) -> &mut Scope {
        if location.global {
            self.callers.first_mut().unwrap_or(&mut self.scope)
        } else {
            &mut self.scope
        }
    }

    pub(super) fn lookup(&self, name: &[u8]) -> Option<Value> {
        self.scope.get(name).map(|value| value.borrow().clone())
    }

    pub(super) fn variable_name(&mut self, expression: &Expr) -> Result<Vec<u8>, Error> {
        let value = self.eval(expression)?;
        self.string_bytes(&value, expression.line)
    }

    pub(super) fn bind_global(&mut self, name: Vec<u8>) {
        let globals = self.callers.first_mut().unwrap_or(&mut self.scope);
        let value = globals.get_or_insert(&name, Value::Null);
        self.scope.insert(name, value);
    }

    pub(super) fn bind_static(
        &mut self,
        name: &[u8],
        initializer: Option<&Expr>,
    ) -> Result<(), Error> {
        if let Some(closure) = self.current_closure.clone()
            && let Some(environment) = &closure.environment
        {
            let stored = environment.statics.borrow().get(name).cloned();
            let value = if let Some(value) = stored {
                value
            } else {
                let initial = match initializer {
                    Some(expression) => self.eval(expression)?,
                    None => Value::Null,
                };
                environment
                    .statics
                    .borrow_mut()
                    .entry(name.to_vec())
                    .or_insert_with(|| cell(initial))
                    .clone()
            };
            self.scope.insert(name.to_vec(), value);
            return Ok(());
        }
        let stored = self
            .static_variables
            .get(&self.current_function)
            .and_then(|vars| vars.get(name))
            .cloned();
        let value = if let Some(stored) = stored {
            stored
        } else {
            let initial = match initializer {
                Some(expression) => self.eval(expression)?,
                None => Value::Null,
            };
            // Reentrant initializers may populate this slot while evaluating
            // initial. Keep the first value that completed initialization.
            self.static_variables
                .entry(self.current_function.clone())
                .or_default()
                .get_or_insert(name, initial)
        };
        self.scope.insert(name.to_vec(), value);
        Ok(())
    }
}
