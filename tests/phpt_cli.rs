use std::process::Command;

fn run(path: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_php-rs-phpt"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .arg(path)
        .output()
        .unwrap()
}

#[test]
fn runs_fixture_directory() {
    let output = run("tests/phpt");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("30 passed, 0 failed, 0 unsupported"));
}

#[test]
fn unsupported_is_reported_and_exits_unsuccessfully() {
    let output = run("tests/fixtures/unsupported.phpt");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 passed, 0 failed, 1 unsupported"));
}

#[test]
fn mismatch_is_reported_and_exits_unsuccessfully() {
    let output = run("tests/fixtures/mismatch.phpt");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("0 passed, 1 failed, 0 unsupported"));
}

#[test]
fn missing_path_cannot_be_mistaken_for_success() {
    let output = run("tests/path-that-does-not-exist");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("path-that-does-not-exist"));
}
