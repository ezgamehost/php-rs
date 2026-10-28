use super::*;
use crate::{
    PhpObject,
    ast::{Argument, Attribute, NativeMethod, TraitAdaptation, Visibility},
    object::{Class, ClassContext},
};

mod attributes;
mod constants;
mod declarations;
mod enums;
mod functions;
mod global_constants;
mod parameters;
mod properties;
use properties::PropertyReflection;

#[derive(Clone)]
pub(crate) enum ReflectionData {
    Class(Rc<Class>),
    ClassConstant {
        owner: Rc<Class>,
        name: Vec<u8>,
    },
    Object(Rc<PhpObject>),
    Constant(Rc<super::constants::GlobalConstant>),
    Extension(String),
    Property(PropertyReflection),
    Function(FunctionReflection),
    Parameter {
        function: FunctionReflection,
        index: usize,
    },
    Type(crate::ast::TypeDecl),
    Attribute {
        owner: AttributeContext,
        attribute: Attribute,
        repeated: bool,
    },
}

#[derive(Clone)]
pub(crate) struct FunctionReflection {
    target: super::functions::CallTarget,
    is_method: bool,
}

#[derive(Clone)]
pub(crate) struct AttributeContext {
    class: Option<Rc<Class>>,
    source: Rc<crate::source::Source>,
    strict_types: bool,
    target: i64,
}
impl AttributeContext {
    fn class(class: &Rc<Class>) -> Self {
        Self {
            class: Some(class.clone()),
            source: class.declaration.source.clone(),
            strict_types: class.declaration.strict_types,
            target: 1,
        }
    }
}

fn string(bytes: impl AsRef<[u8]>) -> Value {
    Value::String(bytes.as_ref().to_vec().into())
}

fn literal(value: Value, line: usize) -> Expr {
    Expr {
        kind: ExprKind::Literal(value),
        line,
    }
}

impl Runtime {
    fn reflection_object(&mut self, kind: &[u8], data: ReflectionData) -> Value {
        let kind = if kind == b"ReflectionClass"
            && matches!(&data, ReflectionData::Class(class) if class.declaration.is_enum)
        {
            b"ReflectionEnum".as_slice()
        } else {
            kind
        };
        let class = self.find_class(kind).expect("reflection class");
        let mut properties = PhpArray::default();
        match &data {
            ReflectionData::Constant(constant) => {
                properties.insert(ArrayKey::String(b"name".to_vec()), string(&constant.name));
            }
            ReflectionData::Extension(name) => {
                properties.insert(ArrayKey::String(b"name".to_vec()), string(name));
            }
            ReflectionData::Attribute { attribute, .. } => {
                properties.insert(ArrayKey::String(b"name".to_vec()), string(&attribute.name));
            }
            ReflectionData::Class(class) => {
                properties.insert(ArrayKey::String(b"name".to_vec()), string(&class.name));
            }
            ReflectionData::Object(object) => {
                properties.insert(
                    ArrayKey::String(b"name".to_vec()),
                    string(&object.class.name),
                );
            }
            ReflectionData::ClassConstant { owner, name } => {
                properties.insert(ArrayKey::String(b"name".to_vec()), string(name));
                properties.insert(ArrayKey::String(b"class".to_vec()), string(&owner.name));
            }
            ReflectionData::Property(property) => {
                properties.insert(ArrayKey::String(b"name".to_vec()), string(&property.name));
                properties.insert(
                    ArrayKey::String(b"class".to_vec()),
                    string(&property.owner.name),
                );
            }
            ReflectionData::Function(function) => {
                properties.insert(ArrayKey::String(b"name".to_vec()), string(function.name()));
                if function.is_method {
                    properties.insert(
                        ArrayKey::String(b"class".to_vec()),
                        string(&function.target.context.as_ref().unwrap().scope.name),
                    );
                }
            }
            ReflectionData::Parameter { function, index } => {
                properties.insert(
                    ArrayKey::String(b"name".to_vec()),
                    string(&function.parameters()[*index].name),
                );
            }
            _ => {}
        }
        let handle = self.allocate_object_handle();
        Value::Object(Rc::new(PhpObject {
            reflection: RefCell::new(Some(data)),
            throwable: RefCell::default(),
            class_name: String::from_utf8_lossy(&class.name).into_owned(),
            class,
            properties: RefCell::new(properties),
            unset_properties: RefCell::default(),
            reinitializable: RefCell::default(),
            object_id: handle.id,
            _handle: handle,
        }))
    }

