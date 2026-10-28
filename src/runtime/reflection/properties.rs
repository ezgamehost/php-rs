use super::*;
use crate::ast::PropertyDecl;

#[derive(Clone)]
pub(crate) struct PropertyReflection {
    pub(super) scope: Rc<Class>,
    pub(super) owner: Rc<Class>,
    pub(super) name: Vec<u8>,
    declaration: Option<PropertyDecl>,
    is_static: bool,
}

impl PropertyReflection {
    fn modifiers(&self) -> i64 {
        let Some(property) = &self.declaration else {
            return 1;
        };
        visibility_flags(property.visibility)
            | (i64::from(self.is_static) * 16)
            | (i64::from(property.is_final) * 32)
            | (i64::from(property.is_readonly) * 128)
            | if property.set_visibility != property.visibility {
                match property.set_visibility {
                    Visibility::Protected => 2048,
                    Visibility::Private => 4096,
                    Visibility::Public => 0,
                }
            } else {
                0
            }
    }
}

// Both storage kinds share a declaration-order namespace in PHP reflection.
fn properties(class: &Rc<Class>) -> Vec<PropertyReflection> {
    let mut own: Vec<_> = class
        .declaration
        .properties
        .iter()
        .map(|p| (p, false))
        .chain(
            class
                .declaration
                .static_properties
                .iter()
                .map(|p| (p, true)),
        )
        .collect();
    own.sort_by_key(|(p, _)| p.order);
    let mut result: Vec<_> = own
        .into_iter()
        .map(|(p, is_static)| PropertyReflection {
            scope: class.clone(),
            owner: class.clone(),
            name: p.name.clone(),
            declaration: Some(p.clone()),
            is_static,
        })
        .collect();
    if let Some(parent) = &class.parent {
        for mut property in properties(parent) {
            if property.declaration.as_ref().unwrap().visibility != Visibility::Private
                && !result.iter().any(|own| own.name == property.name)
            {
                property.scope = class.clone();
                result.push(property);
            }
        }
    }
    result
}

impl Runtime {
    fn reflected_property(
        &self,
        class: &Rc<Class>,
        object: Option<&Rc<PhpObject>>,
        name: &[u8],
    ) -> Option<PropertyReflection> {
        properties(class)
            .into_iter()
            .find(|p| p.name == name)
            .or_else(|| {
                // Private ancestor slots are declarations, even when absent from
                // the public reflection table; they must not become dynamic slots.
                if class
                    .properties
                    .iter()
                    .chain(&class.static_properties)
                    .any(|p| p.name == name)
                {
                    return None;
                }
                object
                    .filter(|o| {
                        o.properties
                            .borrow()
                            .get(&ArrayKey::String(name.to_vec()))
                            .is_some()
                    })
                    .map(|_| PropertyReflection {
                        scope: class.clone(),
                        owner: class.clone(),
                        name: name.to_vec(),
                        declaration: None,
                        is_static: false,
                    })
            })
    }

    fn missing_reflected_property(&mut self, class: &Class, name: &[u8], line: usize) -> Error {
        self.reflection_error(
            format!(
                "Property {}::${} does not exist",
                class.display_name(),
                String::from_utf8_lossy(name)
            ),
            line,
        )
    }

    pub(super) fn reflection_property_constructor(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let value = self.lookup(b"class").unwrap();
        let class = self.reflected_class(value.clone(), false, line)?;
        let receiver = match &value {
            Value::Object(object) => Some(object),
            _ => None,
        };
        let name = self.lookup(b"property").unwrap().to_bytes();
        let property = self
            .reflected_property(&class, receiver, &name)
            .ok_or_else(|| self.missing_reflected_property(&class, &name, line))?;
        object
            .properties
            .borrow_mut()
            .insert(ArrayKey::String(b"name".to_vec()), string(&property.name));
        object.properties.borrow_mut().insert(
            ArrayKey::String(b"class".to_vec()),
            string(&property.owner.name),
        );
        *object.reflection.borrow_mut() = Some(ReflectionData::Property(property));
        Ok(Value::Null)
    }

