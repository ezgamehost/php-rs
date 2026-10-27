use php_rs::{ErrorKind, Runtime, lint};

fn check(code: &str, expected: &str) {
    let mut rt = Runtime::default();
    rt.execute_code(code.as_bytes()).unwrap();
    assert_eq!(rt.output, expected.as_bytes());
    assert!(rt.warnings.is_empty(), "{:?}", rt.warnings);
}

#[test]
fn forward_backward_and_consecutive_labels() {
    check(
        "$i=0; again: echo ++$i; if($i<3)goto again; goto end; echo 'bad'; end: alias: echo 'done';",
        "123done",
    );
}

#[test]
fn labels_are_case_sensitive_and_accept_identifier_bytes() {
    check(
        "goto Label; label: echo 'lower'; goto end; Label: echo 'upper'; goto label; end: café: echo 'ok';",
        "upperlowerok",
    );
    assert!(
        lint(b"<?php goto Label; label:")
            .unwrap_err()
            .message
            .contains("undefined label 'Label'")
    );
}

#[test]
fn jumps_enter_branches_without_testing_conditions() {
    check(
        "$n=0; goto inside; if(++$n){echo 'bad'; inside: echo 'yes';}else{echo 'bad';} echo $n; goto other; if(++$n){echo 'bad';}elseif(++$n){other: echo 'other';}else{echo 'bad';} echo $n;",
        "yes0other0",
    );
}

#[test]
fn jumps_cross_sibling_branches_and_nested_blocks() {
    check(
        "if(true){{echo 'a';goto other;echo 'bad';}}else{{other:echo 'b';}} {goto nested;} {echo 'bad';{nested: echo 'c';}echo 'd';}echo 'e';",
        "abcde",
    );
}

#[test]
fn alternative_syntax_labels_resume_inside_selected_branch() {
    check(
        "goto target;if(false):echo 'bad';elseif(false):target:echo 'a';else:echo 'bad';endif;echo 'b';",
        "ab",
    );
}

#[test]
fn jumps_within_loops_preserve_iteration_state() {
    check(
        "for($i=0;$i<3;$i++){echo $i;goto tail;echo 'bad';tail:echo 'x';} $i=0;while($i<2){start:echo ++$i;if($i<2)goto start;} do{goto last;echo 'bad';last:echo 'd';}while(false);",
        "0x1x2x12d",
    );
}

#[test]
fn jumping_out_skips_loop_steps_and_conditions() {
    check(
        "$i=0;for(;$i<3;$i++){while(true){goto out;}}out:echo $i;do{goto done;}while(++$i);done:echo $i;",
        "00",
    );
}

#[test]
fn jump_from_inner_loop_can_resume_inside_outer_loop() {
    check(
        "for($i=0;$i<2;$i++){foreach([1,2] as $v){goto next;}next:echo $i,$v;}echo 'end';",
        "0111end",
    );
}

#[test]
fn jumps_in_a_single_statement_loop_body_resolve_across_if_branches() {
    check(
        "$i=0;while($i<2) if(true){goto other;}else{other:echo ++$i;}echo 'end';",
        "12end",
    );
}

#[test]
fn jumps_between_switch_cases_do_not_repeat_dispatch() {
    check(
        "$n=0;$i=0;switch(++$n){case 1:start:echo ++$i;if($i<2)goto other;break;case 2:other:echo 'x';goto start;default:echo 'bad';}echo ':',$n;",
        "1x2:1",
    );
    check(
        "switch(1){case 1:goto tail;case 2:if(false){tail:echo 'a';}echo 'b';case 3:echo 'c';}echo 'd';",
        "abcd",
    );
}

#[test]
fn foreach_jumps_keep_snapshot_and_leave_cleanly() {
    check(
        "$a=[1,2,3];foreach($a as $v){$a=[];goto emit;echo 'bad';emit:echo $v;if($v===2)goto done;}done:echo count($a);",
        "120",
    );
}

