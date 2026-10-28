use super::*;
use crate::ast::TypeDecl;

impl Runtime {
    pub(super) fn terminate(&mut self, status: &Value, line: usize) -> Result<Value, Error> {
        let status = if !self.strict_types && matches!(status, Value::Null) {
            self.warnings.push(format!("Deprecated: exit(): Passing null to parameter #1 ($status) of type string|int is deprecated on line {line}"));
            Value::Int(0)
        } else {
            let ty = TypeDecl {
                members: vec!["string".into(), "int".into()],
            };
            self.coerce_type(status.clone(), &ty, self.strict_types, line)?.ok_or_else(|| {
                let given = match status {
                    Value::Bool(true) => "true",
                    Value::Bool(false) => "false",
                    value => builtins::debug_type(value),
                };
                Error::new(ErrorKind::TypeError, format!("exit(): Argument #1 ($status) must be of type string|int, {given} given"), line)
            })?
        };
        match status {
            Value::String(message) => {
                self.output.extend(message);
                self.exit_status.get_or_insert(0);
            }
            Value::Int(code) => self.exit_status = Some(code as i32),
            _ => unreachable!("exit accepts only string or int after coercion"),
        }
        // Unwind evaluation, functions, and included/evaluated units. Only the
        // outer request boundary consumes this signal; the host remains alive.
        Err(Error::new(ErrorKind::UnwindExit, "", line))
    }
}
