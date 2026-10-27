use php_rs::{ErrorKind, Runtime, run};

#[test]
fn map_preserves_single_array_keys() {
    assert_eq!(
        run(b"<?php var_dump(array_map(fn($x)=>$x*2,['a'=>3,7=>4]));").unwrap(),
        b"array(2) {\n  [\"a\"]=>\n  int(6)\n  [7]=>\n  int(8)\n}\n"
    );
}

#[test]
fn map_multiple_arrays_zips_by_position_and_pads_with_null() {
    assert_eq!(run(b"<?php var_dump(array_map(fn($a,$b)=>[$a,$b],['x'=>1,8=>2],['y'=>3]));").unwrap(),b"array(2) {\n  [0]=>\n  array(2) {\n    [0]=>\n    int(1)\n    [1]=>\n    int(3)\n  }\n  [1]=>\n  array(2) {\n    [0]=>\n    int(2)\n    [1]=>\n    NULL\n  }\n}\n");
    assert_eq!(
        run(b"<?php var_dump(array_map(null,[],[2]));").unwrap(),
        b"array(1) {\n  [0]=>\n  array(2) {\n    [0]=>\n    NULL\n    [1]=>\n    int(2)\n  }\n}\n"
    );
}

#[test]
fn map_null_callback_retains_single_input_append_history() {
    assert_eq!(run(b"<?php $a=[1,2];unset($a[1]);$b=array_map(null,$a);$b[]=3;$c=array_map(fn($x)=>$x,$a);$c[]=3;var_dump(array_keys($b),array_keys($c));").unwrap(),b"array(2) {\n  [0]=>\n  int(0)\n  [1]=>\n  int(2)\n}\narray(2) {\n  [0]=>\n  int(0)\n  [1]=>\n  int(1)\n}\n");
}

#[test]
fn filter_without_callback_removes_false_values_and_keeps_keys() {
    assert_eq!(
        run(b"<?php var_dump(array_filter([null,false,0,0.0,'','0',[],2,'a']));").unwrap(),
        b"array(2) {\n  [7]=>\n  int(2)\n  [8]=>\n  string(1) \"a\"\n}\n"
    );
}

#[test]
fn filter_can_pass_keys_or_both_value_and_key() {
    assert_eq!(run(b"<?php $a=['a'=>1,'b'=>2];var_dump(array_filter($a,fn($k)=>$k==='b',ARRAY_FILTER_USE_KEY),array_filter($a,fn($v,$k)=>$v===1&&$k==='a',ARRAY_FILTER_USE_BOTH));").unwrap(),b"array(1) {\n  [\"b\"]=>\n  int(2)\n}\narray(1) {\n  [\"a\"]=>\n  int(1)\n}\n");
}

#[test]
fn other_filter_modes_pass_only_values() {
    assert_eq!(
        run(b"<?php var_dump(array_filter(['a'=>2],fn(...$v)=>count($v)===1&&$v[0]===2,3));")
            .unwrap(),
        b"array(1) {\n  [\"a\"]=>\n  int(2)\n}\n"
    );
}

#[test]
fn reduce_threads_carry_in_insertion_order_and_handles_empty_input() {
    assert_eq!(run(b"<?php $f=fn($carry,$v)=>$carry.$v;var_dump(array_reduce(['b'=>2,'a'=>1],$f,'x'),array_reduce([],$f),array_reduce([],$f,7));").unwrap(),b"string(3) \"x21\"\nNULL\nint(7)\n");
}

