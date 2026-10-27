use super::*;

impl Parser {
    pub(super) fn global_declaration(&mut self) -> Result<Stmt, Error> {
        let mut variables = Vec::new();
        loop {
            let variable = self.expr(0)?;
            if !matches!(
                variable.kind,
                ExprKind::Variable(_) | ExprKind::DynamicVariable(_)
            ) {
                return Err(Error::parse(
                    "Expected variable in global declaration",
                    variable.line,
                ));
            }
            variables.push(variable);
            if !self.eat(",") {
                break;
            }
        }
        self.expect(";")?;
        Ok(Stmt::Global(variables))
    }

    pub(super) fn static_declaration(&mut self) -> Result<Stmt, Error> {
        let mut declarations = Vec::new();
        loop {
            let line = self.token().line;
            let Kind::Variable(name) = self.token().kind.clone() else {
                return Err(Error::parse(
                    "Expected variable in static declaration",
                    line,
                ));
            };
            self.pos += 1;
            if !self.static_names.insert(name.clone()) {
                return Err(Error::parse(
                    format!(
                        "Duplicate declaration of static variable ${}",
                        String::from_utf8_lossy(&name)
                    ),
                    line,
                ));
            }
            let initializer = if self.eat("=") {
                Some(self.expr(0)?)
            } else {
                None
            };
            declarations.push((name, initializer));
            if !self.eat(",") {
                break;
            }
        }
        self.expect(";")?;
        Ok(Stmt::Static(declarations))
    }

    // The leading '$' was consumed by the expression parser. A bare dynamic
    // name is itself a variable; postfix indexing belongs to the outer value.
    pub(super) fn dynamic_variable(&mut self) -> Result<ExprKind, Error> {
        let line = self.token().line;
        let name = if self.eat("{") {
            let expression = self.expr(0)?;
            self.expect("}")?;
            expression
        } else {
            let token = self.token().clone();
            self.pos += 1;
            let kind = match token.kind {
                Kind::Variable(name) => ExprKind::Variable(name),
                Kind::Symbol(op) if op == "$" => self.dynamic_variable()?,
                _ => return Err(Error::parse("Expected variable name", line)),
            };
            Expr { kind, line }
        };
        // Literal string names (including concatenated literals) are compiled
        // as ordinary named variables by PHP, including its auto-globals.
        if let Some(name) = literal_name(&name) {
            return Ok(ExprKind::Variable(name));
        }
        Ok(ExprKind::DynamicVariable(Box::new(name)))
    }
}

fn literal_name(expr: &Expr) -> Option<Vec<u8>> {
    match &expr.kind {
        ExprKind::Literal(Value::String(name)) => Some(name.clone()),
        ExprKind::Group(inner) => literal_name(inner),
        ExprKind::Binary(op, left, right) if op == "." => {
            let mut name = literal_name(left)?;
            name.extend(literal_name(right)?);
            Some(name)
        }
        _ => None,
    }
}
