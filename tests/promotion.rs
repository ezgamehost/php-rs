use php_rs::{ErrorKind, Runtime, run};

#[test]
fn promoted_properties_are_assigned_before_the_body_and_use_coerced_arguments() {
    assert_eq!(run(b"<?php class A{function __construct(public int $x,public string $y='ok'){echo $this->x,':',$this->y;}}new A('2');").unwrap(),b"2:ok");
}
#[test]
fn promoted_parameters_and_properties_have_independent_array_values() {
    assert_eq!(run(b"<?php class A{function __construct(public array $x){$x[]=2;$this->x[]=3;echo count($x),count($this->x);}}$v=[1];$a=new A($v);echo count($v);var_dump($a->x);").unwrap(),b"221array(2) {\n  [0]=>\n  int(1)\n  [1]=>\n  int(3)\n}\n");
}
#[test]
fn defaults_belong_to_parameters_not_inherited_property_storage() {
    assert_eq!(run(b"<?php class A{function __construct(public int $x=2,public $y=3){}}class B extends A{function __construct(){}}$b=new B;var_dump(isset($b->x),$b->y);echo(new A)->x;").unwrap(),b"bool(false)\nNULL\n2");
}
#[test]
fn named_and_inherited_constructor_calls_initialize_promoted_properties() {
    assert_eq!(run(b"<?php class A{function __construct(public int $x,public string $y='a'){}}class B extends A{}class C extends A{function __construct(public int $z){parent::__construct($z+1,'c');}}$b=new B(y:'b',x:2);$c=new C(3);echo $b->x,$b->y,$c->x,$c->y,$c->z;").unwrap(),b"2b4c3");
}
#[test]
fn readonly_promotion_accepts_parameter_defaults_and_rejects_reinitialization() {
    let mut rt = Runtime::default();
    rt.execute_code(
        b"class A{function __construct(public readonly int $x=2){echo $this->x;}}$a=new A;",
    )
    .unwrap();
    assert_eq!(
        rt.execute_code(b"$a->__construct(3);").unwrap_err().kind,
        ErrorKind::Error
    );
    assert_eq!(rt.output, b"2");
    assert_eq!(run(b"<?php readonly class B{function __construct(public int $x=3){}}class C{function __construct(readonly int $x=4){}}echo(new B)->x,(new C)->x;").unwrap(),b"34");
}
#[test]
fn promoted_and_explicit_properties_cannot_redeclare_the_same_name() {
    for code in [
        "class A{public $x;function __construct(public $x){}}",
        "class A{function __construct(public $x){}public $x;}",
        "class A{public static $x;function __construct(public $x){}}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}
#[test]
fn promotion_rejects_nonconstructors_variadics_callable_types_and_implicit_nullable_defaults() {
    for code in [
        "function f(public int $x){}",
        "class A{function f(public int $x){}}",
        "$f=function(public int $x){};",
        "class A{function __construct(public ...$x){}}",
        "class A{function __construct(public callable $x){}}",
        "class A{function __construct(public int $x=null){}}",
        "readonly class A{function __construct(public $x){}}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}
#[test]
fn argument_validation_finishes_before_any_promoted_property_assignment() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{function __construct(public int $x=1,public int $y=2){}}$a=new A;")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"$a->__construct(3,'bad');")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    rt.execute_code(b"echo $a->x,$a->y;").unwrap();
    assert_eq!(rt.output, b"12");
}
#[test]
fn promotions_follow_declaration_order_and_preserve_parameter_introspection() {
    assert_eq!(run(b"<?php class A{public $a=1;function __construct(public $b=2){echo func_num_args();var_dump(func_get_args());$b=3;echo $this->b;}public $c=4;}$a=new A;foreach($a as $k=>$v){echo $k,$v;}").unwrap(),b"0array(0) {\n}\n2a1b2c4");
}
#[test]
fn inherited_promotions_validate_property_type_invariance_and_contextual_types() {
    assert_eq!(run(b"<?php class A{}class B extends A{function __construct(public parent $x){}}echo get_class((new B(new A))->x);").unwrap(),b"A");
    assert!(Runtime::default().execute_code(b"class A{public int $x;}class B extends A{function __construct(public string $x){}}").is_err());
}
