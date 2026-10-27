use php_rs::{Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn short_and_list_forms_assign_in_source_order() {
    check(
        "<?php [$a,$b]=[1,2];list($c,$d)=[3,4];echo $a,$b,$c,$d;",
        "1234",
    );
}

#[test]
fn omitted_entries_use_positions_and_do_not_fetch_values() {
    check(
        "<?php [,,$a,]=[2=>'x'];list(,$b,)=[1=>'y'];echo $a,$b;",
        "xy",
    );
}

#[test]
fn nested_destructuring_supports_positional_and_keyed_lists() {
    check(
        "<?php ['x'=>[$a,[$b]],'y'=>$c]=['x'=>[1,[2]],'y'=>3];list(list($d),$e)=[[4],5];echo $a,$b,$c,$d,$e;",
        "12345",
    );
}

#[test]
fn destructuring_returns_original_rhs_and_is_safe_for_swapping() {
    check(
        "<?php $a=1;$b=2;$r=([$a,$b]=[$b,$a]);echo $a,$b,$r[0],$r[1];",
        "2121",
    );
}

#[test]
fn source_is_snapshotted_before_targets_or_keys_modify_it() {
    check(
        "<?php $a=[10,20];[0=>$x,($a[1]=1)=>$y]=$a;echo $x,':',$y,':',$a[1];$a=[1,2];[$a,$b]=$a;echo ':',$a,$b;",
        "10:20:1:12",
    );
}

#[test]
fn targets_can_append_and_use_previously_assigned_indexes() {
    check(
        "<?php [$i,$a[$i]]=[2,3];[$a[],$a[]]=[4,5];foreach($a as $k=>$v)echo $k,$v;",
        "233445",
    );
}

#[test]
fn missing_keys_assign_null_and_warn_for_each_fetch() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php [$a,,$b]=[1];echo $a,':',$b===null?1:0;")
        .unwrap();
    assert_eq!(runtime.output, b"1:1");
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn scalar_sources_assign_null_and_null_sources_are_silent() {
    for (value, warnings) in [
        ("'abc'", 2),
        ("1", 2),
        ("false", 2),
        ("1.5", 2),
        ("null", 0),
    ] {
        let mut runtime = Runtime::default();
        runtime
            .execute(format!("<?php [$a,$b]={value};echo $a===null?1:0,$b===null?1:0;").as_bytes())
            .unwrap();
        assert_eq!(runtime.output, b"11");
        assert_eq!(runtime.warnings.len(), warnings, "{value}");
    }
}

#[test]
fn foreach_destructures_rows_and_preserves_keys() {
    check(
        "<?php foreach([[1,2],[3,4]] as $k=>[$a,$b])echo $k,$a,$b;foreach([['x'=>5]] as list('x'=>$c))echo $c;",
        "0121345",
    );
}

#[test]
fn keyed_null_coercion_warns_only_when_source_is_array() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php [null=>$a]=[''=>2];[null=>$b]=null;echo $a,$b===null?1:0;")
        .unwrap();
    assert_eq!(runtime.output, b"21");
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn key_expressions_and_targets_are_evaluated_once_in_order() {
    check(
        "<?php function mark_key($x){echo $x;return 0;}[mark_key('a')=>$v[mark_key('b')],mark_key('c')=>$w]=[7];echo $v[0],$w;",
        "abc77",
    );
}

#[test]
fn nested_missing_values_warn_only_on_the_missing_array_fetch() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php [[$a,$b]]=[];echo $a===null?1:0,$b===null?1:0;")
        .unwrap();
    assert_eq!(runtime.output, b"11");
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn invalid_patterns_are_rejected_even_when_unreachable() {
    for statement in [
        "[]=[1];",
        "list()=[1];",
        "[,,]=[1];",
        "[[]]=[[1]];",
        "[$a,1=>$b]=[1,2];",
        "[,'x'=>$a]=['x'=>1];",
        "[$a,list($b)]=[1,[2]];",
        "list($a,[$b])=[1,[2]];",
        "array($a,$b)=[1,2];",
        "[...$a]=[1,2];",
        "[1]=[1];",
        "[$a]+=1;",
        "foreach([[1]] as [$k]=>$v){}",
        "echo list($a);",
        "$a=[,1];",
        "$a=array(,1);",
        "$a=[list($x)];",
    ] {
        assert!(
            lint(format!("<?php if(false){{{statement}}}").as_bytes()).is_err(),
            "{statement}"
        );
    }
}

#[test]
fn destructuring_objects_reports_php_errors_without_panicking() {
    for value in ["new stdClass", "fn()=>1"] {
        let code = format!("<?php [$x]={value};");
        let error = php_rs::run(code.as_bytes()).unwrap_err();
        assert_eq!(error.kind, php_rs::ErrorKind::Error);
        assert!(error.message.contains("Cannot use object of type"));
    }
}
