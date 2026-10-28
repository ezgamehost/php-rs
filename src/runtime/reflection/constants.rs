use super::*;

impl Runtime {
    pub(super) fn reflection_class_constant_constructor(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let value = self.lookup(b"class").unwrap();
        let class = self.reflected_class(value, false, line)?;
        let name = self.lookup(b"constant").unwrap().to_bytes();
        let owner = class.constant_owner(&name).ok_or_else(|| {
            self.reflection_error(
                format!(
                    "Constant {}::{} does not exist",
                    class.display_name(),
                    String::from_utf8_lossy(&name)
                ),
                line,
            )
        })?;
        object
            .properties
            .borrow_mut()
            .insert(ArrayKey::String(b"name".to_vec()), string(&name));
        object
            .properties
            .borrow_mut()
            .insert(ArrayKey::String(b"class".to_vec()), string(&owner.name));
        *object.reflection.borrow_mut() = Some(ReflectionData::ClassConstant { owner, name });
        Ok(Value::Null)
    }

    pub(super) fn reflect_class_constants(
        &mut self,
        class: &Rc<Class>,
        method: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        if method == b"getreflectionconstant" {
            let name = self.lookup(b"name").unwrap().to_bytes();
            return Ok(match class.constant_owner(&name) {
                Some(owner) => self.reflection_object(
                    b"ReflectionClassConstant",
                    ReflectionData::ClassConstant { owner, name },
                ),
                None => Value::Bool(false),
            });
        }
        let filter = match self.lookup(b"filter") {
            Some(Value::Int(mask)) => mask,
            _ => 7,
        };
        let mut result = PhpArray::default();
        for name in class.constant_names() {
            let owner = class.constant_owner(&name).unwrap();
            let constant = owner
                .declaration
                .constants
                .iter()
                .find(|c| c.name == name)
                .unwrap();
            let flags = visibility_flags(constant.visibility) | (i64::from(constant.is_final) * 32);
            if flags & filter != 0 {
                result.append(
                    self.reflection_object(
                        b"ReflectionClassConstant",
                        ReflectionData::ClassConstant { owner, name },
                    ),
                    line,
                )?;
            }
        }
        Ok(Value::Array(Rc::new(result)))
    }

    pub(super) fn reflect_class_constant_method(
        &mut self,
        object: &Rc<PhpObject>,
        owner: &Rc<Class>,
        name: &[u8],
        method: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let constant = owner
            .declaration
            .constants
            .iter()
            .find(|c| c.name == name)
            .unwrap();
        Ok(match method {
            b"getname" | b"getvalue" | b"__tostring" => {
                let reflected_name = object
                    .properties
                    .borrow()
                    .get(&ArrayKey::String(b"name".to_vec()))
                    .map(Value::dereferenced)
                    .ok_or_else(|| Self::uninitialized_property(&object.class, b"name", line))?;
                if method == b"getname" {
                    return Ok(reflected_name);
                }
                let value = self.evaluate_class_constant(owner, name, line)?;
                if method == b"getvalue" {
                    return Ok(value);
                }
                let mut out = Vec::new();
                if let Some(comment) = &constant.doc_comment {
                    out.extend(comment);
                    out.push(b'\n');
                }
                out.extend(b"Constant [ ");
                if constant.is_final {
                    out.extend(b"final ");
                }
                out.extend(match constant.visibility {
                    Visibility::Public => b"public".as_slice(),
                    Visibility::Protected => b"protected",
                    Visibility::Private => b"private",
                });
                out.push(b' ');
                let ty = constant.ty.as_ref().map_or_else(
                    || builtins::debug_type(&value).to_owned(),
                    ToString::to_string,
                );
                out.extend(ty.bytes());
                out.push(b' ');
                out.extend(reflected_name.to_bytes());
                out.extend(b" ] { ");
                match value {
                    Value::Array(_) => out.extend(b"Array"),
                    Value::Object(_) | Value::Closure(_) => out.extend(b"Object"),
                    _ => out.extend(self.string_bytes(&value, line)?),
                }
                out.extend(b" }\n");
                string(out)
            }
            b"ispublic" => Value::Bool(constant.visibility == Visibility::Public),
            b"isprotected" => Value::Bool(constant.visibility == Visibility::Protected),
            b"isprivate" => Value::Bool(constant.visibility == Visibility::Private),
            b"isfinal" => Value::Bool(constant.is_final),
            b"getmodifiers" => Value::Int(
                visibility_flags(constant.visibility) | (i64::from(constant.is_final) * 32),
            ),
            b"getdeclaringclass" => {
                self.reflection_object(b"ReflectionClass", ReflectionData::Class(owner.clone()))
            }
            b"getdoccomment" => constant
                .doc_comment
                .as_ref()
                .map_or(Value::Bool(false), string),
            b"getattributes" => {
                let mut context = AttributeContext::class(owner);
                context.target = 16;
                return self.reflected_attributes(
                    &constant.attributes,
                    &context,
                    "ReflectionClassConstant",
                    line,
                );
            }
            b"isenumcase" => Value::Bool(false),
            b"isdeprecated" => Value::Bool(
                constant
                    .attributes
                    .iter()
                    .any(|a| a.name.eq_ignore_ascii_case(b"Deprecated")),
            ),
            b"hastype" => Value::Bool(constant.ty.is_some()),
            b"gettype" => constant
                .ty
                .clone()
                .map_or(Value::Null, |ty| self.reflection_type(ty)),
            _ => unreachable!("unhandled class constant reflection method"),
        })
    }
}
