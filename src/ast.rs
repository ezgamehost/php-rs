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
    Constant(Vec<u8>),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Array(Vec<(Option<Expr>, Expr)>),
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

#[derive(Debug, Clone)]
pub(crate) struct MatchArm {
    /// An empty condition list denotes the default arm.
    pub conditions: Vec<Expr>,
    pub result: Expr,
}

#[derive(Debug, Clone)]
pub(crate) enum Stmt {
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
            ExprKind::Variable(_) => true,
            ExprKind::Index(base, _) => base.is_writable(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Parameter {
    pub name: Vec<u8>,
    pub default: Option<Expr>,
    pub variadic: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct FunctionDecl {
    pub name: Vec<u8>,
    pub parameters: Vec<Parameter>,
    pub body: Vec<Stmt>,
    pub line: usize,
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
            ExprKind::Index(a, Some(b)) | ExprKind::Binary(_, a, b) => {
                a.is_constant_expression() && b.is_constant_expression()
            }
            ExprKind::Conditional(a, b, c) => {
                a.is_constant_expression()
                    && b.as_ref().is_none_or(|b| b.is_constant_expression())
                    && c.is_constant_expression()
            }
            ExprKind::Array(elements) => elements.iter().all(|(k, v)| {
                k.as_ref().is_none_or(Expr::is_constant_expression) && v.is_constant_expression()
            }),
            _ => false,
        }
    }
}
