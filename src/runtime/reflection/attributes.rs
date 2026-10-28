use super::*;

impl Runtime {
    pub(super) fn reflected_attributes(
        &mut self,
        attributes: &[Attribute],
        owner: &AttributeContext,
        label: &str,
        line: usize,
    ) -> Result<Value, Error> {
        let name = self.lookup(b"name").unwrap();
        let flags = self.lookup(b"flags").unwrap();
        if !matches!(flags, Value::Int(0 | 2)) {
            return Err(Error::new(
                ErrorKind::ValueError,
                format!(
                    "{label}::getAttributes(): Argument #2 ($flags) must be a valid attribute filter flag"
                ),
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
        for attribute in attributes {
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
            let repeated = attributes
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

    pub(super) fn attribute_arguments(
        &mut self,
        owner: &AttributeContext,
        attribute: &Attribute,
    ) -> Result<PhpArray, Error> {
        let outer = std::mem::replace(
            &mut self.class_context,
            owner.class.as_ref().map(|class| ClassContext {
                scope: class.clone(),
                called: class.clone(),
            }),
        );
        let source = self.replace_source(owner.source.clone());
        let strict = std::mem::replace(&mut self.strict_types, owner.strict_types);
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
        self.replace_source(source);
        self.strict_types = strict;
        result
    }

    pub(super) fn reflect_attribute_method(
        &mut self,
        owner: &AttributeContext,
        attribute: &Attribute,
        repeated: bool,
        name: &[u8],
        line: usize,
    ) -> Result<Value, Error> {
        Ok(match name {
            b"getname" => string(&attribute.name),
            b"gettarget" => Value::Int(owner.target),
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
                let marker_args =
                    self.attribute_arguments(&AttributeContext::class(&class), &marker)?;
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
                if flags & owner.target == 0 {
                    return Err(Error::new(
                        ErrorKind::Error,
                        format!(
                            "Attribute \"{}\" cannot target {} (allowed targets: {})",
                            String::from_utf8_lossy(&attribute.name),
                            attribute_target_names(owner.target),
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
                if let Some(canonical) = canonical
                    && let Some(class) = &owner.class
                {
                    let decl = &class.declaration;
                    let invalid = if decl.is_trait {
                        Some("trait")
                    } else if decl.is_interface {
                        Some("interface")
                    } else if decl.is_enum {
                        Some("enum")
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
                                class.display_name()
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
                let source = self.replace_source(owner.source.clone());
                let strict = std::mem::replace(&mut self.strict_types, owner.strict_types);
                let result = self.instantiate_class(
                    class,
                    &[Argument::Unpack(literal(
                        Value::Array(Rc::new(arguments)),
                        attribute.line,
                    ))],
                    true,
                    attribute.line,
                );
                self.replace_source(source);
                self.strict_types = strict;
                result?
            }
            _ => unreachable!("unhandled attribute reflection method"),
        })
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
