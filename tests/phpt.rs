use php_rs::phpt::{Case, Outcome};

#[test]
fn output_mismatch_fails() {
    let case =
        Case::parse(b"--TEST--\nMismatch\n--FILE--\n<?php echo 'actual';\n--EXPECT--\nexpected\n")
            .unwrap();
    assert!(matches!(case.run(), Outcome::Failed(_)));
}

#[test]
fn diagnostics_cannot_silently_pass() {
    for code in ["echo $undefined;", "echo 1 / 0;", "echo ;"] {
        let case = Case::parse(
            format!("--TEST--\nError\n--FILE--\n<?php {code}\n--EXPECT--\n").as_bytes(),
        )
        .unwrap();
        assert!(matches!(case.run(), Outcome::Failed(_)), "{code}");
    }
}

#[test]
fn unsupported_sections_are_not_counted_as_passes() {
    for section in [
        "SKIPIF",
        "INI",
        "ENV",
        "EXPECTF",
        "EXPECTREGEX",
        "CLEAN",
        "EXTENSIONS",
    ] {
        let case = Case::parse(format!("--TEST--\nUnsupported\n--FILE--\n<?php echo 'ok';\n--{section}--\ncontent\n--EXPECT--\nok\n").as_bytes());
        // Multiple EXPECT variants are malformed; other sections remain explicit unsupported outcomes.
        match case {
            Ok(case) => assert!(matches!(case.run(), Outcome::Unsupported(_))),
            Err(_) => assert!(section.starts_with("EXPECT")),
        }
    }
}

#[test]
fn malformed_test_is_rejected() {
    for data in [
        "",
        "--TEST--\nNo file",
        "--FILE--\n<?php\n--EXPECT--\n",
        "--TEST--\nX\n--FILE--\n<?php\n--FILE--\n<?php\n--EXPECT--\n",
    ] {
        assert!(Case::parse(data.as_bytes()).is_err(), "{data}");
    }
}

#[test]
fn binary_output_and_line_ending_normalization() {
    let case = Case::parse(
        b"--TEST--\r\nBinary\r\n--FILE--\r\n<?php echo \"\\xff\\r\\n\";\r\n--EXPECT--\r\n\xff\r\n",
    )
    .unwrap();
    assert_eq!(case.run(), Outcome::Passed);
}
