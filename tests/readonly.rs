use php_rs::{ErrorKind, Runtime, run};

#[test]
fn readonly_properties_initialize_once_with_normal_type_coercion() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public readonly int $x;function set($v){return $this->x=$v;}}$a=new A;var_dump($a->set('2'),$a->x);").unwrap();
    assert_eq!(rt.output, b"int(2)\nint(2)\n");
    assert_eq!(
        rt.execute_code(b"$a->set(3);").unwrap_err().kind,
        ErrorKind::Error
    );
    rt.execute_code(b"echo $a->x;").unwrap();
    assert!(rt.output.ends_with(b"2"));
}
#[test]
fn readonly_initialization_requires_protected_set_scope() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public readonly int $x;}$a=new A;")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"$a->x=2;").unwrap_err().kind,
        ErrorKind::Error
    );
    rt.execute_code(b"var_dump(isset($a->x));").unwrap();
    assert_eq!(rt.output, b"bool(false)\n");
    assert_eq!(run(b"<?php class A{function set($b){$b->x=2;}}class B extends A{public readonly int $x;}class C extends B{function init(){$this->x=3;}}$b=new B;(new A)->set($b);$c=new C;$c->init();echo $b->x,$c->x;").unwrap(),b"23");
}
#[test]
fn closures_keep_initialization_scope_but_named_functions_do_not() {
    assert_eq!(run(b"<?php class A{public readonly int $x;function setter(){return fn($v)=>$this->x=$v;}}$a=new A;$f=$a->setter();$f(2);echo $a->x;").unwrap(),b"2");
    assert_eq!(Runtime::default().execute_code(b"function set($a){$a->x=2;}class A{public readonly int $x;function f(){set($this);}}(new A)->f();").unwrap_err().kind,ErrorKind::Error);
}
#[test]
fn null_counts_as_initialized_for_readonly_storage() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public readonly ?int $x;function set(){$this->x=null;}}$a=new A;$a->set();var_dump(isset($a->x));").unwrap();
    assert_eq!(
        rt.execute_code(b"$a->x??=2;").unwrap_err().kind,
        ErrorKind::Error
    );
    assert_eq!(rt.output, b"bool(false)\n");
}
#[test]
fn readonly_coalescing_assignment_can_initialize_and_skip_existing_values() {
    assert_eq!(run(b"<?php class A{public readonly int $x;function init(){$this->x??=2;}}$a=new A;$a->init();$a->init();echo $a->x;$a->x??=3;").unwrap(),b"2");
}
#[test]
fn readonly_blocks_nested_array_mutation_but_allows_stored_objects_to_change() {
    assert_eq!(run(b"<?php class A{public readonly object $o;public readonly array $a;function __construct(){$this->o=new stdClass;$this->a=[new stdClass];}}$a=new A;$a->o->x=2;$copy=$a->a;$copy[0]->y=3;echo $a->o->x,$a->a[0]->y;").unwrap(),b"23");
    for op in [
        "$a->x[]=2;",
        "$a->x[0]=2;",
        "unset($a->x[0]);",
        "$a->x[0]++;",
        "$a->x=[2];",
    ] {
        let mut rt = Runtime::default();
        rt.execute_code(
            b"class A{public readonly array $x;function __construct(){$this->x=[1];}}$a=new A;",
        )
        .unwrap();
        assert_eq!(
            rt.execute_code(op.as_bytes()).unwrap_err().kind,
            ErrorKind::Error,
            "{op}"
        );
        rt.execute_code(b"echo $a->x[0];").unwrap();
        assert_eq!(rt.output, b"1");
    }
}
#[test]
fn uninitialized_readonly_arrays_cannot_auto_initialize() {
    assert_eq!(
        Runtime::default()
            .execute_code(
                b"class A{public readonly array $x;function f(){$this->x[]=2;}}(new A)->f();"
            )
            .unwrap_err()
            .kind,
        ErrorKind::Error
    );
}
#[test]
fn readonly_unset_distinguishes_initialized_roots_and_missing_offsets() {
    assert_eq!(run(b"<?php class A{public readonly int $x;function f(){unset($this->x);$this->x=2;}}$a=new A;unset($a->x[0]);$a->f();echo $a->x;").unwrap(),b"2");
    for init in ["", "$a->f();"] {
        let mut rt = Runtime::default();
        rt.execute_code(b"class A{public readonly int $x;function f(){$this->x=2;}}$a=new A;")
            .unwrap();
        rt.execute_code(init.as_bytes()).unwrap();
        assert_eq!(
            rt.execute_code(b"unset($a->x);").unwrap_err().kind,
            ErrorKind::Error
        );
    }
}
#[test]
fn readonly_modifiers_validate_types_defaults_and_staticness_before_execution() {
    for declaration in [
        "public readonly $x;",
        "public readonly int $x=2;",
        "public readonly ?int $x=null;",
        "public static readonly int $x;",
        "readonly function f(){}",
        "readonly const X=2;",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';class A{{{declaration}}}").as_bytes())
                .is_err()
        );
        assert!(rt.output.is_empty());
    }
}
#[test]
fn readonly_property_inheritance_cannot_change_readonly_status() {
    for (parent, child) in [("readonly ", ""), ("", "readonly ")] {
        assert!(Runtime::default().execute_code(format!("class A{{public {parent}int $x;}}class B extends A{{public {child}int $x;}}").as_bytes()).is_err());
    }
    assert_eq!(run(b"<?php class A{public readonly int $x;}class B extends A{public readonly int $x;function f(){$this->x=2;}}$b=new B;$b->f();echo $b->x;").unwrap(),b"2");
}
#[test]
fn readonly_classes_apply_to_all_properties_and_forbid_dynamic_ones() {
    let mut rt = Runtime::default();
    rt.execute_code(b"readonly class A{public int $x;const C=3;function __construct(){$this->x=2;}}$a=new A;echo $a->x,A::C;").unwrap();
    assert_eq!(rt.output, b"23");
    assert_eq!(
        rt.execute_code(b"$a->extra=1;").unwrap_err().kind,
        ErrorKind::Error
    );
    assert_eq!(
        rt.execute_code(b"$a->x=3;").unwrap_err().kind,
        ErrorKind::Error
    );
    assert!(
        Runtime::default()
            .execute_code(b"readonly class A{public $x;}")
            .is_err()
    );
}
#[test]
fn readonly_class_inheritance_requires_consistent_status_and_accepts_final_modifiers() {
    assert_eq!(run(b"<?php readonly class A{public int $x;}final readonly class B extends A{function __construct(){$this->x=2;}}readonly final class C{}echo(new B)->x,get_class(new C);").unwrap(),b"2C");
    for code in [
        "readonly class A{}class B extends A{}",
        "class A{}readonly class B extends A{}",
        "readonly class A extends stdClass{}",
        "final readonly class A{}readonly class B extends A{}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}
#[test]
fn failed_type_check_leaves_readonly_property_available_for_initialization() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public readonly int $x;function set($v){$this->x=$v;}}$a=new A;")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"$a->set([]);").unwrap_err().kind,
        ErrorKind::TypeError
    );
    rt.execute_code(b"$a->set(2);echo $a->x;").unwrap();
    assert_eq!(rt.output, b"2");
}

#[test]
fn readonly_array_object_writes_are_indirect_mutations() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public readonly array $x;function __construct(){$this->x=[new stdClass];}}$a=new A;").unwrap();
    assert_eq!(
        rt.execute_code(b"$a->x[0]->y=2;").unwrap_err().kind,
        ErrorKind::Error
    );
    rt.execute_code(b"var_dump(isset($a->x[0]->y));").unwrap();
    assert_eq!(rt.output, b"bool(false)\n");
}
#[test]
fn readonly_keyword_can_still_name_a_function() {
    assert_eq!(
        run(b"<?php function readonly(){return 2;}echo readonly();readonly();").unwrap(),
        b"2"
    );
}
