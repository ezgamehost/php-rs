use php_rs::{Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn switch_uses_loose_comparison_and_falls_through() {
    check(
        "<?php switch('2'){case 1:echo 'bad';break;case 2:echo 'two';case 3:echo 'three';break;default:echo 'bad';}",
        "twothree",
    );
}

#[test]
fn default_is_only_selected_after_all_cases_fail() {
    check(
        "<?php switch(2){default:echo 'bad';break;case 2:echo 'two';}switch(7){case 0:echo 'bad';break;default:echo 'd';case 2:echo 'tail';}",
        "twodtail",
    );
}

#[test]
fn empty_switch_and_no_match_do_nothing() {
    check(
        "<?php switch(1){} switch(1){case 2:echo 'bad';}echo 'ok';",
        "ok",
    );
}

#[test]
fn subject_expression_runs_once_and_case_evaluation_stops_on_match() {
    check(
        "<?php function mark($v){echo $v;return $v;}switch(mark(2)){case mark(1):echo 'bad';break;case mark(2):echo 'yes';case mark(3):echo 'tail';}",
        "212yestail",
    );
}

#[test]
fn variable_subject_observes_case_side_effects_but_indexed_subject_is_copied() {
    check(
        "<?php $a=1;switch(($a)){case ($a=2):echo 'yes';}$a=[1];switch($a[0]){case ($a[0]=2):echo 'bad';break;default:echo 'no';}",
        "yesno",
    );
}

#[test]
fn undefined_subject_is_read_for_each_comparison() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php switch($a){}switch($b){default:echo 'd';}switch($c){case 1:break;case 2:break;}").unwrap();
    assert_eq!(runtime.output, b"d");
    assert_eq!(runtime.warnings.len(), 2);
    assert!(
        runtime
            .warnings
            .iter()
            .all(|warning| warning.contains("$c"))
    );
}

#[test]
fn numbered_control_counts_switch_as_a_level() {
    check(
        "<?php for($i=0;$i<3;$i++){switch($i){case 0:continue 2;case 1:echo 'one';break;case 2:break 2;}echo 'tail';}echo $i;",
        "onetail2",
    );
}

#[test]
fn continue_targeting_switch_breaks_it_and_warns_once_at_compile_time() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php for($i=0;$i<2;$i++){switch(1){case 1:continue;}echo $i;}if(false){switch(1){case 1:while(false){continue 2;}}}").unwrap();
    assert_eq!(runtime.output, b"01");
    assert_eq!(runtime.warnings.len(), 2);
    assert!(
        runtime
            .warnings
            .iter()
            .all(|warning| warning.contains("targeting switch"))
    );
}

#[test]
fn continue_warning_is_local_to_the_function_control_stack() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php switch(1){case 1:function f(){while(false){continue;}}}f();")
        .unwrap();
    assert!(runtime.warnings.is_empty());
}

#[test]
fn case_semicolon_deprecation_is_emitted_for_unexecuted_switches() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php if(false){switch(1){;case 1;break;default;break;}}switch(1):;case 1:echo 'ok';endswitch;").unwrap();
    assert_eq!(runtime.output, b"ok");
    assert_eq!(runtime.warnings.len(), 2);
    assert!(
        runtime
            .warnings
            .iter()
            .all(|warning| warning.contains("semicolon"))
    );
}

#[test]
fn switch_supports_alternative_syntax_and_inline_html() {
    check(
        "<?php switch(2):case 1: ?>bad<?php break;case 2: ?>yes<?php break;default: ?>bad<?php endswitch;",
        "yes",
    );
}

#[test]
fn return_and_function_declarations_work_in_case_bodies() {
    check(
        "<?php function f(){switch(1){case 1:function g(){return 7;}return g();}return 0;}echo function_exists('g')?1:0,f(),g();",
        "077",
    );
}

#[test]
fn switch_function_diagnostics_are_not_hidden_by_unmatched_cases() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php switch(1){case 2:function f($a=1,$b){}} ")
        .unwrap();
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn malformed_switches_fail_before_output() {
    for source in [
        "switch(1){default:break;default:break;}",
        "switch(1){;;case 1:break;}",
        "switch(1){echo 'bad';case 1:break;}",
        "switch(1){case 1 break;}",
        "switch(1){case 1:break 2;}",
        "switch(1){case 1:function f(){break;}}",
        "switch(1):case 1:break;}",
    ] {
        let mut runtime = Runtime::default();
        assert!(
            runtime
                .execute(format!("<?php echo 'bad';{source}").as_bytes())
                .is_err(),
            "{source}"
        );
        assert!(runtime.output.is_empty());
    }
    assert!(lint(b"<?php switch(1):case 1:break;endswitch").is_err());
}
