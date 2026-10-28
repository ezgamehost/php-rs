use php_rs::{Runtime, run};

#[test]
fn asymmetric_static_properties_are_publicly_readable_but_restrict_writes() {
    for setter in ["private", "protected"] {
        let mut rt = Runtime::default();
        rt.execute_code(
            format!("class A{{public {setter}(set) static int $x=1;}}echo A::$x;").as_bytes(),
        )
        .unwrap();
        let error = rt.execute_code(b"A::$x=2;").unwrap_err();
        assert!(error.message.contains(&format!("{setter}(set)")));
        assert_eq!(rt.output, b"1");
    }
}

#[test]
fn setter_only_declarations_default_to_public_reads_and_accept_modifier_orders() {
    for decl in [
        "private(set) static int",
        "static private(set) int",
        "private(set) public static int",
        "public static private(set) int",
        "static public private(set) int",
    ] {
        assert_eq!(
            run(format!(
                "<?php class A{{{decl} $x=1;static function f(){{self::$x++;}}}}A::f();echo A::$x;"
            )
            .as_bytes())
            .unwrap(),
            b"2"
        );
    }
}

#[test]
fn protected_setters_allow_parent_and_child_scopes() {
    assert_eq!(run(b"<?php class A{public protected(set) static int $x=1;static function f(){B::$y++;}}class B extends A{public protected(set) static int $y=3;static function g(){parent::$x++;}}B::g();A::f();echo A::$x,B::$y;").unwrap(),b"24");
}

#[test]
fn private_setters_allow_declaring_scope_through_inherited_storage() {
    assert_eq!(run(b"<?php class A{public private(set) static int $x=1;static function f(){static::$x++;}}class B extends A{}B::f();echo B::$x;").unwrap(),b"2");
    assert!(Runtime::default().execute_code(b"class A{public private(set) static int $x=1;}class B extends A{static function f(){self::$x++;}}B::f();").is_err());
}

#[test]
fn private_set_makes_nonprivate_properties_final() {
    for vis in ["public", "protected"] {
        let mut rt = Runtime::default();
        rt.execute_code(format!("class A{{{vis} private(set) static int $x=1;}}").as_bytes())
            .unwrap();
        assert!(
            rt.execute_code(b"class B extends A{public static int $x=2;}")
                .unwrap_err()
                .message
                .contains("final")
        );
    }
    assert_eq!(run(b"<?php class A{private private(set) static int $x=1;}class B extends A{public static string $x='B';}echo B::$x;").unwrap(),b"B");
}

#[test]
fn explicit_final_static_properties_allow_mutation_but_not_redeclaration() {
    assert_eq!(
        run(b"<?php class A{final public static int $x=1;}A::$x++;echo A::$x;").unwrap(),
        b"2"
    );
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{final protected static int $x=1;}")
        .unwrap();
    assert!(
        rt.execute_code(b"class B extends A{public static int $x=2;}")
            .unwrap_err()
            .message
            .contains("final")
    );
}

#[test]
fn setter_visibility_can_widen_but_not_narrow_in_children() {
    assert_eq!(run(b"<?php class A{public protected(set) static int $x=1;}class B extends A{public static int $x=2;}B::$x=3;echo B::$x;").unwrap(),b"3");
    for code in [
        "class A{public static int $x=1;}class B extends A{public protected(set) static int $x=2;}",
        "class A{protected static int $x=1;}class B extends A{public private(set) static int $x=2;}",
    ] {
        assert!(Runtime::default().execute_code(code.as_bytes()).is_err());
    }
}

#[test]
fn invalid_setter_modifiers_and_final_private_properties_are_rejected() {
    for declaration in [
        "public private(set) static $x=1;",
        "public public(set) static $x=1;",
        "private public(set) static int $x=1;",
        "protected public(set) static int $x=1;",
        "public private(set) protected(set) static int $x=1;",
        "public private(foo) static int $x=1;",
        "final private static int $x=1;",
        "public private(set) static function f(){}",
        "public private(set) const X=1;",
    ] {
        assert!(
            Runtime::default()
                .execute_code(format!("class A{{{declaration}}}").as_bytes())
                .is_err(),
            "{declaration}"
        );
    }
}