#[test]
fn break_continue_and_return_after_jumps_keep_their_meaning() {
    check(
        "for($i=0;$i<3;$i++){goto tail;tail:if($i===0)continue;if($i===2)break;echo $i;}function f(){goto value;return 0;value:return 7;}echo f();",
        "17",
    );
    check(
        "for($i=0;$i<2;$i++){switch(1){case 1:goto target;default:target:continue 2;}echo 'bad';}echo $i;",
        "2",
    );
}

#[test]
fn function_labels_are_separate_and_unconditional_functions_still_hoist() {
    check(
        "goto done;function f($n){done:echo $n;if($n){--$n;goto done;}}done:f(2);",
        "210",
    );
    check(
        "goto end;if(false){function hidden(){}}end:var_dump(function_exists('hidden'));",
        "bool(false)\n",
    );
}

#[test]
fn undefined_and_duplicate_labels_are_rejected_before_output() {
    for (code, message) in [
        ("goto missing;", "undefined label"),
        ("if(false){goto missing;}", "undefined label"),
        ("x:if(false){x:;}", "already defined"),
        ("function f(){goto missing;}", "undefined label"),
        ("goto inner;function f(){inner:;}", "undefined label"),
        ("outer:function f(){goto outer;}", "undefined label"),
    ] {
        let mut rt = Runtime::default();
        let err = rt
            .execute_code(format!("echo 'before';{code}").as_bytes())
            .unwrap_err();
        assert_eq!(err.kind, ErrorKind::ParseError);
        assert!(err.message.contains(message), "{err}");
        assert!(rt.output.is_empty());
    }
}

#[test]
fn jumping_into_any_loop_or_switch_is_a_compile_error() {
    for body in [
        "while(false){target:;}",
        "do{target:;}while(false);",
        "for(;;){target:break;}",
        "foreach([] as $v){target:;}",
        "switch(0){case 0:target:;}",
        "while(false){goto target;}while(false){target:;}",
        "while(false){if(false){goto target;}for(;;){target:;}}",
    ] {
        let code = if body.contains("goto") {
            body.to_owned()
        } else {
            format!("if(false)goto target;{body}")
        };
        let err = lint(format!("<?php {code}").as_bytes()).unwrap_err();
        assert!(
            err.message
                .contains("'goto' into loop or switch statement is disallowed"),
            "{err}"
        );
    }
}

#[test]
fn labels_are_local_to_each_compilation_unit() {
    check(
        "target:eval('goto target;echo 0;target:echo 1;');echo 2;",
        "12",
    );
    let mut rt = Runtime::default();
    rt.execute_code(b"target:").unwrap();
    assert!(
        rt.execute_code(b"goto target;")
            .unwrap_err()
            .message
            .contains("undefined label")
    );
    assert!(
        rt.execute_code(b"target:eval('goto target;');")
            .unwrap_err()
            .message
            .contains("undefined label")
    );
}

#[test]
fn malformed_labels_and_gotos_fail_lint() {
    for code in [
        "goto;",
        "goto 'label';",
        "goto $label;",
        "goto a\\b;",
        "goto \\label;",
        "a\\b:",
        "123:",
        "goto if;",
        "if:",
        "readonly:",
        "exit:",
        "die:",
        "__FILE__:",
    ] {
        assert!(lint(format!("<?php {code}").as_bytes()).is_err(), "{code}");
    }
    check(
        "goto enum;true:echo 't';goto done;enum:echo 'e';goto true;done: int:echo 'i';",
        "eti",
    );
}

#[test]
fn jumps_skip_inline_html_and_preserve_following_text() {
    let mut rt = Runtime::default();
    rt.execute(b"<?php goto next; ?>skip<?php next: ?>keep<?php echo '!'; ?>end")
        .unwrap();
    assert_eq!(rt.output, b"keep!end");
}

#[test]
fn error_lines_point_to_the_jump_or_duplicate_label() {
    let err = lint(b"<?php\ngoto missing;\n").unwrap_err();
    assert_eq!(err.line, 2);
    let err = lint(b"<?php\nx:\n\nx:").unwrap_err();
    assert_eq!(err.line, 4);
}
