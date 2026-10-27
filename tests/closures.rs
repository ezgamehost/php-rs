use php_rs::{ErrorKind, Runtime, run};

#[test]
fn anonymous_functions_capture_values_at_creation() {
    assert_eq!(
        run(b"<?php $x=2;$f=function($n)use($x){return $n+$x;};$x=9;echo $f(3);").unwrap(),
        b"5"
    );
    assert_eq!(
        run(b"<?php $x=2;$f=function()use($x){return ++$x;};echo $f(),$f(),$x;").unwrap(),
        b"332"
    );
}

#[test]
fn reference_captures_follow_outer_mutations_and_survive_scope_exit() {
    assert_eq!(run(b"<?php $x=2;$f=function()use(&$x){return ++$x;};$x=4;echo $f(),$x;function make(){ $n=0;return function()use(&$n){return ++$n;};}$g=make();echo $g(),$g();").unwrap(),b"5512");
}

#[test]
fn arrow_functions_capture_free_variables_and_shadow_parameters() {
    assert_eq!(
        run(b"<?php $x=2;$y=4;$f=fn($x)=>$x+$y;$y=9;echo $f(3);$g=fn()=>++$x;echo $g(),$g(),$x;")
            .unwrap(),
        b"7332"
    );
}

#[test]
fn nested_arrows_capture_variables_needed_by_inner_functions() {
    assert_eq!(run(b"<?php $x=3;$f=fn()=>fn($y)=>$x+$y;$g=fn()=>function()use($x){return $x;};$x=9;echo $f()(2),$g()();").unwrap(),b"53");
}

#[test]
fn inline_closures_work_in_pipes() {
    assert_eq!(
        run(b"<?php echo 2 |> (fn($n)=>$n+1) |> function($n){return $n*2;};").unwrap(),
        b"6"
    );
    for code in ["2 |> fn($n)=>$n+1;", "2 |> static fn($n)=>$n+1;"] {
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
fn closure_statics_are_per_instance_and_shared_by_aliases() {
    assert_eq!(run(b"<?php function make(){return function(){static $n=0;return ++$n;};}$a=make();$b=make();$c=$a;echo $a(),$a(),$b(),$c();").unwrap(),b"1213");
}

#[test]
fn closures_support_recursion_through_reference_capture() {
    assert_eq!(
        run(b"<?php $f=function($n)use(&$f){return $n<2?1:$n*$f($n-1);};echo $f(5);").unwrap(),
        b"120"
    );
}

#[test]
fn missing_captures_warn_at_the_correct_time() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$f=fn()=>$missing;echo 'made';").unwrap();
    assert!(rt.warnings.is_empty());
    rt.execute_code(b"$f();").unwrap();
    assert_eq!(rt.warnings.len(), 1);
    let mut rt = Runtime::default();
    rt.execute_code(b"$f=function()use($missing){};").unwrap();
    assert_eq!(rt.warnings.len(), 1);
    let mut rt = Runtime::default();
    rt.execute_code(b"$f=function()use(&$missing){};var_dump($missing);")
        .unwrap();
    assert!(rt.warnings.is_empty());
    assert_eq!(rt.output, b"NULL\n");
}

#[test]
fn closure_parameters_defaults_variadics_and_types_use_normal_binding() {
    assert_eq!(run(b"<?php $f=static function(int $a,$b=2,...$rest):int{return $a+$b+count($rest);};echo $f(b:4,a:'3'),$f(...[2,3,4]);$g=static fn(?int ...$n):array=>$n;var_dump($g(1,null));").unwrap(),b"76array(2) {\n  [0]=>\n  int(1)\n  [1]=>\n  NULL\n}\n");
    assert_eq!(
        Runtime::default()
            .execute_code(b"declare(strict_types=1);$f=fn(int $n)=>$n;$f('2');")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
}

#[test]
fn captures_are_separate_from_function_argument_inspection() {
    assert_eq!(
        run(
            b"<?php $x=9;$f=function($a)use($x){var_dump(func_num_args(),func_get_args());};$f(2);"
        )
        .unwrap(),
        b"int(1)\narray(1) {\n  [0]=>\n  int(2)\n}\n"
    );
}

#[test]
fn closures_use_global_array_live_without_capturing_it() {
    assert_eq!(
        run(b"<?php $x=1;$f=fn()=>$GLOBALS['x'];$x=3;echo $f();").unwrap(),
        b"3"
    );
}

#[test]
fn invalid_captures_and_cross_closure_jumps_fail_before_execution() {
    for code in [
        "$f=function($x)use($x){};",
        "$f=function()use($x,$x){};",
        "$f=function()use($this){};",
        "$f=function()use($GLOBALS){};",
        "$f=function()use($_GET){};",
        "$f=function()use($x){static $x;};",
        "$f=function(){goto end;};end:;",
        "while(true){$f=function(){break;};}",
        "$f=fn():void=>1;",
        "function(){}();",
    ] {
        assert_eq!(
            Runtime::default()
                .execute_code(format!("echo 'bad';{code}").as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::ParseError,
            "{code}"
        );
    }
}

#[test]
fn closure_identity_is_distinct_for_each_evaluation() {
    assert_eq!(run(b"<?php function make(){return fn()=>1;}$a=make();$b=make();var_dump($a==$b,$a===$b,$a===$a(...));").unwrap(),b"bool(false)\nbool(false)\nbool(true)\n");
}

#[test]
fn anonymous_function_magic_names_use_source_and_line() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$f=function(){echo __FUNCTION__,'/',__METHOD__;};$f();")
        .unwrap();
    assert_eq!(
        rt.output,
        b"{closure:Command line code:1}/{closure:Command line code:1}"
    );
}

#[test]
fn static_closures_can_initialize_php85_constants_and_defaults() {
    assert_eq!(run(b"<?php const F=static function($x){return $x+1;};echo (F)(2);function g($f=static function(){return 4;}){return $f();}echo g();").unwrap(), b"34");
    for code in [
        "const F=function(){};",
        "const F=static fn()=>1;",
        "const F=static function()use($x){};",
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
fn value_capture_arrays_are_independent_and_unset_detaches_references() {
    assert_eq!(run(b"<?php $a=[1];$f=function()use($a){$a[]=2;return count($a);};echo $f(),$f(),count($a);$x=3;$g=function()use(&$x){unset($x);$x=9;return $x;};echo $g(),$x;").unwrap(),b"22193");
}

#[test]
fn closure_context_is_restored_after_eval_and_errors() {
    let mut rt = Runtime::default();
    rt.execute_code(
        b"$f=function(){static $n=0;eval('static $n=7;');return ++$n;};echo $f(),$f();",
    )
    .unwrap();
    assert_eq!(rt.output, b"88");
    let err = rt
        .execute_code(b"$g=function(){static $n=0;++$n;missing();};$g();")
        .unwrap_err();
    assert_eq!(err.kind, ErrorKind::Error);
    rt.execute_code(b"echo $f();").unwrap();
    assert_eq!(rt.output, b"888");
}

#[test]
fn optional_parameter_warnings_are_emitted_once_at_compilation() {
    let mut rt = Runtime::default();
    rt.execute_code(b"if(false){$f=fn($a=1,$b)=>$b;}").unwrap();
    assert_eq!(rt.warnings.len(), 1);
    assert!(rt.warnings[0].contains("Optional parameter $a"));
}
