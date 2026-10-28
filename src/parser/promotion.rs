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
        while self.is("public") || self.is("protected") || self.is("private") || self.is("readonly")
        {
            if !allowed {
                return Err(Error::parse(
                    "Cannot declare promoted property outside a constructor",
                    self.token().line,
                ));
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
            (visibility.is_some() || set_visibility.is_some() || readonly).then_some(Promotion {
                visibility: visibility.unwrap_or(Visibility::Public),
                set_visibility,
                is_readonly: readonly,
            }),
        )
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
        for param in &function.parameters {
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
            let is_readonly = readonly_class || promotion.is_readonly;
            if is_readonly && param.ty.is_none() {
                return Err(Error::parse(
                    "Readonly promoted property must have a type",
                    function.line,
                ));
            }
            let set_visibility = promotion.set_visibility.unwrap_or(
                if is_readonly && promotion.visibility == Visibility::Public {
                    Visibility::Protected
                } else {
                    promotion.visibility
                },
            );
            if set_visibility < promotion.visibility
                || (promotion.set_visibility.is_some() && param.ty.is_none())
            {
                return Err(Error::parse(
                    "Invalid promoted property set visibility",
                    function.line,
                ));
            }
            properties.push(PropertyDecl {
                order: properties.len() + static_properties.len(),
                is_promoted: true,
                attributes: param.attributes.clone(),
                declaring_class: Vec::new(),
                visibility: promotion.visibility,
                set_visibility,
                is_final: set_visibility == Visibility::Private
                    && promotion.visibility != Visibility::Private,
                name: param.name.clone(),
                ty: param.ty.clone(),
                is_readonly,
                default: None,
            });
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
