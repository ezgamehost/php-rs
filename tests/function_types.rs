use php_rs::{ErrorKind, Runtime, Value, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn scalar_parameters_coerce_in_weak_mode() {
    check(
        "<?php function f(int $a,float $b,string $c,bool $d):array {return [$a,$b,$c,$d];} var_dump(f('42',3,true,'0'));",
        "array(4) {\n  [0]=>\n  int(42)\n  [1]=>\n  float(3)\n  [2]=>\n  string(1) \"1\"\n  [3]=>\n  bool(false)\n}\n",
    );
}

#[test]
fn invalid_scalar_arguments_raise_type_errors_before_body() {
    for (ty, value) in [
        ("int", "null"),
        ("int", "'12foo'"),
        ("int", "INF"),
        ("int", "1e30"),
        ("float", "'x'"),
        ("string", "[]"),
        ("bool", "[]"),
    ] {
        let mut rt = Runtime::default();
        let error = rt
            .execute(format!("<?php function f({ty} $x) {{echo 'body';}} f({value});").as_bytes())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::TypeError, "{ty}: {value}");
        assert!(error.message.contains("Argument #1 ($x)"));
        assert!(rt.output.is_empty());
    }
}

#[test]
fn fractional_coercion_warns_but_preserves_caller_value() {
    let mut rt = Runtime::default();
    rt.execute(b"<?php function f(int $x):int{return $x;} $x=2.5; var_dump(f($x),$x);")
        .unwrap();
    assert_eq!(rt.output, b"int(2)\nfloat(2.5)\n");
    assert_eq!(rt.warnings.len(), 1);
    assert!(rt.warnings[0].contains("loses precision"));
}

#[test]
fn unions_prefer_exact_types_then_php_scalar_order() {
    check(
        "<?php function f(bool|float|int $x){var_dump($x);} f('42'); f('42.0'); f('2e1'); f('x'); f(true); function g(string|int $x){var_dump($x);} g('42'); g(2.0);",
        "int(42)\nfloat(42)\nfloat(20)\nbool(true)\nbool(true)\nstring(2) \"42\"\nint(2)\n",
    );
}

