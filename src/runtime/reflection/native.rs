use super::*;

#[derive(Clone, Copy)]
pub(super) struct NativeSignature {
    pub name: &'static str,
    pub extension: &'static str,
    pub declaration: &'static str,
    pub defaults: &'static [Option<&'static str>],
    pub by_value: &'static [bool],
}

impl NativeSignature {
    pub fn function(self) -> Rc<FunctionDecl> {
        thread_local! {
            static DECLARATIONS: RefCell<std::collections::HashMap<&'static str, Rc<FunctionDecl>>> = RefCell::default();
        }
        DECLARATIONS.with(|declarations| {
            declarations
                .borrow_mut()
                .entry(self.declaration)
                .or_insert_with(|| {
                    let tokens =
                        lexer::lex(self.declaration.as_bytes()).expect("native signature tokens");
                    let (mut statements, _, _) =
                        parser::parse(tokens).expect("native signature declaration");
                    let Stmt::Function(mut function) = statements.remove(0) else {
                        unreachable!()
                    };
                    // Native omitted-default sentinels are represented by null for parsing,
                    // but must not acquire userland implicit-nullability semantics.
                    for parameter in &mut Rc::make_mut(&mut function).parameters {
                        if parameter.implicit_nullable {
                            parameter
                                .ty
                                .as_mut()
                                .unwrap()
                                .members
                                .retain(|m| m != "null");
                            parameter.implicit_nullable = false;
                        }
                    }
                    function
                })
                .clone()
        })
    }
}

impl FunctionReflection {
    pub(super) fn signature(&self) -> Option<NativeSignature> {
        self.target
            .builtin
            .and_then(|_| super::native_signatures::lookup(&self.target.name.to_ascii_lowercase()))
    }

    pub(super) fn return_type(&self) -> Option<crate::ast::TypeDecl> {
        self.target
            .function
            .as_ref()
            .and_then(|f| f.return_type.clone())
            .or_else(|| {
                self.signature()
                    .and_then(|s| s.function().return_type.clone())
            })
    }

    pub(super) fn returns_reference(&self) -> bool {
        self.target
            .function
            .as_ref()
            .is_some_and(|f| f.returns_reference)
            || self
                .signature()
                .is_some_and(|s| s.function().returns_reference)
    }

    pub(super) fn extension(&self) -> Option<&'static str> {
        self.signature().map(|s| s.extension).or_else(|| {
            self.target
                .function
                .as_ref()
                .and_then(|f| f.native)
                .map(|native| match native {
                    NativeMethod::Reflection => "Reflection",
                    NativeMethod::RecursiveIterator
                    | NativeMethod::FixedArray
                    | NativeMethod::LinkedList
                    | NativeMethod::ObjectStorage
                    | NativeMethod::Heap
                    | NativeMethod::FileInfo
                    | NativeMethod::DirectoryIterator
                    | NativeMethod::DualIterator
                    | NativeMethod::ArrayIterator => "SPL",
                    _ => "Core",
                })
        })
    }
}
