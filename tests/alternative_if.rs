use php_rs::{Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn alternative_if_selects_only_first_truthy_branch() {
    check(
        "<?php if(false):echo 'bad';elseif(true):echo 'yes';elseif(1/0):echo 'bad';else:echo 'bad';endif;",
        "yes",
    );
}

#[test]
fn alternative_if_supports_empty_branches_and_no_else() {
    check(
        "<?php if(false):endif;if(true):elseif(true):echo 'bad';endif;if(false):elseif(false):else:echo 'else';endif;",
        "else",
    );
}

#[test]
fn alternative_conditionals_nest_with_loops_and_templates() {
    check(
        "<?php foreach([0,1,2] as $n):if($n): ?><<?php if($n===1): ?>one<?php else: ?>two<?php endif; ?>><?php else: ?>zero<?php endif;endforeach;",
        "zero<one><two>",
    );
}

#[test]
fn loop_control_propagates_through_alternative_branches() {
    check(
        "<?php for($i=0;$i<5;$i++):if($i===1):continue;elseif($i===3):break;else:echo $i;endif;endfor;",
        "02",
    );
}

#[test]
fn return_and_conditional_function_registration_are_preserved() {
    check(
        "<?php function f(){if(true):function g(){return 4;}return 3;else:return 0;endif;}echo function_exists('g')?1:0,f(),g();",
        "034",
    );
}

#[test]
fn alternative_function_diagnostics_are_emitted_before_execution() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php if(false):function f($a=1,$b){}endif;")
        .unwrap();
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn regular_else_may_contain_a_separate_alternative_if() {
    check(
        "<?php if(false)echo 'bad';else if(true):echo 'yes';endif;if(true):if(false)echo 'bad';else echo 'nested';endif;",
        "yesnested",
    );
}

#[test]
fn missing_endings_and_mixed_branch_syntax_are_parse_errors() {
    for source in [
        "if(true):echo 1;",
        "if(false):else if(true):endif;",
        "if(false):elseif(true)echo 1;endif;",
        "if(false){}elseif(true):endif;",
        "if(false):else:else:endif;",
        "if(true):endif",
    ] {
        assert!(
            lint(format!("<?php {source}").as_bytes()).is_err(),
            "{source}"
        );
    }
}