#[test]
fn internal_callbacks_coerce_arguments_even_from_strict_callers() {
    assert_eq!(run(b"<?php declare(strict_types=1);var_dump(array_map(fn(int $x)=>$x+1,['2']),array_map('strlen',[42]));").unwrap(),b"array(1) {\n  [0]=>\n  int(3)\n}\narray(1) {\n  [0]=>\n  int(2)\n}\n");
    let mut rt = Runtime::default();
    assert_eq!(
        rt.execute_code(b"declare(strict_types=1);array_map(fn(int $x):string=>$x,['2']);")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
}

#[test]
fn callbacks_accept_named_parameters_on_the_outer_builtin() {
    assert_eq!(run(b"<?php echo array_reduce(callback:fn($c,$v)=>$c+$v,initial:10,array:[1,2]);var_dump(array_map(array:['a'],callback:strlen(...)));").unwrap(),b"13array(1) {\n  [0]=>\n  int(1)\n}\n");
}

#[test]
fn invalid_callbacks_and_arrays_fail_even_for_empty_input() {
    for code in [
        "array_map('missing',[]);",
        "array_filter([],'missing');",
        "array_reduce([],null);",
        "array_map(null,[],3);",
        "array_filter(3);",
        "array_reduce(3,fn($x)=>$x);",
    ] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::TypeError,
            "{code}"
        );
    }
    assert_eq!(
        Runtime::default()
            .execute_code(b"array_map(null,[],arrays:[1]);")
            .unwrap_err()
            .kind,
        ErrorKind::ArgumentCountError
    );
}

#[test]
fn callbacks_iterate_a_snapshot_when_the_original_array_changes() {
    assert_eq!(run(b"<?php $a=[1,2,3];$f=function($v)use(&$a){$a=[];return $v*2;};var_dump(array_map($f,$a),$a);").unwrap(),b"array(3) {\n  [0]=>\n  int(2)\n  [1]=>\n  int(4)\n  [2]=>\n  int(6)\n}\narray(0) {\n}\n");
}

#[test]
fn filter_nan_truth_tests_warn_once_per_element() {
    let mut rt = Runtime::default();
    rt.execute_code(b"array_filter([NAN,NAN]);array_filter([1,2],fn($v)=>NAN);")
        .unwrap();
    assert_eq!(rt.warnings.len(), 4);
    assert!(rt.warnings.iter().all(|s| s.contains("coerced to bool")));
}

#[test]
fn callback_errors_restore_strictness_and_exit_stops_iteration() {
    let mut rt = Runtime::default();
    rt.execute_code(b"function f(int $x){return $x;}").unwrap();
    assert_eq!(
        rt.execute_code(b"declare(strict_types=1);array_map('f',['2']);echo 'mapped';f('2');")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    assert_eq!(rt.output, b"mapped");
    assert_eq!(
        rt.execute_code(b"declare(strict_types=1);array_map('f',['bad']);")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    assert_eq!(
        rt.execute_code(b"declare(strict_types=1);f('2');")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    let mut rt = Runtime::default();
    rt.execute_code(b"array_map(function($v){echo $v;exit(7);},[1,2]);echo 'bad';")
        .unwrap();
    assert_eq!(rt.output, b"1");
    assert_eq!(rt.exit_status(), Some(7));
}

#[test]
fn empty_arrays_validate_callbacks_but_do_not_invoke_them() {
    assert_eq!(run(b"<?php $f=function($a,$b){echo 'bad';};var_dump(array_map($f,[]),array_filter([],$f),array_reduce([],$f,5));").unwrap(),b"array(0) {\n}\narray(0) {\n}\nint(5)\n");
    assert_eq!(
        Runtime::default()
            .execute_code(b"array_map(fn($a,$b)=>$a,[1]);")
            .unwrap_err()
            .kind,
        ErrorKind::ArgumentCountError
    );
}

#[test]
fn nested_callbacks_preserve_capture_and_static_state() {
    assert_eq!(run(b"<?php $n=0;$f=function($row)use(&$n){return array_reduce($row,function($sum,$x)use(&$n){++$n;return $sum+$x;},0);};var_dump(array_map($f,[[1,2],[3]]),$n);").unwrap(),b"array(2) {\n  [0]=>\n  int(3)\n  [1]=>\n  int(3)\n}\nint(3)\n");
}
