use super::*;
use crate::{
    PhpObject,
    ast::{Argument, ClassDecl},
    object::Class,
    runtime::functions::CallTarget,
};

impl Runtime {
    pub(super) fn prepare_classes(&mut self, statements: &mut [Stmt]) -> Result<(), Error> {
        let mut linked = Vec::new();
        // Early binding follows declaration order. An inherited class whose
        // parent is not linked yet stays as a runtime declaration.
        self.link_available_classes(statements, &mut linked)?;
        for class in linked {
            self.validate_overrides(&class)?;
        }
        Ok(())
    }
    fn link_available_classes(
        &mut self,
        statements: &mut [Stmt],
        linked: &mut Vec<Rc<Class>>,
    ) -> Result<(), Error> {
        for statement in statements {
            match statement {
                Stmt::Class(decl)
                    if decl
                        .parent
                        .as_ref()
                        .is_none_or(|name| self.find_class(name).is_some()) =>
                {
                    let class = self.link_class(decl.clone())?;
                    if self.property_variance_unresolved(&class) {
                        continue;
                    }
                    self.classes
                        .insert(class.name.to_ascii_lowercase(), class.clone());
                    linked.push(class);
                    *statement = Stmt::Block(Vec::new());
                }
                Stmt::Block(body) => self.link_available_classes(body, linked)?,
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn register_class(&mut self, decl: Rc<ClassDecl>) -> Result<(), Error> {
        let class = self.link_class(decl)?;
        self.validate_overrides(&class)?;
        self.classes.insert(class.name.to_ascii_lowercase(), class);
        Ok(())
    }
    fn link_class(&self, decl: Rc<ClassDecl>) -> Result<Rc<Class>, Error> {
        let name = decl.name.to_ascii_lowercase();
        if self.classes.contains_key(&name) || name == b"stdclass" || name == b"closure" {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot declare class {}, because the name is already in use",
                    String::from_utf8_lossy(&decl.name)
                ),
                decl.line,
            ));
        }
        let parent = if let Some(name) = &decl.parent {
            Some(self.find_class(name).ok_or_else(|| {
                Error::new(
                    ErrorKind::Error,
                    format!("Class \"{}\" not found", String::from_utf8_lossy(name)),
                    decl.line,
                )
            })?)
        } else {
            None
        };
        if parent.as_ref().is_some_and(|p| p.declaration.is_final) {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Class {} cannot extend final class {}",
                    String::from_utf8_lossy(&decl.name),
                    String::from_utf8_lossy(&parent.as_ref().unwrap().name)
                ),
                decl.line,
            ));
        }
        if let Some(parent) = &parent {
            for constant in &decl.constants {
                if let Some(owner) = parent.constant_owner(&constant.name)
                    && owner
                        .declaration
                        .constants
                        .iter()
                        .any(|c| c.name == constant.name && c.is_final)
                {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Cannot override final class constant",
                        decl.line,
                    ));
                }
            }
            if decl
                .properties
                .iter()
                .any(|p| parent.static_owner(&p.name).is_some())
                || decl.static_properties.iter().any(|p| {
                    parent
                        .properties
                        .iter()
                        .any(|inherited| inherited.name == p.name)
                })
            {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot change property staticness",
                    decl.line,
                ));
            }
        }
        let mut static_properties = parent
            .as_ref()
            .map_or_else(Vec::new, |p| p.static_properties.clone());
        for property in &decl.static_properties {
            if let Some(slot) = static_properties
                .iter_mut()
                .find(|p| p.name == property.name)
            {
                *slot = property.clone();
            } else {
                static_properties.push(property.clone());
            }
        }
        let mut properties = parent
            .as_ref()
            .map_or_else(Vec::new, |p| p.properties.clone());
        for property in &decl.properties {
            if let Some(slot) = properties.iter_mut().find(|p| p.name == property.name) {
                *slot = property.clone();
            } else {
                properties.push(property.clone());
            }
        }
        let mut methods = parent.as_ref().map_or_else(Vec::new, |p| p.methods.clone());
        for function in &decl.methods {
            if let Some(slot) = methods
                .iter_mut()
                .find(|f| method_name(f).eq_ignore_ascii_case(method_name(function)))
            {
                *slot = function.clone();
            } else {
                methods.push(function.clone());
            }
        }
        let allow_dynamic = parent.as_ref().is_some_and(|p| p.allow_dynamic);
        Ok(Rc::new(Class {
            name: decl.name.clone(),
            declaration: decl,
            parent,
            properties,
            methods,
            allow_dynamic,
            defaults: RefCell::new(None),
            static_properties,
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        }))
    }
    pub(super) fn find_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let name = name
            .strip_prefix(b"\\")
            .unwrap_or(name)
            .to_ascii_lowercase();
        if name == b"stdclass" {
            return Some(Rc::new(Class {
                name: b"stdClass".to_vec(),
                parent: None,
                properties: Vec::new(),
                methods: Vec::new(),
                allow_dynamic: true,
                defaults: RefCell::new(None),
                static_properties: Vec::new(),
                static_values: RefCell::default(),
                constant_values: RefCell::default(),
                evaluating_constants: RefCell::default(),
                initialized: std::cell::Cell::new(false),
                initializing: std::cell::Cell::new(false),
                declaration: Rc::new(ClassDecl {
                    name: b"stdClass".to_vec(),
                    parent: None,
                    is_final: false,
                    final_methods: Vec::new(),
                    properties: Vec::new(),
                    static_properties: Vec::new(),
                    constants: Vec::new(),
                    methods: Vec::new(),
                    line: 0,
                }),
            }));
        }
        self.classes.get(&name).cloned()
    }
    pub(super) fn instance_of(
        &mut self,
        value: &Expr,
        class: &Expr,
        line: usize,
    ) -> Result<Value, Error> {
        let value = self.eval(value)?;
        if !matches!(value, Value::Object(_) | Value::Closure(_)) {
            return Ok(Value::Bool(false));
        }

        let name = if let ExprKind::Constant(name) = &class.kind {
            Some(self.contextual_class_name(name, line)?)
        } else {
            match self.eval(class)? {
                Value::String(name) => Some(name),
                Value::Object(object) => Some(object.class.name.clone()),
                Value::Closure(_) => Some(b"Closure".to_vec()),
                _ => None,
            }
        };
        let name = name.ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                "Class name must be a valid object or a string",
                line,
            )
        })?;
        Ok(Value::Bool(match value {
            Value::Object(object) => object.class.is_a(&name),
            Value::Closure(_) => name
                .strip_prefix(b"\\")
                .unwrap_or(&name)
                .eq_ignore_ascii_case(b"Closure"),
            _ => false,
        }))
    }
    pub(super) fn new_object(
        &mut self,
        class: &Expr,
        args: &[Argument],
        line: usize,
    ) -> Result<Value, Error> {
        let name = if let ExprKind::Constant(name) = &class.kind {
            self.contextual_class_name(name, line)?
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
        self.initialize_class(&class)?;
        let properties = self.property_defaults(&class)?;
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
        self.method_target(function, object.class.clone(), Some(object), line)
    }
    fn warn_static_as_instance(&mut self, object: &PhpObject, name: &[u8], line: usize) {
        if object.class.static_owner(name).is_some() {
            self.warnings.push(format!(
                "Notice: Accessing static property {}::${} as non static on line {line}",
                object.class_name,
                String::from_utf8_lossy(name)
            ));
        }
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
            self.warn_static_as_instance(object, name, line);
        }
        if let Some(value) = object
            .properties
            .borrow()
            .get(&ArrayKey::String(name.to_vec()))
        {
            return Ok(value.clone());
        }
        if !silent {
            if let Some(owner) = object.class.property_owner(name)
                && owner
                    .declaration
                    .properties
                    .iter()
                    .any(|p| p.name == name && p.ty.is_some())
            {
                return Err(Self::uninitialized_property(&owner, name, line));
            }
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
    ) -> Result<Value, Error> {
        if name.first() == Some(&0) {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot access property starting with \"\\0\"",
                line,
            ));
        }
        self.warn_static_as_instance(object, name, line);
        let key = ArrayKey::String(name.to_vec());
        if object.properties.borrow().get(&key).is_none()
            && !object.class.allow_dynamic
            && !object.class.properties.iter().any(|p| p.name == name)
        {
            self.warnings.push(format!(
                "Deprecated: Creation of dynamic property {}::${} is deprecated on line {line}",
                object.class_name,
                String::from_utf8_lossy(name)
            ));
        }
        let value = if let Some(owner) = object.class.property_owner(name) {
            let property = owner
                .declaration
                .properties
                .iter()
                .find(|p| p.name == name)
                .expect("property owner");
            self.check_property_value(&owner, property, value, self.strict_types, line)?
        } else {
            value
        };
        object.properties.borrow_mut().insert(key, value.clone());
        Ok(value)
    }
}

pub(super) fn method(class: &Class, name: &[u8]) -> Option<Rc<FunctionDecl>> {
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
                    self.warn_static_as_instance(&object, &name, expr.line);
                    Some(Location {
                        static_class: None,
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

fn method_name(function: &FunctionDecl) -> &[u8] {
    function.name.rsplit(|b| *b == b':').next().unwrap()
}
