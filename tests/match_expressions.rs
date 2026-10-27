use php_rs::{Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn match_compares_types_strictly() {
    check(
        "<?php foreach([1,'1',1.0,true,null] as $v){echo match($v){1=>'i','1'=>'s',1.0=>'f',true=>'b',null=>'n'};}",
        "isfbn",
    );
}

#[test]
fn only_first_matching_arm_result_is_evaluated() {
    check(
        "<?php echo match(2){1=>1/0,2=>'yes',2=>1/0,default=>1/0};",
        "yes",
    );
}

#[test]
fn grouped_conditions_are_lazy_and_allow_trailing_comma() {
    check(
        "<?php function mark($x){echo $x;return $x;}echo match(mark(2)){mark(1),mark(2),mark(3),=>'yes',default,=>'no',};",
        "212yes",
    );
}

#[test]
fn default_can_precede_matching_arms() {
    check(
        "<?php echo match(2){default=>1/0,2=>'yes'},match(9){default=>'fallback',1=>1/0};",
        "yesfallback",
    );
}

#[test]
fn match_can_nest_and_return_arrays_or_callables() {
    check(
        "<?php echo (match(1){1=>[match(2){2=>'x'}]})[0],(match(1){1=>'strlen'})('abc');",
        "x3",
    );
}

#[test]
fn match_array_identity_preserves_order_and_value_types() {
    check(
        "<?php echo match(['x'=>1,'y'=>2]){['y'=>2,'x'=>1]=>'bad',['x'=>'1','y'=>2]=>'bad',['x'=>1,'y'=>2]=>'yes'};",
        "yes",
    );
}

#[test]
fn match_is_an_expression_with_arithmetic_and_assignment_precedence() {
    check(
        "<?php $a=match(1){1=>2}+3; echo $a,2*match(true){false=>0,true=>4},match(false){false=>$a=7},$a;",
        "5877",
    );
}

#[test]
fn variable_subject_stays_live_but_expression_subject_is_copied() {
    check(
        "<?php $v=1;echo match(($v)){($v=2)=>'live',default=>'bad'};$v=[1];echo match($v[0]){($v[0]=2)=>'bad',default=>'copy'};",
        "livecopy",
    );
}

#[test]
fn undefined_subject_diagnostics_follow_comparison_evaluation() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php echo match($a){default=>'a'},match($b){($b=1)=>'b'},match($c){1=>'x',2=>'y',default=>'c'},match($d){true=>'x',false=>'y',default=>'d'};").unwrap();
    assert_eq!(runtime.output, b"abcd");
    assert_eq!(runtime.warnings.len(), 3);
}

#[test]
fn unmatched_values_produce_unhandled_match_errors_and_keep_output() {
    for (expression, message) in [
        ("42", "Unhandled match case 42"),
        ("null", "Unhandled match case NULL"),
        ("true", "Unhandled match case true"),
        ("false", "Unhandled match case false"),
        ("1.25", "Unhandled match case 1.25"),
        ("1.0", "Unhandled match case 1.0"),
        ("-0.0", "Unhandled match case -0.0"),
        ("[]", "Unhandled match case of type array"),
        ("'abc'", "Unhandled match case 'abc'"),
        (
            "'abcdefghijklmnopqrstuvwxyz'",
            "Unhandled match case 'abcdefghijklmno...'",
        ),
    ] {
        let mut runtime = Runtime::default();
        let error = runtime
            .execute(format!("<?php echo 'before'; match({expression}){{}};").as_bytes())
            .unwrap_err();
        assert_eq!(format!("{:?}", error.kind), "UnhandledMatchError");
        assert_eq!(error.message, message);
        assert_eq!(runtime.output, b"before");
    }
}

#[test]
fn unmatched_strings_escape_control_and_non_ascii_bytes() {
    let error = run(br#"<?php match("\n\r\t\v\e\f\0\xFF\\"){};"#).unwrap_err();
    assert_eq!(
        error.message,
        "Unhandled match case '\\n\\r\\t\\v\\e\\f\\x00\\xFF\\\\'"
    );
}

#[test]
fn empty_match_does_not_warn_on_an_undefined_subject() {
    let mut runtime = Runtime::default();
    let error = runtime.execute(b"<?php match($missing){};").unwrap_err();
    assert_eq!(format!("{:?}", error.kind), "UnhandledMatchError");
    assert!(runtime.warnings.is_empty());
}

#[test]
fn invalid_match_syntax_and_constant_defaults_are_rejected() {
    for source in [
        "match(1){default=>1,default=>2};",
        "match(1){default,2=>1};",
        "match(1){1,,2=>1};",
        "match(1){1=>2 3=>4};",
        "match(1){1=>['a']}[0];",
        "match(1){1=>'strlen'}('a');",
        "match(1){1=>2}",
        "function f($a=match(1){1=>2}){}",
    ] {
        assert!(
            lint(format!("<?php {source}").as_bytes()).is_err(),
            "{source}"
        );
    }
}
