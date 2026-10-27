use super::builtins::Builtin;
use super::*;
use crate::ast::{Argument, Parameter};
use std::collections::HashSet;

impl Runtime {
    pub(super) fn prepare_functions(&mut self, stmts: &mut [Stmt]) -> Result<(), Error> {
        diagnose_functions(stmts, &mut self.warnings);
        let mut declarations = Vec::new();
        take_hoisted(stmts, &mut declarations);
        let mut names = HashSet::new();
        for decl in &declarations {
            let name = decl.name.to_ascii_lowercase();
            if self.functions.contains_key(&name)
                || Builtin::lookup(&name).is_some()
                || !names.insert(name)
            {
                return Err(redeclared(decl));
            }
        }
        for decl in declarations {
            self.register_function(decl)?;
        }
        Ok(())
    }
    pub(super) fn register_function(&mut self, decl: Rc<FunctionDecl>) -> Result<(), Error> {
        let name = decl.name.to_ascii_lowercase();
        if self.functions.contains_key(&name) || Builtin::lookup(&name).is_some() {
            return Err(redeclared(&decl));
        }
        self.functions.insert(name, decl);
        Ok(())
    }
    pub(super) fn call(
        &mut self,
        callee: &Expr,
        args: &[Argument],
        line: usize,
    ) -> Result<Value, Error> {
        let name = match &callee.kind {
            ExprKind::Constant(name) => name.clone(),
            _ => match self.eval(callee)? {
                Value::String(name) => name,
                _ => return Err(Error::new(ErrorKind::Error, "Value is not callable", line)),
            },
        };
        let normalized_name = name
            .strip_prefix(b"\\")
            .unwrap_or(&name)
            .to_ascii_lowercase();
        let function = self.functions.get(&normalized_name).cloned();
        let builtin = Builtin::lookup(&normalized_name);
        let parameters = if let Some(function) = &function {
            function.parameters.clone()
        } else if let Some(builtin) = builtin {
            builtin.parameters()
        } else {
            return Err(Error::new(
                ErrorKind::Error,
                format!(
                    "Call to undefined function {}()",
                    String::from_utf8_lossy(&name)
                ),
                line,
            ));
        };
        let mut bound = BoundArguments::new(&parameters);
        for arg in args {
            match arg {
                Argument::Positional(expr) => {
                    let value = self.eval(expr)?;
                    bound.push(None, value, &parameters, line)?;
                }
                Argument::Named(name, expr) => {
                    let value = self.eval(expr)?;
                    bound.push(Some(name.clone()), value, &parameters, line)?;
                }
                Argument::Unpack(expr) => {
                    let Value::Array(array) = self.eval(expr)? else {
                        return Err(Error::new(
                            ErrorKind::TypeError,
                            "Only arrays and Traversables can be unpacked",
                            line,
                        ));
                    };
                    for (key, value) in array.entries() {
                        let name = match key {
                            ArrayKey::Int(_) => None,
                            ArrayKey::String(name) => Some(name.clone()),
                        };
                        bound.push(name, value.clone(), &parameters, line)?;
                    }
                }
            }
        }
        if let Some(builtin) = builtin {
            for (index, param) in parameters.iter().enumerate() {
                if !param.variadic && param.default.is_none() && bound.fixed[index].is_none() {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        format!(
                            "{}(): Argument #{} (${}) not passed",
                            String::from_utf8_lossy(&name),
                            index + 1,
                            String::from_utf8_lossy(&param.name)
                        ),
                        line,
                    ));
                }
            }
            if !bound.variadic && !bound.extra.is_empty() {
                return Err(Error::new(
                    ErrorKind::ArgumentCountError,
                    format!(
                        "{}() expects at most {} arguments",
                        String::from_utf8_lossy(&name),
                        parameters.len()
                    ),
                    line,
                ));
            }
            return self.call_builtin(builtin, &name, bound.fixed, bound.extra, line);
        }
        let function = function.unwrap();
        let caller = std::mem::take(&mut self.variables);
        let result = (|| {
            let last_required = function
                .parameters
                .iter()
                .rposition(|p| p.default.is_none() && !p.variadic);
            for (index, param) in function.parameters.iter().enumerate() {
                let value = if param.variadic {
                    Value::Array(Rc::new(std::mem::take(&mut bound.extra)))
                } else if let Some(value) = bound.fixed[index].take() {
                    value
                } else if last_required.is_none_or(|last| index > last)
                    && let Some(default) = &param.default
                {
                    self.eval(default)?
                } else {
                    return Err(Error::new(
                        ErrorKind::ArgumentCountError,
                        format!(
                            "{}(): Argument #{} (${}) not passed",
                            String::from_utf8_lossy(&function.name),
                            index + 1,
                            String::from_utf8_lossy(&param.name)
                        ),
                        line,
                    ));
                };
                self.variables.insert(param.name.clone(), value);
            }
            match self.block(&function.body)? {
                Flow::Return(value) => Ok(value),
                _ => Ok(Value::Null),
            }
        })();
        self.variables = caller;
        result
    }
}
fn take_hoisted(stmts: &mut [Stmt], declarations: &mut Vec<Rc<FunctionDecl>>) {
    for stmt in stmts {
        match stmt {
            Stmt::Function(decl) => {
                declarations.push(decl.clone());
                *stmt = Stmt::Block(Vec::new());
            }
            Stmt::Block(stmts) => take_hoisted(stmts, declarations),
            _ => {}
        }
    }
}
fn redeclared(decl: &FunctionDecl) -> Error {
    Error::new(
        ErrorKind::Error,
        format!(
            "Cannot redeclare function {}()",
            String::from_utf8_lossy(&decl.name)
        ),
        decl.line,
    )
}
struct BoundArguments {
    fixed: Vec<Option<Value>>,
    extra: PhpArray,
    positional: usize,
    saw_named: bool,
    variadic: bool,
}
impl BoundArguments {
    fn new(parameters: &[Parameter]) -> Self {
        Self {
            fixed: vec![None; parameters.len()],
            extra: PhpArray::default(),
            positional: 0,
            saw_named: false,
            variadic: parameters.last().is_some_and(|p| p.variadic),
        }
    }
    fn push(
        &mut self,
        name: Option<Vec<u8>>,
        value: Value,
        parameters: &[Parameter],
        line: usize,
    ) -> Result<(), Error> {
        if let Some(name) = name {
            self.saw_named = true;
            if let Some(index) = parameters
                .iter()
                .position(|p| p.name == name && !p.variadic)
            {
                if self.fixed[index].is_some() {
                    return Err(overwritten(&name, line));
                }
                self.fixed[index] = Some(value);
            } else if self.variadic {
                let key = ArrayKey::String(name.clone());
                if self.extra.get(&key).is_some() {
                    return Err(overwritten(&name, line));
                }
                self.extra.insert(key, value);
            } else {
                return Err(Error::new(
                    ErrorKind::Error,
                    format!(
                        "Unknown named parameter ${}",
                        String::from_utf8_lossy(&name)
                    ),
                    line,
                ));
            }
        } else {
            if self.saw_named {
                return Err(Error::new(
                    ErrorKind::Error,
                    "Cannot use positional argument after named argument during unpacking",
                    line,
                ));
            }
            let index = self.positional;
            self.positional += 1;
            if index < parameters.len() && !parameters[index].variadic {
                self.fixed[index] = Some(value);
            } else {
                self.extra.append(value, line)?;
            }
        }
        Ok(())
    }
}
fn overwritten(name: &[u8], line: usize) -> Error {
    Error::new(
        ErrorKind::Error,
        format!(
            "Named parameter ${} overwrites previous argument",
            String::from_utf8_lossy(name)
        ),
        line,
    )
}

fn diagnose_functions(stmts: &[Stmt], warnings: &mut Vec<String>) {
    for stmt in stmts {
        match stmt {
            Stmt::Function(decl) => {
                if let Some(last) = decl
                    .parameters
                    .iter()
                    .rposition(|p| p.default.is_none() && !p.variadic)
                {
                    for param in &decl.parameters[..last] {
                        if param.default.is_some() {
                            warnings.push(format!("Deprecated: Optional parameter ${} declared before required parameter is implicitly treated as required on line {}", String::from_utf8_lossy(&param.name), decl.line));
                        }
                    }
                }
                diagnose_functions(&decl.body, warnings);
            }
            Stmt::Block(body) => diagnose_functions(body, warnings),
            Stmt::If(_, yes, no) => {
                diagnose_functions(std::slice::from_ref(yes), warnings);
                if let Some(no) = no {
                    diagnose_functions(std::slice::from_ref(no), warnings);
                }
            }
            Stmt::While(_, body)
            | Stmt::DoWhile(body, _)
            | Stmt::For(_, _, _, body)
            | Stmt::Foreach(_, _, _, body) => {
                diagnose_functions(std::slice::from_ref(body), warnings)
            }
            _ => {}
        }
    }
}
