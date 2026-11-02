use super::*;
use crate::{
    PhpObject,
    ast::{Argument, ClassDecl, Visibility},
    object::Class,
    runtime::functions::CallTarget,
};

impl Runtime {
    pub(super) fn allocate_object_handle(&self) -> crate::object::ObjectHandle {
        crate::object::ObjectHandle::allocate(&self.object_store)
    }

    pub(super) fn prepare_classes(&mut self, statements: &mut [Stmt]) -> Result<(), Error> {
        let mut linked = Vec::new();
        // Early binding follows declaration order. An inherited class whose
        // parent is not linked yet stays as a runtime declaration.
        // Interface inheritance and implementation always resolve at execution.
        self.link_available_classes(statements, &mut linked)?;
        for class in linked {
            self.validate_trait_values(&class)?;
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
                    if decl.interfaces.is_empty()
                        && decl
                            .trait_uses
                            .iter()
                            .flat_map(|use_| &use_.names)
                            .all(|name| {
                                self.find_class(name).is_some_and(|class| {
                                    !class
                                        .declaration
                                        .attributes
                                        .iter()
                                        .any(|a| a.name.eq_ignore_ascii_case(b"Deprecated"))
                                })
                            })
                        && !decl
                            .methods
                            .iter()
                            .any(|m| method_name(m).eq_ignore_ascii_case(b"__tostring"))
                        && decl
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
        if let Some(parent) = &decl.parent {
            self.load_class(parent, decl.line)?;
        }
        for name in &decl.interfaces {
            self.load_class(name, decl.line)?;
        }
        for name in decl.trait_uses.iter().flat_map(|use_| &use_.names) {
            if let Some(class) = self.load_class(name, decl.line)?
                && class.declaration.is_trait
                && let Some(attribute) = class
                    .declaration
                    .attributes
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case(b"Deprecated"))
            {
                let mut message = b"Trait ".to_vec();
                message.extend(&class.name);
                message.extend(b" used by ");
                message.extend(&decl.name);
                message.extend(b" is deprecated");
                message.extend(self.diagnostic_attribute_suffix(
                    attribute,
                    Some(class.clone()),
                    decl.line,
                )?);
                self.attribute_diagnostic(&message, 16384, decl.line, false)?;
            }
        }
        let class = self.link_class(decl)?;
        let name = class.name.to_ascii_lowercase();
        self.classes.insert(name.clone(), class.clone());
        if let Err(error) = self
            .validate_trait_values(&class)
            .and_then(|_| self.validate_overrides(&class))
        {
            self.classes.remove(&name);
            return Err(error);
        }
        Ok(())
    }
    fn link_class(&mut self, decl: Rc<ClassDecl>) -> Result<Rc<Class>, Error> {
        self.build_class(decl.clone()).map_err(|error| {
            if error.kind != ErrorKind::CompileError {
                return error;
            }
            let mut fatal = self.compile_fatal_at(&error.message, &decl.source, error.line);
            // PHP cannot catch a class-linking compile failure. Keep its message
            // available to Rust callers while unwinding the request as a fatal.
            fatal.message = error.message;
            fatal
        })
    }

