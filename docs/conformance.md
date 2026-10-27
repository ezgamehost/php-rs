# Conformance baseline

Measured on 2026-10-26 with the PHP **8.5.0** release source and a locally built
CLI reference. Upstream fixtures run as real files with the source checkout as
their working directory. These are narrow measurements, not full PHP
compatibility claims.

| Suite | Passed | Failed | Unsupported |
| --- | ---: | ---: | ---: |
| Project `tests/phpt`, Rust and reference PHP | 41 | 0 | 0 |
| PHP 8.5.0 `tests/basic`, Rust | 6 | 5 | 99 |
| PHP 8.5.0 `tests/lang`, Rust | 60 | 107 | 127 |
| PHP 8.5.0 `Zend/tests/match`, Rust and reference PHP for passing cases | 13 | 9 | 13 |
| PHP 8.5.0 `Zend/tests/list`, Rust and reference PHP for passing cases | 12 | 16 | 30 |
| PHP 8.5.0 `Zend/tests/array_unpack`, Rust and reference PHP for passing cases | 2 | 5 | 6 |
| PHP 8.5.0 `Zend/tests/return_types`, Rust and reference PHP for passing cases | 7 | 11 | 71 |
| PHP 8.5.0 `Zend/tests/static_variables`, Rust and reference PHP for passing cases | 7 | 5 | 1 |
| PHP 8.5.0 `Zend/tests/restrict_globals`, Rust and reference PHP for passing cases | 2 | 1 | 13 |
| PHP 8.5.0 `Zend/tests/jump`, Rust and reference PHP for passing cases | 6 | 6 | 5 |
| PHP 8.5.0 `Zend/tests/exit`, Rust and reference PHP for passing cases | 1 | 4 | 22 |
| PHP 8.5.0 `Zend/tests/pipe_operator`, Rust and reference PHP for passing cases | 10 | 9 | 12 |
| PHP 8.5.0 `Zend/tests/first_class_callable`, Rust and reference PHP for passing cases | 3 | 18 | 44 |
| PHP 8.5.0 `Zend/tests/closures`, Rust and reference PHP for passing cases | 13 | 63 | 59 |
| PHP 8.5.0 `Zend/tests/arrow_functions`, Rust and reference PHP for passing cases | 2 | 2 | 5 |
| PHP 8.5.0 selected `ext/standard/tests/array` map/filter/reduce cases, Rust and reference PHP for passing cases | 12 | 21 | 6 |
| PHP 8.5.0 `tests/classes`, Rust and reference PHP for passing cases | 19 | 106 | 158 |
| PHP 8.5.0 `Zend/tests/inheritance`, Rust and reference PHP for passing cases | 3 | 31 | 36 |
| PHP 8.5.0 `Zend/tests/lsb`, Rust and reference PHP for passing cases | 6 | 8 | 11 |
| PHP 8.5.0 `Zend/tests/constants`, Rust and reference PHP for passing cases | 19 | 37 | 39 |
| PHP 8.5.0 `Zend/tests/type_declarations/typed_class_constants*.phpt`, Rust and reference PHP for passing cases | 3 | 12 | 14 |

Every language-suite result, including failing test names and reasons, is in
[the language baseline report](conformance/php-8.5.0-lang.txt). The language
suite is not green; interpolation, references, object declarations, missing
control structures, and runtime APIs account for many of the remaining failures.

The [match-suite report](conformance/php-8.5.0-match.txt) records a separate
upstream check. Remaining failures include missing exceptions, objects,
and standard-library functions; unsupported sections remain unexecuted.

The [list/destructuring report](conformance/php-8.5.0-list.txt) and
[array-unpacking report](conformance/php-8.5.0-array_unpack.txt) add targeted
coverage. Their remaining failures include references, objects/iterators, Closure methods,
and unsupported PHPT sections.

The [return-type report](conformance/php-8.5.0-return_types.txt) records the initial
typed-function baseline. Most cases require diagnostic matching, objects,
generators, reflection, references, or other unfinished language features.
Separately, 1,152 argument conversions (24 declarations × 24 values × weak/strict
mode) matched PHP 8.5.0 for output and success/failure status. This matrix did not
compare diagnostic text. Project regressions cover nullable/union types, typed
variadics and defaults, return validation, and strictness across compilation units.

The [static-variable report](conformance/php-8.5.0-static_variables.txt) covers
storage persistence, dynamic and recursive initializers, and global/static
rebinding. Remaining failures need objects, traits, and exceptions.
Real-file PHPT execution and suite-root path resolution add six passing
language-suite cases. All six were individually checked against PHP 8.5.0.

The [$GLOBALS report](conformance/php-8.5.0-restrict_globals.txt) records two
passing upstream cases. The remaining exact-output case needs by-reference
parameters and exceptions; diagnostic cases require unsupported `EXPECTF`.
Project regressions separately cover live access, snapshots, alias detachment,
key conversion, compiled symbol ordering, and mutation restrictions. Thirteen
normal-output snippets and the project PHPT fixture were checked against PHP 8.5.0.
Automatic initialization of the other superglobals remains unimplemented.

