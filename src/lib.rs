mod array;
mod ast;
mod closure;
mod diagnostics;
mod error;
mod lexer;
mod native_directory;
mod object;
mod parser;
pub mod phpt;
mod resource;
mod runtime;
mod source;
mod string;
mod value;

pub use array::{ArrayKey, PhpArray};
pub use closure::Closure;
pub use error::{Error, ErrorKind};
pub use object::PhpObject;
pub use resource::PhpResource;
pub use runtime::Runtime;
pub use string::PhpString;
pub use value::Value;

/// Validate syntax without executing the document.
pub fn lint(source: &[u8]) -> Result<(), Error> {
    parser::parse(lexer::lex(source)?)?;
    Ok(())
}

/// Execute a PHP document and its shutdown callbacks, including text outside PHP
/// tags. Execution errors remain returned even if an exception handler runs.
pub fn run(source: &[u8]) -> Result<Vec<u8>, Error> {
    let mut runtime = Runtime::default();
    let execution = runtime.execute(source);
    if let Err(error) = &execution {
        runtime.report_uncaught(error);
    }
    let shutdown = runtime.shutdown();
    execution?;
    shutdown?;
    Ok(runtime.output)
}
