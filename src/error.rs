use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Exception,
    /// Internal control signal, consumed at the request execution boundary.
    #[doc(hidden)]
    UnwindExit,
    ParseError,
    CompileError,
    TypeError,
    DivisionByZeroError,
    ArithmeticError,
    ArgumentCountError,
    ValueError,
    UnhandledMatchError,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub line: usize,
    pub(crate) throwable: Option<std::rc::Rc<crate::PhpObject>>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>, line: usize) -> Self {
        Self {
            kind,
            message: message.into(),
            line,
            throwable: None,
        }
    }
    pub(crate) fn parse(message: impl Into<String>, line: usize) -> Self {
        Self::new(ErrorKind::ParseError, message, line)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(object) = &self.throwable {
            write!(
                f,
                "{}: {} on line {}",
                object.class_name, self.message, self.line
            )
        } else {
            write!(f, "{:?}: {} on line {}", self.kind, self.message, self.line)
        }
    }
}
impl std::error::Error for Error {}