#[test]
fn nullable_mixed_and_literal_types_preserve_values() {
    check(
        "<?php function f(?int $x):?int{return $x;} function g(mixed $x):mixed{return $x;} function h(false|null $x):false|null{return $x;} function t(true $x):true{return $x;} var_dump(f(null),f('3'),g([1]),h(false),h(null),t(true));",
        "NULL\nint(3)\narray(1) {\n  [0]=>\n  int(1)\n}\nbool(false)\nNULL\nbool(true)\n",
    );
    for value in ["0", "''", "true"] {
        assert_eq!(
            run(format!("<?php function f(false $x){{}} f({value});").as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::TypeError
        );
    }
}

#[test]
fn array_iterable_callable_and_class_types_are_checked() {
    check(
        "<?php function id(array $x):iterable{return $x;} function invoke(callable $f){return $f('abc');} echo id([7])[0],invoke('strlen');",
        "73",
    );
    for (ty, value) in [
        ("array", "1"),
        ("iterable", "'x'"),
        ("callable", "'missing'"),
        ("object", "[]"),
        ("MissingClass", "1"),
    ] {
        assert_eq!(
            run(format!("<?php function f({ty} $x){{}} f({value});").as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::TypeError
        );
    }
}

#[test]
fn typed_variadics_check_elements_and_preserve_named_keys() {
    check(
        "<?php function f(int ...$xs):array{return $xs;} var_dump(f('1',other:'2'));",
        "array(2) {\n  [0]=>\n  int(1)\n  [\"other\"]=>\n  int(2)\n}\n",
    );
    assert_eq!(
        run(b"<?php function f(int ...$xs){} f(1,other:[]);")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
}

#[test]
fn weak_returns_coerce_and_strict_returns_reject() {
    check(
        "<?php function f():int{return '42';} function g():float{return 3;} var_dump(f(),g());",
        "int(42)\nfloat(3)\n",
    );
    assert_eq!(
        run(b"<?php declare(strict_types=1); function f():int{return '42';} f();")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
}

#[test]
fn fallthrough_is_not_an_explicit_null_return() {
    for ty in ["int", "?int", "mixed", "null"] {
        let e = run(format!("<?php function f():{ty}{{}} f();").as_bytes()).unwrap_err();
        assert_eq!(e.kind, ErrorKind::TypeError);
        assert!(e.message.contains("none returned"));
    }
    check(
        "<?php function f():void{return;} function g():void{} function h():null{return null;} var_dump(f(),g(),h());",
        "NULL\nNULL\nNULL\n",
    );
}

#[test]
fn invalid_returns_are_rejected_even_in_unused_functions() {
    for body in [
        "function f():int{return;}",
        "function f():?int{if(false)return;}",
        "function f():void{return null;}",
        "function f():never{return;}",
        "function f():never{return 1;}",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute(format!("<?php echo 'before'; {body}").as_bytes())
                .is_err(),
            "{body}"
        );
        assert!(rt.output.is_empty());
    }
    let e = run(b"<?php function f():never{} f();").unwrap_err();
    assert_eq!(e.kind, ErrorKind::TypeError);
    assert!(e.message.contains("must not implicitly return"));
    check(
        "<?php function f():void {function g():int{return 1;} return;} f(); echo g();",
        "1",
    );
}

#[test]
fn strict_arguments_allow_only_exact_types_and_int_to_float() {
    check(
        "<?php declare(strict_types=1); function f(float $x):float{return $x;} var_dump(f(2));",
        "float(2)\n",
    );
    for (ty, v) in [
        ("int", "'2'"),
        ("int", "2.0"),
        ("string", "2"),
        ("bool", "1"),
        ("float", "'2'"),
    ] {
        assert_eq!(
            run(
                format!("<?php declare(strict_types=1); function f({ty} $x){{}} f({v});")
                    .as_bytes()
            )
            .unwrap_err()
            .kind,
            ErrorKind::TypeError
        );
    }
}

#[test]
fn strictness_follows_caller_for_parameters_and_declaration_for_returns() {
    let mut rt = Runtime::default();
    rt.execute(b"<?php declare(strict_types=1); function strict(int $x):int{return $x;} function strict_return():int{return '3';} function strict_call(){return weak('3');}").unwrap();
    rt.execute(b"<?php function weak(int $x):int{return '3';} function weak_call(){return strict('3');} var_dump(strict('2'));").unwrap();
    assert_eq!(
        rt.execute(b"<?php strict_return();").unwrap_err().kind,
        ErrorKind::TypeError
    );
    assert_eq!(
        rt.execute(b"<?php strict_call();").unwrap_err().kind,
        ErrorKind::TypeError
    );
    rt.execute(b"<?php declare(strict_types=1); var_dump(weak(1),weak_call());")
        .unwrap();
    assert_eq!(rt.output, b"int(2)\nint(3)\nint(3)\n");
}

#[test]
fn strictness_applies_to_internal_functions() {
    for call in [
        "strlen(123)",
        "strlen(null)",
        "count([], '0')",
        "array_keys([],null,1)",
    ] {
        assert_eq!(
            run(format!("<?php declare(strict_types=1); {call};").as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::TypeError,
            "{call}"
        );
    }
    check("<?php declare(strict_types=0); echo strlen(123);", "3");
}

#[test]
fn invalid_type_declarations_are_compile_errors() {
    for ty in [
        "int|INT",
        "bool|false",
        "true|false",
        "mixed|int",
        "void|int",
        "?mixed",
        "?null",
        "?int|string",
        "iterable|array",
        "\\int",
    ] {
        assert!(
            lint(format!("<?php function f({ty} $x){{}}").as_bytes()).is_err(),
            "{ty}"
        );
    }
    for ty in ["void", "never"] {
        assert!(lint(format!("<?php function f({ty} $x){{}}").as_bytes()).is_err());
    }
}

#[test]
fn typed_defaults_are_validated_without_coercion() {
    check(
        "<?php declare(strict_types=1); function f(float $x=2):float{return $x;} var_dump(f());",
        "float(2)\n",
    );
    for value in ["'2'", "2.0", "true", "[]", "1+0.5"] {
        assert!(
            lint(format!("<?php function f(int $x={value}){{}}").as_bytes()).is_err(),
            "{value}"
        );
    }
    assert_eq!(
        run(b"<?php const N=null; function f(int $x=N){} f();")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    check(
        "<?php const N='2'; function f(int $x=N){echo $x;} f();",
        "2",
    );
}

#[test]
fn implicit_nullable_defaults_warn_and_explicit_nullable_defaults_do_not() {
    let mut rt = Runtime::default();
    rt.execute(b"<?php function f(int $x=null):?int{return $x;} function g(?int $x=null):?int{return $x;} var_dump(f(),f(null),g());").unwrap();
    assert_eq!(rt.output, b"NULL\nNULL\nNULL\n");
    assert_eq!(rt.warnings.len(), 1);
    assert!(rt.warnings[0].contains("Implicitly marking parameter $x as nullable"));
}

#[test]
fn strict_declaration_must_be_first_and_use_literal_zero_or_one() {
    check(
        "<?php ; declare(strict_types=0); declare(strict_types=1); echo 1;",
        "1",
    );
    for source in [
        "echo 1; declare(strict_types=1);",
        "declare(strict_types=true);",
        "declare(strict_types=1.0);",
        "declare(strict_types=2);",
        "declare(strict_types=1) {}",
        "function f(){declare(strict_types=1);}",
    ] {
        assert!(
            lint(format!("<?php {source}").as_bytes()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn strict_declarations_accept_parentheses_and_cannot_disable_strict_mode() {
    for declaration in [
        "declare(strict_types=(1));",
        "declare(strict_types=1,strict_types=0);",
        "declare(strict_types=1); declare(strict_types=0);",
    ] {
        let error = run(format!("<?php {declaration} function f(int $x){{}} f('1');").as_bytes())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::TypeError);
    }
    assert!(lint(b"<?php declare(strict_types=1+0);").is_err());
}

#[test]
fn object_iterable_union_keeps_the_array_alternative() {
    check(
        "<?php function f(object|iterable $x):object|iterable{return $x;} echo f([42])[0];",
        "42",
    );
}

#[test]
fn implicit_nullable_parameter_before_required_only_warns_about_nullability() {
    let mut rt = Runtime::default();
    rt.execute(
        b"<?php function f(int $x=null,$y){} function g(?int $x=null,$y){} f(null,1); g(null,1);",
    )
    .unwrap();
    assert_eq!(rt.warnings.len(), 2, "{:?}", rt.warnings);
    assert!(rt.warnings[0].contains("Implicitly marking"));
    assert!(rt.warnings[1].contains("Optional parameter"));
    assert_eq!(
        rt.execute(b"<?php f(y:1);").unwrap_err().kind,
        ErrorKind::ArgumentCountError
    );
}

#[test]
fn type_errors_restore_caller_scope_and_leave_argument_effects() {
    let mut rt = Runtime::default();
    let error = rt
        .execute(b"<?php $x=4; function f(int $a,$b){} f([],$x++);")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::TypeError);
    assert_eq!(rt.variables.get(b"x".as_slice()), Some(&Value::Int(5)));
    rt.execute(b"<?php echo strlen(123);").unwrap();
    assert_eq!(rt.output, b"3");
}
