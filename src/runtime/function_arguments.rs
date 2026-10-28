use super::{builtins::Builtin, *};

/// Fixed parameters are live local slots; extra positional arguments retain
/// their post-coercion values independently of the variadic parameter array.
pub(super) struct FunctionArguments {
    pub fixed: Vec<Vec<u8>>,
    pub extra: Vec<Value>,
}

impl FunctionArguments {
    fn len(&self) -> usize {
        self.fixed.len() + self.extra.len()
    }

    fn get(&self, index: usize, scope: &Scope) -> Value {
        if let Some(name) = self.fixed.get(index) {
            scope
                .get(name)
                .map_or(Value::Null, |cell| cell.borrow().clone())
        } else {
            self.extra[index - self.fixed.len()].dereferenced()
        }
    }
}

impl Runtime {
    pub(super) fn inspect_arguments(
        &mut self,
        builtin: Builtin,
        position: Option<&Value>,
        dynamic: bool,
        line: usize,
    ) -> Result<Value, Error> {
        let name = match builtin {
            Builtin::FuncNumArgs => "func_num_args",
            Builtin::FuncGetArg => "func_get_arg",
            Builtin::FuncGetArgs => "func_get_args",
            _ => unreachable!(),
        };
        // PHP validates the position before checking the execution context,
        // and checks the dynamic-call restriction before the upper bound.
        let position = if let Some(position) = position {
            let position = self.native_int(position, name, 1, line)?;
            if position < 0 {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "func_get_arg(): Argument #1 ($position) must be greater than or equal to 0",
                    line,
                ));
            }
            Some(position as usize)
        } else {
            None
        };
        let arguments = self.current_arguments.as_ref().ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                if matches!(builtin, Builtin::FuncNumArgs) {
                    "func_num_args() must be called from a function context".to_owned()
                } else {
                    format!("{name}() cannot be called from the global scope")
                },
                line,
            )
        })?;
        if dynamic {
            return Err(Error::new(
                ErrorKind::Error,
                format!("Cannot call {name}() dynamically"),
                line,
            ));
        }
        match builtin {
            Builtin::FuncNumArgs => Ok(Value::Int(arguments.len() as i64)),
            Builtin::FuncGetArg => {
                let index = position.unwrap();
                if index >= arguments.len() {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "func_get_arg(): Argument #1 ($position) must be less than the number of the arguments passed to the currently executed function",
                        line,
                    ));
                }
                Ok(arguments.get(index, &self.scope))
            }
            Builtin::FuncGetArgs => {
                let mut array = PhpArray::default();
                for i in 0..arguments.len() {
                    array.append(arguments.get(i, &self.scope), line)?;
                }
                Ok(Value::Array(Rc::new(array)))
            }
            _ => unreachable!(),
        }
    }
}
