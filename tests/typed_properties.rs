use php_rs::{ErrorKind, Runtime, run};

#[test]
fn typed_properties_without_defaults_are_uninitialized_including_nullable_and_mixed() {
    assert_eq!(run(b"<?php class A{public int $x;public ?int $y;public mixed $z;public static int $s;}$a=new A;var_dump(isset($a->x),isset($a->y),isset($a->z),isset(A::$s),empty($a->x),$a->x??3,count(get_object_vars($a)),count((array)$a));").unwrap(),b"bool(false)\nbool(false)\nbool(false)\nbool(false)\nbool(true)\nint(3)\nint(0)\nint(0)\n");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public int $x;public static int $s;}$a=new A;")
        .unwrap();
    for code in [b"$a->x;".as_slice(), b"A::$s;"] {
        let err = rt.execute_code(code).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Error);
        assert!(err.message.contains("before initialization"));
    }
}
#[test]
fn weak_assignments_coerce_values_and_return_the_stored_type() {
    assert_eq!(run(b"<?php class A{public int $i;public float $f;public bool $b;public string $s;public static int $n;}$a=new A;var_dump($a->i='2',$a->f=3,$a->b=1,$a->s=4,A::$n='5');").unwrap(),b"int(2)\nfloat(3)\nbool(true)\nstring(1) \"4\"\nint(5)\n");
}
#[test]
fn strict_assignments_fail_without_mutating_instance_or_static_storage() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public int $x=2;public static int $s=3;}$a=new A;")
        .unwrap();
    for code in [
        b"declare(strict_types=1);$a->x='4';".as_slice(),
        b"declare(strict_types=1);A::$s='4';",
    ] {
        assert_eq!(
            rt.execute_code(code).unwrap_err().kind,
            ErrorKind::TypeError
        );
    }
    rt.execute_code(b"echo $a->x,A::$s;").unwrap();
    assert_eq!(rt.output, b"23");
}
#[test]
fn property_defaults_are_strict_and_integer_defaults_widen_to_float() {
    assert_eq!(run(b"<?php class A{public float $x=2;public static float $s=3;}var_dump((new A)->x,A::$s);").unwrap(),b"float(2)\nfloat(3)\n");
    for declaration in [
        "public int $x='2';",
        "public int $x=null;",
        "public callable $x;",
        "public static $x;public int $x;",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';class A{{{declaration}}}").as_bytes())
                .is_err()
        );
        assert!(rt.output.is_empty());
    }
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public int $x=VALUE;}define('VALUE','2');")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"new A;").unwrap_err().kind,
        ErrorKind::TypeError
    );
}
#[test]
fn unset_reverts_to_uninitialized_and_coalescing_assignment_reinitializes() {
    assert_eq!(run(b"<?php class A{public int $x=1;}$a=new A;unset($a->x);var_dump(isset($a->x),property_exists($a,'x'));var_dump($a->x??='2',$a->x);").unwrap(),b"bool(false)\nbool(true)\nint(2)\nint(2)\n");
}
#[test]
fn typed_arrays_auto_initialize_and_support_nested_writes_and_unsets() {
    assert_eq!(run(b"<?php class A{public array $a;public static array $s;}$a=new A;$a->a['x'][]=2;A::$s[]=3;unset($a->a['x'][0],A::$s[0]);echo count($a->a['x']),count(A::$s);").unwrap(),b"00");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public int $x;}$a=new A;")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"$a->x[]=2;").unwrap_err().kind,
        ErrorKind::TypeError
    );
    assert!(rt.execute_code(b"$a->x;").is_err());
}
#[test]
fn property_inheritance_requires_invariant_types() {
    assert_eq!(run(b"<?php class A{public int|string $x;public static iterable $s=[];}class B extends A{public string|int $x;public static array|Traversable $s=[];}$b=new B;$b->x=2;echo $b->x;").unwrap(),b"2");
    for (a, b) in [
        ("int", "?int"),
        ("?int", "int"),
        ("int", ""),
        ("", "int"),
        ("mixed", ""),
        ("float", "int"),
    ] {
        assert!(
            Runtime::default()
                .execute_code(
                    format!("class A{{public {a} $x;}}class B extends A{{public {b} $x;}}")
                        .as_bytes()
                )
                .is_err()
        );
    }
}
#[test]
fn inherited_static_properties_share_values_and_redeclared_ones_start_uninitialized() {
    assert_eq!(run(b"<?php class A{public static int $x=2;}class B extends A{}class C extends A{public static int $x;}B::$x='3';var_dump(A::$x,isset(C::$x));C::$x??=4;echo A::$x,C::$x;").unwrap(),b"int(3)\nbool(false)\n34");
}
#[test]
fn self_and_parent_property_types_use_declaring_scope() {
    assert_eq!(run(b"<?php class A{public self $x;}class B extends A{public parent $y;}class C extends B{}$c=new C;$c->x=new A;$c->y=new B;echo get_class($c->x),get_class($c->y);").unwrap(),b"AB");
}
#[test]
fn increments_return_coerced_values_and_keep_overflow_failures_atomic() {
    assert_eq!(run(b"<?php class A{public string $x='1';public static string $s='2';}$a=new A;var_dump(++$a->x,++A::$s);").unwrap(),b"string(1) \"2\"\nstring(1) \"3\"\n");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public int $x=PHP_INT_MAX;}$a=new A;")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"$a->x++;").unwrap_err().kind,
        ErrorKind::TypeError
    );
    rt.execute_code(b"var_dump($a->x===PHP_INT_MAX);").unwrap();
    assert_eq!(rt.output, b"bool(true)\n");
}
#[test]
fn object_dumps_show_uninitialized_properties_but_counts_exclude_them() {
    assert_eq!(run(b"<?php class A{public int $x;public ?int $y;public $z;}var_dump(new A);").unwrap(),b"object(A)#1 (1) {\n  [\"x\"]=>\n  uninitialized(int)\n  [\"y\"]=>\n  uninitialized(?int)\n  [\"z\"]=>\n  NULL\n}\n");
}
#[test]
fn typed_property_assignment_uses_the_writing_functions_strictness() {
    let mut rt = Runtime::default();
    rt.execute_code(b"declare(strict_types=1);class A{public int $x;function strictWrite($v){$this->x=$v;}}$a=new A;").unwrap();
    assert_eq!(
        rt.execute_code(b"$a->strictWrite('2');").unwrap_err().kind,
        ErrorKind::TypeError
    );
    rt.execute_code(b"$a->x='3';echo $a->x;").unwrap();
    assert_eq!(rt.output, b"3");
}

