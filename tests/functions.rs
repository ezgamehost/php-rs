use php_rs::{ErrorKind, Runtime, Value, run};
fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn declarations_are_hoisted_through_unconditional_blocks() {
    check(
        "<?php echo ADD(2,3), ':'; { function add($a,$b) { return $a+$b; } } echo add(4,5);",
        "5:9",
    );
}
#[test]
fn recursion_and_local_variables_do_not_leak() {
    check(
        "<?php $n=99; function fact($n) { if($n<2) return 1; return $n*fact($n-1); } echo fact(8), ':', $n;",
        "40320:99",
    );
}
#[test]
fn arguments_evaluate_left_to_right_and_arrays_pass_by_value() {
    check(
        "<?php function f($a,$b,$c) { $c[0]=8; return $a.':'.$b.':'.$c[0]; } $i=0; $a=[1]; echo f($i++,$i++,$a), ':', $i, ':', $a[0];",
        "0:1:8:2:1",
    );
}
#[test]
fn defaults_are_fresh_and_null_is_not_omitted() {
    check(
        "<?php function f($a=[1],$b=2+3) { $a[]=9; return $a[0].':'.$a[1].':'.$b; } echo f(), '|', f(), '|', f([7],null);",
        "1:9:5|1:9:5|7:9:",
    );
}
#[test]
fn named_arguments_bind_by_case_sensitive_parameter_name() {
    check(
        "<?php function f($a,$b=2,$C=3) { return $a.':'.$b.':'.$C; } echo f(C:9,a:1), '|', f(4,C:6,);",
        "1:2:9|4:2:6",
    );
    assert_eq!(
        run(b"<?php function f($A){} f(a:1);").unwrap_err().kind,
        ErrorKind::Error
    );
}
#[test]
fn variadics_keep_unknown_named_keys_and_reindex_positional_values() {
    check(
        "<?php function f($a,...$rest) { echo $a,'|'; foreach($rest as $k=>$v)echo $k,':',$v,';'; } f(1,2,3,z:4,rest:5);",
        "1|0:2;1:3;z:4;rest:5;",
    );
}
#[test]
fn unpacked_arguments_and_named_arguments_can_be_combined() {
    check(
        "<?php function f($a,$b,$c=9,...$rest) { echo $a,':',$b,':',$c,'|'; foreach($rest as $k=>$v)echo $k,':',$v,';'; } f(...[5=>1,8=>2],c:3,z:4);",
        "1:2:3|z:4;",
    );
    check(
        "<?php function f($a,$b=2) {return $a+$b;} echo f(...['b'=>7,'a'=>3]);",
        "10",
    );
}
#[test]
fn conditional_and_nested_functions_are_registered_when_executed() {
    check(
        "<?php if(true) { function first(){ function second(){return 7;} } } first(); echo second();",
        "7",
    );
    assert_eq!(
        run(b"<?php nope(); if(false){function nope(){}}")
            .unwrap_err()
            .kind,
        ErrorKind::Error
    );
}
#[test]
fn variable_function_names_work_case_insensitively() {
    check(
        "<?php function f($a){return [$a,2];} $fn='F'; echo $fn(9)[0], ':', ('f')(3)[1];",
        "9:2",
    );
}
#[test]
fn return_without_value_and_fallthrough_are_null() {
    check(
        "<?php function f(){return;} function g(){} echo f()===null, g()===null;",
        "11",
    );
}
#[test]
fn extra_positional_arguments_are_evaluated_for_user_functions() {
    check(
        "<?php function f($x) {return $x;} $i=0; echo f(7,$i++,$i++), ':', $i;",
        "7:2",
    );
}
#[test]
fn caller_scope_is_restored_after_runtime_errors() {
    let mut runtime = Runtime::default();
    assert!(
        runtime
            .execute(b"<?php $x=9; function fail($x){$x=3; echo 'inner'; return 1/0;} fail(1);")
            .is_err()
    );
    assert_eq!(runtime.variables.get(b"x".as_slice()), Some(&Value::Int(9)));
    assert_eq!(runtime.output, b"inner");
    runtime.execute(b"<?php echo $x;").unwrap();
    assert_eq!(runtime.output, b"inner9");
}
#[test]
fn invalid_calls_are_errors_and_stop_later_argument_evaluation() {
    let mut runtime = Runtime::default();
    let error=runtime.execute(b"<?php function mark($x){echo $x;return $x;} function f($a){} f(z:mark('x'),a:mark('y'));").unwrap_err();
    assert_eq!(error.kind, ErrorKind::Error);
    assert_eq!(runtime.output, b"x");
    for source in ["function f($a){} f();", "function f($a=1,$b){} f(b:2);"] {
        assert_eq!(
            format!(
                "{:?}",
                run(format!("<?php {source}").as_bytes()).unwrap_err().kind
            ),
            "ArgumentCountError"
        );
    }
    for source in [
        "function f($a){} f(1,a:2);",
        "function f(...$a){} f(...['x'=>1,0=>2]);",
        "function f($a){} f(...['a'=>1],a:2);",
    ] {
        assert_eq!(
            run(format!("<?php {source}").as_bytes()).unwrap_err().kind,
            ErrorKind::Error
        );
    }
    assert_eq!(
        run(b"<?php function f(...$a){} f(...1);").unwrap_err().kind,
        ErrorKind::TypeError
    );
}
#[test]
fn duplicate_declarations_are_detected_before_top_level_output() {
    let mut runtime = Runtime::default();
    assert!(
        runtime
            .execute(b"<?php echo 'wrong'; function f(){} function F(){}")
            .is_err()
    );
    assert!(runtime.output.is_empty());
    assert!(run(b"<?php function outer(){function inner(){}} outer(); outer();").is_err());
}
#[test]
fn invalid_parameter_lists_and_argument_order_are_parse_errors() {
    for source in [
        "function f($a,$a){}",
        "function f(...$a,$b){}",
        "function f(...$a=1){}",
        "function f($a=$x){}",
        "function f($a=other()){}",
        "function f($a){} f(a:1,2);",
        "function f($a){} f(...[1],2);",
        "while(false){function f(){break;}}",
    ] {
        assert_eq!(
            run(format!("<?php {source}").as_bytes()).unwrap_err().kind,
            ErrorKind::ParseError,
            "{source}"
        );
    }
}

#[test]
fn constant_array_access_is_allowed_in_default_expressions() {
    check(
        "<?php function f($x=[10,20][1]){return $x;} echo f();",
        "20",
    );
}

#[test]
fn optional_before_required_deprecation_is_reported_when_compiling() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php if(false){function f($a=1,$b){}} echo 'done';")
        .unwrap();
    assert_eq!(runtime.output, b"done");
    assert_eq!(runtime.warnings.len(), 1);
    assert!(runtime.warnings[0].contains("Optional parameter"));
}
