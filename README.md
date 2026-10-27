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
Use `Runtime::execute_file` for file source identity and relative includes, or
`Runtime::execute_code` for code without an opening PHP tag.
`Runtime::set_working_directory` configures relative path resolution for one
request without changing the host process's working directory.
`Runtime::exit_status()` reports termination requested by `exit`/`die`; execution
returns successfully with `Value::Null`, preserving output and global state.
A terminated request does not execute further PHP; create a new `Runtime` for a
new request. The CLI returns the low eight bits of the requested status.

## Current coverage

- Null, booleans, 64-bit integers, floats, and binary-safe strings.
- PHP 8.5 NaN conversion warnings for boolean, string, and array conversions;
  NaN comparisons remain silent and unordered against numbers and strings.
- Explicit scalar and array casts, cast-alias deprecations, and PHP 8.5 `(void)`
  result discarding in statements and permitted `for` clauses.
- Global `const` declarations, `define`/`defined`/`constant`, qualified lookup,
  and core numeric, version, separator, and error-level constants.
- Ordered arrays, PHP key coercion, copy-on-write assignment, nested indexed
  writes, append, array union and comparison, `unset`, `isset`, and `empty`.
- Array unpacking with integer-key reindexing and string-key overwrites.
- `array_map`, `array_filter`, and `array_reduce`, including multiple-array zip
  behavior, null callbacks where allowed, filter key modes, initial reduction
  values, and PHP's weak scalar coercion for internally invoked callbacks.
- Value destructuring with `[]` and `list()`, including nested patterns, explicit
  keys, skipped entries, indexed targets, and `foreach`.
- Variables, assignment, arithmetic, concatenation, scalar comparisons,
  boolean and bitwise operations, signed shifts, null coalescing, ternary expressions, and numeric increments.
- PHP operator precedence, short-circuit evaluation, and integer overflow promotion.
- Named functions, recursion, local scopes, constant default arguments,
  named arguments, variadics, argument unpacking, and string-based function calls.
  `func_num_args`, `func_get_arg`, and `func_get_args` expose supplied arguments,
  with live parameter values, default gaps, and separate variadic argument copies.
- Public instance classes and `stdClass`, constructors, shared object handles,
  dynamic property names, nested property/array writes, and basic live property
  iteration. Instance methods bind `$this`, support named/typed arguments, and
  expose bound first-class callables. Non-static closures retain `$this`.
  Method-array callbacks and invokable objects work with pipes and array callbacks.
- Public class inheritance with inherited constructors/properties/methods,
  virtual dispatch, shared inherited method statics, and `final` classes/methods.
  Overrides validate arity, variadic parameters, parameter contravariance, and
  return covariance for implemented types. Class types accept descendants.
  `instanceof` supports named/dynamic class operands and skips dynamic operands
  for non-object values. Class linking preserves forward-declaration timing.
- Public static methods, parent implementation calls, scoped method callables,
  static-method string/array callbacks, and late static binding. `self`, `parent`,
  and `static` resolve in calls, construction, and contextual types; closures
  retain their declaring and called class contexts. `::class`, `get_called_class`,
  and `get_parent_class` expose class names. Static closure defaults use the
  declaring scope; property defaults are cached separately for each class.
- Public static properties with inherited shared storage, redeclaration, dynamic
  names, array mutation, and `isset`/`empty`/coalescing. Public class constants
  support inheritance, `final`, dynamic names, lazy dependency resolution,
  cycle errors, and lookup through `constant`/`defined`. `self`, `parent`, and
  `static` select the appropriate class for both kinds of member access.
- Object identity and property comparisons, recursive comparison errors,
  recursive-safe object dumps, object-to-array casts, and explicit-object
  `get_class`, `get_object_vars`, `class_exists`, `method_exists`, and
  `property_exists` for implemented classes.
- First-class function callables (`name(...)`, `$name(...)`) with identity,
  normal argument binding, callable/Closure/object types, and PHP 8.5 constant
  initializers. The pipe operator (`|>`) chains callable strings and these
  function objects, preserving left-to-right evaluation and caller strictness.
- Anonymous functions with `use` captures by value or reference, arrow functions
  with automatic value captures, nested closures, recursion, and per-instance
  static variables. Static anonymous functions can initialize PHP 8.5 constants
  and parameter defaults. Inline closures work in pipe chains.
- Live `global` bindings and function-local `static` storage, including dynamic
  initializers, recursive calls, and local `unset` behavior. Variable variables
  (`$$name` and `${expression}`) work with reads, writes, offsets, and probes.
- `$GLOBALS` element reads/writes, nested mutation, probes, and unsetting, with
  independent whole-array snapshots and compile-time mutation restrictions.
  Symbol-table ordering preserves compiled variable slots and dynamic names.
- `eval`, local-file `include`/`require` and their `_once` variants, preserving
  caller variables, per-unit strictness, return values, and canonical file identity.
  Loaded functions retain their defining source directory for later includes.
- Lexical `__LINE__`, `__FILE__`, `__DIR__`, `__FUNCTION__`, and `__METHOD__`,
  plus empty class/trait/namespace constants outside those declarations.
- Parameter and return types for available values, nullable/union types, scalar
  coercion, typed variadics, default validation, and `void`/`never` return rules.
  `declare(strict_types=1)` preserves caller and declaration scope across calls;
  strict calls also check scalar arguments to the supported internal functions.
