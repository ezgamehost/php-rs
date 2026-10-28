use php_rs::{ErrorKind, Runtime, run};

#[test]
fn throw_and_catch_preserve_exception_identity_and_metadata() {
    assert_eq!(run(b"<?php $e=new Exception('hello',7);try{throw $e;}catch(Exception $caught){echo $caught->getMessage(),':',$caught->getCode();var_dump($caught===$e,$caught instanceof Throwable,$caught instanceof Error);}").unwrap(),b"hello:7bool(true)\nbool(true)\nbool(false)\n");
}
#[test]
fn errors_are_caught_through_the_builtin_hierarchy() {
    assert_eq!(run(b"<?php try{1/0;}catch(Exception $e){echo 'bad';}catch(ArithmeticError $e){echo get_class($e),':',$e->getMessage();}try{missing();}catch(Throwable $e){echo ':',get_class($e);}").unwrap(),b"DivisionByZeroError:Division by zero:Error");
}
#[test]
fn catch_unions_undefined_types_and_omitted_variables_work() {
    assert_eq!(run(b"<?php try{throw new TypeError('type');}catch(Missing|Exception){echo 'bad';}catch(TypeError|ValueError){echo 'ok';}").unwrap(),b"ok");
}
#[test]
fn custom_exception_subclasses_inherit_constructors_and_final_getters() {
    assert_eq!(run(b"<?php class Problem extends Exception{}try{throw new Problem('problem');}catch(Exception $e){echo get_class($e),':',$e->getMessage();}").unwrap(),b"Problem:problem");
    assert!(
        Runtime::default()
            .execute_code(b"class Problem extends Exception{function getMessage(){return 'bad';}}")
            .is_err()
    );
    assert!(
        Runtime::default()
            .execute_code(b"class Problem implements Throwable{}")
            .is_err()
    );
}
#[test]
fn native_constructor_calls_work_in_custom_subclasses() {
    assert_eq!(run(b"<?php class Problem extends Exception{function __construct($x){parent::__construct($x.'!',3);}}$e=new Problem('oops');echo $e->getMessage(),$e->getCode();").unwrap(),b"oops!3");
}
#[test]
fn thrown_nonthrowables_become_catchable_errors() {
    for expression in ["1", "null", "'x'", "[]", "new stdClass"] {
        assert_eq!(
            run(
                format!("<?php try{{throw {expression};}}catch(Error $e){{echo get_class($e);}}")
                    .as_bytes()
            )
            .unwrap(),
            b"Error"
        );
    }
}
#[test]
fn finally_runs_for_fallthrough_catches_and_unhandled_errors() {
    assert_eq!(run(b"<?php try{echo 'a';}finally{echo 'b';}try{try{throw new Exception('c');}finally{echo 'd';}}catch(Exception $e){echo $e->getMessage();}try{throw new Exception;}catch(Exception){echo 'e';}finally{echo 'f';}").unwrap(),b"abdcef");
}
#[test]
fn finally_overrides_returns_and_pending_exceptions() {
    assert_eq!(run(b"<?php function f(){try{return 1;}finally{return 2;}}function g(){try{throw new Exception;}finally{return 3;}}echo f(),g();").unwrap(),b"23");
}
#[test]
fn finally_runs_during_loop_control() {
    assert_eq!(
        run(
            b"<?php for($i=0;$i<3;$i++){try{if($i==0)continue;break;}finally{echo $i;}}echo 'end';"
        )
        .unwrap(),
        b"01end"
    );
}
#[test]
fn exit_skips_catches_and_finally() {
    let mut rt = Runtime::default();
    rt.execute_code(b"try{exit('bye');}catch(Throwable){echo 'bad';}finally{echo 'bad';}")
        .unwrap();
    assert_eq!(rt.output, b"bye");
    assert_eq!(rt.exit_status(), Some(0));
}
#[test]
fn finally_exception_retains_pending_error_as_previous() {
    assert_eq!(run(b"<?php try{try{throw new Exception('first');}finally{throw new Exception('second');}}catch(Exception $e){echo $e->getMessage(),':',$e->getPrevious()->getMessage();}").unwrap(),b"second:first");
}
#[test]
fn rethrowing_preserves_identity_and_does_not_make_previous_cycles() {
    assert_eq!(run(b"<?php $e=new Exception('x');try{try{throw $e;}finally{throw $e;}}catch(Exception $caught){var_dump($caught===$e,$caught->getPrevious());}").unwrap(),b"bool(true)\nNULL\n");
}
#[test]
fn explicit_previous_exception_and_named_constructor_arguments_work() {
    assert_eq!(run(b"<?php $a=new Error('a');$b=new Exception(previous:$a,code:9,message:'b');echo $b->getMessage(),$b->getCode(),$b->getPrevious()->getMessage();").unwrap(),b"b9a");
}
#[test]
fn throw_is_an_expression_in_short_circuit_and_arrow_functions() {
    assert_eq!(run(b"<?php $f=fn($x)=>$x??throw new Exception('null');echo $f(4);try{$f(null);}catch(Exception $e){echo $e->getMessage();}true||throw new Exception('bad');").unwrap(),b"4null");
}
#[test]
fn caught_exceptions_restore_function_and_class_scopes() {
    assert_eq!(run(b"<?php class A{private const X=7;static function f(){throw new Exception('x');}static function g(){try{self::f();}catch(Throwable $e){echo self::X,$e->getMessage();}}}$x=1;function f(){$x=2;throw new Exception;}try{f();}catch(Exception){}echo $x;A::g();").unwrap(),b"17x");
}
#[test]
fn runtime_error_origin_line_is_captured_before_unwinding() {
    assert_eq!(
        run(b"<?php\nfunction f(){\n missing();\n}\ntry{f();}catch(Error $e){echo $e->getLine();}")
            .unwrap(),
        b"3"
    );
}
#[test]
fn eval_parse_errors_are_catchable_but_source_parse_errors_are_not() {
    assert_eq!(
        run(b"<?php try{eval('echo ;');}catch(ParseError){echo 'parsed';}").unwrap(),
        b"parsed"
    );
    assert_eq!(
        run(b"<?php try{echo ;}catch(ParseError){}")
            .unwrap_err()
            .kind,
        ErrorKind::ParseError
    );
}
#[test]
fn catch_variables_remain_bound_and_finally_sees_mutations() {
    assert_eq!(run(b"<?php $e='old';try{throw new Exception('new');}catch(Exception $e){$x=4;}finally{echo $x;}echo $e->getMessage();").unwrap(),b"4new");
}
#[test]
fn exceptions_in_callbacks_and_saved_native_method_callables_work() {
    assert_eq!(run(b"<?php try{array_map(fn($x)=>throw new Exception('callback'),[1]);}catch(Exception $e){$f=$e->getMessage(...);echo $f();}").unwrap(),b"callback");
}
#[test]
fn jumps_enter_and_leave_try_regions_and_run_finally() {
    assert_eq!(
        run(b"<?php goto start;try{start:echo 'a';goto end;}finally{echo 'b';}end:echo 'c';")
            .unwrap(),
        b"abc"
    );
    assert!(run(b"<?php try{}finally{goto end;}end:;").is_err());
    assert!(run(b"<?php goto end;try{}finally{end:;}").is_err());
}
#[test]
fn invalid_try_catch_declarations_are_rejected() {
    for code in [
        "try{}",
        "try{}catch(){}",
        "try{}catch(self $e){}",
        "try{}catch(Exception $this){}",
        "try{}catch(Exception $GLOBALS){}",
    ] {
        assert!(
            Runtime::default().execute_code(code.as_bytes()).is_err(),
            "{code}"
        );
    }
}