    pub(super) fn reflect_class_properties(
        &mut self,
        class: &Rc<Class>,
        object: Option<&Rc<PhpObject>>,
        method: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        if method != b"getproperties" {
            let name = self.lookup(b"name").unwrap().to_bytes();
            let mut property = self.reflected_property(class, object, &name);
            if method == b"hasproperty" {
                return Ok(Value::Bool(property.is_some()));
            }
            if property.is_none()
                && let Some(separator) = name.windows(2).position(|s| s == b"::")
            {
                let base_name = name[..separator].to_ascii_lowercase();
                let base = self.reflected_class(string(base_name), false, line)?;
                let short = &name[separator + 2..];
                if !class.is_a(&base.name) {
                    return Err(self.reflection_error(format!("Fully qualified property name {}::${} does not specify a base class of {}", base.display_name(), String::from_utf8_lossy(short), class.display_name()), line));
                }
                property = self.reflected_property(&base, None, short);
                if property.is_none() {
                    return Err(self.missing_reflected_property(&base, short, line));
                }
            }
            let property =
                property.ok_or_else(|| self.missing_reflected_property(class, &name, line))?;
            return Ok(
                self.reflection_object(b"ReflectionProperty", ReflectionData::Property(property))
            );
        }
        let filter = match self.lookup(b"filter") {
            Some(Value::Int(mask)) => mask,
            _ => 23,
        };
        let mut result = PhpArray::default();
        for property in properties(class) {
            if property.modifiers() & filter != 0 {
                result.append(
                    self.reflection_object(
                        b"ReflectionProperty",
                        ReflectionData::Property(property),
                    ),
                    line,
                )?;
            }
        }
        if filter & 1 != 0
            && let Some(object) = object
        {
            let names: Vec<_> = object
                .properties
                .borrow()
                .entries()
                .iter()
                .filter_map(|(key, _)| {
                    let ArrayKey::String(name) = key else {
                        return None;
                    };
                    if name.first() == Some(&0)
                        || class.properties.iter().any(|p| p.storage_name() == *name)
                    {
                        return None;
                    }
                    Some(name.clone())
                })
                .collect();
            for name in names {
                let property = PropertyReflection {
                    scope: class.clone(),
                    owner: class.clone(),
                    name,
                    declaration: None,
                    is_static: false,
                };
                result.append(
                    self.reflection_object(
                        b"ReflectionProperty",
                        ReflectionData::Property(property),
                    ),
                    line,
                )?;
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn reflect_property_method(
        &mut self,
        property: PropertyReflection,
        name: &[u8],
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let decl = property.declaration.as_ref();
        let flags = property.modifiers();
        Ok(match name {
            b"getname" => string(&property.name),
            b"getmangledname" => string(decl.map_or_else(
                || property.name.clone(),
                |p| p.storage_name_for(&property.owner.name),
            )),
            b"ispublic" => Value::Bool(flags & 1 != 0),
            b"isprotected" => Value::Bool(flags & 2 != 0),
            b"isprivate" => Value::Bool(flags & 4 != 0),
            b"isstatic" => Value::Bool(property.is_static),
            b"isreadonly" => Value::Bool(flags & 128 != 0),
            b"isfinal" => Value::Bool(flags & 32 != 0),
            b"isprotectedset" => Value::Bool(flags & 2048 != 0),
            b"isprivateset" => Value::Bool(flags & 4096 != 0),
            b"isabstract" | b"isvirtual" | b"hashooks" => Value::Bool(false),
            b"gethooks" => Value::Array(Rc::new(PhpArray::default())),
            b"isdefault" => Value::Bool(decl.is_some()),
            b"isdynamic" => Value::Bool(decl.is_none()),
            b"ispromoted" => Value::Bool(decl.is_some_and(|p| p.is_promoted)),
            b"getmodifiers" => Value::Int(flags),
            b"getdeclaringclass" => self.reflection_object(
                b"ReflectionClass",
                ReflectionData::Class(property.owner.clone()),
            ),
            b"hastype" => Value::Bool(decl.is_some_and(|p| p.ty.is_some())),
            b"gettype" | b"getsettabletype" => decl
                .and_then(|p| p.ty.clone())
                .map_or(Value::Null, |ty| self.reflection_type(ty)),
            b"hasdefaultvalue" => {
                Value::Bool(decl.is_some_and(|p| p.default.is_some() || p.ty.is_none()))
            }
            b"getdefaultvalue" => {
                let value = match decl {
                    Some(p) => self.property_default(&property.owner, p)?,
                    None => None,
                };
                if value.is_none() {
                    self.diagnostics.push(format!("Deprecated: ReflectionProperty::getDefaultValue() for a property without a default value is deprecated, use ReflectionProperty::hasDefaultValue() to check if the default value exists on line {line}"));
                }
                value.unwrap_or(Value::Null)
            }
            b"getattributes" => {
                let mut context = AttributeContext::class(&property.owner);
                context.target = 8;
                return self.reflected_attributes(
                    decl.map_or(&[], |p| p.attributes.as_slice()),
                    &context,
                    "ReflectionProperty",
                    line,
                );
            }
            b"setaccessible" => {
                self.diagnostics.push(format!("Deprecated: Method ReflectionProperty::setAccessible() is deprecated since 8.5, as it has no effect on line {line}"));
                Value::Null
            }
            b"getvalue" | b"setvalue" | b"isinitialized" => {
                return self.reflected_property_value(&property, name, supplied, line);
            }
            b"__tostring" => return self.reflected_property_string(&property),
            _ => unreachable!("unhandled property reflection method"),
        })
    }

    fn reflected_property_value(
        &mut self,
        property: &PropertyReflection,
        method: &[u8],
        supplied: usize,
        line: usize,
    ) -> Result<Value, Error> {
        let write = method == b"setvalue";
        let value = self
            .lookup(if write { b"objectOrValue" } else { b"object" })
            .unwrap();
        if write && !property.is_static && supplied < 2 {
            return Err(Error::new(
                ErrorKind::ArgumentCountError,
                format!(
                    "ReflectionProperty::setValue() expects exactly 2 arguments, {supplied} given"
                ),
                line,
            ));
        }
        let object = if property.is_static {
            None
        } else {
            let Value::Object(object) = value.clone() else {
                let message = if write {
                    format!(
                        "ReflectionProperty::setValue(): Argument #1 ($objectOrValue) must be of type object, {} given",
                        builtins::debug_type(&value)
                    )
                } else {
                    format!(
                        "ReflectionProperty::{}(): Argument #1 ($object) must be provided for instance properties",
                        if method == b"getvalue" {
                            "getValue"
                        } else {
                            "isInitialized"
                        }
                    )
                };
                return Err(Error::new(ErrorKind::TypeError, message, line));
            };
            if !write && !object.class.is_a(&property.owner.name) {
                return Err(self.reflection_error(
                    "Given object is not an instance of the class this property was declared in",
                    line,
                ));
            }
            Some(object)
        };
        let outer = self.class_context.replace(ClassContext {
            scope: property.scope.clone(),
            called: property.scope.clone(),
        });
        let strict = std::mem::replace(&mut self.strict_types, false);
        let result = (|| {
            if write {
                let value = if property.is_static && supplied == 1 {
                    self.diagnostics.push(format!("Deprecated: Calling ReflectionProperty::setValue() with a single argument is deprecated on line {line}"));
                    value
                } else {
                    if property.is_static
                        && !matches!(value, Value::Null | Value::Object(_) | Value::Closure(_))
                    {
                        self.diagnostics.push(format!("Deprecated: Calling ReflectionProperty::setValue() with a 1st argument which is not null or an object is deprecated on line {line}"));
                    }
                    self.lookup(b"value").unwrap()
                };
                self.write_location(
                    &mut Location {
                        static_class: property.is_static.then(|| property.scope.clone()),
                        object,
                        name: property.name.clone(),
                        overloaded: None,
                        global: false,
                        keys: Vec::new(),
                    },
                    value,
                    line,
                )?;
                Ok(Value::Null)
            } else if property.is_static {
                self.initialize_class(&property.scope)?;
                let value = property
                    .owner
                    .static_values
                    .borrow()
                    .get(&property.name)
                    .map(|c| c.borrow().dereferenced());
                if method == b"isinitialized" {
                    return Ok(Value::Bool(value.is_some()));
                }
                value.ok_or_else(|| Error::new(ErrorKind::Error, format!("Typed static property {}::${} must not be accessed before initialization", property.owner.display_name(), String::from_utf8_lossy(&property.name)), line))
            } else {
                let object = object.unwrap();
                if method == b"isinitialized" {
                    Ok(Value::Bool(
                        object
                            .properties
                            .borrow()
                            .get(&self.instance_property_key(&object, &property.name))
                            .is_some(),
                    ))
                } else {
                    self.read_property_value(&object, &property.name, line, Access::Read)
                }
            }
        })();
        self.class_context = outer;
        self.strict_types = strict;
        result
    }

    fn reflected_property_string(&mut self, property: &PropertyReflection) -> Result<Value, Error> {
        let mut out = b"Property [ ".to_vec();
        if let Some(decl) = &property.declaration {
            if decl.is_final {
                out.extend(b"final ");
            }
            out.extend(visibility_name(decl.visibility));
            out.push(b' ');
            if decl.set_visibility != decl.visibility {
                out.extend(visibility_name(decl.set_visibility));
                out.extend(b"(set) ");
            }
            if property.is_static {
                out.extend(b"static ");
            }
            if decl.is_readonly {
                out.extend(b"readonly ");
            }
            if let Some(ty) = &decl.ty {
                out.extend(ty.to_string().bytes());
                out.push(b' ');
            }
        } else {
            out.extend(b"<dynamic> public ");
        }
        out.push(b'$');
        out.extend(&property.name);
        if let Some(decl) = &property.declaration
            && let Some(value) = self.property_default(&property.owner, decl)?
        {
            out.extend(b" = ");
            format_value(&value, &mut out);
        }
        out.extend(b" ]\n");
        Ok(string(out))
    }
}

fn visibility_name(visibility: Visibility) -> &'static [u8] {
    match visibility {
        Visibility::Public => b"public",
        Visibility::Protected => b"protected",
        Visibility::Private => b"private",
    }
}

fn format_value(value: &Value, out: &mut Vec<u8>) {
    match value.dereferenced() {
        Value::Null => out.extend(b"NULL"),
        Value::Bool(true) => out.extend(b"true"),
        Value::Bool(false) => out.extend(b"false"),
        Value::Float(number) => {
            let formatted = Value::Float(number).to_bytes();
            out.extend(&formatted);
            if number.is_finite() && !formatted.contains(&b'.') {
                out.extend(b".0");
            }
        }
        Value::String(s) => {
            out.push(b'\'');
            for byte in s.to_vec() {
                match byte {
                    b'\\' => out.extend(b"\\\\"),
                    b'\n' => out.extend(b"\\n"),
                    b'\r' => out.extend(b"\\r"),
                    b'\t' => out.extend(b"\\t"),
                    12 => out.extend(b"\\f"),
                    11 => out.extend(b"\\v"),
                    27 => out.extend(b"\\e"),
                    0..=31 | 127..=255 => out.extend(format!("\\x{byte:02X}").bytes()),
                    _ => out.push(byte),
                }
            }
            out.push(b'\'');
        }
        Value::Array(array) => {
            let list = array
                .entries()
                .iter()
                .enumerate()
                .all(|(i, (k, _))| *k == ArrayKey::Int(i as i64));
            out.push(b'[');
            for (i, (key, value)) in array.entries().iter().enumerate() {
                if i != 0 {
                    out.extend(b", ");
                }
                if !list {
                    match key {
                        ArrayKey::Int(n) => out.extend(n.to_string().bytes()),
                        ArrayKey::String(s) => format_value(&string(s), out),
                    }
                    out.extend(b" => ");
                }
                format_value(value, out);
            }
            out.push(b']');
        }
        Value::Object(object) => {
            out.extend(b"object(");
            out.extend(&object.class.name);
            out.push(b')');
        }
        other => out.extend(other.to_bytes()),
    }
}
