use php_rs::{ErrorKind, Runtime, run};

#[test]
fn nonpublic_constants_reject_global_access() {
    for visibility in ["private", "protected"] {
        let mut rt = Runtime::default();
        rt.execute_code(format!("class A{{{visibility} const X=1;}}").as_bytes())
            .unwrap();
        for access in [
            "echo A::X;",
            "echo constant('A::X');",
            "$a='A';$n='X';echo $a::{$n};",
        ] {
            let error = rt.execute_code(access.as_bytes()).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Error);
            assert!(error.message.contains(visibility), "{error:?}");
        }
    }
}

#[test]
fn defined_hides_inaccessible_constants_without_evaluating_them() {
    assert_eq!(run(b"<?php class A{private const X=MISSING;protected const Y=MISSING;}class B extends A{}var_dump(defined('A::X'),defined('A::Y'),defined('B::X'),defined('B::Y')); ").unwrap(), b"bool(false)\nbool(false)\nbool(false)\nbool(false)\n");
}

#[test]
fn private_constants_use_lexical_scope_and_are_not_inherited() {
    assert_eq!(run(b"<?php class A{private const X=1;static function f(){echo self::X,constant('self::X');var_dump(defined('self::X'));}}class B extends A{public const X=2;}B::f();echo B::X;").unwrap(),b"11bool(true)\n2");
    let mut rt = Runtime::default();
    rt.execute_code(
        b"class A{private const X=1;static function f(){return B::X;}}class B extends A{}",
    )
    .unwrap();
    for access in ["echo B::X;", "A::f();"] {
        assert!(
            rt.execute_code(access.as_bytes())
                .unwrap_err()
                .message
                .contains("Undefined constant B::X")
        );
    }
}

#[test]
fn protected_constants_are_available_through_the_class_family() {
    assert_eq!(run(b"<?php class A{protected const X=1;static function f(){echo B::Y;}}class B extends A{protected const Y=2;static function g(){echo parent::X,self::X,constant('A::X');var_dump(defined('A::X'));}}A::f();B::g();").unwrap(), b"2111bool(true)\n");
}

#[test]
fn unrelated_scope_cannot_access_protected_constants() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{protected const X=1;}class B{static function f(){return A::X;}}")
        .unwrap();
    assert!(rt.execute_code(b"B::f();").is_err());
}

#[test]
fn visibility_checks_apply_even_after_constant_values_are_cached() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private const X=1;static function f(){return self::X;}}echo A::f();")
        .unwrap();
    assert!(rt.execute_code(b"echo A::X;").is_err());
    rt.execute_code(b"var_dump(defined('A::X'));").unwrap();
    assert_eq!(rt.output, b"1bool(false)\n");
}

#[test]
fn internal_initialization_can_evaluate_nonpublic_constants_and_defaults() {
    assert_eq!(run(b"<?php class A{private const X=3;protected const Y=self::X+1;public $x=self::X;public static $y=self::Y;}echo(new A)->x,A::$y;").unwrap(),b"34");
}

#[test]
fn child_initialization_does_not_evaluate_uninherited_private_constants() {
    assert_eq!(
        run(b"<?php class A{private const X=MISSING;}class B extends A{}new B;echo 'ok';").unwrap(),
        b"ok"
    );
}

#[test]
fn constant_initializers_enforce_the_declaring_scope() {
    assert_eq!(run(b"<?php class A{private const X=3;const Y=self::X;}class B extends A{const Z=parent::Y;}echo B::Z;").unwrap(),b"3");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private const X=3;}class B extends A{const Y=parent::X;}")
        .unwrap();
    assert!(rt.execute_code(b"echo B::Y;").is_err());
}

#[test]
fn constants_can_widen_visibility_but_cannot_narrow_it() {
    assert_eq!(
        run(b"<?php class A{protected const X=1;}class B extends A{public const X=2;}echo B::X;")
            .unwrap(),
        b"2"
    );
    for (parent, child) in [
        ("public", "protected"),
        ("public", "private"),
        ("protected", "private"),
    ] {
        assert!(
            Runtime::default()
                .execute_code(
                    format!(
                        "class A{{{parent} const X=1;}}class B extends A{{{child} const X=2;}}"
                    )
                    .as_bytes()
                )
                .is_err()
        );
    }
}

#[test]
fn private_constant_redeclarations_have_independent_types() {
    assert_eq!(run(b"<?php class A{private const int X=1;}class B extends A{public const string X='ok';}echo B::X;").unwrap(),b"ok");
    assert_eq!(run(b"<?php class A{private const int X=1;}class B extends A{}class C extends B{const X='ok';}echo C::X;").unwrap(),b"ok");
}

#[test]
fn private_constants_do_not_conflict_with_interface_constants() {
    assert_eq!(run(b"<?php interface I{const X=2;}class A{private const X=1;}class B extends A implements I{}echo B::X;").unwrap(),b"2");
}

#[test]
fn interface_constants_require_public_implementations() {
    for visibility in ["private", "protected"] {
        assert!(
            Runtime::default()
                .execute_code(
                    format!(
                        "interface I{{const X=1;}}class A implements I{{{visibility} const X=2;}}"
                    )
                    .as_bytes()
                )
                .is_err()
        );
        assert!(
            Runtime::default()
                .execute_code(format!("interface I{{{visibility} const X=1;}}").as_bytes())
                .is_err()
        );
    }
}

#[test]
fn private_constants_cannot_be_final_and_protected_final_cannot_be_overridden() {
    for code in [
        "class A{final private const X=1;}",
        "class A{final protected const X=1;}class B extends A{public const X=2;}",
    ] {
        assert!(Runtime::default().execute_code(code.as_bytes()).is_err());
    }
}

#[test]
fn closures_keep_the_class_scope_for_constant_access() {
    assert_eq!(run(b"<?php class A{private const X=7;static function f(){return static fn()=>self::X;}static function g(){return static function(){return constant('A::X');};}}$f=A::f();$g=A::g();echo $f(),$g();").unwrap(),b"77");
}

#[test]
fn interface_initializers_cannot_access_implementing_class_protected_constants() {
    let mut rt = Runtime::default();
    rt.execute_code(b"interface I{const Y=A::X;}class A implements I{protected const X=1;}")
        .unwrap();
    let error = rt.execute_code(b"echo I::Y;").unwrap_err();
    assert!(
        error
            .message
            .contains("Cannot access protected constant A::X")
    );
}