The [function argument inspection report](conformance/php-8.5.0-function_arguments.txt)
records seven passing cases, ten failing cases, and three unsupported cases.
All seven passing cases match PHP 8.5.0, including six newly passing language
fixtures and a named-argument fixture. Remaining cases require references,
exceptions, Closure methods, or diagnostic matching. Project regressions
also cover live parameter changes, variadic copies, recursion, context boundaries,
and validation order; fifteen normal-output examples match the reference.

The [jump-suite report](conformance/php-8.5.0-jump.txt) records six passing
upstream cases, all checked against PHP 8.5.0. The six remaining failures need
classes, non-strict `declare` directives, or `try`/`catch`/`finally`; five diagnostic
cases require unsupported `EXPECTF`. Project regressions cover invalid targets,
case-sensitive labels, branch entry, loop exits, and switch resumption. Eighteen
normal-output examples also match the reference.

The [exit-suite report](conformance/php-8.5.0-exit.txt) records one passing case,
four failures, and 22 unsupported cases. The remaining cases require process execution APIs,
objects, closure debugging, output handling, or additional PHPT sections. Separately,
108 exit cases (18 values × weak/strict typing × three call forms) match PHP 8.5.0
for output, process status, and diagnostic presence. This does not compare
exact diagnostic text or establish shutdown/destructor compatibility. Project
PHPT checks now compare intentional nonzero exit codes with the reference.

NaN compatibility was checked with 265 differential snippets covering conversion
warnings, ordinary and constant logical expressions, array and selection
contexts, casts of finite and non-finite floats, and comparisons. All matched
PHP 8.5.0 for output, process status, and diagnostics after removing source
location formatting and the CLI's extra warning prefix. Project regressions
also cover typed arguments/returns, string offsets, variable names, strict
rejection, and warning timing. This is targeted coverage; exact diagnostic
formatting and full constant/default-expression compatibility remain unfinished.

The [pipe report](conformance/php-8.5.0-pipe_operator.txt) and
[first-class callable report](conformance/php-8.5.0-first_class_callable.txt)
record thirteen passing upstream cases. First-class `exit`/`die` support also adds
one passing case to the exit suite. Remaining cases require
methods, references, namespaces, exceptions, reflection, missing
builtins, or unsupported PHPT sections. Separately, 273 invocation examples
matched PHP 8.5.0 for output, process status, and diagnostic presence, including
weak/strict typing, nine parameter types, object identity, constants/defaults,
pipe precedence, and argument binding. Exact diagnostic text and complete
object-handle reuse are not covered by that matrix.

The [anonymous-function report](conformance/php-8.5.0-closures.txt) and
[arrow-function report](conformance/php-8.5.0-arrow_functions.txt) track closure
coverage. Explicit value/reference captures, implicit arrow captures, nesting,
recursion, per-instance statics, and PHP 8.5 static-closure initializers are
implemented. A separate matrix of 169 snippets matched PHP 8.5.0 for output,
process status, and diagnostic presence. Remaining gaps include by-reference
parameters/returns, `$this` and method binding, reflection, generators, cycle
collection, and complete closure debugging output.

The [array callback report](conformance/php-8.5.0-array_callbacks.txt) selects
upstream `array_map*.phpt`, `array_filter*.phpt`, and `array_reduce*.phpt` cases.
Twelve pass against both interpreters. Remaining cases require references,
objects/methods, exceptions, error suppression, or unsupported PHPT sections.
A matrix of 328 snippets matched PHP 8.5.0 for output, process status, and
diagnostic presence. It covers key preservation, array zipping and null padding,
empty inputs, null callbacks, filter modes, initial reduction values, callback
coercion, input mutation, and termination. By-reference callback parameters
and exact diagnostic formatting remain unfinished.

The upstream basic-suite failures are:

- `bug54514.phpt`: environment/path APIs and executable constants are not implemented.
- `bug71273.phpt`: shell execution and regular-expression APIs are not implemented.
- `encoding.phpt`: declarations and encoding APIs are not implemented.
- `header_register_callback.phpt`: header callback APIs are not implemented.
- `header_register_callback_after_output.phpt`: header callback APIs are not implemented.

The unsupported cases require PHPT sections beyond exact-output testing, such as
`EXPECTF`, `SKIPIF`, `INI`, `ENV`, `GET`, `POST`, or extension configuration. They
have not been executed and are not considered passing. Fatal errors and warnings
currently fail exact-output cases rather than pretending diagnostic fidelity.

```sh
cargo run --bin php-rs-phpt -- tests/phpt
cargo run --bin php-rs-phpt -- --reference /path/to/php-8.5 tests/phpt
cargo run --bin php-rs-phpt -- --cwd /path/to/php-8.5.0 /path/to/php-8.5.0/tests/basic
```

