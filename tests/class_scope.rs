use php_rs::{ErrorKind, Runtime, run};

#[test]
fn parent_calls_use_parent_implementation_and_keep_this() {
    assert_eq!(run(b"<?php class A{public $n;function __construct($n){$this->n=$n;}function f(){return 'A'.$this->n;}}class B extends A{function __construct($n){parent::__construct($n+1);}function f(){return parent::f().'B';}}echo (new B(2))->f();").unwrap(),b"A3B");
}
#[test]
fn static_methods_do_not_bind_this_even_when_called_through_an_object() {
    assert_eq!(run(b"<?php class A{static function f($n=1){var_dump(isset($this));return $n;}}echo A::f(n:2),(new A)->f(3);").unwrap(),b"bool(false)\n2bool(false)\n3");
}
#[test]
fn forwarding_calls_preserve_late_bound_class() {
    assert_eq!(run(b"<?php class A{static function who(){return static::class;}static function f(){echo self::who(),static::who();}}class B extends A{static function g(){parent::f();echo A::who();}}B::g();").unwrap(),b"BBA");
}
#[test]
fn new_self_parent_and_static_follow_lexical_and_called_classes() {
    assert_eq!(run(b"<?php class A{}class B extends A{static function f(){echo get_class(new self),get_class(new parent),get_class(new static);}}class C extends B{}C::f();").unwrap(),b"BAC");
}
#[test]
fn class_names_do_not_require_a_loaded_class() {
    assert_eq!(
        run(b"<?php echo Missing::class,'|';class A{}echo (new A)::class;").unwrap(),
        b"Missing|A"
    );
}
#[test]
fn static_method_callables_keep_called_class_and_compare_context() {
    assert_eq!(run(b"<?php class A{static function f(){return static::class;}}class B extends A{}$a=A::f(...);$b=B::f(...);var_dump($a==$b);echo $a(),$b();").unwrap(),b"bool(false)\nAB");
}
#[test]
fn closures_retain_class_scope_and_called_class() {
    assert_eq!(run(b"<?php class A{static function f(){return static fn()=>self::class.static::class;}}class B extends A{}$f=B::f();echo $f();").unwrap(),b"AB");
}
#[test]
fn string_and_array_static_callbacks_and_dynamic_member_names() {
    assert_eq!(run(b"<?php class A{static function f($x){return $x*2;}}$c='A';$m='f';echo $c::$m(2);echo array_map('A::f',[3])[0],array_map(['A','f'],[4])[0];").unwrap(),b"468");
}
#[test]
fn self_parent_and_static_types_validate_in_declaring_and_called_contexts() {
    assert_eq!(run(b"<?php class A{}class B extends A{function f(parent $a,self $b):static{return $this;}}class C extends B{}echo get_class((new C)->f(new A,new B));").unwrap(),b"C");
    assert_eq!(
        Runtime::default()
            .execute_code(
                b"class A{static function f():static{return new self;}}class B extends A{}B::f();"
            )
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
}
#[test]
fn class_inspection_reports_lexical_parent_and_called_class() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{}class B extends A{static function f(){echo get_class(),get_parent_class(),get_called_class();}}class C extends B{}C::f();").unwrap();
    assert_eq!(rt.output, b"BAC");
    assert_eq!(rt.warnings.len(), 2);
}
#[test]
fn staticness_changes_and_static_constructors_are_invalid() {
    for code in [
        "class A{static function f(){}}class B extends A{function f(){}}",
        "class A{function f(){}}class B extends A{static function f(){}}",
        "class A{static function __construct(){}}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}
#[test]
fn nonstatic_static_calls_and_missing_context_fail() {
    for code in [
        "class A{function f(){}}A::f();",
        "new self;",
        "self::class;",
        "static::class;",
        "parent::f();",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}

#[test]
fn contextual_class_constants_fold_in_property_defaults() {
    assert_eq!(run(b"<?php class A{}class B extends a{public $x=parent::class;public $y=self::class;}class C extends B{}$c=new C;echo $c->x,$c->y;").unwrap(),b"aB");
}
#[test]
fn override_variance_uses_each_methods_declaring_class() {
    assert!(
        Runtime::default()
            .execute_code(b"class A{function f(self $x){}}class B extends A{function f(self $x){}}")
            .is_err()
    );
    assert_eq!(run(b"<?php class A{function f():self{return $this;}}class B extends A{function f():static{return $this;}}echo get_class((new B)->f());").unwrap(),b"B");
    assert!(Runtime::default().execute_code(b"class A{function f():static{return $this;}}class B extends A{function f():self{return $this;}}").is_err());
}

#[test]
fn class_context_is_restored_on_errors_and_not_inherited_by_named_functions() {
    let mut rt = Runtime::default();
    rt.execute_code(
        b"class A{static function f(){missing();}}function outside(){return get_called_class();}",
    )
    .unwrap();
    assert!(rt.execute_code(b"A::f();").is_err());
    assert!(rt.execute_code(b"get_called_class();").is_err());
    assert!(
        rt.execute_code(b"class B{static function f(){return outside();}}B::f();")
            .is_err()
    );
    assert_eq!(run(b"<?php class A{static function f(){eval('echo self::class;');return static fn()=>get_called_class();}}class B extends A{}$f=B::f();echo $f();").unwrap(),b"AB");
}
#[test]
fn parent_class_inspection_handles_global_scope_and_invalid_names() {
    let mut rt = Runtime::default();
    rt.execute_code(
        b"var_dump(get_parent_class(),get_parent_class('stdClass'),get_parent_class('Closure'));",
    )
    .unwrap();
    assert_eq!(rt.output, b"bool(false)\nbool(false)\nbool(false)\n");
    assert_eq!(rt.warnings.len(), 1);
    assert_eq!(
        rt.execute_code(b"get_parent_class('Missing');")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
}
#[test]
fn contextual_type_and_magic_method_restrictions_are_compile_errors() {
    for code in [
        "class A{function f(parent $x){}}",
        "class A{function f():parent{}}",
        "class A{function f(\\self $x){}}",
        "class A{function f(static $x){}}",
        "class A{static function __invoke(){}}",
        "class A{static function __get($x){}}",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';{code}").as_bytes())
                .is_err(),
            "{code}"
        );
        assert!(rt.output.is_empty());
    }
}

#[test]
fn default_closures_use_declaring_scope_and_properties_cache_per_class() {
    assert_eq!(run(b"<?php class A{public $f=static function(){return static::class;};static function f($x=static function(){return static::class;}){return $x;}}class B extends A{}$a=new A;$a2=new A;$b=new B;echo ($a->f)(),($b->f)(),(B::f())();var_dump($a->f===$a2->f,$a->f===$b->f,A::f()===A::f());").unwrap(),b"AAAbool(true)\nbool(false)\nbool(false)\n");
}
#[test]
fn inherited_default_closure_can_construct_declaring_class() {
    assert_eq!(run(b"<?php class A{public $f=static function(){return new self;};}class B extends A{}$b=new B;echo get_class(($b->f)());").unwrap(),b"A");
}

#[test]
fn static_method_callables_are_valid_constant_and_default_initializers() {
    assert_eq!(run(b"<?php class A{static function f(){return static::class;}static function g($f=self::f(...)){return $f();}public $f=self::f(...);}class B extends A{}const F=A::f(...);echo (F)(),B::g(),((new B)->f)();").unwrap(),b"AAA");
    assert!(php_rs::lint(b"<?php class A{function f($f=static::g(...)){}}").is_err());
}
