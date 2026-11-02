// Altered Rust adaptation of PHP 8.5.0 ext/standard/basic_functions.c
// and main/php_variables.c. Copyright (c) The PHP Group.
// See THIRD_PARTY_NOTICES.md for attribution and license.
use super::*;

type Entries = Vec<(Vec<u8>, Vec<u8>)>;

#[derive(Default)]
pub(super) struct Environment {
    initial: Option<Entries>,
    current: Entries,
}

impl Environment {
    fn initialize(&mut self) {
        if self.initial.is_none() {
            self.current = std::env::vars_os()
                .map(|(name, value)| {
                    (
                        name.as_encoded_bytes().to_vec(),
                        value.as_encoded_bytes().to_vec(),
                    )
                })
                .collect();
            self.initial = Some(self.current.clone());
        }
    }

    fn assign(&mut self, name: &[u8], value: Option<&[u8]>) {
        let position = self.current.iter().position(|(key, _)| key == name);
        match (position, value) {
            (Some(position), Some(value)) => self.current[position].1 = value.to_vec(),
            (Some(position), None) => {
                self.current.remove(position);
            }
            (None, Some(value)) => self.current.push((name.to_vec(), value.to_vec())),
            (None, None) => {}
        }
    }
}

impl Runtime {
    pub(crate) fn set_environment(&mut self, values: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>) {
        self.environment.initialize();
        for (name, value) in values {
            self.environment.assign(&name, Some(&value));
        }
        self.environment.initial = Some(self.environment.current.clone());
    }

    pub(super) fn environment_value(&mut self, name: &[u8]) -> Option<Vec<u8>> {
        self.environment.initialize();
        let name = name.split(|b| *b == 0).next().unwrap();
        self.environment
            .current
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    }

    pub(super) fn environment_builtin(
        &mut self,
        set: bool,
        name: &str,
        args: &[Option<Value>],
        line: usize,
    ) -> Result<Value, Error> {
        if set {
            let assignment = self.native_string(args[0].as_ref().unwrap(), name, 1, line)?;
            if assignment.is_empty() || assignment.starts_with(b"=") {
                return Err(Error::new(
                    ErrorKind::ValueError,
                    "putenv(): Argument #1 ($assignment) must have a valid syntax",
                    line,
                ));
            }
            let assignment = assignment.split(|b| *b == 0).next().unwrap();
            let mut parts = assignment.splitn(2, |b| *b == b'=');
            let key = parts.next().unwrap();
            let value = parts.next();
            self.environment.initialize();
            // PHP releases the preceding putenv entry before assigning it again.
            // Restoring the original also preserves native enumeration order.
            let original = self
                .environment
                .initial
                .as_ref()
                .unwrap()
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone());
            self.environment.assign(key, original.as_deref());
            if !key.is_empty() {
                self.environment.assign(key, value);
            }
            return Ok(Value::Bool(true));
        }
        let variable = args[0]
            .as_ref()
            .filter(|v| !matches!(v, Value::Null))
            .map(|v| self.native_string_type(v, name, 1, line, "?string"))
            .transpose()?;
        if let Some(local) = &args[1] {
            self.native_bool(local, name, 2, line)?;
        }
        // The CLI has no separate SAPI environment, so local_only uses this
        // same request environment. No host process globals are mutated.
        if let Some(variable) = variable {
            return Ok(self
                .environment_value(&variable)
                .map_or(Value::Bool(false), |v| Value::String(v.into())));
        }
        self.environment.initialize();
        let mut values = PhpArray::default();
        for (key, value) in self.environment.current.clone() {
            // Enumeration filters names that PHP's environment importer cannot
            // represent; direct getenv(name) still returns their values.
            if key.is_empty() || key.iter().any(|b| matches!(b, b' ' | b'.' | b'[')) {
                continue;
            }
            let key = ArrayKey::from_value(&Value::String(key.into()), line, self)?;
            values.insert(key, Value::String(value.into()));
        }
        Ok(Value::Array(Rc::new(values)))
    }
}
