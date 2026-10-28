use super::*;
use std::collections::HashMap;

impl Parser {
    pub(super) fn label_name(&mut self) -> Result<Vec<u8>, Error> {
        let Kind::Word(name) = &self.token().kind else {
            return Err(Error::parse("Expected label identifier", self.token().line));
        };
        // Labels use T_STRING, not the broader set of identifiers accepted
        // for members. enum is contextual and can be used as a label.
        let reserved = matches!(
            name.to_ascii_lowercase().as_slice(),
            b"include"
                | b"include_once"
                | b"eval"
                | b"require"
                | b"require_once"
                | b"or"
                | b"xor"
                | b"and"
                | b"instanceof"
                | b"new"
                | b"clone"
                | b"exit"
                | b"die"
                | b"if"
                | b"elseif"
                | b"else"
                | b"endif"
                | b"echo"
                | b"do"
                | b"while"
                | b"endwhile"
                | b"for"
                | b"endfor"
                | b"foreach"
                | b"endforeach"
                | b"declare"
                | b"enddeclare"
                | b"as"
                | b"try"
                | b"catch"
                | b"finally"
                | b"throw"
                | b"use"
                | b"insteadof"
                | b"global"
                | b"var"
                | b"unset"
                | b"isset"
                | b"empty"
                | b"continue"
                | b"goto"
                | b"function"
                | b"const"
                | b"return"
                | b"print"
                | b"yield"
                | b"list"
                | b"switch"
                | b"endswitch"
                | b"case"
                | b"default"
                | b"break"
                | b"array"
                | b"callable"
                | b"extends"
                | b"implements"
                | b"namespace"
                | b"trait"
                | b"interface"
                | b"class"
                | b"fn"
                | b"match"
                | b"static"
                | b"abstract"
                | b"final"
                | b"private"
                | b"protected"
                | b"public"
                | b"readonly"
                | b"__halt_compiler"
                | b"__property__"
        );
        if reserved || is_magic_constant(name) || name.contains(&b'\\') {
            return Err(Error::parse("Expected label identifier", self.token().line));
        }
        let name = name.clone();
        self.pos += 1;
        Ok(name)
    }
}

pub(super) fn validate(statements: &[Stmt]) -> Result<(), Error> {
    let mut scope = JumpScope::default();
    scope.collect(statements, &mut Vec::new())?;
    for (name, source, line) in scope.jumps {
        let target = scope.labels.get(&name).ok_or_else(|| {
            Error::parse(
                format!(
                    "'goto' to undefined label '{}'",
                    String::from_utf8_lossy(&name)
                ),
                line,
            )
        })?;
        if source
            .iter()
            .any(|r| scope.finally_regions.contains(r) && !target.contains(r))
        {
            return Err(Error::parse(
                "jump out of a finally block is disallowed",
                line,
            ));
        }
        if !source.starts_with(target) {
            return Err(Error::parse(
                "'goto' into loop or switch statement is disallowed",
                line,
            ));
        }
    }
    Ok(())
}

#[derive(Default)]
struct JumpScope {
    finally_regions: std::collections::HashSet<usize>,
    labels: HashMap<Vec<u8>, Vec<usize>>,
    jumps: Vec<(Vec<u8>, Vec<usize>, usize)>,
    next_region: usize,
}

impl JumpScope {
    fn collect(&mut self, statements: &[Stmt], regions: &mut Vec<usize>) -> Result<(), Error> {
        for statement in statements {
            match statement {
                Stmt::Label(name, line) => {
                    if self.labels.insert(name.clone(), regions.clone()).is_some() {
                        return Err(Error::parse(
                            format!("Label '{}' already defined", String::from_utf8_lossy(name)),
                            *line,
                        ));
                    }
                }
                Stmt::Goto(name, line) => self.jumps.push((name.clone(), regions.clone(), *line)),
                Stmt::Function(function) => validate(&function.body)?,
                Stmt::Block(body) => self.collect(body, regions)?,
                Stmt::Try(body, catches, finally) => {
                    self.collect(body, regions)?;
                    for catch in catches {
                        self.collect(&catch.body, regions)?;
                    }
                    if let Some(finally) = finally {
                        self.next_region += 1;
                        self.finally_regions.insert(self.next_region);
                        regions.push(self.next_region);
                        self.collect(finally, regions)?;
                        regions.pop();
                    }
                }
                Stmt::If(_, yes, no) => {
                    self.collect(std::slice::from_ref(yes), regions)?;
                    if let Some(no) = no {
                        self.collect(std::slice::from_ref(no), regions)?;
                    }
                }
                Stmt::While(_, body)
                | Stmt::DoWhile(body, _)
                | Stmt::For(_, _, _, body)
                | Stmt::Foreach(_, _, _, body) => {
                    self.next_region += 1;
                    regions.push(self.next_region);
                    self.collect(std::slice::from_ref(body), regions)?;
                    regions.pop();
                }
                Stmt::Switch(_, cases) => {
                    self.next_region += 1;
                    regions.push(self.next_region);
                    for case in cases {
                        self.collect(&case.body, regions)?;
                    }
                    regions.pop();
                }
                _ => {}
            }
        }
        Ok(())
    }
}
