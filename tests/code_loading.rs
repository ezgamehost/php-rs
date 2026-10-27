use php_rs::{ErrorKind, Runtime, Value, lint, run};
use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "php-rs-loading-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, source: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, source).unwrap();
        path
    }
    fn quoted(&self, name: &str) -> String {
        format!(
            "'{}'",
            self.0
                .join(name)
                .to_str()
                .unwrap()
                .replace('\\', "\\\\")
                .replace('\'', "\\'")
        )
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn check(source: &str, expected: &str) {
    assert_eq!(run(source.as_bytes()).unwrap(), expected.as_bytes());
}

#[test]
fn eval_shares_locals_but_declares_functions_globally() {
    check(
        "<?php $x=9; function outer(){ $x=2; eval('$x+=3; function inner(){return 7;}'); echo $x; } outer(); echo $x,inner();",
        "597",
    );
}

#[test]
fn eval_returns_values_and_accepts_mixed_document_segments() {
    check(
        "<?php var_dump(eval(''),eval('return;'),eval('return [1,2];')); echo eval('?>text<?php return 4; echo 9;');",
        "bool(false)\nNULL\narray(2) {\n  [0]=>\n  int(1)\n  [1]=>\n  int(2)\n}\ntext4",
    );
}

#[test]
fn eval_parses_entire_input_before_any_inner_effects() {
    let mut rt = Runtime::default();
    let error = rt
        .execute(b"<?php echo 'outer'; eval('echo \"inner\"; echo ;');")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::ParseError);
    assert_eq!(rt.output, b"outer");
    assert!(run(b"<?php eval('<?php echo 1;');").is_err());
}

#[test]
fn eval_is_a_language_construct_with_string_conversion() {
    check(
        "<?php declare(strict_types=1); var_dump(eval(null),eval(false),function_exists('eval'));",
        "bool(false)\nbool(false)\nbool(false)\n",
    );
    for code in [
        "eval(code:'echo 1;');",
        "eval('echo 1;', 'echo 2;');",
        "eval();",
    ] {
        assert!(lint(format!("<?php {code}").as_bytes()).is_err());
    }
    let mut rt = Runtime::default();
    assert_eq!(
        rt.execute(b"<?php eval([]);").unwrap_err().kind,
        ErrorKind::ParseError
    );
    assert_eq!(rt.warnings.len(), 1);
}

#[test]
fn eval_has_its_own_strictness_and_return_context() {
    check(
        "<?php declare(strict_types=1); function f(int $x){echo $x;} eval('f(\"2\");'); function v():void{echo eval('return 3;');} v();",
        "23",
    );
    let mut rt = Runtime::default();
    let error = rt
        .execute(b"<?php declare(strict_types=1); function f(int $x){} eval('f(\"2\");'); f('2');")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::TypeError);
    assert_eq!(
        run(b"<?php function f(int $x){} eval('declare(strict_types=1); f(\"2\");');")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    assert!(run(b"<?php while(true){eval('break;');}").is_err());
}

#[test]
fn eval_statics_belong_to_each_compilation() {
    check(
        "<?php function f(){static $x=1; eval('static $x=8; ++$x;'); echo $x;} f(); f(); eval('function g(){static $x=0;echo ++$x;}'); g();g();",
        "9912",
    );
}

#[test]
fn magic_constants_are_lexical_and_case_insensitive() {
    check(
        "<?php\necho __line__,':',__FUNCTION__,':',__CLASS__,':',__NAMESPACE__,':',__TRAIT__,';';\nfunction MixedName($name=__FUNCTION__){echo $name,':',__METHOD__,':',__LINE__; }\nMixedName();",
        "2::::;MixedName:MixedName:3",
    );
    check(
        "<?php function f(){echo eval('return __FUNCTION__;'),':',__FUNCTION__;} f();",
        ":f",
    );
}

#[test]
fn eval_source_names_include_parent_filename_and_call_line() {
    check(
        "<?php\necho eval('return __FILE__;');",
        "Standard input code(2) : eval()'d code",
    );
    check("<?php echo eval('return __LINE__;');", "1");
}

