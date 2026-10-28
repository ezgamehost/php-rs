use super::*;
use crate::runtime::constants::GlobalConstant;

impl Runtime {
    pub(super) fn reflection_constant_constructor(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let name = self.lookup(b"name").unwrap().to_bytes();
        let constant = self.constant_metadata(&name).ok_or_else(|| {
            self.reflection_error(
                format!(
                    "Constant \"{}\" does not exist",
                    String::from_utf8_lossy(&name)
                ),
                line,
            )
        })?;
        object
            .properties
            .borrow_mut()
            .insert(ArrayKey::String(b"name".to_vec()), string(name));
        *object.reflection.borrow_mut() = Some(ReflectionData::Constant(constant));
        Ok(Value::Null)
    }

    pub(super) fn reflect_constant_method(
        &mut self,
        constant: &Rc<GlobalConstant>,
        method: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        Ok(match method {
            b"getname" => string(&constant.name),
            b"getnamespacename" => string(
                constant
                    .name
                    .iter()
                    .rposition(|b| *b == b'\\')
                    .map_or(&[][..], |i| &constant.name[..i]),
            ),
            b"getshortname" => string(constant.name.rsplit(|b| *b == b'\\').next().unwrap()),
            b"getvalue" => constant.value.clone(),
            b"isdeprecated" => Value::Bool(constant.is_deprecated()),
            b"getfilename" => constant
                .source
                .as_ref()
                .map_or(Value::Bool(false), |s| string(&s.name)),
            b"getextensionname" => constant.extension.map_or(Value::Bool(false), string),
            b"getextension" => constant.extension.map_or(Value::Null, |name| {
                self.reflection_object(
                    b"ReflectionExtension",
                    ReflectionData::Extension(name.to_owned()),
                )
            }),
            b"getattributes" => {
                let context = AttributeContext {
                    class: None,
                    source: constant.source.clone().unwrap_or_default(),
                    strict_types: constant.strict_types,
                    target: 64,
                };
                return self.reflected_attributes(
                    &constant.attributes,
                    &context,
                    "ReflectionConstant",
                    line,
                );
            }
            b"__tostring" => {
                let mut out = b"Constant [ ".to_vec();
                let mut flags = Vec::new();
                if constant.extension.is_some() {
                    flags.push("persistent");
                }
                if constant.is_deprecated() {
                    flags.push("deprecated");
                }
                if !flags.is_empty() {
                    out.push(b'<');
                    out.extend(flags.join(", ").bytes());
                    out.extend(b"> ");
                }
                out.extend(builtins::debug_type(&constant.value).bytes());
                out.push(b' ');
                out.extend(constant.name.split(|b| *b == 0).next().unwrap());
                out.extend(b" ] { ");
                match &constant.value {
                    Value::Array(_) => out.extend(b"Array"),
                    Value::String(s) => out.extend(s.to_vec().split(|b| *b == 0).next().unwrap()),
                    value => out.extend(self.string_bytes(value, line)?),
                }
                out.extend(b" }\n");
                string(out)
            }
            _ => unreachable!("unhandled global constant reflection method"),
        })
    }

    pub(super) fn reflection_extension_constructor(
        &mut self,
        object: &Rc<PhpObject>,
        line: usize,
    ) -> Result<Value, Error> {
        let name = self.lookup(b"name").unwrap().to_bytes();
        let canonical = ["Core", "standard", "Reflection", "SPL"]
            .into_iter()
            .find(|canonical| canonical.as_bytes().eq_ignore_ascii_case(&name))
            .ok_or_else(|| {
                self.reflection_error(
                    format!(
                        "Extension \"{}\" does not exist",
                        String::from_utf8_lossy(&name)
                    ),
                    line,
                )
            })?;
        object
            .properties
            .borrow_mut()
            .insert(ArrayKey::String(b"name".to_vec()), string(canonical));
        *object.reflection.borrow_mut() = Some(ReflectionData::Extension(canonical.to_owned()));
        Ok(Value::Null)
    }

    pub(super) fn reflect_extension_method(
        &self,
        extension: &str,
        method: &[u8],
    ) -> Result<Value, Error> {
        Ok(match method {
            b"getname" => string(extension),
            b"getversion" => string("8.5.0"),
            b"ispersistent" => Value::Bool(true),
            b"istemporary" => Value::Bool(false),
            _ => unreachable!("unhandled extension reflection method"),
        })
    }
}
