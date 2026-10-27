use super::*;
use crate::ast::{Promotion, PropertyDecl};

impl Parser {
    pub(super) fn parameter_promotion(
        &mut self,
        allowed: bool,
    ) -> Result<Option<Promotion>, Error> {
        let mut public = false;
        let mut readonly = false;
        while self.is("public") || self.is("readonly") {
            if !allowed {
                return Err(Error::parse(
                    "Cannot declare promoted property outside a constructor",
                    self.token().line,
                ));
            }
            let flag = if self.eat("public") {
                &mut public
            } else {
                self.expect("readonly")?;
                &mut readonly
            };
            if *flag {
                return Err(Error::parse(
                    "Duplicate promotion modifier",
                    self.token().line,
                ));
            }
            *flag = true;
        }
        Ok((public || readonly).then_some(Promotion {
            is_readonly: readonly,
        }))
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
            properties.push(PropertyDecl {
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
                                kind: ExprKind::Literal(Value::String(param.name.clone())),
                            }),
                        ),
                    }),
                    "=".to_owned(),
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
