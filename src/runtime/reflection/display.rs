use super::*;

impl Runtime {
    pub(super) fn reflection_parameter_string(
        &mut self,
        function: &FunctionReflection,
        index: usize,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        let parameters = function.parameters();
        let parameter = &parameters[index];
        let optional = parameters
            .iter()
            .rposition(|p| !p.variadic && p.default.is_none())
            .is_none_or(|last| index > last);
        let mut out = format!(
            "Parameter #{index} [ <{}> ",
            if optional { "optional" } else { "required" }
        )
        .into_bytes();
        if let Some(ty) = &parameter.ty {
            out.extend(functions::normalize_type(ty.clone()).to_string().bytes());
            out.push(b' ');
        }
        if parameter.by_reference {
            out.push(b'&');
        }
        if parameter.variadic {
            out.extend(b"...");
        }
        out.push(b'$');
        out.extend(&parameter.name);
        if optional && !parameter.variadic {
            if let Some(signature) = function.signature() {
                out.extend(b" = ");
                out.extend(signature.defaults[index].unwrap_or("<default>").bytes());
            } else if let Some(default) = &parameter.default {
                out.extend(b" = ");
                if expressions::literal_expression(default) {
                    let context = function.attribute_context(self);
                    let values = self.attribute_arguments(
                        &context,
                        &Attribute {
                            name: Vec::new(),
                            line,
                            arguments: vec![Argument::Positional(default.clone())],
                        },
                    )?;
                    properties::format_value(&values.get(&ArrayKey::Int(0)).unwrap(), &mut out);
                } else {
                    expressions::expression(default, &mut out);
                }
            }
        }
        out.extend(b" ]");
        Ok(out)
    }

    pub(super) fn reflection_function_string(
        &mut self,
        reflection: &FunctionReflection,
        line: usize,
    ) -> Result<Vec<u8>, Error> {
        self.reflection_function_string_indented(reflection, line, b"")
    }

