use php_rs::{Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn scalar_casts_convert_without_implicit_numeric_warnings() {
    check(
        "<?php echo (int)'12tail',':',(int)'bad',':',(int)-1.9,':',(float)'2.5tail',':',(bool)'0'?1:0,':',(string)true;",
        "12:0:-1:2.5:0:1",
    );
}

#[test]
fn array_cast_preserves_arrays_wraps_scalars_and_empties_null() {
    check(
        "<?php $a=(array)null;$b=(array)'x';$c=(array)['y'=>2];echo count($a),$b[0],$c['y'];",
        "0x2",
    );
}

#[test]
fn casts_from_arrays_follow_empty_and_nonempty_rules() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php echo (int)[],(int)[1],(float)[],(float)[1],(bool)[]?1:0,(bool)[1]?1:0,(string)[1];").unwrap();
    assert_eq!(runtime.output, b"010101Array");
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn casts_bind_below_power_and_above_multiplication() {
    check(
        "<?php echo (int)2.5**2,':',(int)2.5*2,':',(string)[12][0],':',(int)(float)'3.5';",
        "6:4:12:3",
    );
}

#[test]
fn canonical_casts_are_case_insensitive_and_allow_spaces_and_tabs() {
    check("<?php echo ( InT\t)'4',( STRING )5;", "45");
}

#[test]
fn aliases_warn_once_per_source_occurrence_including_unreachable_code() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php if(false){(integer)1;(double)2;}for($i=0;$i<2;$i++){echo (binary)'x';}(boolean)false;").unwrap();
    assert_eq!(runtime.output, b"xx");
    assert_eq!(runtime.warnings.len(), 4);
    assert!(
        runtime
            .warnings
            .iter()
            .all(|warning| warning.contains("Non-canonical cast"))
    );
}

#[test]
fn constant_defaults_can_contain_casts() {
    check(
        "<?php function f($a=(int)'2.9',$b=(array)null){echo $a,count($b);}f();",
        "20",
    );
}

#[test]
fn casting_array_keeps_independent_copy_on_write_values() {
    check(
        "<?php $a=[[1]];$b=(array)$a;$b[0][0]=2;echo $a[0][0],$b[0][0];",
        "12",
    );
}

#[test]
fn out_of_range_float_and_float_string_integer_casts_differ() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php echo (int)'1e30',':',(int)1e30,':',(int)'1e999';")
        .unwrap();
    assert_eq!(runtime.output, b"9223372036854775807:5076964154930102272:0");
    assert_eq!(runtime.warnings.len(), 3);
}

#[test]
fn float_cast_preserves_negative_zero_from_strings() {
    check("<?php var_dump((float)'  -0tail');", "float(-0)\n");
}

#[test]
fn cast_tokens_do_not_allow_comments_or_newlines_inside() {
    for source in ["(\nint\n)1;", "(/*x*/int)1;", "(real)1;", "(unset)1;"] {
        assert!(lint(format!("<?php {source}").as_bytes()).is_err());
    }
}

#[test]
fn void_cast_discards_statement_and_for_expression_results() {
    check(
        "<?php (void) print 's';for((void)$i=0;(void)print 'c',$i<2;(void)$i++){echo $i;}",
        "sc0c1c",
    );
}

#[test]
fn void_cast_cannot_be_used_as_a_value_or_final_for_condition() {
    for source in [
        "$a=(void)1;",
        "echo (void)1;",
        "(void)(void)1;",
        "for(;(void)1;){}",
    ] {
        assert!(lint(format!("<?php {source}").as_bytes()).is_err());
    }
}
