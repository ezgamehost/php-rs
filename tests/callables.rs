use php_rs::{ErrorKind, Runtime, run};

#[test]
fn pipe_chains_strings_and_first_class_functions_left_to_right() {
    assert_eq!(
        run(b"<?php function twice($x){return $x*2;} echo 'abcd' |> strlen(...) |> twice(...);")
            .unwrap(),
        b"8"
    );
    assert_eq!(
        run(b"<?php function twice($x){return $x*2;} echo 3 |> 'twice' |> 'twice';").unwrap(),
        b"12"
    );
}

#[test]
fn pipe_evaluates_input_then_callable_once() {
    assert_eq!(run(b"<?php function input(){echo 'L';return 3;} function select(){echo 'R';return 'twice';} function twice($x){echo 'F';return $x*2;} echo input() |> select();").unwrap(), b"LRF6");
}

#[test]
fn pipe_precedence_sits_between_comparison_and_concatenation() {
    assert_eq!(run(b"<?php function twice($x){return $x*2;} var_dump(1+2 |> twice(...), 6 == 3 |> twice(...), false && 3 |> twice(...), 'a'.'bc' |> strlen(...));").unwrap(), b"int(6)\nbool(true)\nbool(false)\nint(3)\n");
}

#[test]
fn first_class_values_use_normal_named_and_unpacked_argument_binding() {
    assert_eq!(run(b"<?php function f($a,$b=2,...$tail){return $a+$b+count($tail);} $g=f(...);echo $g(b:4,a:3),$g(...[2,3,4]);$s='strlen';echo $s(...)(string:'abc');").unwrap(), b"763");
}

#[test]
fn first_class_acquisition_does_not_call_the_function() {
    assert_eq!(
        run(b"<?php function f(){echo 'called';} $f=f(...);echo 'ready';").unwrap(),
        b"ready"
    );
    let mut rt = Runtime::default();
    let error = rt
        .execute_code(b"echo 'before';missing(...);echo 'after';")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Error);
    assert_eq!(rt.output, b"before");
}

#[test]
fn first_class_identity_and_callable_object_types() {
    assert_eq!(run(b"<?php $a=strlen(...);$b=strlen(...);$c=$a(...);var_dump($a===$b,$a==$b,$a===$c,gettype($a),get_debug_type($a),is_object($a));function f(callable $f):Closure{return $f;}echo f($a)('abcd');").unwrap(), b"bool(false)\nbool(true)\nbool(true)\nstring(6) \"object\"\nstring(7) \"Closure\"\nbool(true)\n4");
}

#[test]
fn callables_retain_function_statics_and_argument_inspection() {
    assert_eq!(run(b"<?php function f($x){static $n=0;return ++$n+func_num_args()+$x;} $a=f(...);$b=f(...);echo $a(2),$b(2),f(2);").unwrap(), b"456");
}

#[test]
fn pipe_honors_optional_variadic_and_void_results() {
    assert_eq!(run(b"<?php function f($a,$b=3,...$rest){return $a+$b+count($rest);} function v($x):void{echo $x;}var_dump(2 |> f(...));var_dump(4 |> v(...));").unwrap(), b"int(5)\n4NULL\n");
}

#[test]
fn pipe_and_callable_invocation_use_caller_strictness() {
    assert_eq!(
        run(b"<?php function f(int $x){return $x+1;} echo '2' |> f(...);").unwrap(),
        b"3"
    );
    for call in ["'2' |> f(...)", "f(...)('2')"] {
        let mut rt = Runtime::default();
        let err = rt
            .execute_code(
                format!("declare(strict_types=1);function f(int $x){{}}{call};").as_bytes(),
            )
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::TypeError);
    }
}

#[test]
fn invalid_pipes_preserve_evaluation_order_and_fail_normally() {
    let mut rt = Runtime::default();
    let err = rt
        .execute_code(b"function input(){echo 'L';return 1;}input() |> 'missing';")
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Error);
    assert_eq!(rt.output, b"L");
    for code in ["1 |> 42;", "1 |> strlen;", "1 |> [1,2];"] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::Error
        );
    }
    assert_eq!(
        Runtime::default()
            .execute_code(b"function f($a,$b){}1 |> f(...);")
            .unwrap_err()
            .kind,
        ErrorKind::ArgumentCountError
    );
}

#[test]
fn callables_reject_scalar_coercions_and_string_output() {
    for code in [
        "echo strlen(...);",
        "strlen(strlen(...));",
        "function f(string $x){}f(strlen(...));",
    ] {
        assert!(Runtime::default().execute_code(code.as_bytes()).is_err());
    }
    assert_eq!(
        run(b"<?php var_dump((bool)strlen(...));").unwrap(),
        b"bool(true)\n"
    );
}

#[test]
fn pipe_can_terminate_a_request() {
    let mut rt = Runtime::default();
    rt.execute_code(b"'done' |> exit(...);echo 'after';")
        .unwrap();
    assert_eq!(rt.output, b"done");
    assert_eq!(rt.exit_status(), Some(0));
}

#[test]
fn die_and_exit_acquire_the_same_canonical_function() {
    assert_eq!(
        run(b"<?php $a=exit(...);$b=die(...);var_dump($a==$b,$a===$b);").unwrap(),
        b"bool(true)\nbool(false)\n"
    );
}

#[test]
fn php85_callables_work_in_constants_and_defaults() {
    assert_eq!(run(b"<?php const LENGTH=strlen(...);echo (LENGTH)('abc');function f($g=strlen(...)){return $g;}echo f()('abcd');var_dump(f()===f());").unwrap(), b"34bool(false)\n");
    assert_eq!(
        run(b"<?php function f(){static $c=strlen(...);return $c;}var_dump(f()===f());").unwrap(),
        b"bool(true)\n"
    );
}

#[test]
fn pipe_is_not_a_constant_expression_and_callable_arguments_are_not_placeholders() {
    for code in [
        "const X=1 |> 'strlen';",
        "function f($a=1 |> strlen(...)){}",
        "strlen(...,);",
        "strlen(1,...);",
        "const X=$f(...);",
    ] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::ParseError
        );
    }
}

#[test]
fn callable_objects_have_numeric_cast_and_comparison_diagnostics() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$c=strlen(...);var_dump((int)$c,(float)$c,$c==1,[1]==[$c],$c<'x');foreach($c as $v){echo 'bad';}").unwrap();
    assert_eq!(
        rt.output,
        b"int(1)\nfloat(1)\nbool(true)\nbool(true)\nbool(false)\n"
    );
    assert_eq!(rt.warnings.len(), 4);
    assert!(rt.warnings[0].contains("converted to int"));
    assert!(rt.warnings[1].contains("converted to float"));
    assert!(rt.warnings[2].starts_with("Notice:"));
}

#[test]
fn closure_array_cast_keeps_identity_and_aliases_remain_distinct() {
    assert_eq!(
        run(b"<?php $c=strlen(...);$a=(array)$c;var_dump($a[0]===$c,count(...)==sizeof(...));")
            .unwrap(),
        b"bool(true)\nbool(false)\n"
    );
    for code in ["$c=strlen(...);echo $c[0];", "$c=strlen(...);isset($c[0]);"] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::Error
        );
    }
}
