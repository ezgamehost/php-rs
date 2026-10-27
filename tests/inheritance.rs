use php_rs::{ErrorKind, Runtime, run};

#[test]
fn inherits_properties_constructor_and_overrides_virtual_calls() {
    assert_eq!(run(b"<?php class Child extends Base{public $name='child';function label(){return $this->name;}}class Base{public $name='base';public $n=0;function __construct($n){$this->n=$n;}function label(){return 'base';}function show(){return $this->label().$this->n;}}echo (new Child(3))->show();").unwrap(),b"child3");
}
#[test]
fn overridden_properties_retain_parent_order_and_use_child_defaults() {
    assert_eq!(run(b"<?php class A{public $a=1,$b=2;}class B extends A{public $a=3,$c=4;}foreach(new B as $k=>$v){echo $k,$v;}").unwrap(),b"a3b2c4");
}
#[test]
fn inherited_methods_and_callbacks_keep_shared_static_storage() {
    assert_eq!(run(b"<?php class A{function f(){static $n=0;return ++$n;}}class B extends A{}class C extends A{function f(){static $n=10;return ++$n;}}echo (new A)->f(),(new B)->f(),((new B)->f(...))(),(new C)->f();").unwrap(),b"12311");
}
#[test]
fn parent_types_accept_descendants_in_arguments_and_returns() {
    assert_eq!(run(b"<?php class A{}class B extends A{}class C extends B{}function f(A $v):B{return $v;}$v=f(new C);var_dump($v instanceof A,$v instanceof B,$v instanceof C,$v instanceof Missing);").unwrap(),b"bool(true)\nbool(true)\nbool(true)\nbool(false)\n");
}
#[test]
fn instanceof_dynamic_names_objects_and_non_objects() {
    assert_eq!(run(b"<?php class A{}class B extends A{}$b=new B;$name='a';$a=new A;var_dump($b instanceof $name,$b instanceof $a,$a instanceof B,null instanceof A,1 instanceof $name,!$b instanceof A,$b instanceof A instanceof B);").unwrap(),b"bool(true)\nbool(true)\nbool(false)\nbool(false)\nbool(false)\nbool(false)\nbool(false)\n");
}
#[test]
fn conditional_parent_registration_controls_child_linking() {
    assert_eq!(
        run(b"<?php if(true){class A{public $x=2;}}class B extends A{}echo (new B)->x;").unwrap(),
        b"2"
    );
    let mut rt = Runtime::default();
    assert_eq!(
        rt.execute_code(b"echo 'before';class C extends Missing{}echo 'bad';")
            .unwrap_err()
            .kind,
        ErrorKind::Error
    );
    assert_eq!(rt.output, b"before");
}
#[test]
fn inherited_constructor_can_be_replaced_without_implicit_parent_call() {
    assert_eq!(run(b"<?php class A{public $n=1;function __construct($x){$this->n=$x;}}class B extends A{function __construct(){}}echo (new B)->n;").unwrap(),b"1");
}
#[test]
fn stdclass_subclasses_inherit_dynamic_property_permission() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A extends stdClass{}class B extends A{}$b=new B;$b->x=2;var_dump($b instanceof stdClass);").unwrap();
    assert_eq!(rt.output, b"bool(true)\n");
    assert!(rt.warnings.is_empty());
}
#[test]
fn final_classes_and_methods_cannot_be_overridden() {
    for code in [
        "final class A{}class B extends A{}",
        "class A{final function f(){}}class B extends A{function f(){}}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
    assert_eq!(
        run(b"<?php final class A{public final function f(){return 1;}}echo (new A)->f();")
            .unwrap(),
        b"1"
    );
}
#[test]
fn method_signatures_allow_parameter_contravariance_and_return_covariance() {
    assert_eq!(run(b"<?php class A{}class B extends A{}class P{function f(B $x):A{return $x;}}class C extends P{function f(A $x):B{return new B;}}echo get_class((new C)->f(new A));").unwrap(),b"B");
}
#[test]
fn incompatible_overrides_fail_before_output() {
    for method in [
        "function f(string $x):int{return 1;}",
        "function f(int $x,int $y):int{return 1;}",
        "function f(int $x){return 1;}",
        "function f(int $x):string{return 'a';}",
    ] {
        let mut rt = Runtime::default();
        assert!(rt.execute_code(format!("echo 'bad';class A{{function f(int $x):int{{return 1;}}}}class B extends A{{{method}}}").as_bytes()).is_err());
        assert!(rt.output.is_empty());
    }
}

#[test]
fn instanceof_skips_dynamic_rhs_for_non_objects() {
    let mut rt = Runtime::default();
    rt.execute_code(b"function name(){echo 'bad';return 'stdClass';}var_dump(1 instanceof (name()),null instanceof $missing);").unwrap();
    assert_eq!(rt.output, b"bool(false)\nbool(false)\n");
    assert!(rt.warnings.is_empty());
}
#[test]
fn instanceof_rhs_syntax_and_exponent_precedence() {
    assert_eq!(run(b"<?php $a=new stdClass;var_dump($a instanceof (new stdClass),1 instanceof stdClass ** 2);").unwrap(),b"bool(true)\nint(0)\n");
    for expr in [
        "1 instanceof 'stdClass'",
        "1 instanceof $f()",
        "1 instanceof []",
        "1 instanceof (3)",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';{expr};").as_bytes())
                .is_err()
        );
        assert!(rt.output.is_empty());
    }
}
#[test]
fn explicit_mixed_return_cannot_be_removed_in_override() {
    assert!(
        Runtime::default()
            .execute_code(b"class A{function f():mixed{}}class B extends A{function f(){}}")
            .is_err()
    );
}

#[test]
fn early_binding_respects_declaration_order() {
    assert_eq!(run(b"<?php var_dump(class_exists('B'));class B extends A{}class A{}var_dump(class_exists('B'));").unwrap(),b"bool(false)\nbool(true)\n");
    assert_eq!(
        run(b"<?php var_dump(class_exists('B'));class A{}class B extends A{}").unwrap(),
        b"bool(true)\n"
    );
    let mut rt = Runtime::default();
    assert!(
        rt.execute_code(b"echo 'before';class C extends B{}class B extends A{}class A{}")
            .is_err()
    );
    assert_eq!(rt.output, b"before");
}
#[test]
fn closure_is_covariant_with_callable() {
    assert_eq!(run(b"<?php class P{function f():callable{return fn()=>1;}}class Q extends P{function f():Closure{return fn()=>2;}}echo ((new Q)->f())();").unwrap(),b"2");
}
