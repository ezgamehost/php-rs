use super::*;
use crate::ast::{Promotion, PropertyDecl};

impl Parser {
    pub(super) fn parameter_promotion(
        &mut self,
        allowed: bool,
    ) -> Result<Option<Promotion>, Error> {
        let mut visibility = None;
        let mut set_visibility = None;
        let mut readonly = false;
        let mut is_final = false;
        while self.is("public")
            || self.is("protected")
            || self.is("private")
            || self.is("readonly")
            || self.is("final")
        {
            if !allowed {
                return Err(Error::parse(
                    "Cannot declare promoted property outside a constructor",
                    self.token().line,
                ));
            }
            if self.eat("final") {
                if is_final {
                    return Err(Error::parse(
                        "Duplicate promotion modifier",
                        self.token().line,
                    ));
                }
                is_final = true;
                continue;
            }
            if self.eat("readonly") {
                if readonly {
                    return Err(Error::parse(
                        "Duplicate promotion modifier",
                        self.token().line,
                    ));
                }
                readonly = true;
                continue;
            }
            let access = if self.eat("public") {
                Visibility::Public
            } else if self.eat("protected") {
                Visibility::Protected
            } else {
                self.expect("private")?;
                Visibility::Private
            };
            let slot = if self.eat("(") {
                self.expect("set")?;
                self.expect(")")?;
                &mut set_visibility
            } else {
                &mut visibility
            };
            if slot.replace(access).is_some() {
                return Err(Error::parse(
                    "Duplicate promotion modifier",
                    self.token().line,
                ));
            }
        }
        Ok(
            (visibility.is_some() || set_visibility.is_some() || readonly || is_final).then_some(
                Promotion {
                    hooks: None,
                    doc_comment: None,
                    attributes: Vec::new(),
                    is_final,
                    visibility: visibility.unwrap_or(Visibility::Public),
                    set_visibility,
                    is_readonly: readonly,
                },
            ),
        )
    }

    pub(super) fn parameter_hooks(
        &mut self,
        promotion: &mut Option<Promotion>,
        name: &[u8],
        ty: &Option<TypeDecl>,
        allowed: bool,
    ) -> Result<(), Error> {
        if !self.is("{") {
            return Ok(());
        }
        if !allowed {
            return Err(Error::new(
                crate::ErrorKind::CompileError,
                "Cannot declare promoted property outside a constructor",
                self.token().line,
            ));
        }
        let promotion = promotion.get_or_insert_with(|| Promotion {
            hooks: None,
            doc_comment: None,
            attributes: Vec::new(),
            is_final: false,
            visibility: Visibility::Public,
            set_visibility: None,
            is_readonly: false,
        });
        if promotion.is_readonly {
            return Err(Error::new(
                crate::ErrorKind::CompileError,
                "Hooked properties cannot be readonly",
                self.token().line,
            ));
        }
        let property = Self::promoted_property(promotion, name, ty, 0, false);
        promotion.hooks = Some(self.property_hooks(&property, false)?);
        Ok(())
    }

    fn promoted_property(
        promotion: &Promotion,
        name: &[u8],
        ty: &Option<TypeDecl>,
        order: usize,
        readonly_class: bool,
    ) -> PropertyDecl {
        let is_readonly = readonly_class || promotion.is_readonly;
        let set_visibility = promotion.set_visibility.unwrap_or(
            if is_readonly && promotion.visibility == Visibility::Public {
                Visibility::Protected
            } else {
                promotion.visibility
            },
        );
        PropertyDecl {
            is_abstract: false,
            hooks: promotion.hooks.clone(),
            doc_comment: promotion.doc_comment.clone(),
            order,
            is_promoted: true,
            attributes: promotion
                .attributes
                .iter()
                .filter(|a| !a.name.eq_ignore_ascii_case(b"SensitiveParameter"))
                .cloned()
                .collect(),
            declaring_class: Vec::new(),
            visibility: promotion.visibility,
            set_visibility,
            is_final: promotion.is_final
                || (set_visibility == Visibility::Private
                    && promotion.visibility != Visibility::Private),
            name: name.to_vec(),
            ty: ty.clone(),
            is_readonly,
            default: None,
        }
    }

    pub(super) fn promote_properties(
        &self,
        method: &mut Rc<FunctionDecl>,
        properties: &mut Vec<PropertyDecl>,
        static_properties: &[PropertyDecl],
        readonly_class: bool,
    ) -> Result<(), Error> {
        let function = Rc::make_mut(method);
        let mut assignments = Vec::new();
        for param in &mut function.parameters {
            let Some(promotion) = &param.promotion else {
                continue;
            };
            if properties
                .iter()
                .chain(static_properties)
                .any(|p| p.name == param.name)
            {
                return Err(Error::parse(
                    "Cannot redeclare promoted property",
                    function.line,
                ));
            }
            let property = Self::promoted_property(
                promotion,
                &param.name,
                &param.ty,
                properties.len() + static_properties.len(),
                readonly_class,
            );
            if property.is_readonly && property.hooks.is_some() {
                return Err(Error::new(
                    crate::ErrorKind::CompileError,
                    "Hooked properties cannot be readonly",
                    function.line,
                ));
            }
            if property.is_readonly && param.ty.is_none() {
                return Err(Error::parse(
                    "Readonly promoted property must have a type",
                    function.line,
                ));
            }
            if property.set_visibility < promotion.visibility
                || (promotion.set_visibility.is_some() && param.ty.is_none())
            {
                return Err(Error::parse(
                    "Invalid promoted property set visibility",
                    function.line,
                ));
            }
            properties.push(property);
            param
                .attributes
                .retain(|attribute| !attribute.name.eq_ignore_ascii_case(b"Override"));
            let line = function.line;
            // Promotion is an assignment at constructor entry. The parameter
            // retains its own slot; defaults do not become property defaults.
            assignments.push(Stmt::Expr(Expr {
                line,
                kind: ExprKind::Assign(
                    Box::new(Expr {
                        line,
                        kind: ExprKind::Property(
                            Box::new(Expr {
                                line,
                                kind: ExprKind::Variable(b"this".to_vec()),
                            }),
                            Box::new(Expr {
                                line,
                                kind: ExprKind::Literal(Value::String(param.name.clone().into())),
                            }),
                        ),
                    }),
                    if param.by_reference { "=&" } else { "=" }.to_owned(),
                    Box::new(Expr {
                        line,
                        kind: ExprKind::Variable(param.name.clone()),
                    }),
                ),
            }));
        }
        function.body.splice(0..0, assignments);
        Ok(())
    }
}
