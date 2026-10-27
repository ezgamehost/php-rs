use php_rs::{ErrorKind, Runtime, Value, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn globals_are_live_across_nested_calls_and_ignore_caller_locals() {
    check(
        "<?php $x=1; function inner(){global $x; $x+=2;} function outer(){global $x; echo $x; inner(); echo $x; $x+=4;} function local(){ $x=99; outer(); echo $x; } local(); echo ':',$x;",
        "1399:7",
    );
}

#[test]
fn global_binding_replaces_local_and_parameter_bindings() {
    check(
        "<?php $x=4; function f($x){echo $x; global $x; echo ++$x;} f(9); echo $x;",
        "955",
    );
}

#[test]
fn missing_globals_are_created_as_null_without_a_warning() {
    let mut rt = Runtime::default();
    rt.execute(b"<?php function f(){global $missing; var_dump($missing);} f();")
        .unwrap();
    assert_eq!(rt.output, b"NULL\n");
    assert_eq!(rt.variables.get(b"missing".as_slice()), Some(&Value::Null));
    assert!(rt.warnings.is_empty());
}

#[test]
fn unsetting_global_alias_detaches_only_the_local_name() {
    check(
        "<?php $x=3; function f(){global $x; unset($x); $x=9; global $x; echo ++$x;} f(); echo $x;",
        "44",
    );
}

#[test]
fn global_arrays_strings_and_destructuring_write_through() {
    check(
        "<?php $a=['x'=>[1,2]]; $s='abc'; function f(){global $a,$s; $a['x'][]=3; unset($a['x'][0]); $s[1]='Z'; [$a['y'],$s[2]]=[9,'Q'];} f(); echo $a['x'][1],$a['x'][2],$a['y'],':',$s;",
        "239:aZQ",
    );
}

#[test]
fn global_changes_survive_errors_and_caller_scope_is_restored() {
    let mut rt = Runtime::default();
    let error = rt
        .execute(b"<?php $x=1; function f(){global $x; $x=7; $local=9; return 1/0;} f();")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::DivisionByZeroError);
    assert_eq!(rt.variables.get(b"x".as_slice()), Some(&Value::Int(7)));
    assert!(!rt.variables.contains_key(b"local".as_slice()));
    rt.variables.insert(b"x".to_vec(), Value::Int(11));
    rt.execute(b"<?php function g(){global $x; echo $x;} g();")
        .unwrap();
    assert_eq!(rt.output, b"11");
}

#[test]
fn global_declarations_at_script_scope_preserve_values() {
    check(
        "<?php $x=2; global $x,$y; echo ++$x; var_dump($y);",
        "3NULL\n",
    );
}

#[test]
fn statics_persist_per_function_and_request() {
    let mut rt = Runtime::default();
    rt.execute(b"<?php function f(){static $x=0; return ++$x;} function g(){static $x=10; return ++$x;} echo f(),g(),f();").unwrap();
    rt.execute(b"<?php echo f();").unwrap();
    assert_eq!(rt.output, b"11123");
    assert!(!rt.variables.contains_key(b"x".as_slice()));
    check(
        "<?php function f(){static $x=0; return ++$x;} echo f();",
        "1",
    );
}

#[test]
fn statics_share_updates_across_recursive_frames() {
    check(
        "<?php function f($n){static $x=0; ++$x; if($n)f($n-1); echo $x;} f(2); f(0);",
        "3334",
    );
}

#[test]
fn dynamic_static_initializers_run_once_when_reached() {
    check(
        "<?php $calls=0; function init($x){global $calls; ++$calls; return $x+1;} function f($take,$x){if($take){static $v=init($x); echo ++$v;}else{echo 'skip';}} f(false,0); f(true,4); f(true,99); echo ':',$calls;",
        "skip67:1",
    );
}

