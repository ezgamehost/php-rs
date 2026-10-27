use php_rs::{ErrorKind, Runtime};

fn check(code: &str, expected: &str) {
    let mut rt = Runtime::default();
    rt.execute_code(code.as_bytes()).unwrap();
    assert_eq!(rt.output, expected.as_bytes());
    assert!(rt.warnings.is_empty(), "{:?}", rt.warnings);
}

fn error(code: &str, kind: ErrorKind, message: &str) {
    let mut rt = Runtime::default();
    let err = rt.execute_code(code.as_bytes()).unwrap_err();
    assert_eq!(err.kind, kind, "{err}");
    assert!(err.message.contains(message), "{err}");
}

#[test]
fn supplied_arguments_include_extras_but_exclude_omitted_defaults() {
    check(
        "function f($a=10,$b=20){echo func_num_args(),':';foreach(func_get_args() as $v)echo $v,',';echo ';';}f();f(1);f(1,2,3,4);",
        "0:;1:1,;4:1,2,3,4,;",
    );
}

#[test]
fn named_arguments_fill_default_gaps_in_parameter_order() {
    check(
        "function f($a=1,$b=2,$c=3){echo func_num_args(),':';foreach(func_get_args() as $v)echo $v;echo ':',func_get_arg(1),';';} f(c:8);f(c:9,a:7);f(...['c'=>6,'b'=>5]);",
        "3:128:2;3:729:2;3:156:5;",
    );
}

#[test]
fn variadic_inspection_excludes_unknown_named_arguments() {
    check(
        "function f($a=1,...$v){echo func_num_args(),':';foreach(func_get_args() as $x)echo $x;echo ':',count($v),';';} f(z:9);f(2,3,4,z:9);f(v:3);f(a:8,z:9);",
        "0::1;3:234:3;0::1;1:8:1;",
    );
}

#[test]
fn declared_parameters_are_live_and_extra_arguments_are_snapshots() {
    check(
        "function f($a,...$v){$copy=func_get_args();$a=7;$v[0]=8;$v[]=9;echo func_get_arg(0),func_get_arg(1),':';foreach(func_get_args() as $x)echo $x;echo ':';foreach($copy as $x)echo $x;}f(1,2);",
        "72:72:12",
    );
}

#[test]
fn inspection_observes_coerced_fixed_and_variadic_values() {
    check(
        "function f(int $a,int ...$v){var_dump(func_get_args());} f('1','2','3');",
        "array(3) {\n  [0]=>\n  int(1)\n  [1]=>\n  int(2)\n  [2]=>\n  int(3)\n}\n",
    );
}

#[test]
fn unset_parameters_are_null_until_reassigned() {
    check(
        "function f($a){unset($a);var_dump(func_get_arg(0),func_get_args()[0]);$a=9;echo func_get_arg(0),':',func_num_args();}f(2);",
        "NULL\nNULL\n9:1",
    );
}

#[test]
fn global_and_static_rebinding_changes_visible_parameter_values() {
    check(
        "$a=5;function f($a){global $a;echo func_get_arg(0);$a=7;echo func_get_arg(0);static $a=9;echo func_get_args()[0];}f(2);",
        "579",
    );
}

#[test]
fn array_results_are_independent_copies() {
    check(
        "function f($a,...$v){$copy=func_get_args();$copy[0][0]=9;$copy[1][0]=8;$a[0]=3;$v[0][0]=4;echo func_get_arg(0)[0],func_get_arg(1)[0],$copy[0][0],$copy[1][0];}f([1],[2]);",
        "3298",
    );
}

#[test]
fn recursive_frames_restore_arguments_and_argument_evaluation_uses_caller() {
    check(
        "function f($n){echo func_num_args(),func_get_arg(0);if($n)f($n-1,func_num_args());echo func_num_args(),func_get_arg(0);} f(2);",
        "122120202112",
    );
    check(
        "function inner(){echo func_num_args(),func_get_arg(0);}function outer($a){inner(func_get_arg(0));echo func_get_arg(0);}outer(7);",
        "177",
    );
}

