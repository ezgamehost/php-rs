# php-rs

A PHP 8.5 runtime written in Rust. This project is at the interpreter-foundation
stage; it is **not yet a complete PHP implementation**.

```sh
cargo run -- tests/fixtures/hello.php
cargo run -- -r 'echo 6 * 7;'
cargo run -- -l tests/fixtures/hello.php
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --bin php-rs-phpt -- tests/phpt
cargo run --bin php-rs-phpt -- --reference /path/to/php-8.5 tests/phpt
```

With no arguments, the executable reads a PHP document from stdin. File and stdin
mode recognize `<?php` and `<?=` tags and preserve inline HTML. `-r` accepts code
without an opening tag. Fatal errors return exit status 255; warnings go to stderr.

The library exposes `run`, `lint`, and a request-local `Runtime`. `Runtime` retains
output and warnings when execution fails. Parsing happens before execution.

## Current coverage

- Null, booleans, 64-bit integers, floats, and binary-safe strings.
- Ordered arrays, PHP key coercion, copy-on-write assignment, nested indexed
  writes, append, array union and comparison, `unset`, `isset`, and `empty`.
- Variables, assignment, arithmetic, concatenation, scalar comparisons,
  boolean and bitwise operations, signed shifts, null coalescing, ternary expressions, and numeric increments.
- PHP operator precedence, short-circuit evaluation, and integer overflow promotion.
- Untyped named functions, recursion, local scopes, constant default arguments,
  named arguments, variadics, argument unpacking, and string-based function calls.
- Core builtins: `var_dump`, `strlen`, `count`/`sizeof`, `gettype`, `get_debug_type`,
  scalar/array type predicates, `array_keys`, `array_values`, `array_key_exists`,
  and `function_exists`.
- `echo`, blocks, `if` / `elseif` / `else`, `while`, `do … while`, `for`, `foreach`
  by value, numbered `break`/`continue`, and `return`. Loops support alternative
  `: … endfor` / `endwhile` / `endforeach` syntax.
- Decimal, binary, octal, hexadecimal, and floating-point literals with separators;
  quoted string escapes and PHP comments.
- Variable/array interpolation, braced variable calls, Unicode escapes, binary
  string prefixes, and indented heredoc/nowdoc strings.
- Byte string offset reads and writes, negative indexes, space padding, and
  string offset checks with `isset`, `empty`, and null coalescing.

Known gaps include typed functions, closures, most builtins, objects, references, exceptions,
generators, namespaces, attributes, extensions, and most PHP 8.5 additions.
Deprecated `${...}` and object interpolation, array unpacking and
destructuring, some scalar edge cases, complete
syntax validation, resource limits, and exact PHP diagnostic formatting also
remain to be implemented. Passing the repository tests does not establish PHP
conformance. See [ROADMAP.md](ROADMAP.md) for the full target and validation plan.

The PHPT runner currently supports `TEST`, `FILE`, and exact `EXPECT` output,
plus descriptive `DESCRIPTION` and `CREDITS` sections. Other sections are reported
as unsupported and make the run exit unsuccessfully. Unexpected diagnostics also
fail a test. `--reference` checks the PHP major/minor version and validates passing
fixtures against both interpreters. It does not compare exact diagnostic text.
See [docs/conformance.md](docs/conformance.md) for measured upstream results.

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
