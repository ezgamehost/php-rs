mod ast;
mod error;
mod lexer;
mod parser;
pub mod phpt;
mod runtime;
mod value;

pub use error::{Error, ErrorKind};
pub use runtime::Runtime;
pub use value::Value;

/// Validate syntax without executing the document.
pub fn lint(source: &[u8]) -> Result<(), Error> {
    parser::parse(lexer::lex(source)?)?;
    Ok(())
}

/// Execute a PHP document, including any text outside PHP tags.
pub fn run(source: &[u8]) -> Result<Vec<u8>, Error> {
    let mut runtime = Runtime::default();
    runtime.execute(source)?;
    Ok(runtime.output)
}
