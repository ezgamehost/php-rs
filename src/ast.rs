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
    Global(Vec<Expr>),
    Static(Vec<(Vec<u8>, Option<Expr>)>),
    Const(Vec<(Vec<u8>, Expr)>),
    Function(Rc<FunctionDecl>),
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
    pub fn is_writable(&self) -> bool {
        match &self.kind {
            ExprKind::Variable(_) | ExprKind::DynamicVariable(_) => true,
            ExprKind::Index(base, _) => base.is_writable(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Parameter {
    pub name: Vec<u8>,
    pub ty: Option<TypeDecl>,
    pub implicit_nullable: bool,
    pub default: Option<Expr>,
    pub variadic: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct FunctionDecl {
    pub name: Vec<u8>,
    pub parameters: Vec<Parameter>,
    pub body: Vec<Stmt>,
    pub line: usize,
    pub return_type: Option<TypeDecl>,
    pub strict_types: bool,
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
            ExprKind::Literal(_) | ExprKind::Constant(_) => true,
            ExprKind::Group(e) | ExprKind::Unary(_, e) => e.is_constant_expression(),
            ExprKind::Cast(kind, e) => *kind != CastKind::Void && e.is_constant_expression(),
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
