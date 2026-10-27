use php_rs::{ErrorKind, Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn unpacking_reindexes_integer_keys_in_iteration_order() {
    check(
        "<?php $a=[...[9=>'a',-2=>'b'],...[4=>'c'],'d'];foreach($a as $k=>$v)echo $k,$v;",
        "0a1b2c3d",
    );
}

#[test]
fn string_keys_overwrite_without_moving_their_positions() {
    check(
        "<?php $a=[...['x'=>1,'y'=>2],'x'=>3,...['x'=>4,'z'=>5]];foreach($a as $k=>$v)echo $k,$v;",
        "x4y2z5",
    );
}

#[test]
fn numeric_string_keys_follow_their_stored_key_types() {
    check(
        "<?php $a=[...['2'=>'a','02'=>'b','-1'=>'c']];foreach($a as $k=>$v)echo $k,':',$v,';';",
        "0:a;02:b;1:c;",
    );
}

#[test]
fn unpacking_respects_destination_append_index_including_negative_keys() {
    check(
        "<?php $a=[-3=>'a',...[12=>'b',15=>'c']];foreach($a as $k=>$v)echo $k,$v;",
        "-3a-2b-1c",
    );
}

#[test]
fn unpacking_evaluates_operands_once_in_source_order() {
    check(
        "<?php function items($x){echo $x;return [$x];}$a=[...items('a'),print 'b',...items('c')];foreach($a as $v)echo $v;",
        "abca1c",
    );
}

#[test]
fn unpacking_preserves_nested_value_copy_semantics() {
    check(
        "<?php $a=[[1]];$b=[...$a];$b[0][0]=2;echo $a[0][0],$b[0][0];",
        "12",
    );
}

#[test]
fn long_array_syntax_empty_operands_and_constant_defaults_are_supported() {
    check(
        "<?php function f($x=[...[1,2],...[]]){return $x;}$a=array(...[],...f(),3,);foreach($a as $v)echo $v;",
        "123",
    );
}

#[test]
fn scalar_operands_fail_at_runtime_without_evaluating_later_elements() {
    for value in ["null", "true", "1", "1.5", "'abc'"] {
        let mut runtime = Runtime::default();
        let error = runtime
            .execute(format!("<?php $x={value};echo 'before';$a=[...$x,print 'after'];").as_bytes())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Error);
        assert!(
            error
                .message
                .contains("Only arrays and Traversables can be unpacked")
        );
        assert_eq!(runtime.output, b"before");
    }
}

#[test]
fn unpacking_an_integer_after_the_maximum_key_fails() {
    let error = run(b"<?php $a=[9223372036854775807=>1,...[2]];").unwrap_err();
    assert_eq!(error.kind, ErrorKind::Error);
}

#[test]
fn unpacking_cannot_have_an_explicit_key() {
    assert!(lint(b"<?php $a=['x'=>...[1]];").is_err());
    assert!(lint(b"<?php $a=[...[1]=>2];").is_err());
}