#[test]
fn omitted_parameters_do_not_become_arguments_after_assignment() {
    check(
        "function f($a=1){$a=9;echo func_num_args(),count(func_get_args());}f();",
        "00",
    );
    error(
        "function f($a=1){$a=9;func_get_arg(0);}f();",
        ErrorKind::ValueError,
        "must be less than the number",
    );
}

#[test]
fn inspection_requires_function_context_including_eval_units() {
    error(
        "func_num_args();",
        ErrorKind::Error,
        "must be called from a function context",
    );
    error(
        "func_get_args();",
        ErrorKind::Error,
        "cannot be called from the global scope",
    );
    error(
        "func_get_arg(0);",
        ErrorKind::Error,
        "cannot be called from the global scope",
    );
    error(
        "function f($a){eval('func_get_args();');}f(1);",
        ErrorKind::Error,
        "cannot be called from the global scope",
    );
    check(
        "function f($a){eval('$a=8;');echo func_get_arg(0);eval('function g(){echo func_num_args();} g(1,2);');echo func_get_arg(0);}f(1);",
        "828",
    );
}

#[test]
fn errors_restore_argument_context_across_execute_calls() {
    let mut rt = Runtime::default();
    assert!(
        rt.execute_code(b"function bad($a){func_get_arg(9);}bad(1);")
            .is_err()
    );
    let err = rt.execute_code(b"func_num_args();").unwrap_err();
    assert!(err.message.contains("function context"));
    rt.execute_code(b"function good($a){echo func_num_args(),func_get_arg(0);}good(8);")
        .unwrap();
    assert_eq!(rt.output, b"18");
}

#[test]
fn dynamic_calls_are_rejected_after_parameter_and_context_validation() {
    for name in ["func_num_args", "func_get_args", "func_get_arg"] {
        let args = if name == "func_get_arg" { "0" } else { "" };
        error(
            &format!("function f(){{$n='{name}';$n({args});}} f(1);"),
            ErrorKind::Error,
            &format!("Cannot call {name}() dynamically"),
        );
    }
    error("$n='func_get_args';$n();", ErrorKind::Error, "global scope");
    error(
        "function f(){$n='func_get_arg';$n(-1);}f();",
        ErrorKind::ValueError,
        "greater than or equal to 0",
    );
    check(
        "function f(){echo \\FUNC_NUM_ARGS(),\\func_get_arg(position:0);}f(7);",
        "17",
    );
}

#[test]
fn position_validation_obeys_scalar_coercion_and_strict_types() {
    check(
        "function f(){echo func_get_arg(position:'1'),func_get_arg(true);}f(8,9);",
        "99",
    );
    error(
        "function f(){func_get_arg(-1);}f(1);",
        ErrorKind::ValueError,
        "greater than or equal to 0",
    );
    error(
        "function f(){func_get_arg(1);}f(1);",
        ErrorKind::ValueError,
        "must be less than the number",
    );
    error(
        "function f(){func_get_arg([]);}f(1);",
        ErrorKind::TypeError,
        "must be of type int",
    );
    error(
        "declare(strict_types=1);function f(){func_get_arg('0');}f(1);",
        ErrorKind::TypeError,
        "must be of type int",
    );
    let mut rt = Runtime::default();
    rt.execute_code(b"function f(){echo func_get_arg(null),func_get_arg(0.5);}f(7);")
        .unwrap();
    assert_eq!(rt.output, b"77");
    assert_eq!(rt.warnings.len(), 2);
}

#[test]
fn ordinary_builtin_arity_and_registration_rules_apply() {
    for code in ["func_get_arg();", "func_get_args(1);", "func_num_args(1);"] {
        error(code, ErrorKind::ArgumentCountError, "");
    }
    error(
        "function func_get_args(){}",
        ErrorKind::Error,
        "Cannot redeclare",
    );
    check(
        "var_dump(function_exists('func_get_arg'),function_exists('func_get_args'),function_exists('func_num_args'));",
        "bool(true)\nbool(true)\nbool(true)\n",
    );
}

#[test]
fn included_files_have_no_argument_context() {
    let fixture = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/arguments_in_include.php"
    )
    .replace('\\', "/")
    .replace('\'', "\\'");
    error(
        &format!("function f($a){{include '{fixture}';}}f(1);"),
        ErrorKind::Error,
        "cannot be called from the global scope",
    );
}