#[test]
fn jumps_between_try_and_catch_regions_run_finally_once() {
    assert_eq!(run(b"<?php try{echo 'a';goto caught;}catch(Exception $e){caught:echo 'b';}finally{echo 'f';}").unwrap(),b"abf");
    assert_eq!(run(b"<?php $i=0;try{again:echo ++$i;if($i<2)throw new Exception;}catch(Exception){goto again;}finally{echo 'f';}").unwrap(),b"12f");
}

#[test]
fn eval_parse_error_metadata_uses_the_evaluated_source() {
    let mut rt = Runtime::default();
    rt.execute_code(b"try{eval('echo ;');}catch(ParseError $e){echo $e->getFile();}")
        .unwrap();
    assert!(String::from_utf8_lossy(&rt.output).contains("eval()'d code"));
}

#[test]
fn loop_control_cannot_jump_out_of_finally_but_inner_loops_are_allowed() {
    for jump in ["break", "continue", "break 2", "continue 2"] {
        assert!(
            Runtime::default()
                .execute_code(
                    format!("if(false){{for(;;){{for(;;){{try{{}}finally{{{jump};}}}}}}}}")
                        .as_bytes()
                )
                .is_err()
        );
    }
    assert_eq!(run(b"<?php try{}finally{for($i=0;$i<1;$i++){break;}function f(){for(;;){break;}}$f=function(){for(;;){break;}};f();$f();echo 'ok';}").unwrap(),b"ok");
}

#[test]
fn native_exception_constructors_weakly_accept_null_with_deprecations() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$e=new Exception(null,null);echo $e->getMessage(),$e->getCode();")
        .unwrap();
    assert_eq!(rt.output, b"0");
    assert_eq!(rt.warnings.len(), 2);
    assert_eq!(run(b"<?php declare(strict_types=1);try{new Exception(null);}catch(TypeError){echo 'strict';}").unwrap(),b"strict");
}

#[test]
fn uncaught_error_results_preserve_the_thrown_error_kind_and_class_name() {
    let error = run(b"<?php throw new TypeError('bad');").unwrap_err();
    assert_eq!(error.kind, ErrorKind::TypeError);
    let error =
        run(b"<?php class Problem extends Exception{}throw new Problem('bad');").unwrap_err();
    assert!(error.to_string().starts_with("Problem: bad"));
}
