use super::*;
use crate::{
    PhpObject,
    ast::{Argument, Attribute, NativeMethod, TraitAdaptation, Visibility},
    object::{Class, ClassContext},
};

#[derive(Clone)]
pub(crate) enum ReflectionData {
    Class(Rc<Class>),
    Attribute {
        owner: Rc<Class>,
        attribute: Attribute,
        repeated: bool,
    },
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
    pub(super) fn builtin_reflection_class(&self, name: &[u8]) -> Option<Rc<Class>> {
        let (canonical, parent_name, body) = match name {
            b"reflectionclass" => (
                "ReflectionClass",
                None,
                "
                public const int IS_IMPLICIT_ABSTRACT=16, IS_EXPLICIT_ABSTRACT=64,
                    IS_FINAL=32, IS_READONLY=65536;
                public readonly string $name;
                private function __clone(): void {}
                public function __construct(object|string $objectOrClass) {}
                public function getName(): string {}
                public function getShortName(): string {}
                public function getNamespaceName(): string {}
                public function inNamespace(): bool {}
                public function isInternal(): bool {}
                public function isUserDefined(): bool {}
                public function isAnonymous(): bool {}
                public function isInstantiable(): bool {}
                public function isCloneable(): bool {}
                public function isInterface(): bool {}
                public function isTrait(): bool {}
                public function isEnum(): bool {}
                public function isAbstract(): bool {}
                public function isFinal(): bool {}
                public function isReadOnly(): bool {}
                public function getModifiers(): int {}
                public function isInstance(object $object): bool {}
                public function getParentClass(): ReflectionClass|false {}
                public function isSubclassOf(ReflectionClass|string $class): bool {}
                public function implementsInterface(ReflectionClass|string $interface): bool {}
                public function getInterfaceNames(): array {}
                public function getInterfaces(): array {}
                public function getTraitNames(): array {}
                public function getTraits(): array {}
                public function getTraitAliases(): array {}
                public function hasMethod(string $name): bool {}
                public function hasProperty(string $name): bool {}
                public function hasConstant(string $name): bool {}
                public function getConstant(string $name): mixed {}
                public function getConstants(?int $filter=null): array {}
                public function getFileName(): string|false {}
                public function getStartLine(): int|false {}
                public function getEndLine(): int|false {}
                public function getStaticProperties(): array {}
                public function getStaticPropertyValue(string $name, mixed $default=null): mixed {}
                public function setStaticPropertyValue(string $name, mixed $value): void {}
                public function getDefaultProperties(): array {}
                public function getAttributes(?string $name=null, int $flags=0): array {}
                public function newInstance(mixed ...$args): object {}
                public function newInstanceArgs(array $args=[]): ?object {}
                public function newInstanceWithoutConstructor(): object {}
            ",
            ),
            b"reflectionobject" => (
                "ReflectionObject",
                Some("ReflectionClass"),
                "
                public function __construct(object $object) {}
            ",
            ),
            b"reflectionattribute" => (
                "ReflectionAttribute",
                None,
                "
                public const int IS_INSTANCEOF=2;
                private function __construct() {}
                private function __clone(): void {}
                public function getName(): string {}
                public function getTarget(): int {}
                public function isRepeated(): bool {}
                public function getArguments(): array {}
                public function newInstance(): object {}
            ",
            ),
            _ => return None,
        };
        let code = format!("<?php class {canonical} {{ {body} }}");
        let (mut statements, _, _) =
            parser::parse(lexer::lex(code.as_bytes()).expect("reflection source"))
                .expect("reflection declarations");
        let Stmt::Class(mut declaration) = statements.remove(0) else {
            unreachable!()
        };
        for method in &mut Rc::make_mut(&mut declaration).methods {
            Rc::make_mut(method).native = Some(NativeMethod::Reflection);
        }
        let parent = parent_name.and_then(|name| self.find_class(name.as_bytes()));
        let mut methods = parent.as_ref().map_or_else(Vec::new, |p| p.methods.clone());
        for own in &declaration.methods {
            let name = own.name.rsplit(|b| *b == b':').next().unwrap();
            methods.retain(|method| {
                !method
                    .name
                    .rsplit(|b| *b == b':')
                    .next()
                    .unwrap()
                    .eq_ignore_ascii_case(name)
            });
            methods.push(own.clone());
        }
        let mut properties = parent
            .as_ref()
            .map_or_else(Vec::new, |p| p.properties.clone());
        for property in &declaration.properties {
            let mut property = property.clone();
            property.declaring_class = declaration.name.clone();
            properties.push(property);
        }
        let class = Rc::new(Class {
            aliases: RefCell::default(),
            name: canonical.as_bytes().to_vec(),
            methods,
            declaration,
            properties,
            parent,
            interfaces: Vec::new(),
            static_properties: Vec::new(),
            allow_dynamic: false,
            forbid_dynamic: false,
            defaults: RefCell::default(),
            static_values: RefCell::default(),
            constant_values: RefCell::default(),
            evaluating_constants: RefCell::default(),
            initialized: std::cell::Cell::new(false),
            initializing: std::cell::Cell::new(false),
        });
        self.builtin_classes
            .borrow_mut()
            .insert(name.to_vec(), class.clone());
        Some(class)
    }

