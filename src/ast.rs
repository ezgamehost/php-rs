use crate::Value;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub(crate) struct Expr {
    pub kind: ExprKind,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum ExprKind {
    Literal(Value),
    Interpolated(Vec<Expr>),
    Group(Box<Expr>),
    Variable(Vec<u8>),
    DynamicVariable(Box<Expr>),
    Constant(Vec<u8>),
    NamespacedName(Box<ResolvedName>),
    Unary(String, Box<Expr>),
    Cast(CastKind, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Array(Vec<ArrayElement>),
    LongArray(Vec<ArrayElement>),
    List(Vec<ArrayElement>),
    Destructure(Vec<ArrayElement>),
    Index(Box<Expr>, Option<Box<Expr>>),
    Call(Box<Expr>, Vec<Argument>),
    Callable(Box<Expr>),
    Lambda(Rc<LambdaDecl>),
    AnonymousClass(Rc<ClassDecl>),
    New(Box<Expr>, Vec<Argument>),
    Property(Box<Expr>, Box<Expr>),
    NullsafeProperty(Box<Expr>, Box<Expr>),
    ScopedMember(Box<Expr>, Box<Expr>),
    StaticProperty(Box<Expr>, Box<Expr>),
    ClassName(Box<Expr>),
    Eval(Box<Expr>),
    Include(IncludeKind, Box<Expr>),
    Isset(Vec<Expr>),
    Empty(Box<Expr>),
    Print(Box<Expr>),
    Throw(Box<Expr>),
    Assign(Box<Expr>, String, Box<Expr>),
    Increment(Box<Expr>, i64, bool),
    Conditional(Box<Expr>, Option<Box<Expr>>, Box<Expr>),
    Match(Box<Expr>, Vec<MatchArm>),
}

/// Static source names resolve separately for classes, functions, and constants.
/// Dynamic strings deliberately do not carry this lexical namespace context.
#[derive(Debug, Clone)]
pub(crate) struct ResolvedName {
    pub class: Vec<u8>,
    pub function: Vec<u8>,
    pub constant: Vec<u8>,
    pub function_fallback: Option<Vec<u8>>,
    pub constant_fallback: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CastKind {
    Int,
    Float,
    Bool,
    String,
    Array,
    Object,
    Void,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum IncludeKind {
    Include,
    IncludeOnce,
    Require,
    RequireOnce,
}

#[derive(Debug, Clone)]
pub(crate) enum ArrayElement {
    Reference(Option<Expr>, Expr),
    Entry(Option<Expr>, Expr),
    Unpack(Expr),
    Hole,
}

#[derive(Debug, Clone)]
pub(crate) struct MatchArm {
    /// An empty condition list denotes the default arm.
    pub conditions: Vec<Expr>,
    pub result: Expr,
}

#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Label(Vec<u8>, usize),
    Goto(Vec<u8>, usize),
    Global(Vec<Expr>),
    Static(Vec<(Vec<u8>, Option<Expr>)>),
    Const(Vec<(Vec<u8>, Expr)>),
    Function(Rc<FunctionDecl>),
    Class(Rc<ClassDecl>),
    Text(Vec<u8>),
    Echo(Vec<Expr>),
    Expr(Expr),
    Block(Vec<Stmt>),
    Try(Vec<Stmt>, Vec<CatchClause>, Option<Vec<Stmt>>),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(Vec<Expr>, Vec<Expr>, Vec<Expr>, Box<Stmt>),
    Switch(Expr, Vec<SwitchCase>),
    Foreach(Expr, Option<Expr>, Expr, Box<Stmt>),
    Unset(Vec<Expr>),
    Break(usize),
    Continue(usize, usize),
    Return(Option<Expr>),
}

#[derive(Debug, Clone)]
pub(crate) struct SwitchCase {
    pub condition: Option<Expr>,
    pub body: Vec<Stmt>,
    pub semicolon_line: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct CatchClause {
    pub types: Vec<Vec<u8>>,
    pub variable: Option<Vec<u8>>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum NativeMethod {
    ThrowableConstruct,
    ErrorExceptionConstruct,
    ThrowableToString,
    GetMessage,
    GetCode,
    GetFile,
    GetLine,
    GetPrevious,
    GetTrace,
    GetTraceAsString,
    GetSeverity,
}

impl Expr {
    pub fn static_class_name(&self) -> Option<&[u8]> {
        match &self.kind {
            ExprKind::Constant(name) => Some(name),
            ExprKind::NamespacedName(name) => Some(&name.class),
            _ => None,
        }
    }

    pub fn is_globals(&self) -> bool {
        match &self.kind {
            ExprKind::Variable(name) => name == b"GLOBALS",
            ExprKind::Group(inner) => inner.is_globals(),
            _ => false,
        }
    }

    pub fn is_writable(&self) -> bool {
        if self.has_nullsafe_chain() {
            return false;
        }
        match &self.kind {
            ExprKind::Variable(_)
            | ExprKind::DynamicVariable(_)
            | ExprKind::Property(_, _)
            | ExprKind::StaticProperty(_, _) => true,
            ExprKind::Index(base, _) => {
                let mut base = base.as_ref();
                while let ExprKind::Group(inner) = &base.kind {
                    base = inner;
                }
                base.is_writable()
            }
            _ => false,
        }
    }

    /// Only member calls propagate a short circuit. Arguments, dynamic calls,
    /// class constants, and class-name expressions start independent chains.
    pub fn is_nullsafe_callee(&self) -> bool {
        match &self.kind {
            ExprKind::NullsafeProperty(_, _) => true,
            ExprKind::Property(base, _) | ExprKind::ScopedMember(base, _) => {
                base.has_nullsafe_chain()
            }
            _ => false,
        }
    }

    pub fn has_nullsafe_chain(&self) -> bool {
        match &self.kind {
            ExprKind::NullsafeProperty(_, _) => true,
            ExprKind::Property(base, _)
            | ExprKind::StaticProperty(base, _)
            | ExprKind::Index(base, _)
            | ExprKind::Group(base) => base.has_nullsafe_chain(),
            ExprKind::Call(callee, _) => callee.is_nullsafe_callee(),
            ExprKind::Callable(callee) => {
                matches!(&callee.kind, ExprKind::ScopedMember(base, _) if base.has_nullsafe_chain())
            }
            _ => false,
        }
    }

    pub fn is_isset_target(&self) -> bool {
        match &self.kind {
            ExprKind::Variable(_)
            | ExprKind::DynamicVariable(_)
            | ExprKind::Property(_, _)
            | ExprKind::NullsafeProperty(_, _)
            | ExprKind::StaticProperty(_, _)
            | ExprKind::Index(_, _) => true,
            ExprKind::Group(inner) => inner.is_isset_target(),
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClassDecl {
    pub is_interface: bool,
    pub is_trait: bool,
    pub trait_uses: Vec<TraitUse>,
    pub trait_requirements: Vec<Rc<FunctionDecl>>,
    pub trait_property_checks: Vec<(PropertyDecl, PropertyDecl)>,
    pub trait_constant_checks: Vec<(ClassConstant, ClassConstant)>,
    pub interfaces: Vec<Vec<u8>>,
    pub parent: Option<Vec<u8>>,
    pub is_abstract: bool,
    pub is_final: bool,
    pub is_readonly: bool,
    pub final_methods: Vec<Vec<u8>>,
    pub name: Vec<u8>,
    pub properties: Vec<PropertyDecl>,
    pub static_properties: Vec<PropertyDecl>,
    pub constants: Vec<ClassConstant>,
    pub methods: Vec<Rc<FunctionDecl>>,
    pub line: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TraitUse {
    pub names: Vec<Vec<u8>>,
    pub adaptations: Vec<TraitAdaptation>,
}

#[derive(Debug, Clone)]
pub(crate) enum TraitAdaptation {
    InsteadOf {
        source: Vec<u8>,
        method: Vec<u8>,
        excluded: Vec<Vec<u8>>,
    },
    Alias {
        source: Option<Vec<u8>>,
        method: Vec<u8>,
        alias: Option<Vec<u8>>,
        visibility: Option<Visibility>,
        is_final: bool,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PropertyDecl {
    pub declaring_class: Vec<u8>,
    pub visibility: Visibility,
    pub set_visibility: Visibility,
    pub is_final: bool,
    pub is_readonly: bool,
    pub name: Vec<u8>,
    pub default: Option<Expr>,
    pub ty: Option<TypeDecl>,
}

impl PropertyDecl {
    pub fn storage_name(&self) -> Vec<u8> {
        self.storage_name_for(&self.declaring_class)
    }
    pub fn storage_name_for(&self, owner: &[u8]) -> Vec<u8> {
        let mut key = Vec::new();
        if self.visibility != Visibility::Public {
            key.push(0);
            key.extend(if self.visibility == Visibility::Private {
                owner
            } else {
                b"*"
            });
            key.push(0);
        }
        key.extend(&self.name);
        key
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ClassConstant {
    pub visibility: Visibility,
    pub name: Vec<u8>,
    pub value: Expr,
    pub ty: Option<TypeDecl>,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Promotion {
    pub visibility: Visibility,
    pub set_visibility: Option<Visibility>,
    pub is_readonly: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Parameter {
    pub by_reference: bool,
    pub promotion: Option<Promotion>,
    pub name: Vec<u8>,
    pub ty: Option<TypeDecl>,
    pub implicit_nullable: bool,
    pub default: Option<Expr>,
    pub variadic: bool,
}
#[derive(Debug, Clone)]
pub(crate) struct FunctionDecl {
    pub trait_origin: Option<Vec<u8>>,
    pub native: Option<NativeMethod>,
    pub visibility: Visibility,
    pub is_abstract: bool,
    pub is_static: bool,
    pub name: Vec<u8>,
    pub parameters: Vec<Parameter>,
    pub body: Vec<Stmt>,
    pub line: usize,
    pub return_type: Option<TypeDecl>,
    pub strict_types: bool,
    pub source: Rc<crate::source::Source>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Visibility {
    Public,
    Protected,
    Private,
}

#[derive(Debug, Clone)]
pub(crate) struct LambdaDecl {
    pub function: Rc<FunctionDecl>,
    pub captures: Vec<Capture>,
    pub arrow: bool,
    pub is_static: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct Capture {
    pub name: Vec<u8>,
    pub by_reference: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TypeDecl {
    pub members: Vec<String>,
}

impl TypeDecl {
    pub fn contains(&self, name: &str) -> bool {
        self.members.iter().any(|member| member == name)
    }

    pub fn accepts(&self, value: &Value) -> bool {
        self.contains("mixed")
            || self.members.iter().any(|member| {
                if let Value::Object(object) = value
                    && (member == "object" || object.class.is_a(member.as_bytes()))
                {
                    return true;
                }
                if matches!(value, Value::Closure(_))
                    && member
                        .trim_start_matches('\\')
                        .eq_ignore_ascii_case("Closure")
                {
                    return true;
                }
                matches!(
                    (member.as_str(), value),
                    ("null", Value::Null)
                        | ("bool", Value::Bool(_))
                        | ("true", Value::Bool(true))
                        | ("false", Value::Bool(false))
                        | ("int", Value::Int(_))
                        | ("float", Value::Float(_))
                        | ("string", Value::String(_))
                        | ("array" | "iterable", Value::Array(_))
                        | ("object" | "callable", Value::Closure(_))
                )
            })
    }
}

impl std::fmt::Display for TypeDecl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.members.len() == 2 && self.contains("null") {
            write!(
                f,
                "?{}",
                self.members.iter().find(|m| *m != "null").unwrap()
            )
        } else {
            f.write_str(&self.members.join("|"))
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) enum Argument {
    Positional(Expr),
    Named(Vec<u8>, Expr),
    Unpack(Expr),
}
impl Expr {
    pub fn is_constant_expression(&self) -> bool {
        self.constant_expression(true)
    }

    pub fn is_class_initializer(&self) -> bool {
        self.constant_expression(false)
    }

    fn constant_expression(&self, allow_object_cast: bool) -> bool {
        match &self.kind {
            ExprKind::Lambda(lambda) => {
                lambda.is_static && !lambda.arrow && lambda.captures.is_empty()
            }
            ExprKind::Callable(callee) => match &callee.kind {
                ExprKind::Constant(_) | ExprKind::NamespacedName(_) => true,
                ExprKind::ScopedMember(class, name) => {
                    class
                        .static_class_name()
                        .is_some_and(|name| !name.eq_ignore_ascii_case(b"static"))
                        && matches!(name.kind, ExprKind::Literal(Value::String(_)))
                }
                _ => false,
            },
            ExprKind::ScopedMember(class, name) => {
                class
                    .static_class_name()
                    .is_some_and(|name| !name.eq_ignore_ascii_case(b"static"))
                    && name.constant_expression(allow_object_cast)
            }
            ExprKind::Literal(_) | ExprKind::Constant(_) | ExprKind::NamespacedName(_) => true,
            ExprKind::ClassName(class) => class
                .static_class_name()
                .is_some_and(|name| !name.eq_ignore_ascii_case(b"static")),
            ExprKind::Unary(op, _) if op == "@" => false,
            ExprKind::Group(e) | ExprKind::Unary(_, e) => e.constant_expression(allow_object_cast),
            ExprKind::Cast(kind, e) => {
                *kind != CastKind::Void
                    && (allow_object_cast || *kind != CastKind::Object)
                    && e.constant_expression(allow_object_cast)
            }
            ExprKind::Binary(op, _, _) if matches!(op.as_str(), "|>" | "instanceof") => false,
            ExprKind::Index(a, Some(b))
            | ExprKind::Binary(_, a, b)
            | ExprKind::Property(a, b)
            | ExprKind::NullsafeProperty(a, b) => {
                a.constant_expression(allow_object_cast) && b.constant_expression(allow_object_cast)
            }
            ExprKind::Conditional(a, b, c) => {
                a.constant_expression(allow_object_cast)
                    && b.as_ref()
                        .is_none_or(|b| b.constant_expression(allow_object_cast))
                    && c.constant_expression(allow_object_cast)
            }
            ExprKind::Array(elements) | ExprKind::LongArray(elements) => {
                elements.iter().all(|element| match element {
                    ArrayElement::Entry(k, v) => {
                        k.as_ref()
                            .is_none_or(|key| key.constant_expression(allow_object_cast))
                            && v.constant_expression(allow_object_cast)
                    }
                    ArrayElement::Unpack(v) => v.constant_expression(allow_object_cast),
                    ArrayElement::Reference(_, _) | ArrayElement::Hole => false,
                })
            }
            _ => false,
        }
    }
}
