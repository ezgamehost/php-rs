use php_rs::{ErrorKind, Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn const_declarations_support_dependencies_arrays_and_casts() {
    check(
        "<?php const A=2,B=A+3,C=[A,B],D=(int)'4.5';echo A,B,C[1],D;",
        "2554",
    );
}

#[test]
fn declarations_are_not_hoisted_ahead_of_execution() {
    let mut runtime = Runtime::default();
    let error = runtime
        .execute(b"<?php echo 'before';echo LATER;const LATER=2;")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Error);
    assert_eq!(runtime.output, b"before");
}

#[test]
fn defined_constants_are_visible_in_functions_and_defaults() {
    check(
        "<?php const START=3;function f($x=START){define('END',$x+1);return END;}echo f(),END;",
        "44",
    );
}

#[test]
fn define_is_conditional_and_names_are_case_sensitive() {
    check(
        "<?php if(false)define('ABSENT',1);define('Value',2);echo defined('ABSENT')?1:0,defined('value')?1:0,defined('Value')?1:0,constant('Value');",
        "0012",
    );
}

#[test]
fn define_supports_named_arguments_and_arrays_copy_on_write() {
    check(
        "<?php define(value:[1],constant_name:'VALUES');$a=VALUES;$a[0]=2;echo VALUES[0],$a[0];",
        "12",
    );
}

#[test]
fn duplicate_definitions_warn_and_retain_the_original_value() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php const X=1;const X=2;echo define('X',3)?1:0,X;")
        .unwrap();
    assert_eq!(runtime.output, b"01");
    assert_eq!(runtime.warnings.len(), 2);
    assert!(
        runtime
            .warnings
            .iter()
            .all(|warning| warning.contains("already defined"))
    );
}

#[test]
fn case_insensitive_flag_warns_without_changing_name_matching() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php define('FOO',2,true);echo defined('foo')?1:0,FOO;")
        .unwrap();
    assert_eq!(runtime.output, b"02");
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn predefined_values_cover_platform_numbers_and_literal_constants() {
    check(
        "<?php echo PHP_INT_SIZE,':',PHP_INT_MAX,':',PHP_INT_MIN,':',PHP_MAJOR_VERSION,'.',PHP_MINOR_VERSION,'.',PHP_RELEASE_VERSION,PHP_EOL;echo PHP_FLOAT_DIG,':',PHP_FLOAT_EPSILON>0?1:0,':',NAN!==NAN?1:0,':',INF>PHP_FLOAT_MAX?1:0,':',defined('TRUE')?1:0,constant('null')===null?1:0;",
        "8:9223372036854775807:-9223372036854775808:8.5.0\n15:1:1:1:11",
    );
}

#[test]
fn deprecated_constant_warns_on_read_but_not_existence_check() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php echo defined('E_STRICT')?1:0,E_STRICT,constant('E_STRICT');")
        .unwrap();
    assert_eq!(runtime.output, b"120482048");
    assert_eq!(runtime.warnings.len(), 2);
}

#[test]
fn qualified_names_preserve_constant_case_and_fold_namespace_case() {
    check(
        r"<?php define('Ns\Value',7);echo \Ns\Value,constant('\ns\Value'),defined('NS\value')?1:0,\PHP_INT_SIZE;",
        "7708",
    );
}

#[test]
fn qualified_calls_are_allowed_but_qualified_function_declarations_are_not() {
    check(r"<?php echo \strlen('abc');", "3");
    assert!(lint(br"<?php function Ns\f(){}").is_err());
    assert!(lint(br"<?php function \f(){}").is_err());
}

#[test]
fn dynamic_names_can_be_empty_and_define_does_not_strip_leading_separator() {
    check(
        r"<?php define('',2);define('\Odd',3);echo constant(''),defined('\Odd')?1:0,constant('\\\\Odd');",
        "203",
    );
}

#[test]
fn undefined_dynamic_constant_and_class_constant_definition_fail() {
    assert_eq!(
        run(b"<?php constant('absent');").unwrap_err().kind,
        ErrorKind::Error
    );
    assert_eq!(
        run(b"<?php define('C::X',1);").unwrap_err().kind,
        ErrorKind::ValueError
    );
}

#[test]
fn constants_persist_across_executions_in_one_runtime_only() {
    let mut runtime = Runtime::default();
    runtime.execute(b"<?php const VALUE=5;").unwrap();
    runtime.execute(b"<?php echo VALUE;").unwrap();
    assert_eq!(runtime.output, b"5");
    assert!(run(b"<?php echo VALUE;").is_err());
}

#[test]
fn invalid_constant_declarations_are_rejected_before_execution() {
    for source in [
        "if(false){const A=1;}",
        "{const A=1;}",
        "function f(){const A=1;}",
        "const A=$x;",
        "const A=strlen('x');",
        "const TRUE=1;",
        "const null=1;",
        "const A=1,;",
        "const Ns\\A=1;",
    ] {
        assert!(
            lint(format!("<?php {source}").as_bytes()).is_err(),
            "{source}"
        );
    }
}