- Core builtins: `var_dump`, `strlen`, `count`/`sizeof`, `gettype`, `get_debug_type`,
  scalar/array type predicates, `array_keys`, `array_values`, `array_key_exists`,
  `function_exists`, and `get_included_files`/`get_required_files`.
- `echo`, blocks, `if` / `elseif` / `else`, `while`, `do … while`, `for`, `foreach`
  by value, numbered `break`/`continue`, and `return`. Loops support alternative
  `: … endfor` / `endwhile` / `endforeach` syntax.
- `exit`/`die` with bare syntax, ordinary/dynamic calls, named and unpacked
  arguments, PHP 8.5 scalar coercion, strict types, and request-local unwinding.
- `goto` and case-sensitive labels, with jumps across ordinary blocks/branches,
  loop exits, and jumps within active loops/switches. Invalid label targets and
  jumps into loop/switch regions are rejected before execution.
- Alternative `if` / `elseif` / `else` syntax with `endif`, including templates.
- `switch` with loose case comparison, fall-through, numbered control flow,
  alternative syntax, and PHP 8.5 case-semicolon deprecation warnings.
- `match` expressions with strict comparisons, grouped conditions, lazy arms,
  default selection, and `UnhandledMatchError` for unmatched values.
- `print` expressions with PHP precedence, string conversion, and return values.
- Decimal, binary, octal, hexadecimal, and floating-point literals with separators;
  quoted string escapes and PHP comments.
- Variable/array interpolation, braced variable calls, Unicode escapes, binary
  string prefixes, and indented heredoc/nowdoc strings.
- Byte string offset reads and writes, negative indexes, space padding, and
  string offset checks with `isset`, `empty`, and null coalescing.

Known gaps include intersection/DNF types, most builtins, references, exceptions,
generators, namespaces, attributes, extensions, and most PHP 8.5 additions.
The object implementation is a foundation: interfaces, traits,
visibility modifiers other than public, typed/readonly properties, hooks,
typed class constants, complete constant/default-expression compatibility, cloning,
most magic methods, autoloading, and reflection remain unfinished. Closure methods,
complete closure debugging and magic-constant context, exact object handle reuse, and iteration edge cases
involving deletion/reinsertion of visited properties also remain unfinished.
Callable checks cover function names, Closure objects, instance-method arrays,
and invokable instances of implemented classes.
Closure reference captures work, but general references and by-reference
parameters/returns remain unfinished. Reference cycles are not yet collected.
Other `declare` directives are not implemented. Shutdown callbacks, output
handlers, and object destructor execution remain unimplemented.
Automatic initialization and scope handling for the other superglobals are not
implemented; `$GLOBALS` currently exposes the variables created by PHP code or
provided through `Runtime::variables`. File loading currently
uses the working directory and the executing source directory; configurable
`include_path`, stream wrappers, and PHP configuration are not implemented.
Deprecated `${...}` and complete object interpolation,
some scalar edge cases, complete
syntax validation, resource limits, and exact PHP diagnostic formatting also
remain to be implemented. Passing the repository tests does not establish PHP
conformance. See [ROADMAP.md](ROADMAP.md) for the full target and validation plan.

The PHPT runner currently supports `TEST`, `FILE`, and exact `EXPECT` output,
plus descriptive `DESCRIPTION` and `CREDITS` sections. Other sections are reported
as unsupported and make the run exit unsuccessfully. Unexpected diagnostics also
fail a test. `--reference` checks the PHP major/minor version and validates passing
fixtures against both interpreters, including process exit status. Intentional
nonzero exits can pass when output and reference status match. It does not
compare exact diagnostic text.
The runner writes each supported `FILE` section to a temporary `.php` sibling
of the `.phpt` fixture and runs both interpreters against that filename. It
refuses to overwrite an existing file and removes generated files after the run.
Use `--cwd /path/to/php-src` to select the suite root; the default is the current
directory. Test code is not sandboxed, and process timeouts remain unfinished.
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
- [PHP 8.5 incompatibilities](https://www.php.net/manual/en/migration85.incompatible.php)
- [Type declarations and strict typing](https://www.php.net/manual/en/language.types.declarations.php)
- [Function argument inspection](https://www.php.net/manual/en/function.func-get-args.php)
- [First-class callables](https://www.php.net/manual/en/functions.first_class_callable_syntax.php)
- [Anonymous functions](https://www.php.net/manual/en/functions.anonymous.php)
- [Arrow functions](https://www.php.net/manual/en/functions.arrow.php)
- [Pipe operator](https://www.php.net/manual/en/language.operators.functional.php)
- [Exit](https://www.php.net/manual/en/function.exit.php)
- [Goto](https://www.php.net/manual/en/control-structures.goto.php)
- [Variable scope](https://www.php.net/manual/en/language.variables.scope.php)
- [$GLOBALS](https://www.php.net/manual/en/reserved.variables.globals.php)
- [File inclusion](https://www.php.net/manual/en/function.include.php)
- [Eval](https://www.php.net/manual/en/function.eval.php)
- [Upstream PHP tests](https://github.com/php/php-src/tree/PHP-8.5/tests)

The implementation uses a byte-oriented lexer, a precedence parser, and an AST
interpreter. PHP behavior takes priority over Rust's native type conversions.
