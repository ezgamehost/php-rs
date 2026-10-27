use php_rs::{ErrorKind, Runtime, run};

#[test]
fn private_and_protected_methods_reject_external_calls_before_arguments() {
    for visibility in ["private", "protected"] {
        let mut rt = Runtime::default();
        rt.execute_code(
            format!(
                "class A{{{visibility} function f($x){{}}}}function arg(){{echo 'arg';}}$a=new A;"
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(
            rt.execute_code(b"$a->f(arg());").unwrap_err().kind,
            ErrorKind::Error
        );
        assert!(rt.output.is_empty());
    }
}
#[test]
fn protected_methods_are_accessible_across_the_inheritance_family() {
    assert_eq!(run(b"<?php class A{protected function f(){echo 'A';}function call($x){$x->f();}}class B extends A{function g($x){$x->f();}}(new B)->g(new A);(new A)->call(new B);").unwrap(),b"AA");
}
#[test]
fn private_instance_dispatch_retains_lexical_parent_implementation() {
    assert_eq!(run(b"<?php class A{private function f(){echo 'A';}function g($b){$b->f();B::f();$f=[$b,'f'];$f();}}class B extends A{function f(){echo 'B';}}$b=new B;$b->g($b);").unwrap(),b"ABA");
}
#[test]
fn scoped_static_calls_select_explicit_target_class() {
    assert_eq!(run(b"<?php class A{private static function f(){echo 'A';}static function g(){B::f();static::f();self::f();}}class B extends A{static function f(){echo 'B';}}B::g();").unwrap(),b"BBA");
}
#[test]
fn saved_method_closures_keep_authorized_access_and_binding() {
    assert_eq!(run(b"<?php class A{private function f(){return 3;}protected static function g(){return 4;}function first(){return $this->f(...);}static function second(){return self::g(...);}}$f=(new A)->first();$g=A::second();echo $f(),$g();").unwrap(),b"34");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private function f(){}}$a=new A;")
        .unwrap();
    assert!(rt.execute_code(b"$f=$a->f(...);").is_err());
}
#[test]
fn nonpublic_constructors_use_caller_scope_and_fail_before_initialization() {
    assert_eq!(run(b"<?php class A{private function __construct(){echo 'A';}static function make(){return new self;}}A::make();").unwrap(),b"A");
    let mut rt = Runtime::default();
    rt.execute_code(
        b"class A{const X=MISSING;private function __construct($x){}}function arg(){echo 'arg';}",
    )
    .unwrap();
    let error = rt.execute_code(b"new A(arg());").unwrap_err();
    assert!(error.message.contains("private"));
    assert!(rt.output.is_empty());
}
#[test]
fn constructor_dispatch_does_not_select_parent_private_method() {
    assert_eq!(run(b"<?php class A{private function __construct(){echo 'A';}static function make(){return new B;}}class B extends A{function __construct(){echo 'B';}}A::make();").unwrap(),b"B");
}
#[test]
fn nonpublic_clone_methods_enforce_access_and_use_actual_class_method() {
    assert_eq!(run(b"<?php class A{private function __clone(){echo 'A';}function copy(){return clone $this;}}class B extends A{function __clone(){echo 'B';}}(new B)->copy();(new A)->copy();").unwrap(),b"BA");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{protected function __clone(){echo 'bad';}}$a=new A;")
        .unwrap();
    assert!(rt.execute_code(b"clone $a;").is_err());
    assert!(rt.output.is_empty());
}
#[test]
fn private_methods_do_not_impose_override_signatures_or_staticness() {
    assert_eq!(run(b"<?php class A{private function f(int $x):int{return $x;}}class B extends A{static function f(string $x):string{return $x;}}echo B::f('ok');").unwrap(),b"ok");
}
#[test]
fn nonprivate_visibility_can_widen_but_cannot_narrow() {
    for code in [
        "class A{public function f(){}}class B extends A{protected function f(){}}",
        "class A{protected function f(){}}class B extends A{private function f(){}}",
        "interface I{function f();}class A implements I{protected function f(){}}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
    assert_eq!(run(b"<?php abstract class A{abstract protected function f():int;}class B extends A{public function f():int{return 5;}}echo(new B)->f();").unwrap(),b"5");
}
#[test]
fn final_private_methods_warn_but_final_private_constructors_remain_final() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{final private function f(){}}class B extends A{function f(){echo 'B';}}(new B)->f();").unwrap();
    assert_eq!(rt.output, b"B");
    assert_eq!(rt.warnings.len(), 1);
    assert!(Runtime::default().execute_code(b"class A{final private function __construct(){}}class B extends A{function __construct(){}}").is_err());
}
#[test]
fn method_exists_distinguishes_inherited_private_methods_for_names_and_objects() {
    assert_eq!(run(b"<?php class A{private function f(){}}class B extends A{}var_dump(method_exists('A','f'),method_exists('B','f'),method_exists(new B,'f'));").unwrap(),b"bool(true)\nbool(false)\nbool(true)\n");
}
#[test]
fn nonpublic_invoke_warns_but_implicit_calls_remain_available() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private function __invoke(){return 7;}}$a=new A;echo $a();")
        .unwrap();
    assert_eq!(rt.output, b"7");
    assert_eq!(rt.warnings.len(), 1);
    assert!(rt.execute_code(b"$a->__invoke();").is_err());
}
#[test]
fn invalid_visibility_declarations_are_rejected() {
    for code in [
        "class A{public private function f(){}}",
        "abstract class A{abstract private function f();}",
        "interface I{protected function f();}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}

#[test]
fn native_callbacks_can_bind_this_within_the_named_class_scope() {
    assert_eq!(run(b"<?php class A{protected function f($x){return $x+1;}function g(){return array_map('A::f',[2]);}}class B extends A{function h(){return array_map(['A','f'],[3]);}}$b=new B;echo $b->g()[0],$b->h()[0];").unwrap(),b"34");
    assert!(
        Runtime::default()
            .execute_code(b"class A{function f(){}function g(){$f='A::f';$f();}}(new A)->g();")
            .is_err()
    );
    assert!(Runtime::default().execute_code(b"class A{function f(){}function g(){array_map('B::f',[1]);}}class B extends A{}(new B)->g();").is_err());
}

#[test]
fn nonpublic_tostring_is_rejected_when_its_class_is_linked() {
    let mut rt = Runtime::default();
    let error = rt
        .execute_code(b"echo 'before';class A{protected function __toString(){}}echo 'after';")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Error);
    assert_eq!(rt.output, b"before");
    assert_eq!(rt.warnings.len(), 1);
    assert_eq!(
        run(b"<?php if(false){class A{private function __toString(){}}}echo 'ok';").unwrap(),
        b"ok"
    );
}