#[test]
fn includes_share_scope_and_return_one_or_explicit_values() {
    let files = Files::new();
    files.write(
        "vars.php",
        "before<?php $x+=2; $created=8; return [$x,$created]; ?>after",
    );
    files.write("empty.php", "<?php $x++;");
    let source = format!(
        "<?php $x=99; function f(){{$x=3;$r=include {}; echo $r[0],$created,':',include {},':',$x;}} f();echo ':',$x;",
        files.quoted("vars.php"),
        files.quoted("empty.php")
    );
    check(&source, "before58:1:6:99");
}

#[test]
fn once_variants_share_canonical_file_identity() {
    let files = Files::new();
    files.write("once.php", "<?php $calls++; return 7;");
    let source = format!(
        "<?php $calls=0;var_dump(include {},require_once {},include_once {},include {});echo $calls;",
        files.quoted("once.php"),
        files.quoted("./once.php"),
        files.quoted("once.php"),
        files.quoted("once.php")
    );
    check(&source, "int(7)\nbool(true)\nbool(true)\nint(7)\n2");
}

#[cfg(unix)]
#[test]
fn include_once_resolves_symlinks() {
    let files = Files::new();
    let path = files.write("once.php", "<?php echo 'once';");
    std::os::unix::fs::symlink(path, files.0.join("link.php")).unwrap();
    check(
        &format!(
            "<?php include_once {}; include_once {};",
            files.quoted("once.php"),
            files.quoted("link.php")
        ),
        "once",
    );
}

#[test]
fn once_marks_before_execution_to_stop_self_inclusion() {
    let files = Files::new();
    files.write(
        "self.php",
        "<?php echo 'start'; var_dump(require_once __FILE__); echo 'end';",
    );
    check(
        &format!("<?php include {};", files.quoted("self.php")),
        "startbool(true)\nend",
    );
}

#[test]
fn missing_include_warns_and_returns_false_while_require_fails() {
    let files = Files::new();
    let mut rt = Runtime::default();
    rt.execute(
        format!(
            "<?php var_dump(include {});echo 'after';",
            files.quoted("missing.php")
        )
        .as_bytes(),
    )
    .unwrap();
    assert_eq!(rt.output, b"bool(false)\nafter");
    assert_eq!(rt.warnings.len(), 2);
    let mut rt = Runtime::default();
    assert_eq!(
        rt.execute(
            format!(
                "<?php require {};echo 'after';",
                files.quoted("missing.php")
            )
            .as_bytes()
        )
        .unwrap_err()
        .kind,
        ErrorKind::Error
    );
    assert!(rt.output.is_empty());
    assert_eq!(rt.warnings.len(), 1);
    assert_eq!(
        run(b"<?php include ''; ").unwrap_err().kind,
        ErrorKind::ValueError
    );
}

#[test]
fn include_parse_errors_preserve_outer_output_and_skip_inner_output() {
    let files = Files::new();
    files.write("bad.php", "<?php echo 'inner';echo ;");
    let mut rt = Runtime::default();
    assert_eq!(
        rt.execute(format!("<?php echo 'outer';include {};", files.quoted("bad.php")).as_bytes())
            .unwrap_err()
            .kind,
        ErrorKind::ParseError
    );
    assert_eq!(rt.output, b"outer");
}

#[test]
fn relative_includes_use_the_defining_file_directory() {
    let files = Files::new();
    files.write("lib/child.php", "<?php return 42;");
    files.write(
        "lib/parent.php",
        "<?php function loaded(){return include 'child.php';}",
    );
    check(
        &format!(
            "<?php include {}; echo loaded();",
            files.quoted("lib/parent.php")
        ),
        "42",
    );
}

#[test]
fn include_precedence_keeps_filename_concatenation_inside_operand() {
    let files = Files::new();
    files.write("value.php", "<?php return 7;");
    check(
        &format!("<?php $x=include {}.'.php';echo $x;", files.quoted("value")),
        "7",
    );
}

