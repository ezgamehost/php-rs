use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Suite(PathBuf);
impl Suite {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = std::env::temp_dir().join(format!(
            "php-rs-phpt-files-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root.canonicalize().unwrap())
    }
    fn write(&self, name: &str, data: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, data).unwrap();
        path
    }
    fn case(&self, name: &str, source: &str, expected: &str) -> PathBuf {
        self.write(
            name,
            &format!("--TEST--\nFile context\n--FILE--\n{source}\n--EXPECT--\n{expected}\n"),
        )
    }
    fn run(&self, path: &Path, cwd: bool) -> Output {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_php-rs-phpt"));
        if cwd {
            cmd.arg("--cwd").arg(&self.0);
        }
        cmd.arg(path).output().unwrap()
    }
}
impl Drop for Suite {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fixture_executes_as_a_real_file_with_sibling_includes() {
    let suite = Suite::new();
    suite.write("nested/child.inc", "<?php return 42;");
    let source = "<?php echo include __DIR__.'/child.inc'; echo ':', __FILE__ === get_included_files()[0]; var_dump(include_once __FILE__);";
    let path = suite.case("nested/context.phpt", source, "42:1bool(true)");
    let result = suite.run(&path, false);
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!path.with_extension("php").exists());
}

#[test]
fn explicit_working_directory_is_the_suite_root_not_the_fixture_directory() {
    let suite = Suite::new();
    suite.write("support.inc", "<?php return 'root';");
    suite.write("nested/support.inc", "<?php return 'fixture';");
    let path = suite.case(
        "nested/root.phpt",
        "<?php echo include './support.inc'; echo ':',include __DIR__.'/support.inc';",
        "root:fixture",
    );
    let result = suite.run(&path, true);
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn existing_php_files_are_not_overwritten_or_removed() {
    let suite = Suite::new();
    let path = suite.case("collision.phpt", "<?php echo 'test';", "test");
    suite.write("collision.php", "original contents");
    let result = suite.run(&path, false);
    assert!(!result.status.success());
    assert_eq!(
        fs::read(path.with_extension("php")).unwrap(),
        b"original contents"
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("already exists"));
}

#[cfg(unix)]
#[test]
fn existing_symlinks_are_not_followed_or_removed() {
    let suite = Suite::new();
    let target = suite.write("original.inc", "keep");
    let path = suite.case("link.phpt", "<?php echo 'test';", "test");
    std::os::unix::fs::symlink(&target, path.with_extension("php")).unwrap();
    let result = suite.run(&path, false);
    assert!(!result.status.success());
    assert_eq!(fs::read(&target).unwrap(), b"keep");
    assert!(path.with_extension("php").is_symlink());
}

#[test]
fn failing_and_malformed_fixtures_leave_no_generated_php_file() {
    let suite = Suite::new();
    for (name, code) in [
        ("parse", "<?php echo ;"),
        ("runtime", "<?php echo 1/0;"),
        ("mismatch", "<?php echo 2;"),
    ] {
        let path = suite.case(&format!("{name}.phpt"), code, "1");
        assert!(!suite.run(&path, false).status.success());
        assert!(!path.with_extension("php").exists());
    }
    let path = suite.write("malformed.phpt", "not a test");
    assert!(!suite.run(&path, false).status.success());
    assert!(!path.with_extension("php").exists());
}

#[test]
fn unsupported_fixtures_do_not_materialize_code() {
    let suite = Suite::new();
    let path = suite.write(
        "unsupported.phpt",
        "--TEST--\nUnsupported\n--FILE--\n<?php echo 1;\n--EXPECTF--\n%d\n",
    );
    let result = suite.run(&path, false);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("1 unsupported"));
    assert!(!path.with_extension("php").exists());
}

#[test]
fn invalid_working_directory_fails_without_running_tests() {
    let suite = Suite::new();
    let path = suite.case("test.phpt", "<?php echo 'ran';", "ran");
    let result = Command::new(env!("CARGO_BIN_EXE_php-rs-phpt"))
        .arg("--cwd")
        .arg(suite.0.join("missing"))
        .arg(&path)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!path.with_extension("php").exists());
}

#[cfg(unix)]
#[test]
fn reference_checks_intentional_exit_status_and_still_rejects_diagnostics() {
    use std::os::unix::fs::PermissionsExt;
    let suite = Suite::new();
    let path = suite.case("exit.phpt", "<?php echo 'ok';exit(7);echo 'bad';", "ok");
    for (status, diagnostic, passed) in [
        (7, "", true),
        (0, "", false),
        (255, "", false),
        (7, "echo diagnostic >&2", false),
    ] {
        let reference=suite.write("reference.sh", &format!("#!/bin/sh\nif [ \"$2\" = \"-r\" ]; then printf 8.5; exit 0; fi\nprintf ok\n{diagnostic}\nexit {status}\n"));
        fs::set_permissions(&reference, fs::Permissions::from_mode(0o755)).unwrap();
        let result = Command::new(env!("CARGO_BIN_EXE_php-rs-phpt"))
            .arg("--reference")
            .arg(&reference)
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            passed,
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(!path.with_extension("php").exists());
    }
}