#[test]
fn coalescing_skips_setter_checks_when_no_write_is_needed() {
    assert_eq!(run(b"<?php class A{public private(set) static array $x=[1];}echo A::$x[0]??=2;var_dump(isset(A::$x),empty(A::$x));").unwrap(),b"1bool(true)\nbool(false)\n");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public private(set) static ?int $x;}")
        .unwrap();
    assert!(rt.execute_code(b"A::$x??=2;").is_err());
}

#[test]
fn array_mutations_require_setter_access_even_for_missing_unset_keys() {
    for operation in [
        "A::$x[]=2;",
        "A::$x[0]=2;",
        "A::$x[0]++;",
        "A::$x[0]+=2;",
        "unset(A::$x[0]);",
        "unset(A::$x[9]);",
    ] {
        let mut rt = Runtime::default();
        rt.execute_code(b"class A{public private(set) static array $x=[1];}")
            .unwrap();
        assert!(
            rt.execute_code(operation.as_bytes())
                .unwrap_err()
                .message
                .contains("indirectly modify private(set)"),
            "{operation}"
        );
        rt.execute_code(b"echo A::$x[0];").unwrap();
        assert_eq!(rt.output, b"1");
    }
}

#[test]
fn authorized_setters_can_initialize_and_mutate_typed_arrays() {
    assert_eq!(run(b"<?php class A{public private(set) static array $x;static function f(){self::$x[]='a';self::$x[]='b';unset(self::$x[0]);}}A::f();echo A::$x[1];").unwrap(),b"b");
}

#[test]
fn direct_object_mutations_are_allowed_but_array_object_writes_need_setter_access() {
    assert_eq!(run(b"<?php class Box{public $x=1;}class A{public private(set) static object $o;static function f(){self::$o=new Box;}}A::f();A::$o->x=2;echo A::$o->x;").unwrap(),b"2");
    let mut rt = Runtime::default();
    rt.execute_code(b"class Box{public $x=1;}class A{public private(set) static array $a;static function f(){self::$a=[new Box];}}A::f();").unwrap();
    assert!(
        rt.execute_code(b"A::$a[0]->x=2;")
            .unwrap_err()
            .message
            .contains("indirectly modify private(set)")
    );
    rt.execute_code(b"$copy=A::$a;$copy[0]->x=3;echo A::$a[0]->x;")
        .unwrap();
    assert_eq!(rt.output, b"3");
}

#[test]
fn setter_validation_precedes_arithmetic_and_type_checks_after_rhs() {
    for operation in ["A::$x=f();", "A::$x+=f();"] {
        let mut rt = Runtime::default();
        rt.execute_code(
            b"class A{public private(set) static int $x=1;}function f(){echo 'rhs';return [];}",
        )
        .unwrap();
        assert!(
            rt.execute_code(operation.as_bytes())
                .unwrap_err()
                .message
                .contains("private(set)")
        );
        assert_eq!(rt.output, b"rhs");
    }
}

#[test]
fn uninitialized_increment_and_unset_keep_their_original_behavior() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public private(set) static int $x;}")
        .unwrap();
    assert!(
        rt.execute_code(b"A::$x++;")
            .unwrap_err()
            .message
            .contains("before initialization")
    );
    rt.execute_code(b"unset(A::$x[0]);").unwrap();
}

#[test]
fn closures_retain_setter_access_and_object_syntax_creates_separate_dynamic_storage() {
    assert_eq!(run(b"<?php class A{public private(set) static int $x=1;static function f(){return static fn()=>++self::$x;}}$f=A::f();echo $f();$a=new A;$a->x=8;echo $a->x,A::$x;").unwrap(),b"282");
}

#[test]
fn nested_unset_rejects_scalar_offsets_in_authorized_setter_scope() {
    for value in ["1", "1.5", "true"] {
        let mut rt = Runtime::default();
        rt.execute_code(format!("class A{{public private(set) static mixed $x={value};static function f(){{unset(self::$x[0]->n);}}}}").as_bytes()).unwrap();
        assert!(
            rt.execute_code(b"A::f();")
                .unwrap_err()
                .message
                .contains("Cannot unset offset")
        );
    }
}

#[test]
fn uninitialized_asymmetric_static_storage_can_be_probed_by_nested_unset() {
    assert_eq!(run(b"<?php class A{public private(set) static array $x;}unset(A::$x[0]->n);var_dump(isset(A::$x));").unwrap(),b"bool(false)\n");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private static array $x;}")
        .unwrap();
    assert!(
        rt.execute_code(b"unset(A::$x[0]->n);")
            .unwrap_err()
            .message
            .contains("private property")
    );
}
