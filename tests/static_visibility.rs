use php_rs::{ErrorKind, Runtime, run};

#[test]
fn nonpublic_static_properties_reject_external_reads_and_writes() {
    for visibility in ["private", "protected"] {
        let mut rt = Runtime::default();
        rt.execute_code(format!("class A{{{visibility} static $x=1;}}").as_bytes())
            .unwrap();
        for access in ["echo A::$x;", "A::$x=2;", "A::$x++;", "unset(A::$x[0]);"] {
            let error = rt.execute_code(access.as_bytes()).unwrap_err();
            assert_eq!(error.kind, ErrorKind::Error);
            assert!(error.message.contains(visibility), "{error:?}");
        }
    }
}

#[test]
fn inaccessible_static_probes_are_silent_and_do_not_initialize_defaults() {
    assert_eq!(run(b"<?php class A{private static $x=MISSING;protected static $y=1;}var_dump(isset(A::$x),empty(A::$y),A::$x??2);").unwrap(),b"bool(false)\nbool(true)\nint(2)\n");
}

#[test]
fn inherited_private_storage_is_accessible_from_declaring_scope() {
    assert_eq!(run(b"<?php class A{private static $x=1;static function f(){B::$x++;echo self::$x,static::$x;}}class B extends A{}B::f();").unwrap(),b"22");
}

#[test]
fn explicit_child_static_access_selects_child_redeclaration() {
    assert_eq!(run(b"<?php class A{private static $x=1;static function f(){echo B::$x,static::$x,self::$x;}}class B extends A{public static $x=2;}B::f();").unwrap(),b"221");
}

#[test]
fn protected_static_storage_is_shared_across_the_class_family() {
    assert_eq!(run(b"<?php class A{protected static int $x=1;static function f(){B::$y++;echo self::$x;}}class B extends A{protected static int $y=4;static function g(){parent::$x++;echo self::$x,self::$y;}}B::g();A::f();B::g();").unwrap(),b"24235");
}

#[test]
fn private_redeclarations_have_independent_types_and_storage() {
    assert_eq!(run(b"<?php class A{private static int $x=1;static function f(){echo self::$x;}}class B extends A{public static string $x='B';}A::f();echo B::$x;").unwrap(),b"1B");
}

#[test]
fn private_static_property_can_be_redeclared_as_an_instance_property() {
    assert_eq!(run(b"<?php class A{private static $x=1;static function f(){echo self::$x;}}class B extends A{public $x=2;}echo(new B)->x;A::f();").unwrap(),b"21");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private static $x=1;}class B extends A{public $x=2;}")
        .unwrap();
    assert!(
        rt.execute_code(b"echo B::$x;")
            .unwrap_err()
            .message
            .contains("undeclared static property")
    );
}

