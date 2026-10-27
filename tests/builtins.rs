use php_rs::{ErrorKind, Runtime, run};
fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn var_dump_formats_all_supported_values_and_nested_arrays() {
    check(
        "<?php var_dump(null,true,42,1.5,'abc',['x'=>[1,null]]);",
        "NULL\nbool(true)\nint(42)\nfloat(1.5)\nstring(3) \"abc\"\narray(1) {\n  [\"x\"]=>\n  array(2) {\n    [0]=>\n    int(1)\n    [1]=>\n    NULL\n  }\n}\n",
    );
    assert_eq!(
        run(b"<?php var_dump('\xff');").unwrap(),
        b"string(1) \"\xff\"\n"
    );
}
#[test]
fn var_dump_uses_roundtrip_float_precision() {
    check(
        "<?php var_dump(1e14,1e17,1e-5,-0.0,1.2345678901234567);",
        "float(100000000000000)\nfloat(1.0E+17)\nfloat(1.0E-5)\nfloat(-0)\nfloat(1.2345678901234567)\n",
    );
}
#[test]
fn type_inspection_and_predicates_do_not_coerce_values() {
    check(
        "<?php foreach([null,false,1,1.0,'1',[]] as $v) echo gettype($v), '/', get_debug_type($v), ';'; echo is_int(1),is_integer(1),is_long(1),is_float(1.0),is_double(1.0),is_bool(false),is_null(null),is_string(''),is_array([]); echo is_int('1')? 'bad':'ok';",
        "NULL/null;boolean/bool;integer/int;double/float;string/string;array/array;111111111ok",
    );
}
#[test]
fn strlen_counts_bytes_and_supports_named_arguments() {
    check(
        r"<?php echo strlen(string:'é'), ':', strlen('a\0b'), ':', strlen(123);",
        "2:4:3",
    );
    assert_eq!(run(b"<?php echo strlen(\"a\0b\");").unwrap(), b"3");
}
#[test]
fn null_scalar_arguments_emit_deprecations() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php echo strlen(null), count([],null);")
        .unwrap();
    assert_eq!(runtime.output, b"00");
    assert_eq!(runtime.warnings.len(), 2);
    assert!(runtime.warnings.iter().all(|w| w.contains("Passing null")));
}
#[test]
fn count_and_sizeof_handle_nested_arrays_and_recursive_mode() {
    check(
        "<?php $a=[1,[2,[3]],[]]; echo count($a), ':', sizeof($a,COUNT_RECURSIVE), ':', count(mode:COUNT_NORMAL,value:$a);",
        "3:6:3",
    );
}
#[test]
fn array_keys_values_and_filters_preserve_php_order() {
    check(
        "<?php $a=['x'=>1,7=>'1','n'=>null]; foreach(array_keys($a) as $v)echo $v,','; echo '|'; foreach(array_values($a) as $v)echo $v,','; echo '|'; foreach(array_keys($a,1) as $v)echo $v,','; echo '|'; foreach(array_keys($a,1,true) as $v)echo $v,','; echo '|'; foreach(array_keys($a,null) as $v)echo $v,',';",
        "x,7,n,|1,1,,|x,7,|x,|n,",
    );
}
#[test]
fn array_key_exists_distinguishes_null_values_from_missing_keys() {
    check(
        "<?php $a=['x'=>null,8=>'yes']; echo array_key_exists('x',$a), isset($a['x'])?1:0, array_key_exists('8',$a), key_exists('missing',$a)?1:0;",
        "1010",
    );
}
#[test]
fn function_exists_observes_native_and_user_declaration_timing() {
    check(
        "<?php echo function_exists('STRLEN'),function_exists('late')?1:0,function_exists('early'); if(true){function late(){}} function early(){} echo function_exists('late'),function_exists('does_not_exist')?1:0;",
        "10110",
    );
}
#[test]
fn builtin_argument_counts_and_types_are_validated() {
    for source in [
        "strlen();",
        "strlen('a','b');",
        "count();",
        "var_dump();",
        "array_key_exists('x');",
        "var_dump(value:1,extra:2);",
    ] {
        assert_eq!(
            format!(
                "{:?}",
                run(format!("<?php {source}").as_bytes()).unwrap_err().kind
            ),
            "ArgumentCountError",
            "{source}"
        );
    }
    for source in [
        "strlen([]);",
        "count('abc');",
        "array_keys(1);",
        "array_values(null);",
        "count([], 'bad');",
    ] {
        assert_eq!(
            run(format!("<?php {source}").as_bytes()).unwrap_err().kind,
            ErrorKind::TypeError,
            "{source}"
        );
    }
    assert_eq!(
        format!("{:?}", run(b"<?php count([],2);").unwrap_err().kind),
        "ValueError"
    );
}
#[test]
fn native_names_cannot_be_redeclared() {
    let mut runtime = Runtime::default();
    assert!(
        runtime
            .execute(b"<?php echo 'wrong'; function STRLEN($x){return 4;}")
            .is_err()
    );
    assert!(runtime.output.is_empty());
}

#[test]
fn unknown_native_default_cannot_be_skipped_by_named_arguments() {
    assert_eq!(
        run(b"<?php array_keys(array:[null,1],strict:true);")
            .unwrap_err()
            .kind,
        ErrorKind::ArgumentCountError
    );
}
#[test]
fn native_integer_parameters_reject_out_of_range_values() {
    for value in ["1e20", "'1e20'"] {
        assert_eq!(
            run(format!("<?php count([],{value});").as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::TypeError
        );
    }
}
#[test]
fn fully_qualified_dynamic_function_names_are_supported() {
    check(
        r"<?php $f='\strlen'; echo function_exists('\strlen'), ':', $f('abc'); function custom(){return 7;} $f='\CUSTOM'; echo ':', $f();",
        "1:3:7",
    );
}
