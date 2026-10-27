use crate::Value;

#[derive(Debug, Clone)]
pub(crate) struct Expr {
    pub kind: ExprKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
    Literal(Value),
    Group(Box<Expr>),
    Variable(Vec<u8>),
    Constant(Vec<u8>),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Array(Vec<(Option<Expr>, Expr)>),
    Index(Box<Expr>, Option<Box<Expr>>),
    Isset(Vec<Expr>),
    Empty(Box<Expr>),
    Assign(Box<Expr>, String, Box<Expr>),
    Increment(Box<Expr>, i64, bool),
    Conditional(Box<Expr>, Option<Box<Expr>>, Box<Expr>),
}

#[derive(Debug)]
pub(crate) enum Stmt {
    Text(Vec<u8>),
    Echo(Vec<Expr>),
    Expr(Expr),
    Block(Vec<Stmt>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    Foreach(Expr, Option<Expr>, Expr, Box<Stmt>),
    Unset(Vec<Expr>),
    Break,
    Continue,
    Return(Option<Expr>),
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