    pub(super) fn reflection_function_string_indented(
        &mut self,
        reflection: &FunctionReflection,
        line: usize,
        indent: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let function = reflection.target.function.as_ref();
        let closure = reflection.target.closure.as_ref();
        let context = reflection.target.context.as_ref();
        let method = context.is_some();
        let mut out = Vec::new();
        if let Some(comment) = function.and_then(|f| f.doc_comment.as_ref()) {
            out.extend(indent);
            out.extend(comment);
            out.push(b'\n');
        }
        out.extend(indent);
        out.extend(if closure.is_some() {
            b"Closure [ ".as_slice()
        } else if method {
            b"Method [ ".as_slice()
        } else {
            b"Function [ "
        });
        out.extend(if reflection.internal() {
            b"<internal".as_slice()
        } else {
            b"<user"
        });
        if reflection
            .attributes()
            .iter()
            .any(|a| a.name.eq_ignore_ascii_case(b"Deprecated"))
        {
            out.extend(b", deprecated");
        }
        if let Some(extension) = reflection.extension(self) {
            out.push(b':');
            out.extend(extension.bytes());
        }
        if let Some(context) = context.filter(|_| method) {
            if reflection.is_method && !Rc::ptr_eq(&context.called, &context.scope) {
                out.extend(b", inherits ");
                out.extend(&context.scope.name);
            } else if reflection.is_method
                && let Some(parent) = &context.scope.parent
                && let Some(overwritten) = objects::method(parent, reflection.name())
                && overwritten.visibility != Visibility::Private
            {
                out.extend(b", overwrites ");
                out.extend(method_visibility::declaring_name(&overwritten));
            }
            if let Some(prototype) = method_prototype(&context.scope, reflection.name()) {
                out.extend(b", prototype ");
                out.extend(method_visibility::declaring_name(&prototype));
            }
            if reflection.name().eq_ignore_ascii_case(b"__construct") {
                out.extend(b", ctor");
            }
        }
        out.extend(b"> ");
        if let Some(function) = function {
            if function.is_abstract {
                out.extend(b"abstract ");
            }
            if method && reflection.modifiers() & 32 != 0 {
                out.extend(b"final ");
            }
            if function.is_static {
                out.extend(b"static ");
            }
            if method {
                out.extend(match (closure.is_some(), function.visibility) {
                    (_, Visibility::Public) => b"public ".as_slice(),
                    (true, _) => b"<visibility error> ",
                    (false, Visibility::Protected) => b"protected ",
                    (false, Visibility::Private) => b"private ",
                });
            }
        }
        out.extend(if method {
            b"method ".as_slice()
        } else {
            b"function "
        });
        if reflection.returns_reference() {
            out.push(b'&');
        }
        out.extend(reflection.name());
        out.extend(b" ] {\n");
        if !reflection.internal()
            && let Some(function) = function
        {
            out.extend(indent);
            out.extend(b"  @@ ");
            out.extend(&function.source.name);
            out.extend(format!(" {} - {}\n", function.line, function.end_line).bytes());
        }
        if let Some(environment) = closure.and_then(|c| c.environment.as_ref()) {
            let mut names: Vec<_> = environment
                .captures
                .iter()
                .map(|(n, _)| n.clone())
                .collect();
            if let Some(function) = function {
                static_names(&function.body, &mut names);
            }
            if !names.is_empty() {
                out.push(b'\n');
                out.extend(indent);
                out.extend(format!("  - Bound Variables [{}] {{\n", names.len()).bytes());
                for (i, name) in names.iter().enumerate() {
                    out.extend(indent);
                    out.extend(format!("      Variable #{i} [ $").bytes());
                    out.extend(name);
                    out.extend(b" ]\n");
                }
                out.extend(indent);
                out.extend(b"  }\n");
            }
        }
        let parameters = reflection.parameters();
        let return_type = reflection.return_type();
        let tentative = function.and_then(|f| f.tentative_return_type.as_ref());
        if !parameters.is_empty()
            || return_type.is_some()
            || tentative.is_some()
            || reflection.signature().is_some()
        {
            out.push(b'\n');
            out.extend(indent);
            out.extend(format!("  - Parameters [{}] {{\n", parameters.len()).bytes());
            for i in 0..parameters.len() {
                out.extend(indent);
                out.extend(b"    ");
                out.extend(self.reflection_parameter_string(reflection, i, line)?);
                out.push(b'\n');
            }
            out.extend(indent);
            out.extend(b"  }\n");
        }
        if let Some(ty) = return_type.as_ref().or(tentative) {
            let label = if return_type.is_some() {
                "Return"
            } else {
                "Tentative return"
            };
            out.extend(indent);
            out.extend(
                format!(
                    "  - {label} [ {} ]\n",
                    functions::normalize_type(ty.clone())
                )
                .bytes(),
            );
        }
        out.extend(indent);
        out.extend(b"}\n");
        Ok(out)
    }
}

pub(super) fn method_prototype(class: &Rc<Class>, name: &[u8]) -> Option<Rc<FunctionDecl>> {
    if let Some(parent) = &class.parent
        && let Some(method) = objects::method(parent, name)
        && method.visibility != Visibility::Private
    {
        // A concrete constructor can retain an abstract/interface prototype
        // from an earlier ancestor even though concrete constructors do not
        // themselves impose an overriding signature.
        if let Some(prototype) = method_prototype(parent, name) {
            return Some(prototype);
        }
        if !name.eq_ignore_ascii_case(b"__construct") || method.is_abstract {
            return Some(method);
        }
    }
    for interface in &class.interfaces {
        if let Some(method) = objects::method(interface, name) {
            return method_prototype(interface, name).or(Some(method));
        }
    }
    None
}

fn static_names(statements: &[Stmt], names: &mut Vec<Vec<u8>>) {
    for statement in statements {
        match statement {
            Stmt::Static(variables) => {
                for (name, _) in variables {
                    if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
            }
            Stmt::Block(body) => static_names(body, names),
            Stmt::If(_, yes, no) => {
                static_names(std::slice::from_ref(yes), names);
                if let Some(no) = no {
                    static_names(std::slice::from_ref(no), names);
                }
            }
            Stmt::While(_, body)
            | Stmt::DoWhile(body, _)
            | Stmt::For(_, _, _, body)
            | Stmt::Foreach(_, _, _, _, body) => static_names(std::slice::from_ref(body), names),
            Stmt::Try(body, catches, finally) => {
                static_names(body, names);
                for catch in catches {
                    static_names(&catch.body, names);
                }
                if let Some(finally) = finally {
                    static_names(finally, names);
                }
            }
            Stmt::Switch(_, cases) => {
                for case in cases {
                    static_names(&case.body, names);
                }
            }
            _ => {}
        }
    }
}
