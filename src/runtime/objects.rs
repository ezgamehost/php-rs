use super::*;
use crate::{
    PhpObject,
    ast::{Argument, ClassDecl},
    runtime::functions::CallTarget,
};

impl Runtime {
    pub(super) fn prepare_classes(&mut self, statements: &mut [Stmt]) -> Result<(), Error> {
        for statement in statements {
            match statement {
                Stmt::Class(class) => {
                    self.register_class(class.clone())?;
                    *statement = Stmt::Block(Vec::new());
                }
                Stmt::Block(body) => self.prepare_classes(body)?,
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn register_class(&mut self, class: Rc<ClassDecl>) -> Result<(), Error> {
        let name = class.name.to_ascii_lowercase();
        if self.classes.contains_key(&name) || name == b"stdclass" || name == b"closure" {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot declare class {}, because the name is already in use",
                    String::from_utf8_lossy(&class.name)
                ),
                class.line,
            ));
        }
        self.classes.insert(name, class);
        Ok(())
    }
    pub(super) fn find_class(&self, name: &[u8]) -> Option<Rc<ClassDecl>> {
        let name = name
            .strip_prefix(b"\\")
            .unwrap_or(name)
            .to_ascii_lowercase();
        if name == b"stdclass" {
            return Some(Rc::new(ClassDecl {
                name: b"stdClass".to_vec(),
                properties: Vec::new(),
                methods: Vec::new(),
                line: 0,
            }));
        }
        self.classes.get(&name).cloned()
    }
    pub(super) fn new_object(
        &mut self,
        class: &Expr,
        args: &[Argument],
        line: usize,
    ) -> Result<Value, Error> {
        let name = if let ExprKind::Constant(name) = &class.kind {
            name.clone()
        } else {
            match self.eval(class)? {
                Value::String(name) => name,
                Value::Object(object) => object.class.name.clone(),
                _ => {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Class name must be a valid object or a string",
                        line,
                    ));
                }
            }
        };
        let class = self.find_class(&name).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!("Class \"{}\" not found", String::from_utf8_lossy(&name)),
                line,
            )
        })?;
        let mut properties = PhpArray::default();
        for (name, default) in &class.properties {
            let value = if let Some(default) = default {
                self.eval(default)?
            } else {
                Value::Null
            };
            properties.insert(ArrayKey::String(name.clone()), value);
        }
        self.next_object_id += 1;
        let object = Rc::new(PhpObject {
            class_name: String::from_utf8_lossy(&class.name).into_owned(),
            class,
            properties: RefCell::new(properties),
            object_id: self.next_object_id,
        });
        if method(&object.class, b"__construct").is_some() {
            let target =
                self.resolve_method(Value::Object(object.clone()), b"__construct", line)?;
            self.invoke(target, args, line, false)?;
        } else {
            // PHP evaluates positional arguments even without a constructor.
            for arg in args {
                match arg {
                    Argument::Positional(value) => {
                        self.eval(value)?;
                    }
                    Argument::Named(name, value) => {
                        self.eval(value)?;
                        return Err(Error::new(
                            ErrorKind::Error,
                            format!("Unknown named parameter ${}", String::from_utf8_lossy(name)),
                            line,
                        ));
                    }
                    Argument::Unpack(value) => {
                        let Value::Array(array) = self.eval(value)? else {
                            return Err(Error::new(
                                ErrorKind::TypeError,
                                "Only arrays and Traversables can be unpacked",
                                line,
                            ));
                        };
                        if let Some((ArrayKey::String(name), _)) = array
                            .entries()
                            .iter()
                            .find(|(k, _)| matches!(k, ArrayKey::String(_)))
                        {
                            return Err(Error::new(
                                ErrorKind::Error,
                                format!(
                                    "Unknown named parameter ${}",
                                    String::from_utf8_lossy(name)
                                ),
                                line,
                            ));
                        }
                    }
                }
            }
        }
        Ok(Value::Object(object))
    }
    pub(super) fn resolve_method(
        &self,
        value: Value,
        name: &[u8],
        line: usize,
    ) -> Result<CallTarget, Error> {
        let Value::Object(object) = value else {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Call to a member function {}() on {}",
                    String::from_utf8_lossy(name),
                    builtins::debug_type(&value)
                ),
                line,
            ));
        };
        let function = method(&object.class, name).ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!(
                    "Call to undefined method {}::{}()",
                    object.class_name,
                    String::from_utf8_lossy(name)
                ),
                line,
            )
        })?;
        Ok(CallTarget {
            name: function.name.clone(),
            function: Some(function),
            builtin: None,
            closure: None,
            this: Some(object),
        })
    }
    pub(super) fn read_property_value(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        line: usize,
        silent: bool,
    ) -> Result<Value, Error> {
        if !silent {
            validate_property_name(name, line)?;
        }
        if let Some(value) = object
            .properties
            .borrow()
            .get(&ArrayKey::String(name.to_vec()))
        {
            return Ok(value.clone());
        }
        if !silent {
            self.warnings.push(format!(
                "Undefined property: {}::${} on line {line}",
                object.class_name,
                String::from_utf8_lossy(name)
            ));
        }
        Ok(Value::Null)
    }
    pub(super) fn write_property_value(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        value: Value,
        line: usize,
    ) -> Result<(), Error> {
        if name.first() == Some(&0) {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot access property starting with \"\\0\"",
                line,
            ));
        }
        let key = ArrayKey::String(name.to_vec());
        if object.properties.borrow().get(&key).is_none()
            && object.class.name != b"stdClass"
            && !object.class.properties.iter().any(|(n, _)| n == name)
        {
            self.warnings.push(format!(
                "Deprecated: Creation of dynamic property {}::${} is deprecated on line {line}",
                object.class_name,
                String::from_utf8_lossy(name)
            ));
        }
        object.properties.borrow_mut().insert(key, value);
        Ok(())
    }
}

