use php_rs::{Runtime, lint, run};

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn print_converts_values_and_returns_integer_one() {
    check(
        "<?php $r=print 42; echo ':',$r===1?1:0;print true;print false;print null;",
        "42:11",
    );
}

#[test]
fn print_operand_includes_assignment_and_arithmetic() {
    check(
        "<?php $r=print $a=2;echo ':',$a,':',$r,'|';print(1+2)*3;",
        "2:2:1|9",
    );
}

#[test]
fn print_binds_above_word_logic_and_below_symbol_logic() {
    check(
        "<?php print 'a' and print 'b';print 'x' && print 'c';print 'z' && false;echo '!';",
        "abc1!",
    );
}

#[test]
fn nested_print_evaluates_from_inside_out() {
    check("<?php print print print 'x';", "x11");
}

#[test]
fn print_in_conditional_match_and_function_results_is_lazy() {
    check(
        "<?php false?print 'bad':print 'yes';match(1){1=>print 'match',default=>print 'bad'};function f(){return print 'fn';}echo f();",
        "yesmatchfn1",
    );
}

#[test]
fn print_preserves_binary_strings_and_array_warnings() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php print \"a\\0b\";print [1];")
        .unwrap();
    assert_eq!(runtime.output, b"a\0bArray");
    assert_eq!(runtime.warnings.len(), 1);
}

#[test]
fn print_requires_one_operand_and_is_not_a_constant_expression() {
    for source in [
        "print;",
        "print();",
        "print 'a','b';",
        "function f($a=print 'x'){}",
    ] {
        assert!(
            lint(format!("<?php {source}").as_bytes()).is_err(),
            "{source}"
        );
    }
}
