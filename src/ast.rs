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
    Assign(Vec<u8>, String, Box<Expr>),
    Increment(Vec<u8>, i64, bool),
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
    Break,
    Continue,
    Return(Option<Expr>),
}