    fn reflection_object(&mut self, kind: &[u8], data: ReflectionData) -> Value {
        let class = self.find_class(kind).expect("reflection class");
        let mut properties = PhpArray::default();
        if let ReflectionData::Class(class) = &data {
            properties.insert(ArrayKey::String(b"name".to_vec()), string(&class.name));
        }
        self.next_object_id += 1;
        Value::Object(Rc::new(PhpObject {
            reflection: RefCell::new(Some(data)),
            throwable: RefCell::default(),
            class_name: String::from_utf8_lossy(&class.name).into_owned(),
            class,
            properties: RefCell::new(properties),
            unset_properties: RefCell::default(),
            reinitializable: RefCell::default(),
            object_id: self.next_object_id,
        }))
    }

    fn reflection_error(&mut self, message: impl Into<String>, line: usize) -> Error {
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
            let class = self.reflected_class(value, false, line)?;
            object
                .properties
                .borrow_mut()
                .insert(ArrayKey::String(b"name".to_vec()), string(&class.name));
            *object.reflection.borrow_mut() = Some(ReflectionData::Class(class));
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
            ReflectionData::Class(class) => {
                self.reflect_class_method(&class, &name, supplied, line)
            }
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
        name: &[u8],
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let decl = &class.declaration;
        let concrete = !decl.is_interface && !decl.is_trait && !decl.is_abstract;
        Ok(match name {
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
                            && !class.is_a(b"ReflectionAttribute"))),
            ),
            b"isinterface" => Value::Bool(decl.is_interface),
            b"istrait" => Value::Bool(decl.is_trait),
            b"isenum" => Value::Bool(false),
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
            b"hasmethod" => Value::Bool(
                objects::method(class, &self.lookup(b"name").unwrap().to_bytes()).is_some(),
            ),
            b"hasproperty" => {
                let name = self.lookup(b"name").unwrap().to_bytes();
                Value::Bool(
                    reflected_properties(class, false)
                        .iter()
                        .chain(reflected_properties(class, true).iter())
                        .any(|(_, property)| property.name == name),
                )
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
            b"getattributes" => return self.reflected_attributes(class, line),
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

    fn reflected_attributes(&mut self, owner: &Rc<Class>, line: usize) -> Result<Value, Error> {
        let name = self.lookup(b"name").unwrap();
        let flags = self.lookup(b"flags").unwrap();
        if !matches!(flags, Value::Int(0 | 2)) {
            return Err(Error::new(
                ErrorKind::ValueError,
                "ReflectionClass::getAttributes(): Argument #2 ($flags) must be a valid attribute filter flag",
                line,
            ));
        }
        let filter = match name {
            Value::String(name) => Some(name.to_vec()),
            _ => None,
        };
        if matches!(flags, Value::Int(2))
            && let Some(name) = &filter
            && self.load_class(name, line)?.is_none()
        {
            return Err(Error::new(
                ErrorKind::Error,
                format!("Class \"{}\" not found", String::from_utf8_lossy(name)),
                line,
            ));
        }
        let mut result = PhpArray::default();
        for attribute in &owner.declaration.attributes {
            if let Some(filter) = &filter {
                let matches = if matches!(flags, Value::Int(2)) {
                    self.load_class(&attribute.name, line)?
                        .is_some_and(|class| class.is_a(filter))
                } else {
                    attribute.name.eq_ignore_ascii_case(filter)
                };
                if !matches {
                    continue;
                }
            }
            let repeated = owner
                .declaration
                .attributes
                .iter()
                .filter(|other| other.name.eq_ignore_ascii_case(&attribute.name))
                .count()
                > 1;
            result.append(
                self.reflection_object(
                    b"ReflectionAttribute",
                    ReflectionData::Attribute {
                        owner: owner.clone(),
                        attribute: attribute.clone(),
                        repeated,
                    },
                ),
                line,
            )?;
        }
        Ok(Value::Array(Rc::new(result)))
    }

    fn attribute_arguments(
        &mut self,
        owner: &Rc<Class>,
        attribute: &Attribute,
    ) -> Result<PhpArray, Error> {
        let outer = self.class_context.replace(ClassContext {
            scope: owner.clone(),
            called: owner.clone(),
        });
        let source = std::mem::replace(&mut self.source, owner.declaration.source.clone());
        let strict = std::mem::replace(&mut self.strict_types, owner.declaration.strict_types);
        let result = (|| {
            let mut result = PhpArray::default();
            for argument in &attribute.arguments {
                match argument {
                    Argument::Positional(expr) => {
                        result.append(self.eval_default(expr)?, expr.line)?;
                    }
                    Argument::Named(name, expr) => {
                        result.insert(ArrayKey::String(name.clone()), self.eval_default(expr)?);
                    }
                    Argument::Unpack(_) => unreachable!("attribute unpacking rejected by parser"),
                }
            }
            Ok(result)
        })();
        self.class_context = outer;
        self.source = source;
        self.strict_types = strict;
        result
    }

    fn reflect_attribute_method(
        &mut self,
        owner: &Rc<Class>,
        attribute: &Attribute,
        repeated: bool,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        Ok(match name {
            b"getname" => string(&attribute.name),
            b"gettarget" => Value::Int(1),
            b"isrepeated" => Value::Bool(repeated),
            b"getarguments" => Value::Array(Rc::new(self.attribute_arguments(owner, attribute)?)),
            b"newinstance" => {
                let class = self.load_class(&attribute.name, line)?.ok_or_else(|| {
                    Error::new(
                        ErrorKind::Error,
                        format!(
                            "Attribute class \"{}\" not found",
                            String::from_utf8_lossy(&attribute.name)
                        ),
                        line,
                    )
                })?;
                let marker = class
                    .declaration
                    .attributes
                    .iter()
                    .find(|a| a.name.eq_ignore_ascii_case(b"Attribute"));
                let Some(marker) = marker else {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Attempting to use non-attribute class \"{}\" as attribute",
                            String::from_utf8_lossy(&attribute.name)
                        ),
                        line,
                    ));
                };
                // Zend validates the first marker argument without constructing
                // an Attribute object, coercing it, or evaluating extra arguments.
                let mut marker = marker.clone();
                marker.arguments.truncate(1);
                let marker_args = self.attribute_arguments(&class, &marker)?;
                let flags = marker_args
                    .entries()
                    .first()
                    .map_or(Value::Int(127), |(_, value)| value.dereferenced());
                let Value::Int(flags) = flags else {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Attribute::__construct(): Argument #1 ($flags) must be of type int, {} given",
                            builtins::debug_type(&flags)
                        ),
                        line,
                    ));
                };
                let flags = i64::from(flags as u32);
                if flags & !255 != 0 {
                    return Err(Error::new(
                        ErrorKind::Error,
                        "Invalid attribute flags specified",
                        line,
                    ));
                }
                if flags & 1 == 0 {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Attribute \"{}\" cannot target class (allowed targets: {})",
                            String::from_utf8_lossy(&attribute.name),
                            attribute_target_names(flags)
                        ),
                        line,
                    ));
                }
                let canonical = match class.name.as_slice() {
                    b"Attribute" => Some("Attribute"),
                    b"AllowDynamicProperties" => Some("AllowDynamicProperties"),
                    _ => None,
                };
                if let Some(canonical) = canonical {
                    let decl = &owner.declaration;
                    let invalid = if decl.is_trait {
                        Some("trait")
                    } else if decl.is_interface {
                        Some("interface")
                    } else if canonical == "Attribute" && decl.is_abstract {
                        Some("abstract class")
                    } else if canonical == "AllowDynamicProperties" && decl.is_readonly {
                        Some("readonly class")
                    } else {
                        None
                    };
                    if let Some(kind) = invalid {
                        return Err(Error::new(
                            ErrorKind::Error,
                            format!(
                                "Cannot apply #[\\{canonical}] to {kind} {}",
                                owner.display_name()
                            ),
                            line,
                        ));
                    }
                }
                if repeated && flags & 128 == 0 {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Attribute \"{}\" must not be repeated",
                            String::from_utf8_lossy(&attribute.name)
                        ),
                        line,
                    ));
                }
                let arguments = self.attribute_arguments(owner, attribute)?;
                let source = std::mem::replace(&mut self.source, owner.declaration.source.clone());
                let strict =
                    std::mem::replace(&mut self.strict_types, owner.declaration.strict_types);
                let result = self.instantiate_class(
                    class,
                    &[Argument::Unpack(literal(
                        Value::Array(Rc::new(arguments)),
                        attribute.line,
                    ))],
                    true,
                    attribute.line,
                );
                self.source = source;
                self.strict_types = strict;
                result?
            }
            _ => unreachable!("unhandled attribute reflection method"),
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

fn attribute_target_names(flags: i64) -> String {
    [
        (1, "class"),
        (2, "function"),
        (4, "method"),
        (8, "property"),
        (16, "class constant"),
        (32, "parameter"),
        (64, "constant"),
    ]
    .into_iter()
    .filter_map(|(bit, name)| (flags & bit != 0).then_some(name))
    .collect::<Vec<_>>()
    .join(", ")
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
