use php_rs::{ErrorKind, Runtime, run};
fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn array_literals_and_ordered_iteration() {
    check(
        "<?php $a = ['x'=>1, 2, 'y'=>3, 'x'=>4,]; foreach ($a as $k=>$v) echo $k, ':', $v, ';'; echo array(8,9)[1];",
        "x:4;0:2;y:3;9",
    );
}
#[test]
fn keys_coerce_only_canonical_integers() {
    check(
        "<?php $a=['8'=>'a',8=>'b','08'=>'c','+8'=>'d','-0'=>'e',true=>'f',null=>'g']; foreach ($a as $k=>$v) echo $k, ':', $v, ';';",
        "8:b;08:c;+8:d;-0:e;1:f;:g;",
    );
}
#[test]
fn nested_writes_autovivify_and_evaluate_indexes_once() {
    check(
        "<?php $i=0; $a[$i++]['x'] = 4; $a[0]['x'] += 3; echo $i, ',', $a[0]['x']++, ',', $a[0]['x']; $a[][]=9; echo ',', $a[1][0];",
        "1,7,8,9",
    );
}
#[test]
fn array_assignment_uses_independent_nested_values() {
    check(
        "<?php $a=['x'=>[1,2]]; $b=$a; $b['x'][0]=9; $b['x'][]=3; echo $a['x'][0], ',', $b['x'][0]; foreach($a['x'] as $v) echo ':',$v; foreach($b['x'] as $v) echo ':',$v;",
        "1,9:1:2:9:2:3",
    );
}
#[test]
fn append_index_remembers_deleted_entries_and_negative_keys() {
    check(
        "<?php $a=[-5=>1]; $a[]=2; unset($a[-4]); $a[]=3; foreach($a as $k=>$v) echo $k, ':', $v, ';'; $b=[9=>1]; unset($b[9]); $b[]=2; echo $b[10];",
        "-5:1;-3:3;2",
    );
}
#[test]
fn maximum_integer_key_does_not_wrap() {
    assert_eq!(
        run(b"<?php $a=[9223372036854775807=>1]; $a[]=2;")
            .unwrap_err()
            .kind,
        ErrorKind::Error
    );
    check(
        "<?php $a=[9223372036854775807=>1]; unset($a[9223372036854775807]); $a[]=2; echo $a[9223372036854775807];",
        "2",
    );
}
#[test]
fn union_keeps_left_keys_and_appends_new_right_keys() {
    check(
        "<?php $a=[2=>'a','x'=>'b']; $a += [2=>'wrong',0=>'c','y'=>'d']; foreach($a as $k=>$v) echo $k, ':', $v, ';';",
        "2:a;x:b;0:c;y:d;",
    );
}
#[test]
fn array_comparisons_separate_order_from_loose_key_equality() {
    check(
        "<?php $a=['x'=>1,'y'=>2]; $b=['y'=>'2','x'=>1]; echo $a==$b, ',', $a===$b ? 1:0, ',', []==false, ',', [] < [1], ',', ['a'=>1] <=> ['b'=>1];",
        "1,0,1,1,1",
    );
}
#[test]
fn missing_keys_warn_but_coalescing_and_isset_are_silent() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php $a=[]; echo $a['x']; echo $a['x']['y'] ?? 'fallback'; echo isset($a['x']), empty($a['x']); $a['x'] ??= 4; echo $a['x'];").unwrap();
    assert_eq!(runtime.output, b"fallback14");
    assert_eq!(runtime.warnings.len(), 1);
    assert!(runtime.warnings[0].contains("Undefined array key"));
}
#[test]
fn foreach_by_value_uses_snapshot_and_leaves_last_value() {
    check(
        "<?php $a=[1,2,3]; foreach($a as $k=>$v) { $a[]=9; if($v==2) continue; echo $k, ':', $v, ';'; } echo $v;",
        "0:1;2:3;3",
    );
}
#[test]
fn unset_variables_and_nested_keys_does_not_change_copies() {
    check(
        "<?php $a=['x'=>[1,2]]; $b=$a; unset($b['x'][0],$missing); echo isset($a['x'][0]), isset($b['x'][0]) ? 1:0; unset($a); echo empty($a);",
        "101",
    );
}
#[test]
fn invalid_key_and_scalar_write_produce_type_errors() {
    for code in ["$a=[[]=>1];", "$a=[]; $a[[]]=1;"] {
        assert_eq!(
            run(format!("<?php {code}").as_bytes()).unwrap_err().kind,
            ErrorKind::TypeError
        );
    }
    assert_eq!(
        run(b"<?php $a=42; $a[]=1;").unwrap_err().kind,
        ErrorKind::Error
    );
}
#[test]
fn isset_short_circuits_later_arguments() {
    check(
        "<?php $a=[]; $i=0; echo isset($a['x'], $a[$i++]) ? 1:0; echo $i; $a['x']=null; echo isset($a['x']) ? 1:0; echo empty([1]) ? 1:0;",
        "0000",
    );
}
#[test]
fn assignment_evaluates_key_before_rhs() {
    check("<?php $i=0; $a[$i++] = $i=5; echo $a[0], ',', $i;", "5,5");
}
#[test]
fn array_to_string_and_fractional_key_emit_diagnostics() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $a=[1.5=>7]; echo $a[1], ':', $a; echo 'x'.$a;")
        .unwrap();
    assert_eq!(runtime.output, b"7:ArrayxArray");
    assert_eq!(runtime.warnings.len(), 3);
}

#[test]
fn php85_null_key_deprecation_excludes_unset() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php $a=[null=>7]; echo $a[null], isset($a[null]), empty($a[null]) ? 1:0, $a[null]??0; unset($a[null]);").unwrap();
    assert_eq!(runtime.output, b"7107");
    assert_eq!(runtime.warnings.len(), 5);
    assert!(
        runtime
            .warnings
            .iter()
            .all(|s| s.contains("null as an array offset"))
    );
}

#[test]
fn scalar_and_missing_offset_reads_do_not_convert_key_types() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $a=1; echo $a[[]]??4; echo $missing[null]??5; echo $a[[]];")
        .unwrap();
    assert_eq!(runtime.output, b"45");
    assert_eq!(runtime.warnings.len(), 1);
}
