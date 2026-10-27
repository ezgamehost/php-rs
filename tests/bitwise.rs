use php_rs::{ErrorKind, Runtime, run};

#[test]
fn integer_bitwise_operators_and_php_precedence() {
    assert_eq!(run(b"<?php echo 8 | 4 & 8, ',', 7 ^ 3 & 1, ',', ~2 ** 3, ',', 4 & 4 == 4, ',', 1 << 2 + 1, ',', 'x' . 1 << 3;").unwrap(), b"8,6,-9,0,8,x8");
}

#[test]
fn string_bitwise_operators_preserve_raw_bytes_and_length_rules() {
    assert_eq!(
        run(b"<?php echo 'AB' & 'a', '|', 'AB' | 'a', '|', 'AB' ^ 'a', '|', ~'AB';").unwrap(),
        b"A|aB| |\xbe\xbd"
    );
}

#[test]
fn shifts_use_signed_64_bit_semantics_without_masking_count() {
    assert_eq!(
        run(b"<?php echo -8 >> 2, ',', 1 << 63, ',', 1 << 64, ',', -1 >> 64, ',', 1 >> 64;")
            .unwrap(),
        b"-2,-9223372036854775808,0,-1,0"
    );
}

#[test]
fn bitwise_compound_assignments_update_variables() {
    assert_eq!(
        run(b"<?php $x=5; $x |= 2; $x &= 6; $x ^= 3; $x <<= 2; $x >>= 1; echo $x;").unwrap(),
        b"10"
    );
}

#[test]
fn integer_conversion_warns_and_wraps_out_of_range_floats() {
    let mut runtime = Runtime::default();
    runtime
        .execute(b"<?php echo 1.5 | 2, ',', 1e20 | 0, ',', 1e20 % 3, ',', '2' & 3;")
        .unwrap();
    assert_eq!(runtime.output, b"3,7766279631452241920,2,2");
    assert_eq!(runtime.warnings.len(), 3);
}

#[test]
fn invalid_bitwise_operands_and_negative_shifts_are_php_errors() {
    for source in [
        b"<?php echo ~true;".as_slice(),
        b"<?php echo ~null;",
        b"<?php echo 1 & 'bad';",
    ] {
        assert_eq!(run(source).unwrap_err().kind, ErrorKind::TypeError);
    }
    assert_eq!(
        format!("{:?}", run(b"<?php echo 1 << -1;").unwrap_err().kind),
        "ArithmeticError"
    );
}
