use super::*;
use crate::ast::{Attribute, ClassDecl};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    Class,
    Function,
    Method,
    Property,
    ClassConstant,
    Parameter,
    Constant,
}

impl Target {
    fn name(self) -> &'static str {
        match self {
            Self::Class => "class",
            Self::Function => "function",
            Self::Method => "method",
            Self::Property => "property",
            Self::ClassConstant => "class constant",
            Self::Parameter => "parameter",
            Self::Constant => "constant",
        }
    }
}

impl Parser {
    // Arguments are parsed after the declaration's scope is known, so magic
    // constants and self/parent names use the decorated declaration's context.
    pub(super) fn take_attributes(&mut self) -> Result<Option<(usize, usize)>, Error> {
        if !self.is("#[") {
            return Ok(None);
        }
        let start = self.pos;
        while self.eat("#[") {
            let mut depth = 1;
            while depth > 0 {
                match &self.token().kind {
                    Kind::Symbol(s) if s == "#[" || s == "[" => depth += 1,
                    Kind::Symbol(s) if s == "]" => depth -= 1,
                    Kind::End => {
                        return Err(Error::parse("Unclosed attribute group", self.token().line));
                    }
                    _ => {}
                }
                self.pos += 1;
            }
        }
        Ok(Some((start, self.pos)))
    }

    pub(super) fn parse_attributes_at(
        &mut self,
        range: Option<(usize, usize)>,
        target: Target,
    ) -> Result<Vec<Attribute>, Error> {
        let Some((start, end)) = range else {
            return Ok(Vec::new());
        };
        let outer = std::mem::replace(&mut self.pos, start);
        let result = (|| {
            let mut attributes = Vec::new();
            while self.eat("#[") {
                loop {
                    let line = self.token().line;
                    let Kind::Word(name) = self.token().kind.clone() else {
                        return Err(Error::parse("Expected attribute name", line));
                    };
                    if name.eq_ignore_ascii_case(b"self")
                        || name.eq_ignore_ascii_case(b"parent")
                        || name.eq_ignore_ascii_case(b"static")
                    {
                        return Err(Error::parse(
                            "Cannot use self, parent, or static as an attribute name",
                            line,
                        ));
                    }
                    self.pos += 1;
                    let name = self.resolve_class_name(&name);
                    let arguments = if self.eat("(") {
                        self.arguments()?
                    } else {
                        Vec::new()
                    };
                    attributes.push(Attribute {
                        name,
                        arguments,
                        line,
                    });
                    if !self.eat(",") || self.is("]") {
                        break;
                    }
                }
                self.expect("]")?;
            }
            if self.pos != end {
                return Err(Error::parse("Invalid attribute list", self.token().line));
            }
            Self::validate_attributes(&attributes, target)?;
            Ok(attributes)
        })();
        self.pos = outer;
        result
    }