#[test]
fn null_initializers_are_cached_and_parameters_are_visible() {
    check(
        "<?php function f($x){static $x=$x; var_dump($x);} f(null); f(9); function g(){static $x; var_dump($x); $x=2;} g(); g();",
        "NULL\nNULL\nNULL\nint(2)\n",
    );
}

#[test]
fn first_completed_recursive_initializer_wins() {
    check(
        "<?php function f($n){static $x=$n?f($n-1)+1:10; echo $n,':',$x,';'; return $x;} f(2); f(5);",
        "0:10;1:10;2:10;5:10;",
    );
}

#[test]
fn failed_initializers_retry_and_initialized_values_survive_errors() {
    let mut rt = Runtime::default();
    assert!(
        rt.execute(b"<?php function f($n){static $x=10/$n; return ++$x;} f(0);")
            .is_err()
    );
    rt.execute(b"<?php echo f(2),':',f(0);").unwrap();
    assert_eq!(rt.output, b"6:7");
    assert!(
        rt.execute(b"<?php function g(){static $x=0; echo ++$x; return 1/0;} g();")
            .is_err()
    );
    assert!(rt.execute(b"<?php g();").is_err());
    assert_eq!(rt.output, b"6:712");
}

#[test]
fn unset_static_local_does_not_erase_persistent_value() {
    check(
        "<?php function f(){static $x=0; echo ++$x; unset($x); $x=99;} f(); f();",
        "12",
    );
}

#[test]
fn array_statics_mutate_in_place_but_return_by_value() {
    check(
        "<?php function f(){static $a=['n'=>0]; ++$a['n']; return $a;} $a=f(); $a['n']=99; echo f()['n'];",
        "2",
    );
}

#[test]
fn global_and_static_declarations_rebind_independently() {
    check(
        "<?php $x=9; function f(){global $x; static $x=3; echo ++$x; global $x; echo ++$x;} f(); f(); echo ':',$x;",
        "410511:11",
    );
}

#[test]
fn duplicate_statics_are_compile_errors_but_nested_functions_have_separate_sets() {
    for body in [
        "static $x=1; static $x=2;",
        "if(false){static $x;}else{static $x;}",
        "static $x,$x;",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute(format!("<?php echo 'before'; function f(){{{body}}}").as_bytes())
                .is_err()
        );
        assert!(rt.output.is_empty());
    }
    check(
        "<?php function f(){static $x=1; function g(){static $x=2; echo ++$x;} echo ++$x;} f();g();",
        "23",
    );
}

#[test]
fn script_static_storage_is_local_to_each_compilation() {
    let mut rt = Runtime::default();
    rt.execute(b"<?php $x=9; for($i=0;$i<2;$i++){static $x=1; echo ++$x;}")
        .unwrap();
    rt.execute(b"<?php static $x=10; echo ++$x;").unwrap();
    assert_eq!(rt.output, b"2311");
}

#[test]
fn dynamic_global_names_bind_in_order_and_evaluate_once() {
    check(
        "<?php $which='x'; $x=5; function f(){global $which,$$which; ++$x; global ${$which.'y'}; $xy=8;} f(); echo $x,$xy;",
        "68",
    );
}

#[test]
fn variable_variables_support_indexing_assignment_probes_and_unset() {
    check(
        "<?php $name='a'; $$name=[2,3]; $$name[1]++; echo $$name[1]; ${'a'}[]=5; echo $a[2]; var_dump(isset($$name)); unset($$name); var_dump(empty($$name)); $$name??=9; echo $a;",
        "45bool(true)\nbool(true)\n9",
    );
    check("<?php $a='b'; $b='c'; $c=7; echo $$$a;", "7");
}

#[test]
fn malformed_global_and_static_declarations_fail_lint() {
    for body in [
        "global $x[0];",
        "global $x=1;",
        "global ($x);",
        "global $x,;",
        "static $$x;",
        "static $x,;",
        "static $x=[,];",
    ] {
        assert!(lint(format!("<?php {body}").as_bytes()).is_err(), "{body}");
    }
}
