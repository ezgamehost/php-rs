use php_rs::{ErrorKind, Runtime, run};

#[test]
fn concrete_descendants_implement_abstract_methods_and_inherit_bodies() {
    assert_eq!(run(b"<?php abstract class A{public $x=3;abstract function value():int;function show(){echo $this->value();}}abstract class B extends A{}class C extends B{function value():int{return $this->x;}}$c=new C;$c->show();var_dump($c instanceof A,class_exists('A'),method_exists('A','value'));").unwrap(),b"3bool(true)\nbool(true)\nbool(true)\n");
}
#[test]
fn abstract_instantiation_fails_before_constants_and_arguments_are_evaluated() {
    let mut rt = Runtime::default();
    rt.execute_code(b"abstract class A{const X=MISSING;}function arg(){echo 'arg';}")
        .unwrap();
    let error = rt.execute_code(b"new A(arg());").unwrap_err();
    assert_eq!(error.kind, ErrorKind::Error);
    assert!(
        error
            .message
            .contains("Cannot instantiate abstract class A")
    );
    assert!(rt.output.is_empty());
}
#[test]
fn concrete_classes_cannot_leave_inherited_methods_abstract() {
    assert!(
        Runtime::default()
            .execute_code(b"abstract class A{abstract function f();}class B extends A{}")
            .is_err()
    );
    assert_eq!(run(b"<?php abstract class A{abstract function f();}class B extends A{function f(){return 4;}}class C extends B{}echo(new C)->f();").unwrap(),b"4");
}
#[test]
fn abstract_method_declaration_rules_apply_even_in_unreachable_code() {
    for declaration in [
        "class A{abstract function f();}",
        "abstract final class A{}",
        "abstract abstract class A{}",
        "abstract class A{abstract function f(){}}",
        "abstract class A{function f();}",
        "abstract class A{abstract final function f();}",
        "abstract class A{abstract function __construct(public int $x);}",
        "abstract class A{abstract public int $x;}",
        "abstract class A{abstract const X=1;}",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'before';if(false){{{declaration}}}").as_bytes())
                .is_err(),
            "{declaration}"
        );
        assert!(rt.output.is_empty(), "{declaration}");
    }
}
#[test]
fn abstract_static_methods_dispatch_to_concrete_subclasses() {
    assert_eq!(run(b"<?php abstract class A{abstract static function f():string;static function call(){return static::f();}}class B extends A{static function f():string{return static::class;}}echo B::call();$f=B::f(...);echo $f();").unwrap(),b"BB");
}
#[test]
fn direct_abstract_calls_and_callables_fail_before_argument_evaluation() {
    for call in ["A::f(arg());", "$f=A::f(...);", "$f=['A','f'];$f(arg());"] {
        let mut rt = Runtime::default();
        rt.execute_code(
            b"abstract class A{abstract static function f($x);}function arg(){echo 'arg';}",
        )
        .unwrap();
        let error = rt.execute_code(call.as_bytes()).unwrap_err();
        assert_eq!(error.kind, ErrorKind::Error);
        assert!(error.message.contains("abstract method"));
        assert!(rt.output.is_empty());
    }
    assert!(Runtime::default().execute_code(b"abstract class A{abstract function f();}class B extends A{function f(){parent::f();}}(new B)->f();").unwrap_err().message.contains("abstract method"));
}
#[test]
fn abstract_constructors_require_compatible_implementations() {
    assert!(Runtime::default().execute_code(b"abstract class A{abstract function __construct(int $x);}class B extends A{function __construct(string $x){}}").is_err());
    assert_eq!(run(b"<?php abstract class A{abstract function __construct(int $x);}class B extends A{function __construct(public int|float $x=2){}}echo(new B)->x;").unwrap(),b"2");
}
#[test]
fn abstract_redeclarations_preserve_variance_and_cannot_replace_concrete_methods() {
    assert_eq!(run(b"<?php abstract class A{abstract function f(int $x):object;}abstract class B extends A{abstract function f(int|float $x):B;}class C extends B{function f(int|float $x):C{return $this;}}echo get_class((new C)->f(1));").unwrap(),b"C");
    for code in [
        "class A{function f(){}}abstract class B extends A{abstract function f();}",
        "abstract class A{abstract function f(int|float $x);}abstract class B extends A{abstract function f(int $x);}",
        "abstract class A{abstract static function f();}class B extends A{function f(){}}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}
#[test]
fn readonly_abstract_classes_and_factories_preserve_late_static_binding() {
    assert_eq!(run(b"<?php abstract readonly class A{function __construct(public int $x=5){}static function make():static{return new static;}abstract function value():int;}readonly class B extends A{function value():int{return $this->x;}}echo B::make()->value();").unwrap(),b"5");
}
#[test]
fn conditional_abstract_declarations_follow_runtime_linking() {
    assert_eq!(run(b"<?php if(true){abstract class A{abstract function f();}}class B extends A{function f(){return 7;}}echo(new B)->f();").unwrap(),b"7");
    assert_eq!(run(b"<?php if(false){abstract class A{abstract function f();}}var_dump(class_exists('A'));").unwrap(),b"bool(false)\n");
}

#[test]
fn abstract_constructor_contract_survives_intermediate_implementations() {
    assert!(Runtime::default().execute_code(b"abstract class A{abstract function __construct(int $x);}class B extends A{function __construct(int $x){}}class C extends B{function __construct(string $x){}}").is_err());
    assert_eq!(run(b"<?php abstract class A{abstract function __construct(int $x);}abstract class B extends A{abstract function __construct(int|float $x);}class C extends B{function __construct(int $x){echo $x;}}new C(2);").unwrap(),b"2");
}
