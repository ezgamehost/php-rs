use super::*;

impl Runtime {
    pub(super) fn reflect_enum_method(
        &mut self,
        class: &Rc<Class>,
        method: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        let backed = class.declaration.enum_backing.as_ref();
        let kind = if backed.is_some() {
            b"ReflectionEnumBackedCase".as_slice()
        } else {
            b"ReflectionEnumUnitCase"
        };
        Ok(match method {
            b"isbacked" => Value::Bool(backed.is_some()),
            b"getbackingtype" => backed.map_or(Value::Null, |ty| {
                self.reflection_type(crate::ast::TypeDecl {
                    members: vec![ty.clone()],
                })
            }),
            b"getcases" => {
                let mut result = PhpArray::default();
                for case in class
                    .declaration
                    .constants
                    .iter()
                    .filter(|c| c.is_enum_case)
                {
                    result.append(
                        self.reflection_object(
                            kind,
                            ReflectionData::ClassConstant {
                                owner: class.clone(),
                                name: case.name.clone(),
                            },
                        ),
                        line,
                    )?;
                }
                Value::Array(Rc::new(result))
            }
            b"hascase" | b"getcase" => {
                let name = self.lookup(b"name").unwrap().to_bytes();
                let owner = class.constant_owner(&name);
                let is_case = owner.as_ref().is_some_and(|c| {
                    c.declaration
                        .constants
                        .iter()
                        .any(|c| c.name == name && c.is_enum_case)
                });
                if method == b"hascase" {
                    return Ok(Value::Bool(is_case));
                }
                if !is_case {
                    return Err(self.reflection_error(
                        if owner.is_some() {
                            format!(
                                "{}::{} is not a case",
                                class.display_name(),
                                String::from_utf8_lossy(&name)
                            )
                        } else {
                            format!(
                                "Case {}::{} does not exist",
                                class.display_name(),
                                String::from_utf8_lossy(&name)
                            )
                        },
                        line,
                    ));
                }
                self.reflection_object(
                    kind,
                    ReflectionData::ClassConstant {
                        owner: owner.unwrap(),
                        name,
                    },
                )
            }
            _ => unreachable!("unhandled enum reflection method"),
        })
    }
}
