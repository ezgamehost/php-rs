# php-rs

A PHP 8.5 runtime written in Rust. This project is at the interpreter-foundation
stage; it is **not yet a complete PHP implementation**.

```sh
cargo run -- tests/fixtures/hello.php
cargo run -- -r 'echo 6 * 7;'
cargo run -- -l tests/fixtures/hello.php
cargo test
cargo clippy --all-targets -- -D warnings
```

With no arguments, the executable reads a PHP document from stdin. File and stdin
mode recognize `<?php` and `<?=` tags and preserve inline HTML. `-r` accepts code
without an opening tag. Fatal errors return exit status 255; warnings go to stderr.

The library exposes `run`, `lint`, and a request-local `Runtime`. `Runtime` retains
output and warnings when execution fails. Parsing happens before execution.

## Current coverage

- Null, booleans, 64-bit integers, floats, and binary-safe strings.
- Variables, assignment, arithmetic, concatenation, scalar comparisons,
  boolean operations, null coalescing, ternary expressions, and numeric increments.
- PHP operator precedence, short-circuit evaluation, and integer overflow promotion.
- `echo`, blocks, `if` / `elseif` / `else`, `while`, `break`, `continue`, and `return`.
- Decimal, binary, octal, hexadecimal, and floating-point literals with separators;
  quoted string escapes and PHP comments.

Known gaps include arrays, functions and builtins, objects, references, exceptions,
generators, namespaces, attributes, extensions, and most PHP 8.5 additions. String
interpolation, leading-numeric-string warnings, some scalar edge cases, complete
syntax validation, resource limits, and exact PHP diagnostic formatting also
remain to be implemented. Passing the repository tests does not establish PHP
conformance. See [ROADMAP.md](ROADMAP.md) for the full target and validation plan.

## Disk usage

Development and test profiles disable incremental compilation and reduce debug
information. Build output is ignored by Git. Check `du -sh target` and `df -h .`
between slices; use `cargo clean` after completed slices or whenever build output
grows beyond 2 GiB or available space drops below 10 GiB. Do not delete unrelated
projects' caches. The runtime currently has no third-party Rust dependencies.

## Compatibility references

- [PHP 8.5 release](https://www.php.net/releases/8.5/en.php)
- [Operator precedence](https://www.php.net/manual/en/language.operators.precedence.php)
- [Boolean conversion](https://www.php.net/manual/en/language.types.boolean.php)
- [Upstream PHP tests](https://github.com/php/php-src/tree/PHP-8.5/tests)

The implementation uses a byte-oriented lexer, a precedence parser, and an AST
interpreter. PHP behavior takes priority over Rust's native type conversions.
