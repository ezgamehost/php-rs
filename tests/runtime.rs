use php_rs::run;

fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn inline_html_and_short_echo_tags() {
    check(
        "before<?php echo ' middle'; ?>after<?= 21 * 2 ?>!",
        "before middleafter42!",
    );
}

#[test]
fn arithmetic_precedence_and_power_associativity() {
    check(
        "<?php echo 2 + 3 * 4, ',', (2 + 3) * 4, ',', 2 ** 3 ** 2, ',', -2 ** 2;",
        "14,20,512,-4",
    );
}

#[test]
fn assignment_and_compound_assignment() {
    check(
        "<?php $a = $b = 3; $a += 4; $b *= 2; $s = 'x'; $s .= $a; echo $a, ',', $b, ',', $s;",
        "7,6,x7",
    );
}

#[test]
fn php_scalar_conversions() {
    check(
        "<?php echo true, false, null, 1.5, '|', '12' + 3, '|', true + null;",
        "11.5|15|1",
    );
}

#[test]
fn boolean_truthiness_including_string_zero() {
    check(
        "<?php echo !'0', !'', !null, !false, !0, !0.0, '|', !!'false', !!'00', !!-1;",
        "111111|111",
    );
}

#[test]
fn string_escapes_and_binary_data() {
    check(
        r#"<?php echo 'a\nb', "\n\t\x41\101\0", '\\', '\'';"#,
        "a\\nb\n\tAA\0\\'",
    );
    assert_eq!(run(b"<?php echo '\xff';").unwrap(), b"\xff");
}

#[test]
fn comments_and_close_tag_terminate_statements() {
    check(
        "<?php /* first */ echo 1; // comment\necho 2 # ends at tag ?>\ntext",
        "12text",
    );
}

#[test]
fn concatenation_has_lower_precedence_than_arithmetic() {
    check("<?php echo 'sum=' . 2 + 3 . ';';", "sum=5;");
}

#[test]
fn short_circuit_preserves_side_effects() {
    check(
        "<?php $a = 0; false && ($a = 1); true || ($a = 2); $b = true and false; echo $a, $b;",
        "01",
    );
}

#[test]
fn null_coalescing_and_conditional_are_lazy() {
    check(
        "<?php echo $missing ?? 'fallback', ',', 0 ?? 5, ',', true ? 3 : 1 / 0, ',', 'ok' ?: 1 / 0;",
        "fallback,0,3,ok",
    );
}

#[test]
fn scalar_comparison_php8_rules() {
    check(
        "<?php echo 0 == 'foo' ? 'bad' : 'ok', '|', 12 == '12', '|', 12 === '12' ? 'bad' : 'ok', '|', '10' < '2' ? 'bad' : 'ok', '|', 1 <=> 2;",
        "ok|1|ok|ok|-1",
    );
}

#[test]
fn integer_overflow_promotes_to_float() {
    check(
        "<?php echo (9223372036854775807 + 1) > 9223372036854775807 ? 'yes' : 'no', ',', 9 / 3, ',', 9 / 2, ',', -9 % 4;",
        "no,3,4.5,-1",
    );
}

#[test]
fn numeric_literal_bases_and_separators() {
    check(
        "<?php echo 0xff, ',', 0b101, ',', 077, ',', 0o17, ',', 1_000, ',', 1.25e2, ',', .5;",
        "255,5,63,15,1000,125,0.5",
    );
}

#[test]
fn conditional_blocks_and_else_if() {
    check(
        "<?php $a = 2; if ($a == 1) echo 'a'; elseif ($a == 2) { echo 'b'; } else echo 'c'; if (false) echo 'd'; else if (true) echo 'e';",
        "be",
    );
}

#[test]
fn while_break_continue_and_increment() {
    check(
        "<?php $i = 0; $s = 0; while ($i < 10) { $i++; if ($i == 2) continue; if ($i == 5) break; $s += $i; } echo $s, ',', $i, ',', ++$i, ',', $i--;",
        "8,5,6,6",
    );
}

#[test]
fn return_stops_top_level_execution() {
    check("<?php echo 'before'; return 42; echo 'after';", "before");
}

#[test]
fn keywords_are_case_insensitive_variables_are_not() {
    check("<?php $a = 1; $A = 2; ECHO $a, $A, TRUE;", "121");
}

#[test]
fn invalid_programs_produce_errors_instead_of_panics() {
    for source in [
        "<?php echo ;",
        "<?php echo 'unterminated;",
        "<?php /* unterminated",
        "<?php (1 + 2) = 4;",
        "<?php echo 1 / 0;",
        "<?php echo 'foo' + 1;",
        "<?php echo 0b102;",
        "<?php echo 1__2;",
        "<?php break;",
        "<?php echo unknown;",
    ] {
        assert!(
            run(source.as_bytes()).is_err(),
            "unexpected success: {source}"
        );
    }
}
