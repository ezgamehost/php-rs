use php_rs::{Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn for_evaluates_all_expressions_in_order_and_uses_last_condition() {
    check(
        "<?php function mark($s){echo $s; return false;} for($i=0,mark('I');mark('C'),$i<2;$i++,mark('S')) echo $i;",
        "IC0SC1SC",
    );
}

#[test]
fn for_accepts_empty_clauses_and_runs_step_after_continue() {
    check(
        "<?php $i=0; for(;;$i++){if($i===1)continue; if($i===3)break; echo $i;} echo ':',$i;",
        "02:3",
    );
}

#[test]
fn for_with_false_condition_still_initializes() {
    check("<?php for($i=7;false;$i++)echo 'bad'; echo $i;", "7");
}

#[test]
fn do_while_runs_once_and_checks_condition_after_continue() {
    check(
        "<?php $i=0; do {$i++; continue;} while($i<3); echo $i; do echo 'x'; while(false);",
        "3x",
    );
}

#[test]
fn break_skips_do_condition_and_for_step() {
    check(
        "<?php do {echo 'd'; break;} while(1/0); for(;;1/0){echo 'f'; break;}",
        "df",
    );
}

#[test]
fn numbered_continue_reaches_outer_for_step() {
    check(
        "<?php for($i=0;$i<3;$i++){foreach([1,2] as $j){echo $i,$j;continue 2;}echo 'bad';}echo ':',$i;",
        "011121:3",
    );
}

#[test]
fn numbered_break_propagates_across_mixed_loops() {
    check(
        "<?php foreach([1,2] as $x){do{while(true){echo $x;break ((3));}echo 'bad';}while(true);echo 'bad';}echo 'done';",
        "1done",
    );
}

#[test]
fn numbered_continue_reaches_outer_do_condition() {
    check(
        "<?php $i=0;do{$i++;while(true){echo $i;continue 2;}echo 'bad';}while($i<3);",
        "123",
    );
}

#[test]
fn loop_alternative_syntax_supports_nested_templates() {
    check(
        "<?php for($i=0;$i<2;$i++): ?>[<?php foreach(['a','b'] as $x): echo $x; endforeach; ?>]<?php endfor; $i=0;while($i++<2): echo $i; endwhile;",
        "[ab][ab]12",
    );
}

#[test]
fn return_propagates_out_of_loops_without_condition_or_step() {
    check(
        "<?php function f(){for(;;1/0){do{return 42;}while(1/0);}}echo f();",
        "42",
    );
}

#[test]
fn declarations_in_loops_are_registered_when_executed() {
    check(
        "<?php for(;false;){function absent(){}} do {function present(){return 5;}}while(false);echo function_exists('absent')?1:0,present();",
        "05",
    );
}

#[test]
fn function_diagnostics_include_unexecuted_loop_bodies() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php for(;false;){function f($a=1,$b){}} do{break;function g($a=1,$b){}}while(false);").unwrap();
    assert_eq!(runtime.warnings.len(), 2);
}

#[test]
fn numbered_control_rejects_invalid_depths_before_execution() {
    for control in ["break", "continue"] {
        for level in ["0", "-1", "+1", "1.0", "'1'", "true", "$n", "1+0", "2"] {
            let mut runtime = Runtime::default();
            assert!(
                runtime
                    .execute(
                        format!("<?php echo 'bad';while(false){{{control} {level};}}").as_bytes()
                    )
                    .is_err()
            );
            assert!(runtime.output.is_empty());
        }
    }
}

#[test]
fn loop_depth_does_not_cross_function_boundaries() {
    assert!(lint(b"<?php for(;;){function f(){break;}}").is_err());
    assert!(lint(b"<?php do{function f(){continue;}}while(false);").is_err());
}

#[test]
fn malformed_loop_headers_and_endings_are_rejected() {
    for source in [
        "<?php for($i=0,;false;){}",
        "<?php for(;false,;){}",
        "<?php for(;;$i++,){}",
        "<?php for(;;): endwhile;",
        "<?php while(false): endfor;",
        "<?php do{}while(false)",
    ] {
        assert!(lint(source.as_bytes()).is_err(), "{source}");
    }
}