    pub(super) fn reflection_error(&mut self, message: impl Into<String>, line: usize) -> Error {
        let args = [Argument::Positional(literal(string(message.into()), line))];
        let class = self.find_class(b"ReflectionException").unwrap();
        match self.instantiate_class(class, &args, true, line) {
            Ok(value) => self.throw_value(value, line),
            Err(error) => error,
        }
    }

    fn reflected_class(
        &mut self,
        value: Value,
        unwrap_reflection: bool,
        line: usize,
    ) -> Result<Rc<Class>, Error> {
        if let Value::Object(object) = &value {
            if unwrap_reflection {
                return match &*object.reflection.borrow() {
                    Some(ReflectionData::Class(class)) => Ok(class.clone()),
                    Some(ReflectionData::Object(object)) => Ok(object.class.clone()),
                    _ => Err(Error::new(
                        ErrorKind::Error,
                        "Internal error: Failed to retrieve the reflection object",
                        line,
                    )),
                };
            }
            return Ok(object.class.clone());
        }
        let name = value.to_bytes();
        self.load_class(&name, line)?.ok_or_else(|| {
            self.reflection_error(
                format!(
                    "Class \"{}\" does not exist",
                    String::from_utf8_lossy(&name)
                ),
                line,
            )
        })
    }

    pub(super) fn native_reflection_method(
        &mut self,
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        if self
            .current_function
            .as_ref()
            .is_some_and(|name| name.ends_with(b"::createfrommethodname"))
        {
            return self.reflection_method_factory(line);
        }
        let Some(Value::Object(object)) = self.lookup(b"this") else {
            unreachable!()
        };
        let name = self
            .current_function
            .as_ref()
            .unwrap()
            .rsplit(|b| *b == b':')
            .next()
            .unwrap()
            .to_vec();
        if name == b"__clone" {
            return Err(self.reflection_error("Cannot clone object using __clone()", line));
        }
        if name == b"__construct" {
            if object.class.is_a(b"ReflectionConstant") {
                return self.reflection_constant_constructor(&object, line);
            }
            if object.class.is_a(b"ReflectionExtension") {
                return self.reflection_extension_constructor(&object, line);
            }
            if object.class.is_a(b"ReflectionClassConstant") {
                return self.reflection_class_constant_constructor(&object, line);
            }
            if object.class.is_a(b"ReflectionProperty") {
                return self.reflection_property_constructor(&object, line);
            }
            if object.class.is_a(b"ReflectionParameter") {
                return self.reflection_parameter_constructor(&object, line);
            }
            if object.class.is_a(b"ReflectionFunctionAbstract") {
                return self.reflection_function_constructor(&object, supplied, line);
            }
            if self
                .current_function
                .as_ref()
                .unwrap()
                .starts_with(b"reflectionattribute::")
            {
                return Err(
                    self.reflection_error("Cannot directly instantiate ReflectionAttribute", line)
                );
            }
            let value = self
                .lookup(
                    if self
                        .current_function
                        .as_ref()
                        .unwrap()
                        .starts_with(b"reflectionobject::")
                    {
                        b"object"
                    } else {
                        b"objectOrClass"
                    },
                )
                .unwrap();
            let class = self.reflected_class(value.clone(), false, line)?;
            if object.class.is_a(b"ReflectionEnum") && !class.declaration.is_enum {
                return Err(self.reflection_error(
                    format!("Class \"{}\" is not an enum", class.display_name()),
                    line,
                ));
            }
            object
                .properties
                .borrow_mut()
                .insert(ArrayKey::String(b"name".to_vec()), string(&class.name));
            *object.reflection.borrow_mut() = Some(if object.class.is_a(b"ReflectionObject") {
                let Value::Object(target) = value else {
                    unreachable!()
                };
                ReflectionData::Object(target)
            } else {
                ReflectionData::Class(class)
            });
            return Ok(Value::Null);
        }
        let data = object.reflection.borrow().clone().ok_or_else(|| {
            Error::new(
                ErrorKind::Error,
                "Internal error: Failed to retrieve the reflection object",
                line,
            )
        })?;
        match data {
            ReflectionData::Constant(constant) => {
                self.reflect_constant_method(&constant, &name, line)
            }
            ReflectionData::Extension(extension) => {
                self.reflect_extension_method(&extension, &name)
            }
            ReflectionData::Class(class) => {
                self.reflect_class_method(&class, None, &name, supplied, line)
            }
            ReflectionData::Object(object) => {
                self.reflect_class_method(&object.class, Some(&object), &name, supplied, line)
            }
            ReflectionData::ClassConstant {
                owner,
                name: constant,
            } => self.reflect_class_constant_method(&object, &owner, &constant, &name, line),
            ReflectionData::Property(property) => {
                self.reflect_property_method(property, &name, supplied, line)
            }
            ReflectionData::Function(function) => {
                self.reflect_function_method(function, &name, line)
            }
            ReflectionData::Parameter { function, index } => {
                self.reflect_parameter_method(function, index, &name, line)
            }
            ReflectionData::Type(ty) => self.reflect_type_method(ty, &name, line),
            ReflectionData::Attribute {
                owner,
                attribute,
                repeated,
            } => self.reflect_attribute_method(&owner, &attribute, repeated, &name, line),
        }
    }

