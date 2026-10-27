use php_rs::{ErrorKind, Runtime};

fn check(code: &str, output: &str, kinds: &[&str]) {
    let mut rt = Runtime::default();
    rt.execute_code(code.as_bytes()).unwrap();
    assert_eq!(rt.output, output.as_bytes(), "{code}");
    let expected: Vec<_> = kinds
        .iter()
        .map(|kind| format!("unexpected NAN value was coerced to {kind} on line 1"))
        .collect();
    assert_eq!(rt.warnings, expected, "{code}");
}

#[test]
fn string_output_and_concatenation_warn_once_per_conversion() {
    check(
        "$x=NAN;echo $x;print $x;$s='x'.$x.$x;echo $s;",
        "NANNANxNANNAN",
        &["string", "string", "string", "string"],
    );
    check("$x=NAN;echo \"a{$x}b\";", "aNANb", &["string"]);
}

#[test]
fn casts_warn_only_for_changed_types() {
    check(
        "$x=NAN;var_dump((bool)$x,(string)$x,(array)$x,(float)$x);(void)$x;",
        "bool(true)\nstring(3) \"NAN\"\narray(1) {\n  [0]=>\n  float(NAN)\n}\nfloat(NAN)\n",
        &["bool", "string", "array"],
    );
}

#[test]
fn truth_tests_and_boolean_operations_warn_on_evaluated_operands() {
    check(
        "$x=NAN;if($x)echo 'a';echo $x?'b':'c';var_dump(!$x,empty($x),$x&&$x,$x||$x,$x xor false);",
        "abbool(false)\nbool(false)\nbool(true)\nbool(true)\nbool(true)\n",
        &[
            "bool", "bool", "bool", "bool", "bool", "bool", "bool", "bool",
        ],
    );
    check(
        "$x=NAN;var_dump(false&&$x,true||$x,true&&$x,false||$x);",
        "bool(false)\nbool(true)\nbool(true)\nbool(true)\n",
        &["bool", "bool"],
    );
}

#[test]
fn loop_conditions_convert_on_each_test() {
    check(
        "$x=NAN;$i=0;while($x){echo ++$i;if($i===2)break;} do{echo 'd';$x=false;}while($x);for(;$x;){}",
        "12d",
        &["bool", "bool"],
    );
}

#[test]
fn short_ternary_preserves_nan_after_boolean_conversion() {
    check("$x=NAN;var_dump($x?:0);", "float(NAN)\n", &["bool"]);
}

#[test]
fn native_parameter_coercion_warns_without_duplicate_conversions() {
    check(
        "var_dump(strlen(NAN),array_keys([1],1,NAN));",
        "int(3)\narray(1) {\n  [0]=>\n  int(0)\n}\n",
        &["string", "bool"],
    );
}

#[test]
fn typed_boolean_parameters_and_returns_warn() {
    check(
        "function f(bool $x){var_dump($x);}function g():bool{return NAN;}f(NAN);var_dump(g());",
        "bool(true)\nbool(true)\n",
        &["bool", "bool"],
    );
}

#[test]
fn variable_names_use_string_conversion() {
    check(
        "$x=NAN;$$x=7;echo $NAN;$GLOBALS[NAN]=8;echo $NAN;",
        "78",
        &["string", "string"],
    );
}

#[test]
fn string_offset_conversion_warns_before_truncating() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$s='abc';$s[1]=NAN;echo $s;").unwrap();
    assert_eq!(rt.output, b"aNc");
    assert_eq!(
        rt.warnings,
        [
            "unexpected NAN value was coerced to string on line 1",
            "Only the first byte will be assigned to the string offset on line 1",
        ]
    );
}

#[test]
fn nan_comparisons_remain_silent_and_unordered_against_strings() {
    check(
        "$x=NAN;var_dump($x=='NAN',$x<'text','text'>$x,$x<=>'NAN','NAN'<=>$x,$x==true,$x==false,$x==null,[NAN]==[true]);",
        "bool(false)\nbool(false)\nbool(false)\nint(1)\nint(1)\nbool(true)\nbool(false)\nbool(false)\nbool(true)\n",
        &[],
    );
}

#[test]
fn retaining_nan_as_float_or_mixed_does_not_warn() {
    check(
        "function f(float|bool $x):mixed{return $x;}var_dump(f(NAN),NAN+1,NAN*1,is_float(NAN));",
        "float(NAN)\nfloat(NAN)\nfloat(NAN)\nbool(true)\n",
        &[],
    );
}

#[test]
fn strict_rejection_does_not_emit_a_conversion_warning() {
    for call in ["strlen(NAN)", "array_keys([1],1,NAN)", "f(NAN)"] {
        let mut rt = Runtime::default();
        let error = rt
            .execute_code(
                format!("declare(strict_types=1);function f(bool $x){{}}{call};").as_bytes(),
            )
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::TypeError);
        assert!(rt.warnings.is_empty());
    }
}

#[test]
fn implicit_integer_conversions_keep_both_range_and_precision_diagnostics() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$a=[NAN=>1];echo NAN%1;").unwrap();
    assert_eq!(rt.output, b"0");
    assert_eq!(rt.warnings.len(), 4);
    assert!(rt.warnings[0].contains("not representable as an int"));
    assert!(rt.warnings[1].contains("loses precision"));
    assert!(rt.warnings[2].contains("not representable as an int"));
    assert!(rt.warnings[3].contains("loses precision"));
}

#[test]
fn constant_logical_coercions_happen_at_compile_time() {
    check(
        "if(false){NAN||false;}echo 'done';",
        "done",
        &["bool", "bool"],
    );
    check("if(false){NAN&&false;}echo 'done';", "done", &["bool"]);
    check("if(false){NAN xor false;}echo 'done';", "done", &["bool"]);
    check(
        "if(false){!NAN;(bool)NAN;echo NAN;NAN?1:2;}echo 'done';",
        "done",
        &[],
    );
    check(
        "true||(NAN||false);false&&(NAN xor false);echo 'done';",
        "done",
        &[],
    );
    check("if(false){!0.0||(NAN||false);}", "", &["bool", "bool"]);
    check("if(false){!1.0&&(NAN||false);}", "", &["bool", "bool"]);
}

#[test]
fn constant_initializers_and_parameter_defaults_have_distinct_warning_timing() {
    check(
        "goto end;const X=NAN||false;end:echo 'done';",
        "done",
        &["bool"],
    );
    check("function f(){static $x=NAN||false;}f();f();", "", &["bool"]);
    check("function f(){static $x=NAN?1:2;}", "", &["bool"]);
    check("function f($x=NAN||false){}echo 'done';", "done", &[]);
    check("function f($x=NAN||false){}f(true);", "", &[]);
}

#[test]
fn arrays_and_selection_conditions_use_constant_initializer_rules() {
    check("if(false){[NAN||false];[NAN?1:2];}", "", &["bool", "bool"]);
    check(
        "if(false){switch(1){case NAN||false:break;}}",
        "",
        &["bool"],
    );
    check(
        "if(false){switch(1){case false:break;case NAN||false:break;}}",
        "",
        &["bool", "bool"],
    );
    check(
        "if(false){match(1){NAN||false=>1,default=>2};}",
        "",
        &["bool"],
    );
}
