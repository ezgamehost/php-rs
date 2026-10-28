use super::*;
use crate::ast::ResolvedName;
use std::collections::HashMap;

#[derive(Clone, Default)]
pub(super) struct Namespace {
    pub name: Vec<u8>,
    imports: [HashMap<Vec<u8>, Vec<u8>>; 3],
    declarations: [HashSet<Vec<u8>>; 3],
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Symbol {
    Class,
    Function,
    Constant,
}

impl Symbol {
    fn key(self, name: &[u8]) -> Vec<u8> {
        if self == Self::Constant {
            name.to_vec()
        } else {
            name.to_ascii_lowercase()
        }
    }
}

impl Parser {
    pub(super) fn top_level_statement(&mut self) -> Result<Stmt, Error> {
        if self.eat("use") {
            self.namespace_import()?;
            Ok(Stmt::Block(Vec::new()))
        } else if self.eat("const") {
            self.constant_declaration()
        } else {
            self.statement()
        }
    }

    pub(super) fn namespace_declaration(&mut self) -> Result<(Vec<Stmt>, bool), Error> {
        let name = if self.is("{") {
            Vec::new()
        } else {
            self.namespace_word()?
        };
        if name.starts_with(b"\\")
            || name.ends_with(b"\\")
            || name.eq_ignore_ascii_case(b"namespace")
            || name
                .split(|b| *b == b'\\')
                .next()
                .is_some_and(|part| part.eq_ignore_ascii_case(b"namespace"))
        {
            return Err(Error::parse("Invalid namespace name", self.token().line));
        }
        self.namespace = Namespace {
            name,
            ..Namespace::default()
        };
        if self.eat(";") {
            if self.namespace.name.is_empty() {
                return Err(Error::parse("Expected namespace name", self.token().line));
            }
            return Ok((Vec::new(), false));
        }
        self.expect("{")?;
        let mut body = Vec::new();
        while !self.eat("}") {
            if self.token().kind == Kind::End {
                return Err(Error::parse("Unclosed namespace", self.token().line));
            }
            if self.is("namespace") {
                return Err(Error::parse(
                    "Namespace declarations cannot be nested",
                    self.token().line,
                ));
            }
            body.push(self.top_level_statement()?);
        }
        self.namespace = Namespace::default();
        Ok((body, true))
    }

    fn namespace_word(&mut self) -> Result<Vec<u8>, Error> {
        let Kind::Word(name) = self.token().kind.clone() else {
            return Err(Error::parse("Expected namespace name", self.token().line));
        };
        self.pos += 1;
        Ok(name)
    }

    fn import_kind(&mut self) -> Symbol {
        if self.eat("function") {
            Symbol::Function
        } else if self.eat("const") {
            Symbol::Constant
        } else {
            Symbol::Class
        }
    }

    fn namespace_import(&mut self) -> Result<(), Error> {
        let kind = self.import_kind();
        loop {
            let line = self.token().line;
            let name = self.namespace_word()?;
            // A group prefix may end in the same token or in a separate slash.
            let group = name.ends_with(b"\\") || self.eat("\\");
            if group {
                self.expect("{")?;
                loop {
                    let item_kind = if kind == Symbol::Class {
                        self.import_kind()
                    } else {
                        kind
                    };
                    let item_line = self.token().line;
                    let item = self.namespace_word()?;
                    if item.starts_with(b"\\") || item.ends_with(b"\\") {
                        return Err(Error::parse("Invalid group import name", item_line));
                    }
                    let mut target = name.clone();
                    if !target.ends_with(b"\\") {
                        target.push(b'\\');
                    }
                    target.extend(item);
                    self.add_import(item_kind, target, item_line, true)?;
                    if !self.eat(",") || self.is("}") {
                        break;
                    }
                }
                self.expect("}")?;
            } else {
                self.add_import(kind, name, line, false)?;
            }
            if !self.eat(",") {
                break;
            }
        }
        self.expect(";")
    }

