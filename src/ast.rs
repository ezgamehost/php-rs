use crate::Value;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub(crate) struct Expr {
    pub kind: ExprKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
    Literal(Value),
    Interpolated(Vec<Expr>),
    Group(Box<Expr>),
    Variable(Vec<u8>),
    DynamicVariable(Box<Expr>),
    Constant(Vec<u8>),
    Unary(String, Box<Expr>),
    Cast(CastKind, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Array(Vec<ArrayElement>),
    LongArray(Vec<ArrayElement>),
    List(Vec<ArrayElement>),
    Destructure(Vec<ArrayElement>),
    Index(Box<Expr>, Option<Box<Expr>>),
    Call(Box<Expr>, Vec<Argument>),
    Callable(Box<Expr>),
    Lambda(Rc<LambdaDecl>),
    New(Box<Expr>, Vec<Argument>),
    Property(Box<Expr>, Box<Expr>),
    ScopedMember(Box<Expr>, Box<Expr>),
    StaticProperty(Box<Expr>, Box<Expr>),
    ClassName(Box<Expr>),
    Eval(Box<Expr>),
    Include(IncludeKind, Box<Expr>),
    Isset(Vec<Expr>),
    Empty(Box<Expr>),
    Print(Box<Expr>),
    Assign(Box<Expr>, String, Box<Expr>),
    Increment(Box<Expr>, i64, bool),
    Conditional(Box<Expr>, Option<Box<Expr>>, Box<Expr>),
    Match(Box<Expr>, Vec<MatchArm>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CastKind {
    Int,
    Float,
    Bool,
    String,
    Array,
    Void,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum IncludeKind {
    Include,
    IncludeOnce,
    Require,
    RequireOnce,
}

#[derive(Debug, Clone)]
pub(crate) enum ArrayElement {
    Entry(Option<Expr>, Expr),
    Unpack(Expr),
    Hole,
}

#[derive(Debug, Clone)]
pub(crate) struct MatchArm {
    /// An empty condition list denotes the default arm.
    pub conditions: Vec<Expr>,
    pub result: Expr,
}

#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Label(Vec<u8>, usize),
    Goto(Vec<u8>, usize),
    Global(Vec<Expr>),
    Static(Vec<(Vec<u8>, Option<Expr>)>),
    Const(Vec<(Vec<u8>, Expr)>),
    Function(Rc<FunctionDecl>),
    Class(Rc<ClassDecl>),
    Text(Vec<u8>),
    Echo(Vec<Expr>),
    Expr(Expr),
    Block(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(Vec<Expr>, Vec<Expr>, Vec<Expr>, Box<Stmt>),
    Switch(Expr, Vec<SwitchCase>),
    Foreach(Expr, Option<Expr>, Expr, Box<Stmt>),
    Unset(Vec<Expr>),
    Break(usize),
    Continue(usize, usize),
    Return(Option<Expr>),
}

#[derive(Debug, Clone)]
pub(crate) struct SwitchCase {
    pub condition: Option<Expr>,
    pub body: Vec<Stmt>,
    pub semicolon_line: Option<usize>,
}

impl Expr {
    pub fn is_globals(&self) -> bool {
        match &self.kind {
            ExprKind::Variable(name) => name == b"GLOBALS",
            ExprKind::Group(inner) => inner.is_globals(),
            _ => false,
        }
    }

    pub fn is_writable(&self) -> bool {
        match &self.kind {
            ExprKind::Variable(_)
            | ExprKind::DynamicVariable(_)
            | ExprKind::Property(_, _)
            | ExprKind::StaticProperty(_, _) => true,
            ExprKind::Index(base, _) => {
                let mut base = base.as_ref();
                while let ExprKind::Group(inner) = &base.kind {
                    base = inner;
                }
                base.is_writable()
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClassDecl {
    pub parent: Option<Vec<u8>>,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_readonly: bool,
    pub final_methods: Vec<Vec<u8>>,
    pub name: Vec<u8>,
    pub properties: Vec<PropertyDecl>,
    pub static_properties: Vec<PropertyDecl>,
    pub constants: Vec<ClassConstant>,
    pub methods: Vec<Rc<FunctionDecl>>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct PropertyDecl {
    pub is_readonly: bool,
    pub name: Vec<u8>,
    pub default: Option<Expr>,
    pub ty: Option<TypeDecl>,
}

#[derive(Debug, Clone)]
pub(crate) struct ClassConstant {
    pub name: Vec<u8>,
    pub value: Expr,
    pub ty: Option<TypeDecl>,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Promotion {
    pub is_readonly: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Parameter {
    pub promotion: Option<Promotion>,
    pub name: Vec<u8>,
    pub ty: Option<TypeDecl>,
    pub implicit_nullable: bool,
    pub default: Option<Expr>,
    pub variadic: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct FunctionDecl {
    pub is_abstract: bool,
    pub is_static: bool,
    pub name: Vec<u8>,
    pub parameters: Vec<Parameter>,
    pub body: Vec<Stmt>,
    pub line: usize,
    pub return_type: Option<TypeDecl>,
    pub strict_types: bool,
    pub source: Rc<crate::source::Source>,
}

#[derive(Debug, Clone)]
pub(crate) struct LambdaDecl {
    pub function: Rc<FunctionDecl>,
    pub captures: Vec<Capture>,
    pub arrow: bool,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Capture {
    pub name: Vec<u8>,
    pub by_reference: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TypeDecl {
    pub members: Vec<String>,
}

impl TypeDecl {
    pub fn contains(&self, name: &str) -> bool {
        self.members.iter().any(|member| member == name)
    }

    pub fn accepts(&self, value: &Value) -> bool {
        self.contains("mixed")
            || self.members.iter().any(|member| {
                if let Value::Object(object) = value
                    && (member == "object" || object.class.is_a(member.as_bytes()))
                {
                    return true;
                }
                if matches!(value, Value::Closure(_))
                    && member
                        .trim_start_matches('\\')
                        .eq_ignore_ascii_case("Closure")
                {
                    return true;
                }
                matches!(
                    (member.as_str(), value),
                    ("null", Value::Null)
                        | ("bool", Value::Bool(_))
                        | ("true", Value::Bool(true))
                        | ("false", Value::Bool(false))
                        | ("int", Value::Int(_))
                        | ("float", Value::Float(_))
                        | ("string", Value::String(_))
                        | ("array" | "iterable", Value::Array(_))
                        | ("object" | "callable", Value::Closure(_))
                )
            })
    }
}

impl std::fmt::Display for TypeDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.members.len() == 2 && self.contains("null") {
            write!(
                f,
                "?{}",
                self.members.iter().find(|m| *m != "null").unwrap()
            )
        } else {
            f.write_str(&self.members.join("|"))
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) enum Argument {
    Positional(Expr),
    Named(Vec<u8>, Expr),
    Unpack(Expr),
}
impl Expr {
    pub fn is_constant_expression(&self) -> bool {
        match &self.kind {
            ExprKind::Lambda(lambda) => {
                lambda.is_static && !lambda.arrow && lambda.captures.is_empty()
            }
            ExprKind::Callable(callee) => match &callee.kind {
                ExprKind::Constant(_) => true,
                ExprKind::ScopedMember(class, name) => {
                    matches!(&class.kind, ExprKind::Constant(name) if !name.eq_ignore_ascii_case(b"static"))
                        && matches!(name.kind, ExprKind::Literal(Value::String(_)))
                }
                _ => false,
            },
            ExprKind::ScopedMember(class, name) => {
                matches!(&class.kind, ExprKind::Constant(name) if !name.eq_ignore_ascii_case(b"static"))
                    && name.is_constant_expression()
            }
            ExprKind::Literal(_) | ExprKind::Constant(_) => true,
            ExprKind::ClassName(class) => {
                matches!(&class.kind, ExprKind::Constant(name) if !name.eq_ignore_ascii_case(b"static"))
            }
            ExprKind::Group(e) | ExprKind::Unary(_, e) => e.is_constant_expression(),
            ExprKind::Cast(kind, e) => *kind != CastKind::Void && e.is_constant_expression(),
            ExprKind::Binary(op, _, _) if matches!(op.as_str(), "|>" | "instanceof") => false,
            ExprKind::Index(a, Some(b)) | ExprKind::Binary(_, a, b) => {
                a.is_constant_expression() && b.is_constant_expression()
            }
            ExprKind::Conditional(a, b, c) => {
                a.is_constant_expression()
                    && b.as_ref().is_none_or(|b| b.is_constant_expression())
                    && c.is_constant_expression()
            }
            ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
                elements.iter().all(|element| match element {
                    ArrayElement::Entry(k, v) => {
                        k.as_ref().is_none_or(Expr::is_constant_expression)
                            && v.is_constant_expression()
                    }
                    ArrayElement::Unpack(v) => v.is_constant_expression(),
                    ArrayElement::Hole => false,
                })
            }
            _ => false,
        }
    }
}
