use php_rs::{ErrorKind, Runtime, run};

#[test]
fn interfaces_supply_types_and_inherited_method_contracts() {
    assert_eq!(run(b"<?php interface I{function f():int;}class A implements I{function f():int{return 2;}}class B extends A{}function call(I $x):I{echo $x->f();return $x;}$b=new B;var_dump(call($b) instanceof I,interface_exists('i'),class_exists('I'),method_exists('I','f'));").unwrap(),b"2bool(true)\nbool(true)\nbool(false)\nbool(true)\n");
}
#[test]
fn multiple_interface_inheritance_merges_compatible_contracts() {
    assert_eq!(run(b"<?php interface A{function f(int $x):object;}interface B{function g():int;}interface C extends A,B{}class D implements C,A{function f(int|float $x):D{return $this;}function g():int{return 3;}}$d=new D;echo $d->g();var_dump($d instanceof A,$d instanceof B,$d instanceof C);").unwrap(),b"3bool(true)\nbool(true)\nbool(true)\n");
}
#[test]
fn abstract_classes_may_defer_interface_implementations() {
    assert_eq!(run(b"<?php interface I{function f():int;}abstract class A implements I{}class B extends A{function f():int{return 4;}}echo(new B)->f();").unwrap(),b"4");
    assert!(
        Runtime::default()
            .execute_code(b"interface I{function f();}class A implements I{}")
            .is_err()
    );
}
#[test]
fn interface_signature_variance_and_staticness_are_enforced() {
    for method in [
        "function f(string $x):int{return 1;}",
        "static function f(int $x):int{return 1;}",
        "function f(int $x):string{return '1';}",
        "function f():int{return 1;}",
    ] {
        assert!(
            Runtime::default()
                .execute_code(
                    format!(
                        "interface I{{function f(int $x):int;}}class A implements I{{{method}}}"
                    )
                    .as_bytes()
                )
                .is_err(),
            "{method}"
        );
    }
}
#[test]
fn interfaces_reject_bodies_modifiers_properties_and_promotion() {
    for body in [
        "function f(){}",
        "abstract function f();",
        "final function f();",
        "public $x;",
        "function __construct(public int $x);",
    ] {
        assert!(
            Runtime::default()
                .execute_code(format!("interface I{{{body}}}").as_bytes())
                .is_err(),
            "{body}"
        );
    }
}
#[test]
fn interface_class_roles_and_duplicate_direct_interfaces_are_checked() {
    for code in [
        "interface I{}class A extends I{}",
        "class A{}interface I extends A{}",
        "class A{}class B implements A{}",
        "interface I{}class A implements I,I{}",
        "interface I{}interface J extends I,I{}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}
#[test]
fn interface_instantiation_and_static_calls_fail_before_arguments() {
    let mut rt = Runtime::default();
    rt.execute_code(b"interface I{static function f($x);}function arg(){echo 'arg';}")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"new I(arg());").unwrap_err().kind,
        ErrorKind::Error
    );
    assert_eq!(
        rt.execute_code(b"I::f(arg());").unwrap_err().kind,
        ErrorKind::Error
    );
    assert!(rt.output.is_empty());
}
#[test]
fn interface_constants_support_inheritance_overrides_and_diamonds() {
    assert_eq!(run(b"<?php interface I{const int X=2;}interface J extends I{}interface K extends I{}interface L extends J,K{}class A implements L{}class B implements I{const int X=3;}echo I::X,L::X,A::X,B::X;var_dump(defined('A::X'));echo constant('A::X');").unwrap(),b"2223bool(true)\n2");
}
#[test]
fn interface_constant_ambiguity_finality_and_covariance_are_checked() {
    for code in [
        "interface I{const X=1;}interface J{const X=1;}class A implements I,J{}",
        "interface I{final const X=1;}class A implements I{const X=2;}",
        "interface I{const int X=1;}class A implements I{const string X='1';}",
        "interface I{const X=1;}class B{const X=2;}class A extends B implements I{}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
    assert_eq!(run(b"<?php interface I{const X=1;}interface J{const X=2;}class A implements I,J{const X=3;}echo A::X;").unwrap(),b"3");
}
#[test]
fn interfaces_and_implementors_preserve_declaration_timing() {
    assert_eq!(run(b"<?php var_dump(interface_exists('I'),interface_exists('J'),class_exists('A'));interface I{}interface J extends I{}class A implements J{}var_dump(interface_exists('J'),class_exists('A'));").unwrap(),b"bool(true)\nbool(false)\nbool(false)\nbool(true)\nbool(true)\n");
}
#[test]
fn interface_constructor_contracts_persist_in_descendants() {
    assert!(Runtime::default().execute_code(b"interface I{function __construct(int $x);}class A implements I{function __construct(int $x){}}class B extends A{function __construct(string $x){}}").is_err());
    assert_eq!(run(b"<?php interface I{function __construct(int $x);}class A implements I{function __construct(int|float $x){}}class B extends A{function __construct(int $x){echo $x;}}new B(5);").unwrap(),b"5");
}
#[test]
fn inherited_interface_methods_and_constants_keep_their_declaring_scope() {
    assert_eq!(run(b"<?php interface I{const X=self::Y;const Y=6;function f():self;}interface J extends I{}class A implements J{const Y=7;function f():I{return $this;}}$a=new A;echo A::X;var_dump($a->f() instanceof J);").unwrap(),b"6bool(true)\n");
}
#[test]
fn incompatible_multiple_inherited_methods_fail_without_an_override() {
    assert!(Runtime::default().execute_code(b"interface A{function f(int $x);}interface B{function f(int|float $x);}interface C extends A,B{}").is_err());
    assert_eq!(run(b"<?php interface A{function f(int $x);}interface B{function f(int|float $x);}interface C extends A,B{function f(int|float $x);}echo 'ok';").unwrap(),b"ok");
}
