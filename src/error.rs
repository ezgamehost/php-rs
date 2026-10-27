use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    ParseError,
    TypeError,
    DivisionByZeroError,
    ArithmeticError,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub line: usize,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>, line: usize) -> Self {
        Self {
            kind,
            message: message.into(),
            line,
        }
    }
    pub(crate) fn parse(message: impl Into<String>, line: usize) -> Self {
        Self::new(ErrorKind::ParseError, message, line)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {} on line {}", self.kind, self.message, self.line)
    }
}
impl std::error::Error for Error {}