    fn reflect_class_method(
        &mut self,
        class: &Rc<Class>,
        object: Option<&Rc<PhpObject>>,
        name: &[u8],
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let decl = &class.declaration;
        let concrete = !decl.is_interface && !decl.is_trait && !decl.is_abstract && !decl.is_enum;
        Ok(match name {
            b"hascase" | b"getcase" | b"getcases" | b"isbacked" | b"getbackingtype" => {
                return self.reflect_enum_method(class, name, line);
            }
            b"getname" => string(&class.name),
            b"getshortname" => string(class.name.rsplit(|b| *b == b'\\').next().unwrap()),
            b"getnamespacename" => string(
                class
                    .name
                    .iter()
                    .rposition(|b| *b == b'\\')
                    .map_or(&[][..], |i| &class.name[..i]),
            ),
            b"innamespace" => Value::Bool(class.name.contains(&b'\\')),
            b"isinternal" | b"isuserdefined" => {
                let internal = self
                    .builtin_classes
                    .borrow()
                    .values()
                    .any(|builtin| Rc::ptr_eq(builtin, class));
                Value::Bool(internal == (name == b"isinternal"))
            }
            b"isanonymous" => Value::Bool(class.name.contains(&0)),
            b"isinstantiable" | b"iscloneable" => Value::Bool(
                concrete
                    && objects::method(
                        class,
                        if name == b"isinstantiable" {
                            b"__construct"
                        } else {
                            b"__clone"
                        },
                    )
                    .is_none_or(|method| method.visibility == Visibility::Public)
                    && (name != b"iscloneable"
                        || (!class.is_a(b"Throwable")
                            && !class.is_a(b"ReflectionClass")
                            && !class.is_a(b"ReflectionAttribute")
                            && !class.is_a(b"ReflectionFunctionAbstract")
                            && !class.is_a(b"ReflectionParameter")
                            && !class.is_a(b"ReflectionType")
                            && !class.is_a(b"ReflectionProperty")
                            && !class.is_a(b"ReflectionClassConstant")
                            && !class.is_a(b"ReflectionConstant")
                            && !class.is_a(b"ReflectionExtension"))),
            ),
            b"isinterface" => Value::Bool(decl.is_interface),
            b"istrait" => Value::Bool(decl.is_trait),
            b"isenum" => Value::Bool(decl.is_enum),
            b"isabstract" => Value::Bool(
                (decl.is_abstract && !decl.is_interface)
                    || class.methods.iter().any(|method| method.is_abstract),
            ),
            b"isfinal" => Value::Bool(decl.is_final),
            b"isreadonly" => Value::Bool(decl.is_readonly),
            b"getmodifiers" => Value::Int(
                (i64::from(decl.is_abstract && !decl.is_interface && !decl.is_trait) * 64)
                    | (i64::from(decl.is_final) * 32)
                    | (i64::from(decl.is_readonly) * 65536),
            ),
            b"isinstance" => Value::Bool(
                matches!(self.lookup(b"object"), Some(Value::Object(object)) if object.class.is_a(&class.name)),
            ),
            b"getparentclass" => match &class.parent {
                Some(parent) => self
                    .reflection_object(b"ReflectionClass", ReflectionData::Class(parent.clone())),
                None => Value::Bool(false),
            },
            b"issubclassof" | b"implementsinterface" => {
                let value = self
                    .lookup(if name == b"issubclassof" {
                        b"class"
                    } else {
                        b"interface"
                    })
                    .unwrap();
                let target = self.reflected_class(value, true, line)?;
                if name == b"implementsinterface" && !target.declaration.is_interface {
                    return Err(self.reflection_error(
                        format!("{} is not an interface", target.display_name()),
                        line,
                    ));
                }
                Value::Bool(
                    class.is_a(&target.name)
                        && (name == b"implementsinterface" || !Rc::ptr_eq(class, &target)),
                )
            }
            b"getinterfacenames" | b"getinterfaces" | b"gettraitnames" | b"gettraits" => {
                let mut classes = Vec::new();
                if name == b"getinterfacenames" || name == b"getinterfaces" {
                    collect_interfaces(class, &mut classes);
                } else {
                    for trait_name in decl.trait_uses.iter().flat_map(|use_| &use_.names) {
                        if let Some(trait_class) = self.find_class(trait_name)
                            && !classes.iter().any(|c| Rc::ptr_eq(c, &trait_class))
                        {
                            classes.push(trait_class);
                        }
                    }
                }
                let mut result = PhpArray::default();
                for class in classes {
                    if name == b"getinterfaces" || name == b"gettraits" {
                        result.insert(
                            ArrayKey::String(class.name.clone()),
                            self.reflection_object(
                                b"ReflectionClass",
                                ReflectionData::Class(class),
                            ),
                        );
                    } else {
                        result.append(string(&class.name), line)?;
                    }
                }
                Value::Array(Rc::new(result))
            }
            b"gettraitaliases" => {
                let mut result = PhpArray::default();
                for use_ in &decl.trait_uses {
                    for adaptation in &use_.adaptations {
                        if let TraitAdaptation::Alias {
                            source,
                            method,
                            alias: Some(alias),
                            ..
                        } = adaptation
                        {
                            let owner =
                                use_.names
                                    .iter()
                                    .filter_map(|n| self.find_class(n))
                                    .find(|c| {
                                        source
                                            .as_ref()
                                            .is_none_or(|n| c.name.eq_ignore_ascii_case(n))
                                            && objects::method(c, method).is_some()
                                    });
                            if let Some(owner) = owner {
                                let function = objects::method(&owner, method).unwrap();
                                let mut qualified = owner.name.clone();
                                qualified.extend(b"::");
                                qualified
                                    .extend(function.name.rsplit(|b| *b == b':').next().unwrap());
                                result.insert(ArrayKey::String(alias.clone()), string(qualified));
                            }
                        }
                    }
                }
                Value::Array(Rc::new(result))
            }
            b"getmethod" | b"getconstructor" => {
                let method_name = if name == b"getconstructor" {
                    b"__construct".to_vec()
                } else {
                    self.lookup(b"name").unwrap().to_bytes()
                };
                if name == b"getconstructor" && objects::method(class, &method_name).is_none() {
                    Value::Null
                } else {
                    let function = self.reflected_method(class, &method_name, line)?;
                    self.reflection_object(b"ReflectionMethod", ReflectionData::Function(function))
                }
            }
            b"getmethods" => {
                let mut result = PhpArray::default();
                let filter = self.lookup(b"filter").unwrap();
                let mut methods = class.declaration.methods.clone();
                for inherited in &class.methods {
                    let short = inherited.name.rsplit(|b| *b == b':').next().unwrap();
                    if inherited.visibility != Visibility::Private
                        && !methods.iter().any(|m| {
                            m.name
                                .rsplit(|b| *b == b':')
                                .next()
                                .unwrap()
                                .eq_ignore_ascii_case(short)
                        })
                    {
                        methods.push(inherited.clone());
                    }
                }
                for method in methods {
                    let short = method.name.rsplit(|b| *b == b':').next().unwrap();
                    let function = self.reflected_method(class, short, line)?;
                    if matches!(filter, Value::Int(mask) if mask & function.modifiers() == 0) {
                        continue;
                    }
                    result.append(
                        self.reflection_object(
                            b"ReflectionMethod",
                            ReflectionData::Function(function),
                        ),
                        line,
                    )?;
                }
                Value::Array(Rc::new(result))
            }
            b"hasmethod" => Value::Bool(
                objects::method(class, &self.lookup(b"name").unwrap().to_bytes()).is_some(),
            ),
            b"hasproperty" | b"getproperty" | b"getproperties" => {
                return self.reflect_class_properties(class, object, name, line);
            }
            b"getreflectionconstant" | b"getreflectionconstants" => {
                return self.reflect_class_constants(class, name, line);
            }
            b"hasconstant" => Value::Bool(
                class
                    .constant_owner(&self.lookup(b"name").unwrap().to_bytes())
                    .is_some(),
            ),
            b"getconstant" => {
                let name = self.lookup(b"name").unwrap().to_bytes();
                match class.constant_owner(&name) {
                    Some(owner) => self.evaluate_class_constant(&owner, &name, line)?,
                    None => {
                        self.diagnostics.push(format!("Deprecated: ReflectionClass::getConstant() for a non-existent constant is deprecated, use ReflectionClass::hasConstant() to check if the constant exists on line {line}"));
                        Value::Bool(false)
                    }
                }
            }
            b"getconstants" => {
                let filter = self.lookup(b"filter").unwrap();
                let mut result = PhpArray::default();
                for name in class.constant_names() {
                    let owner = class.constant_owner(&name).unwrap();
                    let constant = owner
                        .declaration
                        .constants
                        .iter()
                        .find(|c| c.name == name)
                        .unwrap();
                    let flags = visibility_flags(constant.visibility)
                        | if constant.is_final { 32 } else { 0 };
                    if matches!(filter, Value::Int(mask) if mask & flags == 0) {
                        continue;
                    }
                    let value = self.evaluate_class_constant(&owner, &name, line)?;
                    result.insert(ArrayKey::String(name), value);
                }
                Value::Array(Rc::new(result))
            }
            b"getfilename" | b"getstartline" | b"getendline" => {
                if self
                    .builtin_classes
                    .borrow()
                    .values()
                    .any(|c| Rc::ptr_eq(c, class))
                {
                    Value::Bool(false)
                } else {
                    match name {
                        b"getfilename" => string(&decl.source.name),
                        b"getstartline" => Value::Int(decl.line as i64),
                        _ => Value::Int(decl.end_line as i64),
                    }
                }
            }
            b"getdefaultproperties" | b"getstaticproperties" => {
                self.initialize_class(class)?;
                let mut result = PhpArray::default();
                for (owner, property) in reflected_properties(class, true) {
                    let value = if name == b"getstaticproperties" {
                        owner
                            .static_values
                            .borrow()
                            .get(&property.name)
                            .map(|cell| cell.borrow().dereferenced())
                    } else {
                        self.property_default(&owner, &property)?
                    };
                    if let Some(value) = value {
                        result.insert(ArrayKey::String(property.name), value);
                    }
                }
                if name == b"getdefaultproperties" {
                    for (owner, property) in reflected_properties(class, false) {
                        if let Some(value) = self.property_default(&owner, &property)? {
                            result.insert(ArrayKey::String(property.name), value);
                        }
                    }
                }
                Value::Array(Rc::new(result))
            }
            b"getstaticpropertyvalue" | b"setstaticpropertyvalue" => {
                let property_name = self.lookup(b"name").unwrap().to_bytes();
                let owner = reflected_properties(class, true)
                    .into_iter()
                    .find(|(_, p)| p.name == property_name)
                    .map(|(owner, _)| owner);
                let Some(owner) = owner else {
                    if name == b"getstaticpropertyvalue" && supplied > 1 {
                        return Ok(self.lookup(b"default").unwrap());
                    }
                    return Err(self.reflection_error(
                        if name == b"getstaticpropertyvalue" {
                            format!(
                                "Property {}::${} does not exist",
                                class.display_name(),
                                String::from_utf8_lossy(&property_name)
                            )
                        } else {
                            format!(
                                "Class {} does not have a property named {}",
                                class.display_name(),
                                String::from_utf8_lossy(&property_name)
                            )
                        },
                        line,
                    ));
                };
                self.initialize_class(class)?;
                if name == b"getstaticpropertyvalue" {
                    let value = owner
                        .static_values
                        .borrow()
                        .get(&property_name)
                        .map(|cell| cell.borrow().dereferenced());
                    value
                        .ok_or_else(|| Self::uninitialized_property(&owner, &property_name, line))?
                } else {
                    let value = self.lookup(b"value").unwrap();
                    let outer = self.class_context.replace(ClassContext {
                        scope: owner.clone(),
                        called: class.clone(),
                    });
                    let strict = std::mem::replace(&mut self.strict_types, false);
                    let result = self.write_static_location(
                        &mut Location {
                            static_class: Some(class.clone()),
                            object: None,
                            overloaded: None,
                            name: property_name,
                            global: false,
                            keys: Vec::new(),
                        },
                        value,
                        line,
                    );
                    self.class_context = outer;
                    self.strict_types = strict;
                    result?;
                    Value::Null
                }
            }
            b"getattributes" => {
                return self.reflected_attributes(
                    &class.declaration.attributes,
                    &AttributeContext::class(class),
                    "ReflectionClass",
                    line,
                );
            }
            b"newinstance" | b"newinstanceargs" | b"newinstancewithoutconstructor" => {
                let args = match self.lookup(b"args") {
                    Some(Value::Array(args)) => callbacks::callback_arguments(&args, line)?,
                    _ => Vec::new(),
                };
                if name != b"newinstancewithoutconstructor" {
                    if let Some(constructor) = objects::method(class, b"__construct") {
                        if constructor.visibility != Visibility::Public {
                            return Err(self.reflection_error(
                                format!(
                                    "Access to non-public constructor of class {}",
                                    class.display_name()
                                ),
                                line,
                            ));
                        }
                    } else if !args.is_empty() {
                        return Err(self.reflection_error(format!("Class {} does not have a constructor, so you cannot pass any constructor arguments", class.display_name()), line));
                    }
                }
                let object = self.instantiate_class(class.clone(), &[], false, line)?;
                if name != b"newinstancewithoutconstructor"
                    && let Some(constructor) = objects::method(class, b"__construct")
                {
                    let Value::Object(receiver) = &object else {
                        unreachable!()
                    };
                    let mut target = self.method_target(
                        constructor,
                        class.clone(),
                        Some(receiver.clone()),
                        line,
                    )?;
                    target.reference_values = true;
                    let strict = std::mem::replace(&mut self.strict_types, false);
                    let result = self.invoke_at(target, &args, line, true, true);
                    self.strict_types = strict;
                    result?;
                }
                return Ok(object);
            }
            _ => unreachable!("unhandled class reflection method"),
        })
    }
}

