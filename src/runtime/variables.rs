use super::*;

pub(super) type Cell = Rc<RefCell<Value>>;
pub(super) type Scope = HashMap<Vec<u8>, Cell>;

pub(super) fn cell(value: Value) -> Cell {
    Rc::new(RefCell::new(value))
}

impl Runtime {
    pub(super) fn lookup(&self, name: &[u8]) -> Option<Value> {
        self.scope.get(name).map(|value| value.borrow().clone())
    }

    pub(super) fn variable_name(&mut self, expression: &Expr) -> Result<Vec<u8>, Error> {
        let value = self.eval(expression)?;
        Ok(string_bytes(&value, expression.line, &mut self.warnings))
    }

    pub(super) fn bind_global(&mut self, name: Vec<u8>) {
        let globals = self.callers.first_mut().unwrap_or(&mut self.scope);
        let value = globals
            .entry(name.clone())
            .or_insert_with(|| cell(Value::Null))
            .clone();
        self.scope.insert(name, value);
    }

    pub(super) fn bind_static(
        &mut self,
        name: &[u8],
        initializer: Option<&Expr>,
    ) -> Result<(), Error> {
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
                .entry(name.to_vec())
                .or_insert_with(|| cell(initial))
                .clone()
        };
        self.scope.insert(name.to_vec(), value);
        Ok(())
    }
}