#[test]
fn inherited_static_visibility_cannot_narrow_and_types_remain_invariant() {
    for code in [
        "class A{public static $x;}class B extends A{protected static $x;}",
        "class A{protected static $x;}class B extends A{private static $x;}",
        "class A{protected static int $x;}class B extends A{public static string $x;}",
        "class A{protected static $x;}class B extends A{public $x;}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
    assert_eq!(run(b"<?php class A{protected static int $x=1;}class B extends A{public static int $x=2;}echo B::$x;").unwrap(),b"2");
}

#[test]
fn static_assignment_rhs_runs_before_visibility_validation() {
    for op in ["=", "+=", "??="] {
        for target in ["A::$x", "A::$x[0]"] {
            let mut rt = Runtime::default();
            rt.execute_code(b"class A{private static $x=[1];}function f(){echo 'rhs';return 2;}")
                .unwrap();
            assert!(
                rt.execute_code(format!("{target}{op}f();").as_bytes())
                    .is_err()
            );
            assert_eq!(rt.output, b"rhs");
        }
    }
}

#[test]
fn compound_static_assignment_reads_storage_after_rhs_side_effects() {
    assert_eq!(run(b"<?php class A{public static $x=1;}function f(){A::$x=10;return 2;}A::$x+=f();echo A::$x;").unwrap(),b"12");
}

#[test]
fn private_static_properties_are_hidden_from_child_property_exists() {
    assert_eq!(run(b"<?php class A{private static $x;protected static $y;}class B extends A{}var_dump(property_exists('A','x'),property_exists('B','x'),property_exists(new B,'x'),property_exists('B','y'));").unwrap(),b"bool(true)\nbool(false)\nbool(false)\nbool(true)\n");
}

#[test]
fn nonpublic_static_properties_enforce_visibility_through_objects() {
    for access in ["echo $a->x;", "$a->x=2;", "unset($a->x);"] {
        let mut rt = Runtime::default();
        rt.execute_code(b"class A{private static $x=1;}$a=new A;")
            .unwrap();
        assert!(
            rt.execute_code(access.as_bytes())
                .unwrap_err()
                .message
                .contains("private")
        );
        assert!(rt.warnings.is_empty());
    }
    assert_eq!(run(b"<?php class A{private static $x=1;}$a=new A;var_dump(isset($a->x),empty($a->x),$a->x??2);").unwrap(),b"bool(false)\nbool(true)\nint(2)\n");
}

#[test]
fn child_dynamic_properties_do_not_alias_parent_private_static_storage() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private static $x=1;static function f(){echo self::$x;}}class B extends A{}$b=new B;$b->x=2;echo $b->x;A::f();").unwrap();
    assert_eq!(rt.output, b"21");
    assert_eq!(rt.warnings.len(), 1);
    assert!(rt.warnings[0].contains("dynamic property"));
}

#[test]
fn cached_static_storage_does_not_bypass_access_checks() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private static $x=1;static function f(){echo self::$x;}}A::f();")
        .unwrap();
    assert!(rt.execute_code(b"echo A::$x;").is_err());
    rt.execute_code(b"var_dump(isset(A::$x));").unwrap();
    assert_eq!(rt.output, b"1bool(false)\n");
}

#[test]
fn typed_nonpublic_static_properties_support_initialization_and_array_mutation() {
    assert_eq!(run(b"<?php class A{private static array $x;static function f(){var_dump(isset(self::$x));self::$x[]='a';self::$x[]='b';unset(self::$x[0]);echo self::$x[1];}}A::f();").unwrap(),b"bool(false)\nb");
}

#[test]
fn static_property_closures_preserve_class_scope() {
    assert_eq!(run(b"<?php class A{private static $x=1;static function f(){return static function(){return ++self::$x;};}}$f=A::f();echo $f(),$f();").unwrap(),b"23");
}

#[test]
fn object_access_to_static_properties_also_evaluates_compound_rhs_first() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private static $x=1;}$a=new A;function f(){echo 'rhs';return 2;}")
        .unwrap();
    assert!(rt.execute_code(b"$a->x+=f();").is_err());
    assert_eq!(rt.output, b"rhs");
}

#[test]
fn compound_assignments_observe_rhs_changes_for_each_storage_kind() {
    for code in [
        "<?php $x=1;function f(){global $x;$x=10;return 2;}$x+=f();echo $x;",
        "<?php $x=[1];function f(){global $x;$x[0]=10;return 2;}$x[0]+=f();echo $x[0];",
        "<?php class A{public $x=1;}$a=new A;function f(){global $a;$a->x=10;return 2;}$a->x+=f();echo $a->x;",
    ] {
        assert_eq!(run(code.as_bytes()).unwrap(), b"12", "{code}");
    }
}

#[test]
fn child_instance_redeclaration_does_not_emit_static_access_notices() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{private static $x=1;static function f($b){echo $b->x;}}class B extends A{public $x=2;}A::f(new B);").unwrap();
    assert_eq!(rt.output, b"2");
    assert!(rt.warnings.is_empty());
}
