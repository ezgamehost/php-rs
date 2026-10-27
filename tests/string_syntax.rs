use php_rs::{ErrorKind, Runtime, run};
fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn interpolates_variables_with_boundaries_and_escaped_dollars() {
    check(
        r#"<?php $a='cat'; $ab='dog'; echo "$a/$ab/{$a}s/\$a/$$a", '|', '$a';"#,
        "cat/dog/cats/$a/$cat|$a",
    );
}
#[test]
fn simple_and_braced_array_interpolation_follow_php_key_rules() {
    check(
        r#"<?php $a=['x'=>'yes','01'=>'zero',1=>['y'=>'nested']]; $k='x'; echo "$a[x]/$a[$k]/$a[01]/{$a[1]['y']}";"#,
        "yes/yes/zero/nested",
    );
}
#[test]
fn braced_expressions_allow_variable_calls_and_index_arithmetic() {
    check(
        r#"<?php function f(){return 'fn';} $f='f'; $a=[3=>'idx']; echo "{$f()}:{$a[1+2]}";"#,
        "fn:idx",
    );
}
#[test]
fn interpolation_uses_byte_strings_and_php_string_conversion() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php $a='\xff'; $b=[1]; echo \"$a:$b:$missing\";")
        .unwrap();
    assert_eq!(runtime.output, b"\xff:Array:");
    assert_eq!(runtime.warnings.len(), 2);
}
#[test]
fn unicode_escapes_encode_php_utf8_including_surrogates() {
    check(
        r#"<?php echo "\u{41}\u{FF}\u{2603}\u{0001f602}", '|', '\u{41}';"#,
        "Aÿ☃😂|\\u{41}",
    );
    assert_eq!(
        run(br#"<?php echo "\u{d800}\u{10ffff}";"#).unwrap(),
        b"\xed\xa0\x80\xf4\x8f\xbf\xbf"
    );
}
#[test]
fn malformed_unicode_escapes_are_parse_errors_before_execution() {
    for literal in [
        r"\u{}",
        r"\u{xyz}",
        r"\u{110000}",
        r"\u{fffffffffffffffff}",
        r"\u{41",
    ] {
        let mut runtime = Runtime::default();
        let source = format!("<?php echo 'before'; echo \"{literal}\";");
        assert_eq!(
            runtime.execute(source.as_bytes()).unwrap_err().kind,
            ErrorKind::ParseError
        );
        assert!(runtime.output.is_empty());
    }
}
#[test]
fn heredoc_interpolates_and_strips_closing_indentation() {
    check(
        "<?php $a='world'; echo <<<TXT\n    Hello $a\n      next\n    TXT;",
        "Hello world\n  next",
    );
}
#[test]
fn nowdoc_preserves_dollars_escapes_and_quotes() {
    check(
        "<?php $a='wrong'; echo <<<'TXT'\n  $a \\n ' \"\n  TXT;",
        "$a \\n ' \"",
    );
}
#[test]
fn heredoc_terminators_can_continue_expressions_and_not_match_prefixes() {
    check(
        "<?php $a=[<<<END\nENDING\nEND, <<<\"X\"\n\\\"\nX]; echo $a[0], '|', $a[1];",
        "ENDING|\\\"",
    );
}
#[test]
fn empty_heredocs_and_indented_blank_lines_are_valid() {
    check(
        "<?php echo <<<E\nE, '|', <<<E\n\n  E, '|', <<<E\n  a\n\n  b\n  E;",
        "||a\n\nb",
    );
}
#[test]
fn malformed_interpolation_and_heredoc_indentation_fail() {
    for source in [
        r#"<?php echo "{$a+1}";"#,
        r#"<?php echo "{$a['x']";"#,
        "<?php echo <<<E\n x\n  E;",
        "<?php echo <<<E\n\tx\n \tE;",
        "<?php echo <<<E\nno end",
    ] {
        assert_eq!(
            run(source.as_bytes()).unwrap_err().kind,
            ErrorKind::ParseError,
            "{source}"
        );
    }
}
#[test]
fn binary_string_prefixes_are_case_insensitive() {
    check(r#"<?php $x='ok'; echo b"$x", B'\n';"#, "ok\\n");
}
