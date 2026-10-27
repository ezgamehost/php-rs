use php_rs::{ErrorKind, Runtime, run};

#[test]
fn scalar_union_and_nullable_constants_keep_their_types() {
    assert_eq!(run(b"<?php class A{const int X=2,Y=3;const float F=2;const string|array S=[];const ?int N=null;const true T=true;const false B=false;}var_dump(A::X,A::Y,A::F,A::S,A::N,A::T,A::B);").unwrap(),b"int(2)\nint(3)\nfloat(2)\narray(0) {\n}\nNULL\nbool(true)\nbool(false)\n");
}
#[test]
fn literal_type_errors_are_rejected_before_execution_even_in_dead_code() {
    for declaration in [
        "const int X='2';",
        "const string X=2;",
        "const bool X=1;",
        "const int X=2.0;",
        "const int X=null;",
        "const array X=false;",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';if(false){{class A{{{declaration}}}}}").as_bytes())
                .is_err(),
            "{declaration}"
        );
        assert!(rt.output.is_empty());
    }
}
#[test]
fn unresolved_values_are_checked_lazily_without_weak_coercion() {
    for strict in [0, 1] {
        let mut rt = Runtime::default();
        rt.execute_code(format!("declare(strict_types={strict});class A{{const int X=VALUE;}}define('VALUE','2');echo 'ready';").as_bytes()).unwrap();
        for _ in 0..2 {
            assert_eq!(
                rt.execute_code(b"echo A::X;").unwrap_err().kind,
                ErrorKind::TypeError
            );
        }
        assert_eq!(rt.output, b"ready");
    }
}
#[test]
fn evaluated_expression_values_and_integer_to_float_conversion_are_cached() {
    assert_eq!(run(b"<?php class A{const int X=1+'2';const float Y=VALUE;}define('VALUE',4);var_dump(A::X,A::Y,A::Y);").unwrap(),b"int(3)\nfloat(4)\nfloat(4)\n");
}
#[test]
fn constant_inheritance_is_covariant_and_preserves_explicit_mixed() {
    assert_eq!(run(b"<?php class A{const X=1;const int|string Y=2;const iterable Z=[];}class B extends A{const int X=2;const int Y=3;const array Z=[];}echo B::X,B::Y,count(B::Z);").unwrap(),b"230");
    for (parent, child) in [
        ("int", "string"),
        ("int", "int|string"),
        ("mixed", ""),
        ("float", "int"),
    ] {
        let mut rt = Runtime::default();
        assert!(rt.execute_code(format!("echo 'bad';class A{{const {parent} X=VALUE;}}class B extends A{{const {child} X=VALUE;}}").as_bytes()).is_err());
        assert!(rt.output.is_empty());
    }
}
#[test]
fn object_and_contextual_types_resolve_in_the_declaring_class() {
    assert_eq!(run(b"<?php class P{}class Q extends P{}define('OBJ',new Q);class A{const P X=OBJ;const ?self Y=null;const ?static N=null;}class B extends A{const Q X=OBJ;const ?self Y=null;const ?parent Z=null;}echo get_class(A::X),get_class(B::X);var_dump(B::N,B::Y,B::Z);").unwrap(),b"QQNULL\nNULL\nNULL\n");
}
#[test]
fn typed_closure_constants_and_inherited_defaults_work() {
    assert_eq!(run(b"<?php class A{const Closure F=static function(){return 3;};const int X=2;static $n=self::X;public $p=self::X;static function f($x=self::X){return $x;}}class B extends A{}echo (B::F)(),B::$n,(new B)->p,B::f();var_dump(A::F===B::F);").unwrap(),b"3222bool(true)\n");
}
#[test]
fn unsupported_constant_types_and_redundant_unions_are_rejected() {
    for ty in [
        "callable",
        "void",
        "never",
        "int|callable",
        "?mixed",
        "bool|true",
        "int|int",
        "\\int",
    ] {
        assert!(
            Runtime::default()
                .execute_code(format!("class A{{const {ty} X=VALUE;}}").as_bytes())
                .is_err(),
            "{ty}"
        );
    }
}
#[test]
fn constant_names_can_still_be_type_keywords() {
    assert_eq!(run(b"<?php class A{const int=2,float=3;const string array='x';}echo A::int,A::float,A::array;").unwrap(),b"23x");
}
#[test]
fn failed_type_checks_restore_context_and_do_not_poison_other_constants() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{const int X=VALUE,Y=3;}define('VALUE','bad');")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"A::X;").unwrap_err().kind,
        ErrorKind::TypeError
    );
    assert!(rt.execute_code(b"self::class;").is_err());
    rt.execute_code(b"echo A::Y;").unwrap();
    assert_eq!(rt.output, b"3");
}

#[test]
fn folded_short_circuit_values_are_checked_before_execution() {
    for value in ["NAN && true", "true || strlen(...)", "false && MISSING"] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(
                format!("echo 'bad';class A{{const int X={value};}}echo 'bad';").as_bytes()
            )
            .is_err()
        );
        assert!(rt.output.is_empty());
    }
}
#[test]
fn closure_values_remain_lazy_even_when_their_declared_type_cannot_match() {
    let mut rt = Runtime::default();
    rt.execute_code(
        b"class A{const int X=static function(){};const int Y=strlen(...);}echo 'ready';",
    )
    .unwrap();
    for code in [b"A::X;".as_slice(), b"A::Y;"] {
        assert_eq!(
            rt.execute_code(code).unwrap_err().kind,
            ErrorKind::TypeError
        );
    }
    assert_eq!(rt.output, b"ready");
}
