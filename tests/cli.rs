use std::{
    io::Write,
    process::{Command, Stdio},
};

fn cli(args: &[&str], input: &[u8]) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_php-rs"))
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn executes_code_without_tags() {
    let result = cli(&["-r", "echo 6 * 7;"], b"");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"42");
    assert!(result.stderr.is_empty());
}

#[test]
fn reads_document_from_stdin() {
    let result = cli(&[], b"header<?php echo 'body'; ?>footer");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"headerbodyfooter");
}

#[test]
fn file_mode_executes_and_lint_does_not() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/hello.php");
    let result = cli(&[path], b"");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"Hello, PHP 8.5!\n");
    let result = cli(&["-l", path], b"");
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("No syntax errors"));
}

#[test]
fn runtime_error_preserves_output_and_returns_failure() {
    let result = cli(&["-r", "echo 'before'; echo 1 / 0;"], b"");
    assert_eq!(result.status.code(), Some(255));
    assert_eq!(result.stdout, b"before");
    assert!(String::from_utf8_lossy(&result.stderr).contains("DivisionByZeroError"));
}

#[test]
fn parse_error_prevents_all_execution() {
    let result = cli(&["-r", "echo 'before'; echo ;"], b"");
    assert_eq!(result.status.code(), Some(255));
    assert!(result.stdout.is_empty());
    assert!(String::from_utf8_lossy(&result.stderr).contains("ParseError"));
}

#[test]
fn undefined_variable_warning_is_separate_from_output() {
    let result = cli(&["-r", "echo $missing, 'after';"], b"");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"after");
    assert!(String::from_utf8_lossy(&result.stderr).contains("Undefined variable $missing"));
}

#[test]
fn rejects_unknown_options_and_missing_arguments() {
    for args in [
        vec!["--unknown"],
        vec!["-r"],
        vec!["-l"],
        vec!["/nonexistent/php-rs.php"],
    ] {
        let result = cli(&args, b"");
        assert!(!result.status.success(), "args: {args:?}");
        assert!(!result.stderr.is_empty());
    }
}