#[test]
fn source_context_and_strictness_survive_returning_from_a_loaded_unit() {
    let files = Files::new();
    let path = files.write(
        "unit.php",
        "<?php function loaded(){return __FILE__;} function weak(int $x){echo $x;} weak('2');",
    );
    let mut rt = Runtime::default();
    let source = format!(
        "<?php declare(strict_types=1); include {}; echo loaded(); weak('2');",
        files.quoted("unit.php")
    );
    assert_eq!(
        rt.execute(source.as_bytes()).unwrap_err().kind,
        ErrorKind::TypeError
    );
    assert_eq!(
        rt.output,
        format!("2{}", path.canonicalize().unwrap().display()).as_bytes()
    );
    rt.output.clear();
    rt.execute(b"<?php echo __FILE__; weak('3');").unwrap();
    assert_eq!(rt.output, b"Standard input code3");
}

#[test]
fn included_files_are_unique_ordered_and_exclude_eval() {
    let files = Files::new();
    let first = files.write("first.php", "<?php return 1;");
    let second = files.write("second.php", "<?php return 2;");
    let mut rt = Runtime::default();
    rt.execute(format!("<?php include {};include {};include {};eval('echo 3;');$files=get_included_files();$same=get_required_files()===$files;",files.quoted("first.php"),files.quoted("second.php"),files.quoted("first.php")).as_bytes()).unwrap();
    let Value::Array(paths) = rt.variables.get(b"files".as_slice()).unwrap() else {
        panic!()
    };
    let names: Vec<_> = paths.entries().iter().map(|(_, v)| v.clone()).collect();
    assert_eq!(
        names,
        vec![
            Value::String(
                first
                    .canonicalize()
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec()
            ),
            Value::String(
                second
                    .canonicalize()
                    .unwrap()
                    .as_os_str()
                    .as_encoded_bytes()
                    .to_vec()
            )
        ]
    );
    assert_eq!(
        rt.variables.get(b"same".as_slice()),
        Some(&Value::Bool(true))
    );
}

#[test]
fn cli_file_mode_sets_canonical_source_and_marks_the_main_file() {
    let files = Files::new();
    let path=files.write("main.php","<?php echo __FILE__,'\n',__DIR__,'\n';var_dump(include_once __FILE__);echo count(get_included_files());");
    let output = Command::new(env!("CARGO_BIN_EXE_php-rs"))
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        format!(
            "{}\n{}\nbool(true)\n1",
            path.canonicalize().unwrap().display(),
            files.0.canonicalize().unwrap().display()
        )
        .as_bytes()
    );
}

#[test]
fn cli_code_mode_uses_command_line_source_identity() {
    let output = Command::new(env!("CARGO_BIN_EXE_php-rs"))
        .args(["-r", "echo __FILE__;"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"Command line code");
}

#[test]
fn failed_compilation_is_recorded_only_for_once_variants() {
    let files = Files::new();
    files.write("bad.php", "<?php echo ;");
    for (kind, expected) in [
        ("include", 0),
        ("require", 0),
        ("include_once", 1),
        ("require_once", 1),
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute(format!("<?php {kind} {};", files.quoted("bad.php")).as_bytes())
                .is_err()
        );
        rt.execute(b"<?php $count=count(get_included_files());")
            .unwrap();
        assert_eq!(
            rt.variables.get(b"count".as_slice()),
            Some(&Value::Int(expected)),
            "{kind}"
        );
    }
}

#[test]
fn nul_bytes_in_include_paths_fail_without_opening_the_prefix() {
    let files = Files::new();
    files.write("valid.php", "<?php echo 'wrong';");
    for (kind, count) in [("include", 1), ("require", 0)] {
        let mut rt = Runtime::default();
        let result = rt.execute(
            format!("<?php {kind} {}.\"\\0suffix\";", files.quoted("valid.php")).as_bytes(),
        );
        assert_eq!(result.is_err(), kind == "require");
        assert!(rt.output.is_empty());
        assert_eq!(rt.warnings.len(), count);
    }
}

#[test]
fn magic_constant_names_are_reserved_in_declarations() {
    for name in [
        "__LINE__",
        "__FILE__",
        "__DIR__",
        "__FUNCTION__",
        "__METHOD__",
        "__CLASS__",
        "__NAMESPACE__",
        "__TRAIT__",
    ] {
        assert!(lint(format!("<?php const {name}=1;").as_bytes()).is_err());
        assert!(lint(format!("<?php function {name}(){{}}").as_bytes()).is_err());
    }
}
