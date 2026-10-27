use php_rs::{ErrorKind, Runtime, Value, lint};

fn terminated(code: &str, expected: &str, status: i32) {
    let mut rt = Runtime::default();
    let result = rt.execute_code(code.as_bytes()).unwrap();
    assert_eq!(result, Value::Null);
    assert_eq!(rt.output, expected.as_bytes());
    assert_eq!(rt.exit_status(), Some(status));
    assert!(rt.warnings.is_empty(), "{:?}", rt.warnings);
}

#[test]
fn bare_parenthesized_and_aliased_exit_stop_execution() {
    for ending in ["exit;", "die;", "exit();", "die();", "EXIT();", "\\exit();"] {
        terminated(&format!("echo 'before';{ending}echo 'after';"), "before", 0);
    }
    terminated("echo 'a',exit('b'),'c';", "ab", 0);
}

#[test]
fn string_status_is_output_even_when_numeric_or_binary() {
    terminated("exit('17');", "17", 0);
    terminated("die(\"a\\0b\");", "a\0b", 0);
    terminated("exit('');", "", 0);
}

#[test]
fn integer_exit_codes_follow_php_c_int_conversion() {
    for (value, status) in [
        (0, 0),
        (17, 17),
        (-1, -1),
        (256, 256),
        (4294967296_i64, 0),
        (4294967297, 1),
        (2147483648, -2147483648),
    ] {
        terminated(&format!("exit({value});"), "", status);
    }
}

#[test]
fn modern_calls_accept_named_unpacked_and_dynamic_arguments() {
    terminated("exit(status:'named');", "named", 0);
    terminated("$f='die';$f(status:'dynamic');", "dynamic", 0);
    terminated("exit(...['status'=>7]);", "", 7);
    terminated("exit(...[]);", "", 0);
    terminated(
        "function stop(callable $f):never{$f('called');}stop('exit');",
        "called",
        0,
    );
}

#[test]
fn termination_unwinds_calls_and_preserves_global_changes() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$x=1;function inner():never{global $x;$x=9;$local=7;exit(4);}function outer(){inner();echo 'bad';}outer();echo 'bad';").unwrap();
    assert_eq!(rt.exit_status(), Some(4));
    assert_eq!(rt.variables.get(b"x".as_slice()), Some(&Value::Int(9)));
    assert!(!rt.variables.contains_key(b"local".as_slice()));
    assert!(rt.output.is_empty());
}

#[test]
fn termination_unwinds_eval_and_argument_evaluation() {
    terminated(
        "function f($a){eval('exit(\"eval\");');echo 'bad';}f(2);echo 'bad';",
        "eval",
        0,
    );
    terminated(
        "function f($a,$b){echo 'bad';} f(exit('arg'),print 'bad');",
        "arg",
        0,
    );
    terminated("$a=[1,exit('array'),print 'bad'];", "array", 0);
}

#[test]
fn terminated_requests_do_not_resume_in_later_execute_calls() {
    let mut rt = Runtime::default();
    rt.execute_code(b"exit('done');").unwrap();
    assert_eq!(rt.execute_code(b"echo 'bad';").unwrap(), Value::Null);
    assert_eq!(rt.execute(b"<?php echo 'bad';").unwrap(), Value::Null);
    assert_eq!(
        rt.execute_file("/nonexistent/php-rs-after-exit.php")
            .unwrap(),
        Value::Null
    );
    assert_eq!(rt.output, b"done");
    assert_eq!(rt.exit_status(), Some(0));
    let mut other = Runtime::default();
    other.execute_code(b"echo 'independent';").unwrap();
    assert_eq!(other.output, b"independent");
    assert_eq!(other.exit_status(), None);
}

#[test]
fn short_circuiting_does_not_terminate_unreached_exit() {
    let mut rt = Runtime::default();
    rt.execute_code(b"false && exit;true || die; $x=1??exit; echo true?'ok':exit;")
        .unwrap();
    assert_eq!(rt.output, b"ok");
    assert_eq!(rt.exit_status(), None);
}

#[test]
fn weak_union_coercion_handles_booleans_and_large_floats() {
    terminated("exit(true);", "", 1);
    terminated("exit(false);", "", 0);
    terminated("exit(2.0);", "", 2);
    terminated("exit(INF);", "INF", 0);
    terminated("exit(-INF);", "-INF", 0);
    let mut rt = Runtime::default();
    rt.execute_code(b"exit(1.5);").unwrap();
    assert_eq!(rt.exit_status(), Some(1));
    assert_eq!(rt.warnings.len(), 1);
    assert!(rt.warnings[0].contains("loses precision"));
}

#[test]
fn null_status_is_deprecated_and_terminates_with_zero() {
    let mut rt = Runtime::default();
    rt.execute_code(b"die(null);").unwrap();
    assert_eq!(rt.exit_status(), Some(0));
    assert_eq!(
        rt.warnings,
        [
            "Deprecated: exit(): Passing null to parameter #1 ($status) of type string|int is deprecated on line 1"
        ]
    );
}

#[test]
fn invalid_types_and_arity_do_not_mark_request_terminated() {
    for (code, kind) in [
        ("exit([]);", ErrorKind::TypeError),
        ("declare(strict_types=1);exit(true);", ErrorKind::TypeError),
        ("declare(strict_types=1);exit(null);", ErrorKind::TypeError),
        ("declare(strict_types=1);exit(1.0);", ErrorKind::TypeError),
        ("exit(1,2);", ErrorKind::ArgumentCountError),
        ("exit(code:1);", ErrorKind::Error),
    ] {
        let mut rt = Runtime::default();
        assert_eq!(rt.execute_code(code.as_bytes()).unwrap_err().kind, kind);
        assert_eq!(rt.exit_status(), None);
        rt.execute_code(b"echo 'alive';").unwrap();
        assert_eq!(rt.output, b"alive");
    }
    terminated("declare(strict_types=1);exit('strict');", "strict", 0);
}

#[test]
fn exit_syntax_is_checked_before_any_code_runs() {
    for code in [
        "exit 'bad';",
        "exit(1);echo ;",
        "function exit(){}",
        "function die(){}",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'before';{code}").as_bytes())
                .is_err()
        );
        assert!(rt.output.is_empty());
        assert_eq!(rt.exit_status(), None);
    }
    assert!(lint(b"<?php exit(0);").is_ok());
    terminated(
        "echo function_exists('exit'),function_exists('die');exit + 1;",
        "11",
        0,
    );
}

#[test]
fn exit_in_included_file_stops_the_entire_request() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/terminate.php")
        .replace('\\', "/")
        .replace('\'', "\\'");
    let mut rt = Runtime::default();
    rt.execute_code(
        format!("$x=1;function f(){{global $x;include '{path}';echo 'bad';}}f();echo 'bad';")
            .as_bytes(),
    )
    .unwrap();
    assert_eq!(rt.output, b"included");
    assert_eq!(rt.exit_status(), Some(12));
    assert_eq!(rt.variables.get(b"x".as_slice()), Some(&Value::Int(8)));
}

#[test]
fn nan_to_string_coercion_warns_for_exit_and_typed_functions() {
    for code in [
        "exit(NAN);",
        "function f(string $x){echo $x;}f(NAN);",
        "function f():string{return NAN;}echo f();",
    ] {
        let mut rt = Runtime::default();
        rt.execute_code(code.as_bytes()).unwrap();
        assert_eq!(rt.output, b"NAN");
        assert_eq!(
            rt.warnings,
            ["unexpected NAN value was coerced to string on line 1"]
        );
    }
}
