use super::*;
use crate::object::Class;
use functions::CallTarget;

#[derive(Clone)]
pub(super) struct Autoloader {
    target: CallTarget,
    value: Value,
}

impl Autoloader {
    fn matches(&self, other: &Self) -> bool {
        let target = &other.target;
        fn same<T>(left: &Option<Rc<T>>, right: &Option<Rc<T>>) -> bool {
            match (left, right) {
                (Some(left), Some(right)) => Rc::ptr_eq(left, right),
                (None, None) => true,
                _ => false,
            }
        }
        matches!(self.value, Value::Object(_)) == matches!(other.value, Value::Object(_))
            && self.target.name.eq_ignore_ascii_case(&target.name)
            && same(&self.target.this, &target.this)
            && same(&self.target.closure, &target.closure)
            && self.target.context.as_ref().map(|c| &c.called.name)
                == target.context.as_ref().map(|c| &c.called.name)
    }
}

impl Runtime {
    pub(super) fn load_class(
        &mut self,
        name: &[u8],
        line: usize,
    ) -> Result<Option<Rc<Class>>, Error> {
        if let Some(class) = self.find_class(name) {
            return Ok(Some(class));
        }
        let name = name.strip_prefix(b"\\").unwrap_or(name);
        if name.is_empty()
            || name.eq_ignore_ascii_case(b"Closure")
            || self.folding_constants
            || self.diagnostics.compiling
            || !name
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'\\') || *b >= 128)
        {
            return Ok(None);
        }
        let key = name.to_ascii_lowercase();
        if !self.autoloading.insert(key.clone()) {
            return Ok(None);
        }
        let result = self.perform_autoload(name, line);
        self.autoloading.remove(&key);
        result?;
        Ok(self.find_class(name))
    }

    fn perform_autoload(&mut self, name: &[u8], line: usize) -> Result<(), Error> {
        // Tombstones keep the active index stable when callbacks unregister
        // themselves or a later callback. Newly appended callbacks remain visible.
        let mut index = 0;
        while index < self.autoloaders.len() {
            if let Some(loader) = self.autoloaders[index].clone() {
                self.call_callback(&loader.target, &[Value::String(name.to_vec().into())], line)?;
                if self.find_class(name).is_some() {
                    break;
                }
            }
            index += 1;
        }
        Ok(())
    }

    fn autoloader(
        &mut self,
        value: &Value,
        register: bool,
        line: usize,
    ) -> Result<Autoloader, Error> {
        let function = if register {
            "spl_autoload_register"
        } else {
            "spl_autoload_unregister"
        };
        let target = if register && matches!(value, Value::Null) {
            self.resolve_callable(Value::String(b"spl_autoload".to_vec().into()), line)?
        } else {
            self.native_callback(value, function, 1, register, line)?
                .expect("non-null callback")
        };
        let value = if let Some(closure) = &target.closure {
            Value::Closure(closure.clone())
        } else if matches!(value, Value::Object(_)) {
            value.clone()
        } else if let Some(context) = &target.context {
            let mut array = PhpArray::default();
            array.append(
                target
                    .this
                    .clone()
                    .map(Value::Object)
                    .unwrap_or_else(|| Value::String(context.called.name.clone().into())),
                line,
            )?;
            let name = target
                .function
                .as_ref()
                .map_or(target.name.as_slice(), |f| f.name.as_slice());
            array.append(
                Value::String(name.rsplit(|b| *b == b':').next().unwrap().to_vec().into()),
                line,
            )?;
            Value::Array(Rc::new(array))
        } else {
            Value::String(
                target
                    .function
                    .as_ref()
                    .map_or_else(|| target.name.clone(), |f| f.name.clone())
                    .into(),
            )
        };
        Ok(Autoloader { target, value })
    }

    pub(super) fn autoload_builtin(
        &mut self,
        builtin: builtins::Builtin,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        use builtins::Builtin;
        match builtin {
            Builtin::AutoloadRegister => {
                let loader =
                    self.autoloader(args[0].as_ref().unwrap_or(&Value::Null), true, line)?;
                let throw = args[1]
                    .as_ref()
                    .map(|v| self.native_bool(v, "spl_autoload_register", 2, line))
                    .transpose()?
                    .unwrap_or(true);
                let prepend = args[2]
                    .as_ref()
                    .map(|v| self.native_bool(v, "spl_autoload_register", 3, line))
                    .transpose()?
                    .unwrap_or(false);
                if !throw {
                    self.diagnostics.push(format!("Notice: spl_autoload_register(): Argument #2 ($do_throw) has been ignored, spl_autoload_register() will always throw on line {line}"));
                }
                if matches!(loader.target.builtin, Some(Builtin::AutoloadCall)) {
                    return Err(Error::new(
                        ErrorKind::ValueError,
                        "spl_autoload_register(): Argument #1 ($callback) must not be the spl_autoload_call() function",
                        line,
                    ));
                }
                if !self
                    .autoloaders
                    .iter()
                    .flatten()
                    .any(|old| old.matches(&loader))
                {
                    if prepend {
                        self.autoloaders.insert(0, Some(loader));
                    } else {
                        self.autoloaders.push(Some(loader));
                    }
                }
                Ok(Value::Bool(true))
            }
            Builtin::AutoloadUnregister => {
                let loader = self.autoloader(args[0].as_ref().unwrap(), false, line)?;
                if matches!(loader.target.builtin, Some(Builtin::AutoloadCall)) {
                    self.diagnostics.push(format!("Deprecated: Using spl_autoload_call() as a callback for spl_autoload_unregister() is deprecated, to remove all registered autoloaders, call spl_autoload_unregister() for all values returned from spl_autoload_functions() on line {line}"));
                    for slot in &mut self.autoloaders {
                        *slot = None;
                    }
                    return Ok(Value::Bool(true));
                }
                if let Some(slot) = self
                    .autoloaders
                    .iter_mut()
                    .find(|old| old.as_ref().is_some_and(|old| old.matches(&loader)))
                {
                    *slot = None;
                    Ok(Value::Bool(true))
                } else {
                    Ok(Value::Bool(false))
                }
            }
            Builtin::AutoloadFunctions => {
                let mut array = PhpArray::default();
                for loader in self.autoloaders.iter().flatten() {
                    array.append(loader.value.clone(), line)?;
                }
                Ok(Value::Array(Rc::new(array)))
            }
            Builtin::AutoloadCall => {
                let name =
                    self.native_string(args[0].as_ref().unwrap(), "spl_autoload_call", 1, line)?;
                self.perform_autoload(&name, line)?;
                Ok(Value::Null)
            }
            Builtin::AutoloadExtensions => {
                if let Some(value) = args[0].as_ref().filter(|v| !matches!(v, Value::Null)) {
                    self.autoload_extensions =
                        Some(self.native_string(value, "spl_autoload_extensions", 1, line)?);
                }
                Ok(Value::String(
                    self.autoload_extensions
                        .clone()
                        .unwrap_or_else(|| b".inc,.php".to_vec())
                        .into(),
                ))
            }
            Builtin::Autoload => {
                let name =
                    self.native_string(args[0].as_ref().unwrap(), "spl_autoload", 1, line)?;
                let extensions = args[1]
                    .as_ref()
                    .filter(|v| !matches!(v, Value::Null))
                    .map(|v| self.native_string(v, "spl_autoload", 2, line))
                    .transpose()?
                    .or_else(|| self.autoload_extensions.clone())
                    .unwrap_or_else(|| b".inc,.php".to_vec());
                self.default_autoload(&name, &extensions)?;
                Ok(Value::Null)
            }
            _ => unreachable!(),
        }
    }
}
