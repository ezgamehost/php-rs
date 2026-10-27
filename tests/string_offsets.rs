use php_rs::{ErrorKind, Runtime, run};
fn check(source: &str, expected: &[u8]) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected);
}
#[test]
fn string_offsets_read_bytes_and_negative_positions() {
    check(
        r#"<?php $s='éab'; echo $s[0],$s[1],$s[-1],$s['+2'],$s[' 02'];"#,
        b"\xc3\xa9baa",
    );
}
#[test]
fn out_of_bounds_reads_warn_and_probes_are_silent() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $s='ab'; echo $s[2],$s[-3],isset($s[2])?1:0,empty($s[-3])?1:0,$s[2]??'x';")
        .unwrap();
    assert_eq!(runtime.output, b"01x");
    assert_eq!(runtime.warnings.len(), 2);
}
#[test]
fn strings_support_indexed_interpolation() {
    check(r#"<?php $s='abc'; echo "$s[1]/{$s[-1]}";"#, b"b/c");
}
#[test]
fn offset_writes_return_the_assigned_byte_and_pad_with_spaces() {
    let mut runtime = Runtime::default();
    runtime
        .execute(
            b"<?php $s='abc'; $r=($s[0]='xyz'); $s[5]=7; echo $r,':',$s; $s[-1]='Z'; echo ':',$s;",
        )
        .unwrap();
    assert_eq!(runtime.output, b"x:xbc  7:xbc  Z");
    assert_eq!(runtime.warnings.len(), 1);
}
#[test]
fn string_mutation_does_not_affect_copies_or_nested_array_copies() {
    check(
        "<?php $a=['abc']; $b=$a; $b[0][1]='X'; $s=$a[0]; $s[0]='Y'; echo $a[0],':',$b[0],':',$s;",
        b"abc:aXc:Ybc",
    );
}
#[test]
fn invalid_negative_writes_return_null_without_modification() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $s='ab'; echo ($s[-3]='xyz')===null?1:0,':',$s;")
        .unwrap();
    assert_eq!(runtime.output, b"1:ab");
    assert_eq!(runtime.warnings.len(), 1);
}
#[test]
fn string_offset_coercion_differs_from_array_key_coercion() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $s='abc'; echo $s[1.5],$s[true],$s[null],$s['1tail'];")
        .unwrap();
    assert_eq!(runtime.output, b"bbab");
    assert_eq!(runtime.warnings.len(), 4);
}
#[test]
fn coalescing_and_probing_apply_distinct_numeric_prefix_rules() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php $s='abc'; echo isset($s['1tail'])?1:0,empty($s['1tail'])?1:0,$s['1tail']??'x', $s['bad']??'y';").unwrap();
    assert_eq!(runtime.output, b"01by");
    assert_eq!(runtime.warnings.len(), 1);
}
#[test]
fn coalescing_assignment_writes_missing_offsets_but_keeps_existing_bytes() {
    check(
        "<?php $s='abc'; $s[1]??='X'; $s[4]??='Z'; echo $s;",
        b"abc Z",
    );
}
#[test]
fn invalid_offset_types_are_errors_for_reads_but_false_for_isset() {
    for key in ["'abc'", "'1.5'", "[]"] {
        assert_eq!(
            run(format!("<?php $s='abc'; echo $s[{key}];").as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::TypeError
        );
        check(&format!("<?php $s='abc'; echo isset($s[{key}])?1:0;"), b"0");
    }
}
#[test]
fn unsupported_string_mutations_fail_without_changing_the_string() {
    for operation in [
        "$s[]='x';",
        "$s[0]++;",
        "$s[0].='x';",
        "$s[0][0]='x';",
        "$s[0]='';",
        "unset($s[0]);",
    ] {
        let mut runtime = Runtime::default();
        let error = runtime
            .execute(format!("<?php $s='abc'; {operation}").as_bytes())
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::Error, "{operation}");
        assert_eq!(
            runtime.variables.get(b"s".as_slice()),
            Some(&php_rs::Value::String(b"abc".to_vec()))
        );
    }
}
#[test]
fn array_keys_are_coerced_once_for_read_modify_write() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $a=[''=>1]; $a[null]+=1; echo $a[''];")
        .unwrap();
    assert_eq!(runtime.output, b"2");
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn fractional_probe_keys_warn_but_coalescing_does_not() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $s='abc'; echo isset($s[1.5])?1:0,empty($s[1.5])?1:0,$s[1.5]??'x';")
        .unwrap();
    assert_eq!(runtime.output, b"10b");
    assert_eq!(runtime.warnings.len(), 2);
    assert!(
        runtime
            .warnings
            .iter()
            .all(|w| w.contains("loses precision"))
    );
}
#[test]
fn array_keys_are_invalid_for_string_coalescing_but_false_for_isset() {
    assert_eq!(
        run(b"<?php $s='abc'; echo $s[[]]??'x';").unwrap_err().kind,
        ErrorKind::TypeError
    );
    check("<?php $s='abc'; echo isset($s[[]])?1:0;", b"0");
}