#[test]
fn typed_integer_limits_reject_increment_and_decrement_even_with_string_unions() {
    for ty in ["int", "int|string"] {
        for (value, op) in [("PHP_INT_MIN", "--"), ("PHP_INT_MAX", "++")] {
            let mut rt = Runtime::default();
            rt.execute_code(format!("class A{{public static {ty} $x={value};}}").as_bytes())
                .unwrap();
            assert_eq!(
                rt.execute_code(format!("A::$x{op};").as_bytes())
                    .unwrap_err()
                    .kind,
                ErrorKind::TypeError
            );
            rt.execute_code(format!("var_dump(A::$x==={value});").as_bytes())
                .unwrap();
            assert_eq!(rt.output, b"bool(true)\n");
        }
    }
}
#[test]
fn unresolved_inherited_property_types_defer_class_binding() {
    let mut rt = Runtime::default();
    assert!(
        rt.execute_code(b"echo 'before';class A{public B $x;}class B extends A{public A $x;}")
            .is_err()
    );
    assert_eq!(rt.output, b"before");
}

#[test]
fn var_syntax_supports_typed_declarations() {
    assert_eq!(run(b"<?php class A{var int $x=2;var string $y;}$a=new A;echo $a->x;var_dump(isset($a->y));").unwrap(),b"2bool(false)\n");
}