fn collect_interfaces(class: &Rc<Class>, result: &mut Vec<Rc<Class>>) {
    if let Some(parent) = &class.parent {
        collect_interfaces(parent, result);
    }
    let mut direct = Vec::new();
    for interface in &class.interfaces {
        if !result
            .iter()
            .any(|previous| Rc::ptr_eq(previous, interface))
        {
            result.push(interface.clone());
            direct.push(interface);
        }
    }
    // Zend appends each direct interface's already-linked ancestors in reverse.
    for interface in direct {
        let mut inherited = Vec::new();
        collect_interfaces(interface, &mut inherited);
        for ancestor in inherited.into_iter().rev() {
            if !result
                .iter()
                .any(|previous| Rc::ptr_eq(previous, &ancestor))
            {
                result.push(ancestor);
            }
        }
    }
}

fn visibility_flags(visibility: Visibility) -> i64 {
    match visibility {
        Visibility::Public => 1,
        Visibility::Protected => 2,
        Visibility::Private => 4,
    }
}

// Reflection lists declarations before inherited properties and hides private
// ancestor slots, unlike the physical object layout used by dumps/iteration.
fn reflected_properties(
    class: &Rc<Class>,
    is_static: bool,
) -> Vec<(Rc<Class>, crate::ast::PropertyDecl)> {
    let own = if is_static {
        &class.declaration.static_properties
    } else {
        &class.declaration.properties
    };
    let mut properties: Vec<_> = own.iter().map(|p| (class.clone(), p.clone())).collect();
    if let Some(parent) = &class.parent {
        for (owner, property) in reflected_properties(parent, is_static) {
            if property.visibility != Visibility::Private
                && !properties.iter().any(|(_, own)| own.name == property.name)
            {
                properties.push((owner, property));
            }
        }
    }
    properties
}
