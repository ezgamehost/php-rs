use php_rs::{ErrorKind, Runtime, run};

#[test]
fn inherited_constants_keep_declaring_scope_and_allow_overrides() {
    assert_eq!(run(b"<?php class A{const X=1,Y=self::X;static function f(){echo self::X,static::X;}}class B extends A{const X=2,Z=parent::Y;}echo B::Y,B::Z;B::f();").unwrap(),b"1112");
}
#[test]
fn constants_support_dynamic_names_objects_arrays_and_builtins() {
    assert_eq!(run(b"<?php class A{public const X=['a'=>3];}$c='a';$n='X';echo $c::{$n}['a'],(new A)::X['a'],constant('A::X')['a'];var_dump(defined('A::X'),defined('A::x'),defined('Missing::X'));").unwrap(),b"333bool(true)\nbool(false)\nbool(false)\n");
}
#[test]
fn constants_are_lazy_and_cycles_report_errors() {
    assert_eq!(
        run(b"<?php class A{const X=MISSING,Y=2;}echo A::Y;").unwrap(),
        b"2"
    );
    for code in [
        "class A{const X=self::X;}echo A::X;",
        "class A{const X=B::X;}class B{const X=A::X;}echo A::X;",
    ] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::Error
        );
    }
}
#[test]
fn constant_defaults_and_final_inheritance_are_validated() {
    assert_eq!(run(b"<?php class A{final const X=3;public $x=self::X;static function f($x=self::X){return $x;}}class B extends A{}echo (new B)->x,B::f();").unwrap(),b"33");
    for code in [
        "class A{const X=1,X=2;}",
        "class A{const class=2;}",
        "class A{final const X=1;}class B extends A{const X=2;}",
        "class A{const X=static::Y;}",
        "class A{var const X=1;}",
        "class A{var static $x;}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}
#[test]
fn invalid_constant_name_types_do_not_coerce() {
    assert_eq!(
        Runtime::default()
            .execute_code(b"class A{const X=1;}$n=1;echo A::{$n};")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
}
#[test]
fn inherited_static_storage_is_shared_until_redeclared() {
    assert_eq!(run(b"<?php class A{public static $n=1;}class B extends A{}class C extends A{public static $n=8;}B::$n++;echo A::$n,B::$n,C::$n;C::$n=9;echo A::$n,C::$n;").unwrap(),b"22829");
}
#[test]
fn static_properties_follow_self_parent_and_late_static_binding() {
    assert_eq!(run(b"<?php class A{static $n=1;static function f(){echo self::$n,static::$n;}}class B extends A{static $n=2;static function g(){parent::$n=3;parent::f();}}B::g();").unwrap(),b"32");
}
#[test]
fn dynamic_static_names_and_methods_are_distinct() {
    assert_eq!(run(b"<?php class A{static $x=3;static function x(){return 4;}}$c='A';$p='x';echo $c::$x,$c::$$p,$c::${$p},$c::$p();").unwrap(),b"3334");
}
#[test]
fn static_arrays_support_lvalues_probes_and_unset_offsets() {
    assert_eq!(run(b"<?php class A{static $a=[],$n;}A::$a[]=1;A::$a['x']??=2;++A::$a['x'];unset(A::$a[0]);echo A::$a['x'];var_dump(isset(A::$n),empty(A::$missing),A::$missing??4,count(A::$a));").unwrap(),b"3bool(false)\nbool(true)\nint(4)\nint(1)\n");
}
#[test]
fn static_properties_are_inspectable_but_not_instance_properties() {
    assert_eq!(run(b"<?php class A{static $x=2;public $y=3;}class B extends A{}$b=new B;var_dump(property_exists('B','x'),property_exists($b,'x'));echo count(get_object_vars($b));").unwrap(),b"bool(true)\nbool(true)\n1");
}
#[test]
fn static_defaults_are_initialized_once_with_declaring_scope() {
    assert_eq!(run(b"<?php class A{const X=2;static $n=self::X,$f=static function(){return self::class;};}class B extends A{const X=3;}echo B::$n,(B::$f)();B::$n=4;new A;echo A::$n;").unwrap(),b"2A4");
}
#[test]
fn class_initialization_checks_all_defaults_but_static_calls_do_not() {
    assert_eq!(
        run(b"<?php class A{const X=MISSING;static function f(){echo 2;}}A::f();").unwrap(),
        b"2"
    );
    for code in [
        "class A{const X=MISSING;}new A;",
        "class A{static $x=1,$y=MISSING;}echo A::$x;",
        "class A{public $x=MISSING;static $y=2;}echo A::$y;",
        "class A{static $x=MISSING;}new A;",
    ] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::Error,
            "{code}"
        );
    }
}
#[test]
fn static_property_errors_and_staticness_conflicts_are_rejected() {
    for code in [
        "class A{}echo A::$x;",
        "class A{}A::$x=1;",
        "class A{static $x;}unset(A::$x);",
        "class A{}unset(A::$x);",
        "class A{static $x;}class B extends A{public $x;}",
        "class A{public $x;}class B extends A{static $x;}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}

#[test]
fn missing_static_probes_skip_defaults_and_nested_unset_requires_declaration() {
    assert_eq!(run(b"<?php class A{static $x=MISSING;}var_dump(isset(A::$missing),empty(A::$missing),A::$missing??3);").unwrap(),b"bool(false)\nbool(true)\nint(3)\n");
    assert!(
        Runtime::default()
            .execute_code(b"class A{}unset(A::$x[0]);")
            .is_err()
    );
}
#[test]
fn overridden_properties_resolve_parent_defaults_but_constants_remain_lazy() {
    assert_eq!(run(b"<?php class A{const X=MISSING;static $y=2;}class B extends A{const X=1;}new B;echo B::X;").unwrap(),b"1");
    for code in [
        "class A{static $x=MISSING;}class B extends A{static $x=1;}new B;",
        "class A{public $x=MISSING;}class B extends A{public $x=1;}new B;",
    ] {
        assert!(Runtime::default().execute_code(code.as_bytes()).is_err());
    }
}
#[test]
fn dynamic_class_constant_returns_canonical_class_name() {
    assert_eq!(
        run(b"<?php class A{}$c='a';$n='CLASS';echo $c::{$n};").unwrap(),
        b"A"
    );
}

#[test]
fn instance_access_to_static_names_warns_and_uses_separate_storage() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{static $x=2;}$a=new A;$a->x=3;echo $a->x,A::$x;unset($a->x);")
        .unwrap();
    assert_eq!(rt.output, b"32");
    assert_eq!(rt.warnings.len(), 4);
}
#[test]
fn dynamic_class_pseudo_constant_is_not_resolved_in_initializers() {
    assert!(
        Runtime::default()
            .execute_code(b"class A{const X=self::{'class'};}echo A::X;")
            .is_err()
    );
    assert_eq!(
        run(b"<?php class A{const X=self::class;}echo A::X,self_does_not_exist::class;").unwrap(),
        b"Aself_does_not_exist"
    );
}
#[test]
fn dynamic_static_names_are_evaluated_once_per_assignment() {
    assert_eq!(
        run(
            b"<?php class A{static $x=1;}function f(){echo 'f';return 'x';}A::${f()}++;echo A::$x;"
        )
        .unwrap(),
        b"f2"
    );
}
#[test]
fn constant_errors_restore_scope_and_allow_later_resolution() {
    let mut rt = Runtime::default();
    assert!(
        rt.execute_code(b"class A{const X=MISSING;}echo A::X;")
            .is_err()
    );
    assert!(rt.execute_code(b"self::class;").is_err());
    rt.execute_code(b"define('MISSING',3);echo A::X;").unwrap();
    assert_eq!(rt.output, b"3");
}
