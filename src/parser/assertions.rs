// Altered Rust adaptation of Zend/zend_ast.c expression export for assert().
// See THIRD_PARTY_NOTICES.md.
use super::*;

pub(super) fn add_description(callee: &Expr, args: &mut Vec<Argument>, line: usize) {
    let is_assert = match &callee.kind {
        ExprKind::Constant(name) => name.eq_ignore_ascii_case(b"assert"),
        ExprKind::NamespacedName(name) => {
            name.function.eq_ignore_ascii_case(b"assert")
                || name
                    .function_fallback
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(b"assert"))
        }
        _ => false,
    };
    if !is_assert || args.len() != 1 {
        return;
    }
    let mut writer = Export(Vec::from(b"assert("));
    if writer.argument(&args[0], 0).is_none() {
        return;
    }
    writer.put(b")");
    let description = Expr {
        kind: ExprKind::Literal(Value::String(writer.0.into())),
        line,
    };
    args.push(if matches!(args[0], Argument::Named(..)) {
        Argument::Named(b"description".to_vec(), description)
    } else {
        Argument::Positional(description)
    });
}

struct Export(Vec<u8>);
impl Export {
    fn put(&mut self, s: &[u8]) {
        self.0.extend(s);
    }
    fn indent(&mut self, n: usize) {
        self.0.extend(std::iter::repeat_n(b' ', n * 4));
    }
    fn quoted(&mut self, s: &[u8]) {
        self.put(b"'");
        for b in s {
            if matches!(b, b'\'' | b'\\') {
                self.put(b"\\");
            }
            self.0.push(*b);
        }
        self.put(b"'");
    }
    fn argument(&mut self, arg: &Argument, indent: usize) -> Option<()> {
        let expr = match arg {
            Argument::Positional(e) => e,
            Argument::Named(name, e) => {
                self.put(name);
                self.put(b": ");
                e
            }
            Argument::Unpack(e) => {
                self.put(b"...");
                e
            }
        };
        self.expr(expr, 0, indent)
    }
    fn arguments(&mut self, args: &[Argument], indent: usize) -> Option<()> {
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                self.put(b", ");
            }
            self.argument(arg, indent)?;
        }
        Some(())
    }
    fn member(&mut self, expr: &Expr, indent: usize) -> Option<()> {
        if let ExprKind::Literal(Value::String(name)) = &expr.kind {
            self.put(&name.bytes());
        } else {
            self.put(b"{");
            self.expr(expr, 0, indent)?;
            self.put(b"}");
        }
        Some(())
    }
    fn prefix(
        &mut self,
        text: &[u8],
        e: &Expr,
        own: u16,
        priority: u16,
        indent: usize,
    ) -> Option<()> {
        if priority > own {
            self.put(b"(");
        }
        self.put(text);
        self.expr(e, own + 1, indent)?;
        if priority > own {
            self.put(b")");
        }
        Some(())
    }
    fn expr(&mut self, e: &Expr, priority: u16, indent: usize) -> Option<()> {
        match &e.kind {
            ExprKind::Group(inner) => {
                let arrow = matches!(&inner.kind, ExprKind::Lambda(l) if l.arrow);
                if arrow {
                    self.put(b"(");
                }
                self.expr(inner, priority, indent)?;
                if arrow {
                    self.put(b")");
                }
            }
            ExprKind::Literal(Value::String(s)) => self.quoted(&s.bytes()),
            ExprKind::Literal(Value::Null) => self.put(b"null"),
            ExprKind::Literal(Value::Bool(b)) => self.put(if *b { b"true" } else { b"false" }),
            ExprKind::Literal(Value::Int(n)) => self.put(n.to_string().as_bytes()),
            ExprKind::Literal(Value::Float(n)) => {
                let mut s = Value::Float(*n).to_bytes();
                if !s.contains(&b'.') {
                    if let Some(i) = s.iter().position(|b| matches!(b, b'e' | b'E')) {
                        s.splice(i..i, b".0".iter().copied());
                    } else {
                        s.extend(b".0");
                    }
                }
                self.put(&s);
            }
            ExprKind::Interpolated(parts) => {
                self.put(b"\"");
                for (i, part) in parts.iter().enumerate() {
                    if let ExprKind::Literal(Value::String(text)) = &part.expression.kind {
                        for byte in text.bytes().iter().copied() {
                            match byte {
                                b'\n' => self.put(b"\\n"),
                                b'\r' => self.put(b"\\r"),
                                b'\t' => self.put(b"\\t"),
                                12 => self.put(b"\\f"),
                                11 => self.put(b"\\v"),
                                27 => self.put(b"\\e"),
                                0..=31 => self.put(format!("\\0{:02o}", byte).as_bytes()),
                                b'"' | b'$' | b'\\' => {
                                    self.put(b"\\");
                                    self.0.push(byte);
                                }
                                _ => self.0.push(byte),
                            }
                        }
                    } else {
                        let following = parts.get(i + 1).and_then(|p| {
                            if let ExprKind::Literal(Value::String(s)) = &p.expression.kind {
                                s.bytes().first().copied()
                            } else {
                                None
                            }
                        });
                        let braces = !matches!(part.expression.kind, ExprKind::Variable(_))
                            || following.is_some_and(|b| {
                                b == b'[' || b == b'_' || b.is_ascii_alphanumeric() || b >= 128
                            });
                        if braces {
                            self.put(b"{");
                        }
                        self.expr(&part.expression, 0, indent)?;
                        if braces {
                            self.put(b"}");
                        }
                    }
                }
                self.put(b"\"");
            }
            ExprKind::Variable(name) => {
                self.put(b"$");
                self.put(name);
            }
            ExprKind::DynamicVariable(inner) => {
                self.put(b"${");
                self.expr(inner, 0, indent)?;
                self.put(b"}");
            }
            ExprKind::Constant(name) => self.put(name),
            ExprKind::NamespacedName(name) => self.put(&name.original),
            ExprKind::Unary(op, inner) => {
                self.prefix(op.as_bytes(), inner, 240, priority, indent)?
            }
            ExprKind::Cast(kind, inner) => {
                let name = match kind {
                    CastKind::Int => "int",
                    CastKind::Float => "float",
                    CastKind::Bool => "bool",
                    CastKind::String => "string",
                    CastKind::Array => "array",
                    CastKind::Object => "object",
                    CastKind::Void => "void",
                };
                self.prefix(format!("({name})").as_bytes(), inner, 240, priority, indent)?;
            }
            ExprKind::Binary(op, left, right) | ExprKind::Assign(left, op, right) => {
                let (own, l, r) = priorities(op)?;
                if priority > own {
                    self.put(b"(");
                }
                self.expr(left, l, indent)?;
                self.put(b" ");
                self.put(match op.as_str() {
                    "and" => b"&&",
                    "or" => b"||",
                    "<>" => b"!=",
                    _ => op.as_bytes(),
                });
                self.put(b" ");
                self.expr(right, r, indent)?;
                if priority > own {
                    self.put(b")");
                }
            }
            ExprKind::Conditional(cond, yes, no) => {
                if priority > 100 {
                    self.put(b"(");
                }
                self.expr(cond, 100, indent)?;
                if let Some(yes) = yes {
                    self.put(b" ? ");
                    self.expr(yes, 101, indent)?;
                    self.put(b" : ");
                } else {
                    self.put(b" ?: ");
                }
                self.expr(no, 101, indent)?;
                if priority > 100 {
                    self.put(b")");
                }
            }
            ExprKind::Increment(inner, delta, prefix) => {
                let op = if *delta > 0 { b"++" } else { b"--" };
                if *prefix {
                    self.prefix(op, inner, 240, priority, indent)?;
                } else {
                    if priority > 240 {
                        self.put(b"(");
                    }
                    self.expr(inner, 241, indent)?;
                    self.put(op);
                    if priority > 240 {
                        self.put(b")");
                    }
                }
            }
            ExprKind::Array(items)
            | ExprKind::LongArray(items)
            | ExprKind::List(items)
            | ExprKind::Destructure(items) => {
                self.put(b"[");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.put(b", ");
                    }
                    match item {
                        ArrayElement::Hole => {}
                        ArrayElement::Unpack(value) => {
                            self.put(b"...");
                            self.expr(value, 80, indent)?;
                        }
                        ArrayElement::Entry(key, value) | ArrayElement::Reference(key, value) => {
                            if let Some(key) = key {
                                self.expr(key, 80, indent)?;
                                self.put(b" => ");
                            }
                            if matches!(item, ArrayElement::Reference(..)) {
                                self.put(b"&");
                            }
                            self.expr(value, 80, indent)?;
                        }
                    }
                }
                self.put(b"]");
            }
            ExprKind::Index(base, key) => {
                self.expr(base, 260, indent)?;
                self.put(b"[");
                if let Some(key) = key {
                    self.expr(key, 0, indent)?;
                }
                self.put(b"]");
            }
            ExprKind::Property(base, name)
            | ExprKind::NullsafeProperty(base, name)
            | ExprKind::ScopedMember(base, name)
            | ExprKind::StaticProperty(base, name) => {
                self.expr(base, 260, indent)?;
                self.put(match e.kind {
                    ExprKind::Property(..) => b"->",
                    ExprKind::NullsafeProperty(..) => b"?->",
                    ExprKind::StaticProperty(..) => b"::$",
                    _ => b"::",
                });
                self.member(name, indent)?;
            }
            ExprKind::ClassName(base) => {
                self.expr(base, 0, indent)?;
                self.put(b"::class");
            }
            ExprKind::Call(callee, args) => {
                let mut inner = callee.as_ref();
                while let ExprKind::Group(e) = &inner.kind {
                    inner = e;
                }
                let closure = matches!(inner.kind, ExprKind::Lambda(_));
                if closure {
                    self.put(b"(");
                }
                self.expr(inner, 0, indent)?;
                if closure {
                    self.put(b")");
                }
                self.put(b"(");
                self.arguments(args, indent)?;
                self.put(b")");
            }
            ExprKind::Callable(callee) => {
                self.expr(callee, 0, indent)?;
                self.put(b"(...)");
            }
            ExprKind::New(class, args) => {
                self.put(b"new ");
                if let ExprKind::AnonymousClass(class) = &class.kind {
                    self.attributes(&class.attributes, indent)?;
                    if class.is_readonly {
                        self.put(b"readonly ");
                    }
                    self.put(b"class");
                    if !args.is_empty() {
                        self.put(b"(");
                        self.arguments(args, indent)?;
                        self.put(b")");
                    }
                    self.class_body(class, indent)?;
                } else {
                    self.expr(class, 0, indent)?;
                    self.put(b"(");
                    self.arguments(args, indent)?;
                    self.put(b")");
                }
            }
            ExprKind::Print(e) => self.prefix(b"print ", e, 60, priority, indent)?,
            ExprKind::Throw(e) => self.prefix(b"throw ", e, 50, priority, indent)?,
            ExprKind::Eval(inner) | ExprKind::Empty(inner) => {
                self.put(if matches!(e.kind, ExprKind::Eval(_)) {
                    b"eval("
                } else {
                    b"empty("
                });
                self.expr(inner, 0, indent)?;
                self.put(b")");
            }
            ExprKind::Isset(items) => {
                self.put(b"isset(");
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        self.put(b", ");
                    }
                    self.expr(item, 0, indent)?;
                }
                self.put(b")");
            }
            ExprKind::Lambda(lambda) => {
                self.attributes(&lambda.function.attributes, indent)?;
                if lambda.is_static {
                    self.put(b"static ");
                }
                self.put(if lambda.arrow { b"fn" } else { b"function " });
                self.function(
                    &lambda.function,
                    lambda.arrow,
                    Some(&lambda.captures),
                    indent,
                )?;
            }
            _ => return None,
        }
        Some(())
    }
    fn attributes(&mut self, attrs: &[crate::ast::Attribute], indent: usize) -> Option<()> {
        for attr in attrs {
            self.put(b"#[");
            self.put(&attr.name);
            if !attr.arguments.is_empty() {
                self.put(b"(");
                self.arguments(&attr.arguments, indent)?;
                self.put(b")");
            }
            self.put(b"] ");
        }
        Some(())
    }
    fn function(
        &mut self,
        f: &FunctionDecl,
        arrow: bool,
        captures: Option<&[crate::ast::Capture]>,
        indent: usize,
    ) -> Option<()> {
        if f.returns_reference && captures.is_some() {
            self.put(b"&");
        }
        self.put(b"(");
        for (i, p) in f.parameters.iter().enumerate() {
            if i > 0 {
                self.put(b", ");
            }
            self.attributes(
                p.promotion
                    .as_ref()
                    .map_or(&p.attributes, |p| &p.attributes),
                indent,
            )?;
            if let Some(promotion) = &p.promotion {
                self.visibility(promotion.visibility);
                if promotion.is_final {
                    self.put(b"final ");
                }
                if promotion.is_readonly {
                    self.put(b"readonly ");
                }
                if let Some(set) = promotion.set_visibility {
                    self.put(match set {
                        Visibility::Public => b"public(set) ",
                        Visibility::Protected => b"protected(set) ",
                        Visibility::Private => b"private(set) ",
                    });
                }
            }
            if let Some(ty) = &p.ty {
                self.put(ty.to_string().as_bytes());
                self.put(b" ");
            }
            if p.by_reference {
                self.put(b"&");
            }
            if p.variadic {
                self.put(b"...");
            }
            self.put(b"$");
            self.put(&p.name);
            if let Some(value) = &p.default {
                self.put(b" = ");
                self.expr(value, 0, indent)?;
            }
        }
        self.put(b")");
        if !arrow
            && let Some(captures) = captures
            && !captures.is_empty()
        {
            self.put(b" use(");
            for (i, c) in captures.iter().enumerate() {
                if i > 0 {
                    self.put(b", ");
                }
                if c.by_reference {
                    self.put(b"&");
                }
                self.put(b"$");
                self.put(&c.name);
            }
            self.put(b")");
        }
        if let Some(ty) = &f.return_type {
            self.put(b": ");
            self.put(ty.to_string().as_bytes());
        }
        if arrow {
            let [Stmt::Return(Some(e))] = f.body.as_slice() else {
                return None;
            };
            self.put(b" => ");
            self.expr(e, 0, indent)?;
        } else {
            self.put(b" {\n");
            let promoted = f
                .parameters
                .iter()
                .filter(|p| p.promotion.is_some())
                .count();
            self.statements(&f.body[promoted..], indent + 1)?;
            self.indent(indent);
            self.put(b"}");
        }
        Some(())
    }
    fn visibility(&mut self, visibility: Visibility) {
        self.put(match visibility {
            Visibility::Public => b"public ",
            Visibility::Protected => b"protected ",
            Visibility::Private => b"private ",
        });
    }
    fn block_attributes(&mut self, attrs: &[crate::ast::Attribute], indent: usize) -> Option<()> {
        for attr in attrs {
            self.attributes(std::slice::from_ref(attr), indent)?;
            self.0.pop();
            self.put(b"\n");
            self.indent(indent);
        }
        Some(())
    }
    fn class_body(&mut self, class: &crate::ast::ClassDecl, indent: usize) -> Option<()> {
        if let Some(parent) = &class.parent {
            self.put(b" extends ");
            self.put(parent);
        }
        if !class.interfaces.is_empty() {
            self.put(b" implements ");
            for (i, name) in class.interfaces.iter().enumerate() {
                if i > 0 {
                    self.put(b", ");
                }
                self.put(name);
            }
        }
        self.put(b" {\n");
        if !class.trait_uses.is_empty() {
            return None;
        }
        for p in class
            .properties
            .iter()
            .chain(&class.static_properties)
            .filter(|p| !p.is_promoted)
        {
            self.indent(indent + 1);
            self.block_attributes(&p.attributes, indent + 1)?;
            self.visibility(p.visibility);
            if p.is_final {
                self.put(b"final ");
            }
            if class.static_properties.iter().any(|q| std::ptr::eq(p, q)) {
                self.put(b"static ");
            }
            if p.is_readonly {
                self.put(b"readonly ");
            }
            if let Some(ty) = &p.ty {
                self.put(ty.to_string().as_bytes());
                self.put(b" ");
            }
            self.put(b"$");
            self.put(&p.name);
            if let Some(value) = &p.default {
                self.put(b" = ");
                self.expr(value, 0, indent + 1)?;
            }
            self.put(b";\n");
        }
        for c in &class.constants {
            self.indent(indent + 1);
            self.block_attributes(&c.attributes, indent + 1)?;
            if c.is_enum_case {
                self.put(b"case ");
            } else {
                self.visibility(c.visibility);
                if c.is_final {
                    self.put(b"final ");
                }
                self.put(b"const ");
                if let Some(ty) = &c.ty {
                    self.put(ty.to_string().as_bytes());
                    self.put(b" ");
                }
            }
            self.put(&c.name);
            if !c.is_enum_case || class.enum_backing.is_some() {
                self.put(b" = ");
                self.expr(&c.value, 0, indent + 1)?;
            }
            self.put(b";\n");
        }
        for f in &class.methods {
            self.indent(indent + 1);
            self.block_attributes(&f.attributes, indent + 1)?;
            self.visibility(f.visibility);
            if f.is_static {
                self.put(b"static ");
            }
            if f.is_abstract {
                self.put(b"abstract ");
            }
            let name = f.name.rsplit(|b| *b == b':').next().unwrap();
            if class
                .final_methods
                .iter()
                .any(|n| n.eq_ignore_ascii_case(name))
            {
                self.put(b"final ");
            }
            self.put(b"function ");
            if f.returns_reference {
                self.put(b"&");
            }
            self.put(name);
            self.function(f, false, None, indent + 1)?;
            self.put(b"\n\n");
        }
        self.indent(indent);
        self.put(b"}");
        Some(())
    }
    fn statements(&mut self, stmts: &[Stmt], indent: usize) -> Option<()> {
        for stmt in stmts {
            if let Stmt::Block(stmts) = stmt {
                self.statements(stmts, indent)?;
                continue;
            }
            self.indent(indent);
            match stmt {
                Stmt::Class(class) => {
                    self.block_attributes(&class.attributes, indent)?;
                    self.put(if class.is_enum {
                        b"enum "
                    } else if class.is_trait {
                        b"trait "
                    } else if class.is_interface {
                        b"interface "
                    } else {
                        b"class "
                    });
                    self.put(class.name.rsplit(|b| *b == b'\\').next().unwrap());
                    if let Some(ty) = &class.enum_backing {
                        self.put(b": ");
                        self.put(ty.as_bytes());
                    }
                    self.class_body(class, indent)?;
                    self.put(b"\n\n");
                    continue;
                }
                Stmt::Expr(e) => self.expr(e, 0, indent)?,
                Stmt::Return(e) => {
                    self.put(b"return");
                    if let Some(e) = e {
                        self.put(b" ");
                        self.expr(e, 0, indent)?;
                    }
                }
                Stmt::Echo(items) => {
                    self.put(b"echo ");
                    for (i, e) in items.iter().enumerate() {
                        if i > 0 {
                            self.put(b", ");
                        }
                        self.expr(e, 0, indent)?;
                    }
                }
                _ => return None,
            }
            self.put(b";\n");
        }
        Some(())
    }
}
fn priorities(op: &str) -> Option<(u16, u16, u16)> {
    Some(match op {
        "=" | "=&" | "+=" | "-=" | "*=" | "/=" | "%=" | ".=" | "**=" | "??=" | "&=" | "|="
        | "^=" | "<<=" | ">>=" => (90, 91, 90),
        "??" => (110, 111, 110),
        "or" | "||" => (120, 120, 121),
        "and" | "&&" => (130, 130, 131),
        "xor" => (40, 40, 41),
        "|" => (140, 140, 141),
        "^" => (150, 150, 151),
        "&" => (160, 160, 161),
        "==" | "!=" | "<>" | "===" | "!==" => (170, 171, 171),
        "<" | "<=" | ">" | ">=" | "<=>" => (180, 181, 181),
        "|>" => (183, 183, 184),
        "." => (185, 185, 186),
        "<<" | ">>" => (190, 190, 191),
        "+" | "-" => (200, 200, 201),
        "*" | "/" | "%" => (210, 210, 211),
        "instanceof" => (230, 231, 231),
        "**" => (250, 251, 250),
        _ => return None,
    })
}