    fn add_import(
        &mut self,
        kind: Symbol,
        name: Vec<u8>,
        line: usize,
        group: bool,
    ) -> Result<(), Error> {
        let target = name.strip_prefix(b"\\").unwrap_or(&name).to_vec();
        let explicit_alias = self.eat("as");
        let alias = if explicit_alias {
            self.namespace_word()?
        } else {
            target
                .rsplit(|b| *b == b'\\')
                .next()
                .unwrap_or_default()
                .to_vec()
        };
        if alias.is_empty()
            || alias.contains(&b'\\')
            || [b"self".as_slice(), b"parent", b"static"]
                .iter()
                .any(|bad| alias.eq_ignore_ascii_case(bad))
        {
            return Err(Error::parse("Invalid import alias", line));
        }
        let key = kind.key(&alias);
        let index = kind as usize;
        if self.namespace.imports[index].contains_key(&key)
            || (self.namespace.declarations[index].contains(&key)
                && kind.key(&target) != kind.key(&self.qualify(&alias)))
        {
            return Err(Error::parse(
                format!(
                    "Cannot use {} as {} because the name is already in use",
                    String::from_utf8_lossy(&target),
                    String::from_utf8_lossy(&alias)
                ),
                line,
            ));
        }
        if self.namespace.name.is_empty() && !target.contains(&b'\\') && !explicit_alias && !group {
            self.warnings.push(format!(
                "The use statement with non-compound name '{}' has no effect on line {line}",
                String::from_utf8_lossy(&target)
            ));
        }
        self.namespace.imports[index].insert(key, target);
        Ok(())
    }

    fn qualify(&self, name: &[u8]) -> Vec<u8> {
        if self.namespace.name.is_empty() {
            return name.to_vec();
        }
        let mut full = self.namespace.name.clone();
        full.push(b'\\');
        full.extend(name);
        full
    }

    pub(super) fn declared_name(
        &mut self,
        name: &[u8],
        kind: Symbol,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        let full = self.qualify(name);
        let key = kind.key(name);
        if self.namespace.imports[kind as usize]
            .get(&key)
            .is_some_and(|target| kind.key(target) != kind.key(&full))
        {
            return Err(Error::parse(
                format!(
                    "Cannot declare {} because the name is already in use",
                    String::from_utf8_lossy(&full)
                ),
                line,
            ));
        }
        self.namespace.declarations[kind as usize].insert(key);
        Ok(full)
    }

    fn resolve_name(&self, name: &[u8], kind: Symbol) -> (Vec<u8>, Option<Vec<u8>>) {
        if let Some(full) = name.strip_prefix(b"\\") {
            return (full.to_vec(), None);
        }
        if name.len() >= 10 && name[..10].eq_ignore_ascii_case(b"namespace\\") {
            return (self.qualify(&name[10..]), None);
        }
        if let Some(separator) = name.iter().position(|b| *b == b'\\') {
            if let Some(prefix) = self.namespace.imports[Symbol::Class as usize]
                .get(&name[..separator].to_ascii_lowercase())
            {
                let mut full = prefix.clone();
                full.extend(&name[separator..]);
                return (full, None);
            }
            return (self.qualify(name), None);
        }
        if let Some(imported) = self.namespace.imports[kind as usize].get(&kind.key(name)) {
            return (imported.clone(), None);
        }
        let fallback =
            (kind != Symbol::Class && !self.namespace.name.is_empty()).then(|| name.to_vec());
        (self.qualify(name), fallback)
    }

    pub(super) fn resolve_class_name(&self, name: &[u8]) -> Vec<u8> {
        if [b"self".as_slice(), b"parent", b"static"]
            .iter()
            .any(|word| name.eq_ignore_ascii_case(word))
        {
            name.to_vec()
        } else {
            self.resolve_name(name, Symbol::Class).0
        }
    }

    pub(super) fn resolve_identifier(&self, name: Vec<u8>) -> ExprKind {
        if [
            b"self".as_slice(),
            b"parent",
            b"static",
            b"exit",
            b"die",
            b"clone",
        ]
        .iter()
        .any(|word| name.eq_ignore_ascii_case(word))
        {
            return ExprKind::Constant(name);
        }
        let class = self.resolve_class_name(&name);
        let (function, function_fallback) = self.resolve_name(&name, Symbol::Function);
        let (constant, constant_fallback) = self.resolve_name(&name, Symbol::Constant);
        if class == function
            && class == constant
            && function_fallback.is_none()
            && constant_fallback.is_none()
        {
            ExprKind::Constant(class)
        } else {
            ExprKind::NamespacedName(Box::new(ResolvedName {
                class,
                function,
                constant,
                function_fallback,
                constant_fallback,
            }))
        }
    }
}