    fn build_class(&self, decl: Rc<ClassDecl>) -> Result<Rc<Class>, Error> {
        let decl = self.compose_traits(decl)?;
        let mut decl = self.prepare_enum(decl)?;
        // __toString implies Stringable in PHP, so its public visibility is
        // checked when the class is linked rather than when its body is parsed.
        if !decl.is_trait
            && decl.methods.iter().any(|m| {
                method_name(m).eq_ignore_ascii_case(b"__tostring")
                    && m.visibility != crate::ast::Visibility::Public
            })
        {
            return Err(Error::new(
                ErrorKind::Error,
                "Method __toString() must have public visibility",
                decl.line,
            ));
        }
        let name = decl.name.to_ascii_lowercase();
        if self.classes.contains_key(&name)
            || name == b"stdclass"
            || name == b"closure"
            || name == b"roundingmode"
            || name == b"propertyhooktype"
            || self.builtin_throwable_class(&name).is_some()
            || self.builtin_array_iterator_class(&name).is_some()
            || self.builtin_recursive_iterator_class(&name).is_some()
            || self.builtin_fixed_array_class(&name).is_some()
            || self.builtin_dual_iterator_class(&name).is_some()
            || self.builtin_directory_iterator_class(&name).is_some()
            || self.builtin_file_info_class(&name).is_some()
            || self.builtin_file_object_class(&name).is_some()
            || self.builtin_heap_class(&name).is_some()
            || self.builtin_object_storage_class(&name).is_some()
            || self.builtin_linked_list_class(&name).is_some()
        {
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
        self.validate_final_properties(&decl, parent.as_ref())?;
        self.link_property_hooks(Rc::make_mut(&mut decl), parent.as_ref())?;
        if parent.as_ref().is_some_and(|p| p.declaration.is_trait) {
            return Err(Error::new(
                ErrorKind::Error,
                "A class cannot extend a trait",
                decl.line,
            ));
        }
        if parent.as_ref().is_some_and(|p| p.declaration.is_interface) {
            return Err(Error::new(
                ErrorKind::Error,
                "A class cannot extend an interface",
                decl.line,
            ));
        }
        let mut interfaces = self.link_interfaces(&decl)?;
        if !decl.is_trait
            && decl
                .methods
                .iter()
                .any(|m| method_name(m).eq_ignore_ascii_case(b"__tostring"))
            && !interfaces.iter().any(|i| i.is_a(b"Stringable"))
            && !parent.as_ref().is_some_and(|p| p.is_a(b"Stringable"))
        {
            interfaces.push(
                self.find_class(b"Stringable")
                    .expect("Stringable interface"),
            );
        }
        if !decl.is_interface
            && interfaces.iter().any(|i| i.is_a(b"Throwable"))
            && !parent.as_ref().is_some_and(|p| p.is_a(b"Throwable"))
        {
            return Err(Error::new(
                ErrorKind::Error,
                "Class must extend Exception or Error to implement Throwable",
                decl.line,
            ));
        }
        if parent
            .as_ref()
            .is_some_and(|p| p.declaration.is_readonly != decl.is_readonly)
        {
            return Err(Error::new(
                ErrorKind::Error,
                "Readonly and non-readonly classes cannot extend each other",
                decl.line,
            ));
        }
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
                if let Some(owner) = parent.inheritable_constant_owner(&constant.name)
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
            if decl.properties.iter().any(|p| {
                parent.static_owner(&p.name).is_some_and(|owner| {
                    owner.declaration.static_properties.iter().any(|inherited| {
                        inherited.name == p.name && inherited.visibility != Visibility::Private
                    })
                })
            }) || decl.static_properties.iter().any(|p| {
                parent.properties.iter().any(|inherited| {
                    inherited.name == p.name && inherited.visibility != Visibility::Private
                })
            }) {
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
        static_properties.retain(|p| !decl.properties.iter().any(|own| own.name == p.name));
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
            let mut property = property.clone();
            property.declaring_class = decl.name.clone();
            if let Some(slot) = properties
                .iter_mut()
                .find(|p| p.name == property.name && p.visibility != Visibility::Private)
            {
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
        let allow_dynamic = decl
            .attributes
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(b"AllowDynamicProperties"))
            || parent.as_ref().is_some_and(|p| p.allow_dynamic);
        for interface in &interfaces {
            for function in &interface.methods {
                if !methods
                    .iter()
                    .any(|f| method_name(f).eq_ignore_ascii_case(method_name(function)))
                {
                    methods.push(function.clone());
                }
            }
        }
        let forbid_dynamic = decl.is_enum;
        Ok(Rc::new(Class {
            aliases: RefCell::default(),
            interfaces,
            name: decl.name.clone(),
            declaration: decl,
            parent,
            properties,
            methods,
            allow_dynamic,
            forbid_dynamic,
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
        if let Some(class) = self.builtin_classes.borrow().get(&name) {
            return Some(class.clone());
        }
        if let Some(class) = self.builtin_dual_iterator_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_directory_iterator_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_file_object_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_file_info_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_heap_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_object_storage_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_linked_list_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_fixed_array_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_recursive_iterator_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_array_iterator_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_reflection_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_attribute_class(&name) {
            return Some(class);
        }
        if let Some(class) = self.builtin_throwable_class(&name) {
            return Some(class);
        }
        if name == b"roundingmode" {
            return Some(self.rounding_mode_class());
        }
        if name == b"propertyhooktype" {
            return Some(self.native_enum_class(
                b"<?php enum PropertyHookType: string { case Get = 'get'; case Set = 'set'; }",
            ));
        }
        if name == b"stdclass" {
            let class = Rc::new(Class {
                aliases: RefCell::default(),
                interfaces: Vec::new(),
                name: b"stdClass".to_vec(),
                parent: None,
                properties: Vec::new(),
                methods: Vec::new(),
                allow_dynamic: true,
                forbid_dynamic: false,
                defaults: RefCell::new(None),
                static_properties: Vec::new(),
                static_values: RefCell::default(),
                constant_values: RefCell::default(),
                evaluating_constants: RefCell::default(),
                initialized: std::cell::Cell::new(false),
                initializing: std::cell::Cell::new(false),
                declaration: Rc::new(ClassDecl {
                    doc_comment: None,
                    source: Rc::default(),
                    strict_types: false,
                    end_line: 0,
                    attributes: Vec::new(),
                    is_trait: false,
                    is_enum: false,
                    enum_backing: None,
                    trait_uses: Vec::new(),
                    trait_requirements: Vec::new(),
                    trait_property_checks: Vec::new(),
                    trait_constant_checks: Vec::new(),
                    is_interface: false,
                    interfaces: Vec::new(),
                    is_abstract: false,
                    name: b"stdClass".to_vec(),
                    parent: None,
                    is_final: false,
                    is_readonly: false,
                    final_methods: Vec::new(),
                    properties: Vec::new(),
                    static_properties: Vec::new(),
                    constants: Vec::new(),
                    methods: Vec::new(),
                    line: 0,
                }),
            });
            self.builtin_classes
                .borrow_mut()
                .insert(name, class.clone());
            return Some(class);
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

        let name = if let Some(name) = class.static_class_name() {
            Some(self.contextual_class_name(name, line)?)
        } else {
            match self.eval(class)? {
                Value::String(name) => Some(name.to_vec()),
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
        let name = if let Some(name) = class.static_class_name() {
            self.contextual_class_name(name, line)?
        } else {
            match self.eval(class)? {
                Value::String(name) => name.to_vec(),
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
        let class = self.load_class(&name, line)?.ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                format!("Class \"{}\" not found", String::from_utf8_lossy(&name)),
                line,
            )
        })?;
        self.instantiate_class(class, args, true, line)
    }

    // Native factories may allocate a class without the instantiability and
    // constructor checks performed by PHP's `new` expression.
    pub(super) fn allocate_class_object(
        &mut self,
        class: Rc<Class>,
        line: usize,
    ) -> Result<Rc<PhpObject>, Error> {
        self.initialize_class(&class)?;
        let mut properties = self.property_defaults(&class)?;
        if class.is_a(b"Throwable") {
            for (name, value) in [
                (b"trace".as_slice(), Value::Array(self.capture_trace())),
                (b"file", Value::String(self.source.name.clone().into())),
                (b"line", Value::Int(line as i64)),
            ] {
                properties.insert(
                    super::throwable_properties::property_key(&class, name),
                    value,
                );
            }
        }
        let handle = self.allocate_object_handle();
        let object_id = handle.id;
        Ok(self.create_object(PhpObject {
            lazy: RefCell::default(),
            reflection: RefCell::default(),
            class_name: String::from_utf8_lossy(&class.name).into_owned(),
            class,
            properties: RefCell::new(properties),
            linked_list: RefCell::default(),
            object_storage: RefCell::default(),
            heap: RefCell::default(),
            file_info: RefCell::default(),
            dual_iterator: RefCell::default(),
            fixed_array: RefCell::default(),
            internal_iterator: RefCell::default(),
            recursive_iterator: RefCell::default(),
            array_iterator: RefCell::default(),
            properties_reordered: std::cell::Cell::new(false),
            property_cursor: RefCell::default(),
            property_iterators: RefCell::default(),
            unset_properties: RefCell::default(),
            reinitializable: RefCell::default(),
            object_id,
            _handle: handle,
        }))
    }

    pub(super) fn instantiate_class(
        &mut self,
        class: Rc<Class>,
        args: &[Argument],
        call_constructor: bool,
        line: usize,
    ) -> Result<Value, Error> {
        if class.declaration.is_enum {
            return Err(Error::new(
                ErrorKind::Error,
                format!("Cannot instantiate enum {}", class.display_name()),
                line,
            ));
        }
        if class.declaration.is_trait {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot instantiate trait {}",
                    String::from_utf8_lossy(&class.name)
                ),
                line,
            ));
        }
        if class.declaration.is_interface {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot instantiate interface {}",
                    String::from_utf8_lossy(&class.name)
                ),
                line,
            ));
        }
        if class.declaration.is_abstract {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot instantiate abstract class {}",
                    String::from_utf8_lossy(&class.name)
                ),
                line,
            ));
        }
        if call_constructor && let Some(constructor) = method(&class, b"__construct") {
            self.check_constructor_access(&constructor, line)?;
        }
        let object = self.allocate_class_object(class, line)?;
        if !call_constructor {
            return Ok(Value::Object(object));
        }
        if let Some(constructor) = method(&object.class, b"__construct") {
            let target = self.method_target(
                constructor,
                object.class.clone(),
                Some(object.clone()),
                line,
            )?;
            if let Err(error) = self.invoke(target, args, line, false) {
                object._handle.destructor_called.set(true);
                return Err(error);
            }
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
        &mut self,
        value: Value,
        name: &[u8],
        line: usize,
    ) -> Result<CallTarget, Error> {
        if matches!(value, Value::Closure(_)) && name.eq_ignore_ascii_case(b"__invoke") {
            return self.resolve_callable(value, line);
        }
        let Value::Object(object) = value else {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Call to a member function {}() on {}",
                    String::from_utf8_lossy(name),
                    match value {
                        Value::Bool(true) => "true",
                        Value::Bool(false) => "false",
                        _ => builtins::debug_type(&value),
                    }
                ),
                line,
            ));
        };
        if object.class.is_a(b"SplFileObject")
            && object
                .file_info
                .borrow()
                .as_ref()
                .is_none_or(|info| info.original_path.is_none())
        {
            return Err(Error::new(
                ErrorKind::Error,
                "The parent constructor was not called: the object is in an invalid state",
                line,
            ));
        }
        if object.class.name == b"__PHP_Incomplete_Class" {
            return Err(self.incomplete_object_error(&object, "call a method", line));
        }
        if let Some(data) = object.recursive_iterator.borrow().as_ref()
            && !data.initialized()
        {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "The {} instance wasn't initialized properly",
                    object.class_name
                ),
                line,
            ));
        }
        if object.class.is_a(b"GlobIterator")
            && directory_iterator::state(&object).borrow().handle.is_none()
        {
            return Err(Error::new(
                ErrorKind::Error,
                "The parent constructor was not called: the object is in an invalid state",
                line,
            ));
        }
        let function = self.instance_method(&object.class, name);
        if function
            .as_ref()
            .is_none_or(|f| self.check_method_access(f, line).is_err())
            && let Some(handler) = method(&object.class, b"__call")
        {
            return self.magic_method_target(
                handler,
                object.class.clone(),
                Some(object),
                name,
                line,
            );
        }
        if function.is_none() && object.recursive_iterator.borrow().is_some() {
            let inner = self.recursive_iterator_step_inner(&object, line)?;
            return self.resolve_method(Value::Object(inner), name, line);
        }
        if function.is_none()
            && let Some(inner) = self.dual_iterator_inner(&object)
        {
            let missing = format!(
                "Call to undefined method {}::{}()",
                inner.class_name,
                String::from_utf8_lossy(name)
            );
            match self.resolve_method(Value::Object(inner), name, line) {
                Err(error) if error.kind == ErrorKind::Error && error.message == missing => {}
                result => return result,
            }
        }
        let function = function.ok_or_else(|| {
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
    fn warn_static_as_instance(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        line: usize,
    ) -> Result<(), Error> {
        if self.instance_static_owner(&object.class, name).is_some() {
            self.warn(format!(
                "Notice: Accessing static property {}::${} as non static on line {line}",
                object.class_name,
                String::from_utf8_lossy(name)
            ))?;
        }
        Ok(())
    }
    pub(super) fn read_property_storage(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        line: usize,
        silent: bool,
    ) -> Result<Value, Error> {
        if object.class.name == b"__PHP_Incomplete_Class" {
            let error = self.incomplete_object_error(object, "access a property", line);
            let function = self.current_function.as_deref().unwrap_or(b"main");
            self.warn(format!(
                "Warning: {}(): {} on line {line}",
                String::from_utf8_lossy(function),
                error.message
            ))?;
            return Ok(Value::Null);
        }
        if !self.check_instance_property_access(object, name, silent, line)? {
            return Ok(Value::Null);
        }
        if let Some(owner) = self.instance_static_owner(&object.class, name)
            && !self.check_static_property_access(&object.class, &owner, name, line, silent)?
        {
            return Ok(Value::Null);
        }
        if !silent {
            validate_property_name(name, line)?;
            self.warn_static_as_instance(object, name, line)?;
        }
        let target = self.lazy_property_target(object, name, line)?;
        if !Rc::ptr_eq(&target, object) {
            return self.read_property_storage(&target, name, line, silent);
        }
        if let Some(value) = object
            .properties
            .borrow()
            .get(&self.instance_property_key(object, name))
        {
            return Ok(value.dereferenced());
        }
        if !silent {
            if let Some(owner) = self.instance_property_owner(&object.class, name)
                && owner
                    .declaration
                    .properties
                    .iter()
                    .any(|p| p.name == name && p.ty.is_some())
            {
                return Err(Self::uninitialized_property(&owner, name, line));
            }
            self.warn(format!(
                "Undefined property: {}::${} on line {line}",
                object.class_name,
                String::from_utf8_lossy(name)
            ))?;
        }
        Ok(Value::Null)
    }
    pub(super) fn prepare_property_write(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        line: usize,
    ) -> Result<ArrayKey, Error> {
        if object.class.name == b"__PHP_Incomplete_Class" {
            return Err(self.incomplete_object_error(object, "modify a property", line));
        }
        if matches!(name, b"name" | b"class")
            && (object.class.is_a(b"ReflectionClass")
                || object.class.is_a(b"ReflectionFunctionAbstract")
                || object.class.is_a(b"ReflectionParameter")
                || object.class.is_a(b"ReflectionProperty")
                || object.class.is_a(b"ReflectionClassConstant")
                || object.class.is_a(b"ReflectionConstant")
                || object.class.is_a(b"ReflectionExtension")
                || object.class.is_a(b"ReflectionAttribute")
                || object.class.is_a(b"ReflectionType"))
            && object.class.properties.iter().any(|p| p.name == name)
        {
            return Err(self.reflection_error(
                format!(
                    "Cannot set read-only property {}::${}",
                    object.class_name,
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        if name.first() == Some(&0) {
            return Err(Error::new(
                ErrorKind::Error,
                "Cannot access property starting with \"\\0\"",
                line,
            ));
        }
        if let Some(owner) = self.instance_static_owner(&object.class, name) {
            self.check_static_property_access(&object.class, &owner, name, line, false)?;
        }
        self.warn_static_as_instance(object, name, line)?;
        let key = self.instance_property_key(object, name);
        if (object.class.declaration.is_readonly || object.class.forbid_dynamic)
            && self.instance_property_owner(&object.class, name).is_none()
        {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot create dynamic property {}::${}",
                    object.class_name,
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        if object.properties.borrow().get(&key).is_none()
            && !object.class.allow_dynamic
            && self.instance_property_owner(&object.class, name).is_none()
        {
            self.warn(format!(
                "Deprecated: Creation of dynamic property {}::${} is deprecated on line {line}",
                object.class_name,
                String::from_utf8_lossy(name)
            ))?;
        }
        Ok(key)
    }

    pub(super) fn write_property_value(
        &mut self,
        object: &Rc<PhpObject>,
        name: &[u8],
        value: Value,
        line: usize,
    ) -> Result<Value, Error> {
        if self.write_property_hook(object, name, &value, line)? {
            return Ok(value);
        }
        let key = self.prepare_property_write(object, name, line)?;
        let value = value.dereferenced();
        let value = if let Some(owner) = self.instance_property_owner(&object.class, name) {
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
        if let ArrayKey::String(key) = &key {
            object.reinitializable.borrow_mut().remove(key);
            object.unset_properties.borrow_mut().remove(key);
        }
        let reference = match object.properties.borrow().get(&key) {
            Some(Value::Reference(reference)) => Some(reference.clone()),
            _ => None,
        };
        if let Some(reference) = reference {
            return self.write_reference_value(&reference, value, self.strict_types, line);
        } else {
            object.properties.borrow_mut().insert(key, value.clone());
        }
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
    pub(super) fn prepare_property_unset(
        &mut self,
        object: &PhpObject,
        name: &[u8],
        line: usize,
    ) -> Result<(), Error> {
        if object.class.name == b"__PHP_Incomplete_Class" {
            return Err(self.incomplete_object_error(object, "modify a property", line));
        }
        if self.property_hooks(object, name).is_some() {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Cannot unset hooked property {}::${}",
                    object.class_name,
                    String::from_utf8_lossy(name)
                ),
                line,
            ));
        }
        validate_property_name(name, line)?;
        if let Some(owner) = self.instance_static_owner(&object.class, name) {
            self.check_static_property_access(&object.class, &owner, name, line, false)?;
        }
        self.warn_static_as_instance(object, name, line)?;
        Ok(())
    }

    pub(super) fn unset_location(&mut self, expr: &Expr) -> Result<Option<Location>, Error> {
        match &expr.kind {
            ExprKind::Group(inner) => self.unset_location(inner),
            ExprKind::Property(base, name) => {
                let value = self.property_write_base(base, true)?;
                let name = self.variable_name(name)?;
                Ok(if let Value::Object(object) = value {
                    Some(Location {
                        offset_reference: None,
                        overloaded: None,
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
        object: &Rc<PhpObject>,
        key_target: Option<&Expr>,
        value_target: &Expr,
        by_reference: bool,
        body: &Stmt,
    ) -> Result<Flow, Error> {
        let initialized = self.initialize_lazy_object(object, value_target.line)?;
        let object = &initialized;
        if self.has_property_hooks(object) {
            return self.foreach_hooked_object(
                object,
                key_target,
                value_target,
                by_reference,
                body,
            );
        }
        let cursor = Rc::new(RefCell::new(crate::object::PropertyCursor::default()));
        {
            let mut iterators = object.property_iterators.borrow_mut();
            iterators.retain(|iterator| iterator.strong_count() > 0);
            iterators.push(Rc::downgrade(&cursor));
        }
        let mut visited = HashSet::new();
        loop {
            let properties = object.snapshot();
            let Some((key, value)) = properties
                .entries()
                .iter()
                .find(|(key, _)| !visited.contains(key) && self.property_key_visible(object, key))
                .cloned()
            else {
                break;
            };
            let key = key.clone();
            let value = value.dereferenced();
            // Iteration retains the object handle, not a copy of its property
            // table: temporary snapshots must not prolong reference sharing.
            drop(properties);
            visited.insert(key.clone());
            if by_reference {
                let reference =
                    self.iterated_property_reference(object, &key, value_target.line)?;
                if matches!(value_target.kind, ExprKind::Destructure(_)) {
                    self.destructure_reference(value_target, reference)?;
                } else {
                    let mut target = self.location(value_target)?;
                    self.bind_reference_location(&mut target, reference, value_target.line)?;
                }
            } else {
                self.assign_target(value_target, value)?;
            }
            if let Some(target) = key_target {
                let raw = key.to_value().to_bytes();
                let name = match crate::object::property_name(&raw) {
                    Ok(name) => name.name,
                    Err(message) => {
                        self.warn(format!("Notice: {message} on line {}", target.line))?;
                        &raw
                    }
                };
                self.assign_target(target, Value::String(name.to_vec().into()))?;
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

impl Runtime {
    pub(super) fn incomplete_object_error(
        &self,
        object: &PhpObject,
        action: &str,
        line: usize,
    ) -> Error {
        let name = object
            .properties
            .borrow()
            .get(&ArrayKey::String(b"__PHP_Incomplete_Class_Name".to_vec()));
        let name = match name {
            Some(Value::String(name)) => name.to_vec(),
            _ => b"unknown".to_vec(),
        };
        Error::new(
            ErrorKind::Error,
            format!(
                "The script tried to {action} on an incomplete object. Please ensure that the class definition \"{}\" of the object you are trying to operate on was loaded _before_ unserialize() gets called or provide an autoloader to load the class definition",
                String::from_utf8_lossy(&name)
            ),
            line,
        )
    }

    // object_properties_load bypasses normal write hooks and type coercion.
    // It still resolves mangled visibility and protects initialized readonly slots.
    pub(super) fn load_native_object_member(
        &mut self,
        object: &PhpObject,
        key: ArrayKey,
        value: Value,
        line: usize,
    ) -> Result<(), Error> {
        let ArrayKey::String(raw) = &key else {
            if !object.class.allow_dynamic {
                self.warn(format!(
                    "Deprecated: Creation of dynamic property {}::${} is deprecated on line {line}",
                    object.class_name,
                    String::from_utf8_lossy(&key.to_value().to_bytes())
                ))?;
            }
            object.properties.borrow_mut().insert(key, value);
            return self.drain_destructors(Ok(()), line);
        };
        let decoded = crate::object::property_name(raw);
        let previous = self.class_context.clone();
        if let Ok(decoded) = &decoded
            && let Some(scope) = decoded.scope.filter(|scope| *scope != b"*")
        {
            self.class_context = self
                .find_class(scope)
                .map(|scope| crate::object::ClassContext {
                    scope: scope.clone(),
                    called: scope,
                });
        }
        let owner = decoded.as_ref().ok().and_then(|decoded| {
            self.instance_property_owner(&object.class, decoded.name)
                .filter(|owner| {
                    let property = owner
                        .declaration
                        .properties
                        .iter()
                        .find(|p| p.name == decoded.name)
                        .unwrap();
                    self.can_access_instance_visibility(owner, decoded.name, property.visibility)
                })
        });
        self.class_context = previous;
        let stored_key = if let Some(owner) = owner {
            let decoded = decoded.as_ref().unwrap();
            let property = owner
                .declaration
                .properties
                .iter()
                .find(|p| p.name == decoded.name)
                .unwrap();
            let storage = property.storage_name_for(&owner.name);
            let key = ArrayKey::String(storage.clone());
            if property.is_readonly
                && object.properties.borrow().get(&key).is_some()
                && !object.reinitializable.borrow_mut().remove(&storage)
            {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Cannot modify readonly property {}::${}",
                        owner.display_name(),
                        String::from_utf8_lossy(&property.name)
                    ),
                    line,
                ));
            }
            object.unset_properties.borrow_mut().remove(&storage);
            key
        } else {
            let name = decoded
                .as_ref()
                .map_or(b"".as_slice(), |decoded| decoded.name);
            if object.class.forbid_dynamic {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Cannot create dynamic property {}::${}",
                        object.class_name,
                        String::from_utf8_lossy(name)
                    ),
                    line,
                ));
            }
            if !object.class.allow_dynamic {
                self.warn(format!(
                    "Deprecated: Creation of dynamic property {}::${} is deprecated on line {line}",
                    object.class_name,
                    String::from_utf8_lossy(name)
                ))?;
            }
            key
        };
        object.properties.borrow_mut().insert(stored_key, value);
        self.drain_destructors(Ok(()), line)
    }
}