The upstream basic suite currently returns status 1. The runner also returns
status 1 for malformed tests, empty test selections, missing paths, and unsupported
sections. The project regression suite is separate and runs with `cargo test`.
The runner materializes supported `FILE` sections as `.php` siblings of the
fixtures, refuses existing-file collisions, and cleans up its generated files.
`--cwd` selects a shared suite root for Rust and reference PHP. This follows the
file naming and working-directory model in upstream `run-tests.php`; it does
not yet reproduce the complete upstream environment or configuration.

Local reference build used:

```sh
curl -fLO https://www.php.net/distributions/php-8.5.0.tar.xz
tar -xJf php-8.5.0.tar.xz
cd php-8.5.0
./configure --disable-all --enable-cli --disable-cgi --disable-phpdbg --without-pear
make -j4
./sapi/cli/php -v
```

The current session's reference lives under `/tmp/php-rs-reference/php-8.5.0`;
it is not bundled with this repository. Test isolation and execution timeouts
remain unfinished, so large upstream scans should use an external timeout.

The [class-suite report](conformance/php-8.5.0-classes.txt) establishes the
public-instance object baseline. Nineteen cases pass both runtimes; 106 fail and 158
require unsupported PHPT sections. Public declarations, constructors, property
access, shared identity, bound method/closure calls, invokable callbacks, and
basic object inspection now have project regressions. The new project PHPT
fixture matches PHP 8.5.0. Separately, 251 focused examples matched stdout, exit
status, and diagnostic presence; they do not establish exact diagnostic wording.
These examples include scalar/object comparisons, property-name validation,
object callbacks, type checking, recursive dumps/comparison errors, and mutation
during property iteration. Visibility, typed properties,
most magic methods, reflection, handle reuse, and complete object
iteration semantics remain targets, not supported features.

The [inheritance report](conformance/php-8.5.0-inheritance.txt) now has three passing
upstream cases, with 31 failures and 36 unsupported fixtures. Three additional
class-suite cases now pass. A new project PHPT covers inherited constructors,
virtual dispatch, compatible overrides, inherited static locals, descendants in
type checks, `instanceof`, and early versus runtime class registration.
A separate matrix of 1,056 snippets matched PHP 8.5.0 for stdout, exit status,
and diagnostic presence. It includes parameter/return variance, arity/variadics,
class declaration permutations, and dynamic `instanceof` operands. Diagnostic
wording, interfaces, abstract methods, autoloading, and deferred variance
dependencies remain unfinished.

The [late-static-binding report](conformance/php-8.5.0-lsb.txt) records six
passing cases, eight failures, and eleven unsupported fixtures. The class-scope
slice also adds one passing class fixture, one inheritance fixture, and four
return-type fixtures. All passing fixtures were checked against PHP 8.5.0.
A 300-case differential check matched stdout, exit status, and diagnostic
presence across static/instance calls, forwarding, saved callbacks, contextual
type variance, and class-name inspection. Project regressions additionally
cover caller-context restoration, eval, default closure scope, and per-class
property-default identity. Magic method dispatch, typed members, non-public
visibility, and complete closure reflection/debugging remain incomplete.

The static-member slice adds nine class-suite passes, two late-static-binding
passes, and one inheritance pass without regressing prior passing cases.
The new [constant-suite report](conformance/php-8.5.0-constants.txt) records 19
passes, 37 failures, and 39 unsupported fixtures. Twenty project regressions
and a reference-checked PHPT cover public class constants, final overrides,
shared and redeclared static properties, dynamic access, array mutations,
probes, initializer timing, and cyclic constant errors. A 214-case differential
check and the existing 300-case class-scope matrix matched stdout, exit status,
and diagnostic presence against PHP 8.5.0. These checks do not claim exact
diagnostic wording. Intersection/DNF constant types, non-public members, all constant
expression forms, and the remaining upstream failures are still unfinished.


The [typed-constant report](conformance/php-8.5.0-typed_class_constants.txt)
adds three passing upstream fixtures. Twelve fail and fourteen require
unsupported PHPT sections. Twelve project tests and a reference-checked PHPT
cover strict scalar and class type checks, nullable/union types, integer-to-float
conversion, lazy failures, compile-time failures, and covariant overrides.
The implementation follows the [typed class constants RFC](https://wiki.php.net/rfc/typed_class_constants)
for strict checking and inheritance, with PHP 8.5.0 probes for evaluation timing.
A 712-case matrix matches stdout, exit status, and diagnostic presence. Closure
values in this matrix are compared by their debug type; complete Closure dumps
remain an existing gap. The 300-case class-scope matrix also remains green.
The class and inheritance suite counts are unchanged. Intersection/DNF types,
non-public constants, interfaces, traits, enums, exception handling, and exact
fatal diagnostic formatting remain incomplete.
