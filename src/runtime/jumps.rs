use super::*;
use crate::ast::SwitchCase;

// Search only regions that can be entered without an active loop/switch frame.
// A jump within a loop is resolved by that loop's body; an outward jump bubbles
// up until its target is found, dropping the abandoned iteration state.
pub(super) fn contains_label(statement: &Stmt, name: &[u8]) -> bool {
    match statement {
        Stmt::Label(label, _) => label == name,
        Stmt::Block(body) => body.iter().any(|stmt| contains_label(stmt, name)),
        Stmt::Try(body, catches, _) => {
            label_index(body, name).is_some()
                || catches.iter().any(|c| label_index(&c.body, name).is_some())
        }
        Stmt::If(_, yes, no) => {
            contains_label(yes, name) || no.as_ref().is_some_and(|no| contains_label(no, name))
        }
        _ => false,
    }
}

pub(super) fn label_index(statements: &[Stmt], name: &[u8]) -> Option<usize> {
    statements
        .iter()
        .position(|stmt| contains_label(stmt, name))
}

impl Runtime {
    pub(super) fn block_from(
        &mut self,
        statements: &[Stmt],
        mut index: usize,
        mut entry: Option<Vec<u8>>,
    ) -> Result<Flow, Error> {
        while let Some(statement) = statements.get(index) {
            let flow = match entry.take() {
                Some(name) => self.resume_statement(statement, &name)?,
                None => self.statement(statement)?,
            };
            match flow {
                Flow::Next => index += 1,
                Flow::Goto(name) => {
                    let Some(target) = label_index(statements, &name) else {
                        return Ok(Flow::Goto(name));
                    };
                    index = target;
                    entry = Some(name);
                }
                flow => return Ok(flow),
            }
        }
        Ok(Flow::Next)
    }

    fn resume_statement(&mut self, statement: &Stmt, name: &[u8]) -> Result<Flow, Error> {
        match statement {
            Stmt::Label(_, _) => Ok(Flow::Next),
            Stmt::Try(body, catches, finally) => {
                self.try_block(body, catches, finally.as_deref(), Some(name))
            }
            Stmt::Block(body) => {
                let index = label_index(body, name).expect("validated block label");
                self.block_from(body, index, Some(name.to_vec()))
            }
            Stmt::If(_, yes, no) => {
                let branch = if contains_label(yes, name) {
                    yes
                } else {
                    no.as_ref().expect("validated branch label")
                };
                self.resume_statement(branch, name)
            }
            _ => unreachable!("label search cannot enter loops or functions"),
        }
    }

    pub(super) fn switch_cases(
        &mut self,
        cases: &[SwitchCase],
        mut index: usize,
    ) -> Result<Flow, Error> {
        let mut entry = None;
        while let Some(case) = cases.get(index) {
            let start = entry.as_ref().map_or(0, |name: &Vec<u8>| {
                label_index(&case.body, name).expect("validated switch label")
            });
            match self.block_from(&case.body, start, entry.take())? {
                Flow::Next => index += 1,
                Flow::Break(1) | Flow::Continue(1) => break,
                Flow::Goto(name) => {
                    let Some(target) = cases
                        .iter()
                        .position(|case| label_index(&case.body, &name).is_some())
                    else {
                        return Ok(Flow::Goto(name));
                    };
                    index = target;
                    entry = Some(name);
                }
                flow => return Ok(flow.leave_level()),
            }
        }
        Ok(Flow::Next)
    }
}
