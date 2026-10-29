use super::*;

impl Runtime {
    /// `None` carries a short circuit through this chain; `Some(Null)` is an
    /// ordinary result, on which subsequent non-nullsafe operations still run.
    pub(super) fn nullsafe_chain(
        &mut self,
        expr: &Expr,
        access: Access,
    ) -> Result<Option<Value>, Error> {
        if !expr.has_nullsafe_chain() {
            return self.chain_read(expr, access).map(Some);
        }
        let literal = |value| {
            Box::new(Expr {
                kind: ExprKind::Literal(value),
                line: expr.line,
            })
        };
        let kind = match &expr.kind {
            ExprKind::Group(inner) => return self.nullsafe_chain(inner, access),
            ExprKind::Property(base, name) | ExprKind::NullsafeProperty(base, name) => {
                let Some(value) = self.nullsafe_chain(base, access.intermediate())? else {
                    return Ok(None);
                };
                if matches!(expr.kind, ExprKind::NullsafeProperty(_, _)) && value == Value::Null {
                    return Ok(None);
                }
                ExprKind::Property(literal(value), name.clone())
            }
            ExprKind::Index(base, key) => {
                let Some(value) = self.nullsafe_chain(base, access.intermediate())? else {
                    return Ok(None);
                };
                ExprKind::Index(literal(value), key.clone())
            }
            ExprKind::StaticProperty(base, name) => {
                let base_access = if access == Access::Coalesce {
                    Access::Read
                } else {
                    access
                };
                let Some(value) = self.nullsafe_chain(base, base_access)? else {
                    return Ok(None);
                };
                ExprKind::StaticProperty(literal(value), name.clone())
            }
            ExprKind::Call(callee, args) => {
                let (ExprKind::Property(base, name)
                | ExprKind::NullsafeProperty(base, name)
                | ExprKind::ScopedMember(base, name)) = &callee.kind
                else {
                    unreachable!()
                };
                let base_access = if matches!(callee.kind, ExprKind::ScopedMember(_, _))
                    && access == Access::Coalesce
                {
                    Access::Read
                } else {
                    access
                };
                let Some(value) = self.nullsafe_chain(base, base_access)? else {
                    return Ok(None);
                };
                if matches!(callee.kind, ExprKind::NullsafeProperty(_, _)) && value == Value::Null {
                    return Ok(None);
                }
                let kind = if matches!(callee.kind, ExprKind::ScopedMember(_, _)) {
                    ExprKind::ScopedMember(literal(value), name.clone())
                } else {
                    ExprKind::Property(literal(value), name.clone())
                };
                return self
                    .call(
                        &Expr {
                            kind,
                            line: callee.line,
                        },
                        args,
                        expr.line,
                        true,
                    )
                    .map(Some);
            }
            ExprKind::Callable(callee) => {
                let ExprKind::ScopedMember(base, name) = &callee.kind else {
                    unreachable!()
                };
                let Some(value) = self.nullsafe_chain(base, Access::Read)? else {
                    return Ok(None);
                };
                let callee = Expr {
                    kind: ExprKind::ScopedMember(literal(value), name.clone()),
                    line: callee.line,
                };
                return self.acquire_callable(&callee, expr.line).map(Some);
            }
            _ => unreachable!("only chain expressions reach this evaluator"),
        };
        // Reuse ordinary access semantics after replacing only the evaluated
        // receiver. Keys, names, and arguments keep their independent chains.
        self.chain_read(
            &Expr {
                kind,
                line: expr.line,
            },
            access,
        )
        .map(Some)
    }

    fn chain_read(&mut self, expr: &Expr, access: Access) -> Result<Value, Error> {
        match access {
            Access::Read => self.eval(expr),
            Access::Probe | Access::Empty | Access::ProbeValue | Access::Coalesce => {
                self.coalesce_read(expr, access)
            }
        }
    }
}