    pub(super) fn validate_attributes(
        attributes: &[Attribute],
        target: Target,
    ) -> Result<(), Error> {
        let delayed = attributes
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(b"DelayedTargetValidation"));
        let mut internal = HashSet::new();
        for attribute in attributes {
            let mut named = HashSet::new();
            for argument in &attribute.arguments {
                let value = match argument {
                    Argument::Positional(value) => value,
                    Argument::Named(name, value) => {
                        if !named.insert(name) {
                            return Err(Error::parse(
                                format!(
                                    "Duplicate named parameter ${}",
                                    String::from_utf8_lossy(name)
                                ),
                                attribute.line,
                            ));
                        }
                        value
                    }
                    Argument::Unpack(_) => {
                        return Err(Error::parse(
                            "Cannot use unpacking in attribute argument list",
                            attribute.line,
                        ));
                    }
                };
                if !value.is_attribute_expression() {
                    return Err(Error::parse(
                        "Constant expression contains invalid operations",
                        value.line,
                    ));
                }
            }
            let lower = attribute.name.to_ascii_lowercase();
            let (canonical, allowed): (&str, &[Target]) = match lower.as_slice() {
                b"attribute" => ("Attribute", &[Target::Class]),
                b"allowdynamicproperties" => ("AllowDynamicProperties", &[Target::Class]),
                b"returntypewillchange" => ("ReturnTypeWillChange", &[Target::Method]),
                b"override" => ("Override", &[Target::Method, Target::Property]),
                b"sensitiveparameter" => ("SensitiveParameter", &[Target::Parameter]),
                b"deprecated" => (
                    "Deprecated",
                    &[
                        Target::Class,
                        Target::Function,
                        Target::Method,
                        Target::ClassConstant,
                        Target::Constant,
                    ],
                ),
                b"nodiscard" => ("NoDiscard", &[Target::Function, Target::Method]),
                b"delayedtargetvalidation" => (
                    "DelayedTargetValidation",
                    &[
                        Target::Class,
                        Target::Function,
                        Target::Method,
                        Target::Property,
                        Target::ClassConstant,
                        Target::Parameter,
                        Target::Constant,
                    ],
                ),
                _ => continue,
            };
            if !allowed.contains(&target) && !delayed {
                return Err(Error::parse(
                    format!(
                        "Attribute \"{canonical}\" cannot target {} (allowed targets: {})",
                        target.name(),
                        allowed
                            .iter()
                            .map(|t| t.name())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    attribute.line,
                ));
            }
            if !internal.insert(lower) {
                return Err(Error::parse(
                    format!("Attribute \"{canonical}\" must not be repeated"),
                    attribute.line,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn attach_class_attributes(
        &mut self,
        range: Option<(usize, usize)>,
        class: &mut ClassDecl,
    ) -> Result<(), Error> {
        let outer_class = std::mem::replace(&mut self.class_name, class.name.clone());
        let outer_parent = std::mem::replace(&mut self.class_parent, class.parent.clone());
        let outer_trait = std::mem::replace(
            &mut self.trait_name,
            if class.is_trait {
                class.name.clone()
            } else {
                Vec::new()
            },
        );
        let outer_function = std::mem::take(&mut self.function_name);
        let result = self.parse_attributes_at(range, Target::Class);
        self.class_name = outer_class;
        self.class_parent = outer_parent;
        self.trait_name = outer_trait;
        self.function_name = outer_function;
        class.attributes = result?;
        let delayed = class
            .attributes
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(b"DelayedTargetValidation"));
        for attribute in &class.attributes {
            let name = attribute.name.to_ascii_lowercase();
            let canonical = match name.as_slice() {
                b"allowdynamicproperties" => "AllowDynamicProperties",
                b"attribute" => "Attribute",
                _ => continue,
            };
            let invalid = if class.is_trait {
                Some("trait")
            } else if class.is_interface {
                Some("interface")
            } else if class.is_enum {
                Some("enum")
            } else if canonical == "AllowDynamicProperties" && class.is_readonly {
                Some("readonly class")
            } else if canonical == "Attribute" && class.is_abstract {
                Some("abstract class")
            } else {
                None
            };
            if let Some(kind) = invalid
                && !delayed
            {
                return Err(Error::parse(
                    format!(
                        "Cannot apply #[\\{canonical}] to {kind} {}",
                        String::from_utf8_lossy(&class.name)
                    ),
                    class.line,
                ));
            }
        }
        Ok(())
    }

    pub(super) fn attach_function_attributes(
        &mut self,
        range: Option<(usize, usize)>,
        function: &mut FunctionDecl,
        method: bool,
    ) -> Result<(), Error> {
        let outer_name = std::mem::replace(&mut self.function_name, function.name.clone());
        let outer_class = self.class_name.clone();
        if !method && !function.name.starts_with(b"{closure:") {
            self.class_name.clear();
        }
        let outer_function_class =
            std::mem::replace(&mut self.function_class, self.class_name.clone());
        let result = self.parse_attributes_at(
            range,
            if method {
                Target::Method
            } else {
                Target::Function
            },
        );
        self.function_name = outer_name;
        self.class_name = outer_class;
        self.function_class = outer_function_class;
        function.attributes = result?;
        if function
            .attributes
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(b"NoDiscard"))
        {
            if let Some(ty) = &function.return_type
                && (ty.contains("void") || ty.contains("never"))
            {
                return Err(Error::parse(
                    format!(
                        "A {} {} does not return a value, but #[\\NoDiscard] requires a return value",
                        if ty.contains("void") {
                            "void"
                        } else {
                            "never returning"
                        },
                        if method { "method" } else { "function" }
                    ),
                    function.line,
                ));
            }
            if method
                && [
                    b"__construct".as_slice(),
                    b"__destruct",
                    b"__clone",
                    b"__set",
                    b"__unset",
                    b"__wakeup",
                    b"__unserialize",
                ]
                .iter()
                .any(|name| function.name.eq_ignore_ascii_case(name))
            {
                return Err(Error::parse(
                    format!(
                        "Method {}::{} cannot be #[\\NoDiscard]",
                        String::from_utf8_lossy(&self.class_name),
                        String::from_utf8_lossy(&function.name)
                    ),
                    function.line,
                ));
            }
        }
        Self::validate_attributes(
            &function.attributes,
            if method {
                Target::Method
            } else {
                Target::Function
            },
        )
    }

    pub(super) fn attributed_statement(&mut self) -> Result<Stmt, Error> {
        let range = self.take_attributes()?;
        let anonymous = self.is("fn")
            || self.is("static")
            || (self.is("function") && {
                let next = self.pos
                    + 1
                    + usize::from(
                        self.tokens
                            .get(self.pos + 1)
                            .is_some_and(|t| matches!(&t.kind, Kind::Symbol(s) if s == "&")),
                    );
                self.tokens
                    .get(next)
                    .is_some_and(|t| matches!(&t.kind, Kind::Symbol(s) if s == "("))
            });
        if anonymous {
            self.pos = range.unwrap().0;
            let value = self.expr(0)?;
            self.expect(";")?;
            return Ok(Stmt::Expr(value));
        }
        let mut statement = self.top_level_statement()?;
        match &mut statement {
            Stmt::Class(class) => self.attach_class_attributes(range, Rc::make_mut(class))?,
            Stmt::Function(function) => {
                self.attach_function_attributes(range, Rc::make_mut(function), false)?
            }
            Stmt::Const(constants, attributes) => {
                if constants.len() != 1 {
                    return Err(Error::parse(
                        "Cannot apply attributes to multiple constants at once",
                        self.token().line,
                    ));
                }
                *attributes = self.parse_attributes_at(range, Target::Constant)?;
            }
            _ => {
                return Err(Error::parse(
                    "Attributes require a declaration",
                    self.token().line,
                ));
            }
        }
        Ok(statement)
    }

    pub(super) fn attributed_lambda(&mut self) -> Result<ExprKind, Error> {
        let range = self.take_attributes()?;
        let is_static = self.eat("static");
        let line = self.token().line;
        let arrow = self.eat("fn");
        if !arrow {
            self.expect("function")?;
        }
        let ExprKind::Lambda(mut lambda) = self.lambda(arrow, is_static, line)? else {
            unreachable!()
        };
        self.attach_function_attributes(
            range,
            Rc::make_mut(&mut Rc::make_mut(&mut lambda).function),
            false,
        )?;
        Ok(ExprKind::Lambda(lambda))
    }
}
