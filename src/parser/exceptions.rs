use super::*;
use crate::ast::CatchClause;

impl Parser {
    fn exception_body(&mut self) -> Result<Vec<Stmt>, Error> {
        self.expect("{")?;
        let mut body = Vec::new();
        while !self.eat("}") {
            body.push(self.statement()?);
        }
        Ok(body)
    }
    pub(super) fn try_statement(&mut self) -> Result<Stmt, Error> {
        let body = self.exception_body()?;
        let mut catches = Vec::new();
        while self.eat("catch") {
            self.expect("(")?;
            let mut types = Vec::new();
            loop {
                let Kind::Word(name) = self.token().kind.clone() else {
                    return Err(Error::parse(
                        "Expected exception class name",
                        self.token().line,
                    ));
                };
                if [b"self".as_slice(), b"parent", b"static"]
                    .iter()
                    .any(|bad| name.eq_ignore_ascii_case(bad))
                {
                    return Err(Error::parse(
                        "Bad class name in catch statement",
                        self.token().line,
                    ));
                }
                self.pos += 1;
                types.push(self.resolve_class_name(&name));
                if !self.eat("|") {
                    break;
                }
            }
            let line = self.token().line;
            let variable = if let Kind::Variable(name) = self.token().kind.clone() {
                if name == b"this" || name == b"GLOBALS" {
                    return Err(Error::parse("Invalid catch variable", self.token().line));
                }
                self.pos += 1;
                Some(name)
            } else {
                None
            };
            self.expect(")")?;
            catches.push(CatchClause {
                line,
                types,
                variable,
                body: self.exception_body()?,
            });
        }
        let finally = if self.eat("finally") {
            self.finally_depths.push(self.control_depth);
            let body = self.exception_body()?;
            self.finally_depths.pop();
            Some(body)
        } else {
            None
        };
        if catches.is_empty() && finally.is_none() {
            return Err(Error::parse(
                "Cannot use try without catch or finally",
                self.token().line,
            ));
        }
        Ok(Stmt::Try(body, catches, finally))
    }
}