pub(super) fn method(class: &ClassDecl, name: &[u8]) -> Option<Rc<FunctionDecl>> {
    class
        .methods
        .iter()
        .find(|m| {
            m.name
                .rsplit(|b| *b == b':')
                .next()
                .is_some_and(|n| n.eq_ignore_ascii_case(name))
        })
        .cloned()
}

impl Runtime {
    pub(super) fn unset_location(&mut self, expr: &Expr) -> Result<Option<Location>, Error> {
        match &expr.kind {
            ExprKind::Group(inner) => self.unset_location(inner),
            ExprKind::Property(base, name) => {
                let value = self.coalesce_read(base, true)?;
                let name = self.variable_name(name)?;
                Ok(if let Value::Object(object) = value {
                    validate_property_name(&name, expr.line)?;
                    Some(Location {
                        object: Some(object),
                        name,
                        global: false,
                        keys: Vec::new(),
                    })
                } else {
                    None
                })
            }
            ExprKind::Index(base, key) if !base.is_globals() => {
                let location = self.unset_location(base)?;
                let key = if let Some(key) = key {
                    Some(self.eval(key)?)
                } else {
                    None
                };
                Ok(location.map(|mut location| {
                    location.keys.push(Offset::new(key));
                    location
                }))
            }
            _ => self.location(expr).map(Some),
        }
    }

    pub(super) fn foreach_object(
        &mut self,
        object: &PhpObject,
        key_target: Option<&Expr>,
        value_target: &Expr,
        body: &Stmt,
    ) -> Result<Flow, Error> {
        let mut visited = HashSet::new();
        loop {
            let properties = object.snapshot();
            let Some((key, value)) = properties
                .entries()
                .iter()
                .find(|(key, _)| !visited.contains(key))
            else {
                break;
            };
            visited.insert(key.clone());
            self.assign_target(value_target, value.clone())?;
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

fn validate_property_name(name: &[u8], line: usize) -> Result<(), Error> {
    if name.first() == Some(&0) {
        return Err(Error::new(
            ErrorKind::Error,
            r#"Cannot access property starting with "\0""#,
            line,
        ));
    }
    Ok(())
}
