use super::*;
use crate::ast::{TraitAdaptation, TraitUse};

impl Parser {
    fn trait_word(&mut self) -> Result<Vec<u8>, Error> {
        let Kind::Word(name) = self.token().kind.clone() else {
            return Err(Error::parse(
                "Expected trait or method name",
                self.token().line,
            ));
        };
        self.pos += 1;
        Ok(name)
    }

    fn trait_reference(&self, name: &[u8]) -> Result<Vec<u8>, Error> {
        if [b"self".as_slice(), b"parent", b"static"]
            .iter()
            .any(|word| name.eq_ignore_ascii_case(word))
        {
            return Err(Error::parse(
                format!(
                    "Cannot use \"{}\" as trait name, as it is reserved",
                    String::from_utf8_lossy(name)
                ),
                self.token().line,
            ));
        }
        Ok(self.resolve_class_name(name))
    }

    pub(super) fn trait_use(&mut self) -> Result<TraitUse, Error> {
        let mut names = Vec::new();
        loop {
            let name = self.trait_word()?;
            names.push(self.trait_reference(&name)?);
            if !self.eat(",") {
                break;
            }
        }
        let mut adaptations = Vec::new();
        if self.eat(";") {
            return Ok(TraitUse { names, adaptations });
        }
        self.expect("{")?;
        while !self.eat("}") {
            let first = self.trait_word()?;
            let (source, method) = if self.eat("::") {
                (Some(self.trait_reference(&first)?), self.trait_word()?)
            } else {
                (None, first)
            };
            if method.contains(&b'\\') {
                return Err(Error::parse(
                    "Expected unqualified method name",
                    self.token().line,
                ));
            }
            if self.eat("insteadof") {
                let source = source.ok_or_else(|| {
                    Error::parse(
                        "Trait precedence requires a qualified method",
                        self.token().line,
                    )
                })?;
                let mut excluded = Vec::new();
                loop {
                    let name = self.trait_word()?;
                    excluded.push(self.trait_reference(&name)?);
                    if !self.eat(",") {
                        break;
                    }
                }
                adaptations.push(TraitAdaptation::InsteadOf {
                    source,
                    method,
                    excluded,
                });
            } else {
                self.expect("as")?;
                if self.is("static") || self.is("abstract") {
                    return Err(Error::parse(
                        "Cannot use static or abstract as method modifier in trait alias",
                        self.token().line,
                    ));
                }
                let mut visibility = None;
                let mut is_final = false;
                loop {
                    let access = if self.eat("public") {
                        Some(Visibility::Public)
                    } else if self.eat("protected") {
                        Some(Visibility::Protected)
                    } else if self.eat("private") {
                        Some(Visibility::Private)
                    } else {
                        None
                    };
                    if let Some(access) = access {
                        if visibility.replace(access).is_some() {
                            return Err(Error::parse(
                                "Multiple access type modifiers are not allowed",
                                self.token().line,
                            ));
                        }
                    } else if self.eat("final") {
                        if is_final {
                            return Err(Error::parse(
                                "Duplicate final modifier",
                                self.token().line,
                            ));
                        }
                        is_final = true;
                    } else {
                        break;
                    }
                }
                let alias = if self.is(";") {
                    None
                } else {
                    Some(self.trait_word()?)
                };
                if alias.as_ref().is_some_and(|name| name.contains(&b'\\'))
                    || alias.is_none() && visibility.is_none() && !is_final
                {
                    return Err(Error::parse("Invalid trait alias", self.token().line));
                }
                adaptations.push(TraitAdaptation::Alias {
                    source,
                    method,
                    alias,
                    visibility,
                    is_final,
                });
            }
            self.expect(";")?;
        }
        Ok(TraitUse { names, adaptations })
    }
}
