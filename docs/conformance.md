# Conformance baseline

Measurements through 2026-10-28 with the PHP **8.5.0** release source and a locally built
CLI reference. Upstream fixtures run as real files with the source checkout as
their working directory. These are narrow measurements, not full PHP
compatibility claims.

| Suite | Passed | Failed | Unsupported |
| --- | ---: | ---: | ---: |
| Project `tests/phpt`, Rust and reference PHP | 53 | 0 | 0 |
| PHP 8.5.0 `tests/basic`, Rust | 6 | 5 | 99 |
| PHP 8.5.0 `tests/lang`, Rust and reference PHP for passing cases | 71 | 96 | 127 |
| PHP 8.5.0 `Zend/tests/match`, Rust and reference PHP for passing cases | 17 | 5 | 13 |
| PHP 8.5.0 `Zend/tests/list`, Rust and reference PHP for passing cases | 12 | 16 | 30 |
| PHP 8.5.0 `Zend/tests/array_unpack`, Rust and reference PHP for passing cases | 2 | 5 | 6 |
| PHP 8.5.0 `Zend/tests/return_types`, Rust and reference PHP for passing cases | 10 | 8 | 71 |
| PHP 8.5.0 `Zend/tests/static_variables`, Rust and reference PHP for passing cases | 7 | 5 | 1 |
| PHP 8.5.0 `Zend/tests/restrict_globals`, Rust and reference PHP for passing cases | 2 | 1 | 13 |
| PHP 8.5.0 `Zend/tests/jump`, Rust and reference PHP for passing cases | 6 | 6 | 5 |
| PHP 8.5.0 `Zend/tests/exit`, Rust and reference PHP for passing cases | 1 | 4 | 22 |
| PHP 8.5.0 `Zend/tests/pipe_operator`, Rust and reference PHP for passing cases | 10 | 9 | 12 |
| PHP 8.5.0 `Zend/tests/first_class_callable`, Rust and reference PHP for passing cases | 7 | 14 | 44 |
| PHP 8.5.0 `Zend/tests/closures`, Rust and reference PHP for passing cases | 28 | 48 | 59 |
| PHP 8.5.0 `Zend/tests/arrow_functions`, Rust and reference PHP for passing cases | 2 | 2 | 5 |
| PHP 8.5.0 selected `ext/standard/tests/array` map/filter/reduce cases, Rust and reference PHP for passing cases | 12 | 21 | 6 |
| PHP 8.5.0 `tests/classes`, Rust and reference PHP for passing cases | 56 | 69 | 158 |
| PHP 8.5.0 `Zend/tests/inheritance`, Rust and reference PHP for passing cases | 24 | 10 | 36 |
| PHP 8.5.0 `Zend/tests/lsb`, Rust and reference PHP for passing cases | 11 | 3 | 11 |
| PHP 8.5.0 `Zend/tests/constants`, Rust and reference PHP for passing cases | 31 | 25 | 39 |
| PHP 8.5.0 `Zend/tests/type_declarations/typed_class_constants*.phpt`, Rust and reference PHP for passing cases | 4 | 11 | 14 |
| PHP 8.5.0 `Zend/tests/type_declarations/typed_properties*.phpt`, Rust and reference PHP for passing cases | 17 | 59 | 44 |
| PHP 8.5.0 `Zend/tests/readonly_props`, Rust and reference PHP for passing cases | 7 | 15 | 18 |
| PHP 8.5.0 `Zend/tests/readonly_classes`, Rust and reference PHP for passing cases | 4 | 3 | 16 |
| PHP 8.5.0 `Zend/tests/ctor_promotion`, Rust and reference PHP for passing cases | 1 | 6 | 11 |
| PHP 8.5.0 `Zend/tests/clone`, Rust and reference PHP for passing cases | 6 | 6 | 14 |
| PHP 8.5.0 `Zend/tests/asymmetric_visibility`, Rust (no passing fixtures) | 0 | 21 | 18 |
| PHP 8.5.0 `Zend/tests/exceptions`, Rust and reference PHP for passing cases | 7 | 33 | 54 |
| PHP 8.5.0 `Zend/tests/throw`, Rust and reference PHP for passing cases | 2 | 2 | 1 |
| PHP 8.5.0 `Zend/tests/try`, Rust and reference PHP for passing cases | 43 | 17 | 20 |

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
during property iteration. Visibility, property hooks,
most magic methods, reflection, handle reuse, and complete object
iteration semantics remain targets, not supported features.

The [inheritance report](conformance/php-8.5.0-inheritance.txt) now has four passing
upstream cases, with 30 failures and 36 unsupported fixtures. Three additional
class-suite cases now pass. A new project PHPT covers inherited constructors,
virtual dispatch, compatible overrides, inherited static locals, descendants in
type checks, `instanceof`, and early versus runtime class registration.
A separate matrix of 1,056 snippets matched PHP 8.5.0 for stdout, exit status,
and diagnostic presence. It includes parameter/return variance, arity/variadics,
class declaration permutations, and dynamic `instanceof` operands. Diagnostic
wording, built-in interfaces, abstract property hooks, autoloading, and deferred variance
dependencies remain unfinished.

The [late-static-binding report](conformance/php-8.5.0-lsb.txt) records six
passing cases, eight failures, and eleven unsupported fixtures. The class-scope
slice also adds one passing class fixture, one inheritance fixture, and four
return-type fixtures. All passing fixtures were checked against PHP 8.5.0.
A 300-case differential check matched stdout, exit status, and diagnostic
presence across static/instance calls, forwarding, saved callbacks, contextual
type variance, and class-name inspection. Project regressions additionally
cover caller-context restoration, eval, default closure scope, and per-class
property-default identity. Most magic method dispatch, non-public
property/constant visibility, and complete closure reflection/debugging remain incomplete.

The static-member slice adds nine class-suite passes, two late-static-binding
passes, and one inheritance pass without regressing prior passing cases.
The new [constant-suite report](conformance/php-8.5.0-constants.txt) records 19
passes, 37 failures, and 39 unsupported fixtures. Twenty project regressions
and a reference-checked PHPT cover public class constants, final overrides,
shared and redeclared static properties, dynamic access, array mutations,
probes, initializer timing, and cyclic constant errors. A 214-case differential
check and the existing 300-case class-scope matrix matched stdout, exit status,
and diagnostic presence against PHP 8.5.0. These checks do not claim exact
diagnostic wording. Intersection/DNF constant types, non-public properties/constants, all constant
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
non-public constants, built-in interfaces, traits, enums, exception handling, and exact
fatal diagnostic formatting remain incomplete.


The [typed-property report](conformance/php-8.5.0-typed_properties.txt) records
nine passing fixtures, 67 failures, and 44 unsupported fixtures. The property
slice also adds one inheritance-suite pass without losing any previous passes.
Fifteen project regressions and a reference-checked PHPT exercise typed public
instance/static properties, uninitialized slots, coercion and assignment
results, strict defaults, array mutations, `unset`, object dumps, and invariant
inherited declarations. The [PHP property manual](https://www.php.net/manual/en/language.oop5.properties.php)
and local PHP 8.5.0 probes establish the initialization behavior.
A 716-case matrix matches stdout, exit status, and diagnostic presence, including
integer-limit failures and deferred class binding for unresolved inherited types.
The existing 300-case class-scope and 712-case constant-type matrices also pass.
General references, property hooks, non-public properties, magic
accessors, full reflection, and exceptions remain unfinished; the passing
counts do not establish compatibility with those features.


The readonly slice adds [class](conformance/php-8.5.0-readonly_classes.txt) and
[property](conformance/php-8.5.0-readonly_props.txt) baselines. One class fixture
passes. None of the property fixtures pass yet: they need combinations of
exceptions, references, magic accessors, and
reflection that remain unimplemented. Unsupported PHPT sections are not run.
Fifteen project regressions and a reference-checked PHPT cover permitted
initialization scopes, initialized-null storage, readonly arrays, interior
object mutation, class restrictions, and inherited readonly status.
The [PHP property manual](https://www.php.net/manual/en/language.oop5.properties.php)
and PHP 8.5.0 probes establish the protected-set initialization behavior.
A 447-case matrix matches stdout, exit status, and diagnostic presence; the
existing 716-case typed-property matrix also passes. Class and inheritance
suite counts were unchanged in that slice. Explicit asymmetric visibility, property
hooks, and the other previously noted gaps remain.

The [constructor-promotion report](conformance/php-8.5.0-ctor_promotion.txt)
records one passing fixture, six failures, and eleven unsupported fixtures.
Ten project regressions and a reference-checked PHPT cover public and readonly
promotion, parameter coercion, assignment before the constructor body, inherited
constructors, declaration order, and separate parameter/property storage.
Defaults belong to parameters, as described in the
[PHP constructor manual](https://www.php.net/manual/en/language.oop5.decon.php).
A 269-case matrix matches PHP 8.5.0 stdout, exit status, and diagnostic presence.
Failed argument validation leaves existing properties unchanged when calling a
constructor again. Remaining upstream failures involve reflection, references,
traits, exception handling, and object-ID reuse. Private/protected property visibility,
advanced promotion modifiers, and object construction in parameter defaults
remain unsupported.
The readonly suite counts are unchanged.


The [cloning report](conformance/php-8.5.0-clone.txt) records two passing
fixtures, ten failures, and fourteen unsupported fixtures. Eighteen new project
regressions and a reference-checked PHPT cover shallow object/closure copies,
inherited `__clone()`, readonly reinitialization, clone-with updates, argument
validation, and property-write failure ordering. The
[PHP cloning manual](https://www.php.net/manual/en/language.oop5.cloning.php),
PHP 8.5.0 source, and local probes establish the behavior. Clone-with updates
use internal weak property coercion while preserving caller access scope.
The implementation also matches PHP 8.5.0's retained reinitialization flags
on untouched readonly slots after nonempty clone-with updates. Normal
`__clone()` invocation clears those flags even if the method fails.
A 619-case differential matrix matches stdout, exit status, and diagnostic
presence; closure results use type inspection because complete closure debug
output is still missing. Assignment operands such as `clone $copy = $original`
now parse correctly, and destructuring objects raises a PHP error instead of
panicking. The class suite gains two passes; the refreshed language baseline
gains eleven passes without losing any prior passing fixtures. Readonly suite
counts remain unchanged. Asymmetric visibility,
property hooks, general references, exceptions, destructors, and exact object-ID
reuse remain incomplete; unsupported PHPT sections are not counted as passes.


The abstract-class slice adds four passes to the
[inheritance report](conformance/php-8.5.0-inheritance.txt) and two to the
[first-class callable report](conformance/php-8.5.0-first_class_callable.txt),
without losing previous passing fixtures. Eleven project regressions and a
reference-checked PHPT cover abstract public methods, concrete implementation
requirements, static dispatch, conditional declarations, readonly abstract
classes, and abstract constructor prototypes. The original abstract constructor
contract persists through intermediate implementations and redeclarations.
Direct calls and acquisition of abstract method callables fail before evaluating
arguments; abstract construction fails before resolving defaults or evaluating
constructor arguments. A 321-case differential matrix matches stdout, exit
status, and diagnostic presence against PHP 8.5.0. The
[PHP abstraction manual](https://www.php.net/manual/en/language.oop5.abstract.php)
and local PHP 8.5.0 source/probes establish the rules. Protected/private property
visibility, built-in interfaces, traits, abstract property hooks, reflection, and deferred
signature checks involving unresolved class dependencies remain incomplete.


The interface slice adds eight class-suite passes, five inheritance-suite passes,
nine constant-suite passes, and one return-type pass, without losing previous
passing fixtures. Thirteen project regressions and a reference-checked PHPT cover
user-defined interfaces, multiple inheritance, signature compatibility,
implementation requirements, interface type checks, typed/final constants,
constant ambiguity and diamond inheritance, constructor contracts, and declaration
timing. Root interfaces can be bound early; interfaces with parents and classes
with an `implements` clause are registered when execution reaches their declaration.
The [PHP interface manual](https://www.php.net/manual/en/language.oop5.interfaces.php)
and PHP 8.5.0 probes establish the rules. A 379-case differential matrix matches
stdout, exit status, and diagnostic presence. Interfaces participate in class
constant initialization without eagerly evaluating overridden constants. Built-in
interfaces and their runtime protocols, interface property hooks, autoloading,
reflection, non-public properties/constants, and deferred signature checks involving unresolved
class dependencies remain incomplete.


The method-visibility slice adds fifteen class-suite passes, eight inheritance
passes, and one first-class callable pass without losing previous passing
fixtures. Sixteen project regressions and a reference-checked PHPT cover
private/protected methods, lexical private dispatch, constructor and clone
access, inherited visibility, saved method callables, and method_exists behavior.
An 863-case differential matrix matches PHP 8.5.0 stdout, exit status, and
diagnostic presence. Native callbacks can bind the active object within the
named class scope; ordinary variable calls retain their separate static-call
restrictions. Non-public `__invoke` warns but remains callable implicitly;
non-public `__toString` warns and fails when its class is linked, as required
by PHP's implicit Stringable contract. Full Stringable behavior is still missing.
The [PHP visibility manual](https://www.php.net/manual/en/language.oop5.visibility.php)
and local PHP 8.5.0 probes establish the access rules. Non-public properties and
constants, property promotion visibility, most magic-method protocols, and the
previously documented gaps remain incomplete.


The class-constant visibility slice adds one pass each in the class, constant,
and typed-constant suites, with no prior passing fixtures lost. Sixteen project
regressions and a reference-checked PHPT cover private/protected access, lexical
scope, cached values, initializer access, inherited visibility, interface
requirements, and independent types for redeclared private constants. A
906-case differential matrix matches PHP 8.5.0 stdout, exit status, and diagnostic
presence. Private constants are excluded from inheritance; `defined()` returns
false for inaccessible constants without evaluating them. Internal class
initialization can resolve non-public constants while checking accesses made by
their initializers. The [PHP visibility manual](https://www.php.net/manual/en/language.oop5.visibility.php)
and [typed class constants RFC](https://wiki.php.net/rfc/typed_class_constants),
checked against PHP 8.5.0 probes, establish these rules. This supersedes the
non-public constant limitations in earlier slice notes. Non-public properties,
property promotion visibility, exact diagnostic parity, and the other previously
documented gaps remain incomplete.


The static-property visibility slice adds six class-suite passes, three inheritance
passes, five late-static-binding passes, and two constant-suite passes, without
losing previous passing fixtures. Nineteen project regressions and a
reference-checked PHPT cover private/protected access, inherited shared storage,
independent private redeclarations, visibility widening, invariant property types,
object access, closures, and `property_exists`. A 1,289-case differential matrix
matches PHP 8.5.0 stdout, exit status, and diagnostic presence. Inaccessible
static-property probes return null/false without initializing class defaults;
ordinary access checks visibility before using cached storage. Private static
storage remains accessible through a child class from its declaring scope.
Compound assignments now evaluate the right-hand side before reading the target
value, including variables, array elements, and instance/static properties.
The [PHP visibility manual](https://www.php.net/manual/en/language.oop5.visibility.php)
and PHP 8.5.0 probes establish these rules. Non-public instance properties,
asymmetric static-property visibility, property promotion visibility, and the
other previously documented gaps remain incomplete.


The asymmetric static-property slice adds seventeen project regressions and a
reference-checked PHPT for PHP 8.5 setter visibility, explicit final static
properties, implicit finality of non-private `private(set)` declarations, and
inheritance of separate read/write permissions. An 827-case differential matrix
matches PHP 8.5.0 stdout, exit status, and diagnostic presence. Direct mutation
of an object stored in a readable static property is allowed; writes through an
array stored there require setter access. Coalescing avoids write checks when
it finds a non-null value. Nested unsets preserve uninitialized storage and
reject scalar offsets. Existing class, inheritance, and typed-property suite
counts remain unchanged, with no passing fixtures lost.
The new [asymmetric-visibility report](conformance/php-8.5.0-asymmetric_visibility.txt)
records 0 passes, 21 failures, and 18 unsupported cases. It includes instance
properties, references, exceptions, hooks, and diagnostic expectations beyond
the current implementation. The [static asymmetric visibility RFC](https://wiki.php.net/rfc/static-aviz)
and local PHP 8.5.0 probes establish the implemented rules. Reference operations,
asymmetric instance properties, final instance properties, exact diagnostic
parity, and the previously documented language gaps remain incomplete.


The exception-handling slice implements throw expressions, ordered catch matching,
union catches, omitted catch variables, rethrowing, and finally unwinding across
normal execution, returns, loop control, and exceptions. Exit bypasses catches
and finally. Goto cannot cross finally boundaries; break/continue cannot leave
a finally block. Built-in Throwable,
Exception, Error, and the core Error subclasses support native constructors,
message/code/file/line/previous getters, subclassing, and saved method callables.
Thrown object identity survives unwinding; a new exception from finally chains
the pending exception without introducing cycles. Runtime errors retain their
origin before function scopes are restored, and eval syntax errors are catchable.
Twenty-six project regressions, a reference-checked PHPT, and a 286-case
differential matrix match PHP 8.5.0 stdout, exit status, and diagnostic presence.
Existing tracked suites gain 34 passing fixtures with none lost. New reports
cover [exceptions](conformance/php-8.5.0-exceptions.txt),
[throw](conformance/php-8.5.0-throw.txt), and
[try/finally](conformance/php-8.5.0-try.txt), with 52 passes combined.
The [PHP exception manual](https://www.php.net/manual/en/language.exceptions.php)
and local PHP 8.5.0 probes establish the semantics. Stack traces, Throwable string
conversion and property layouts, exception/error handlers, ErrorException and SPL
exception classes, complete compile-time fatal-error distinctions, destructor
unwinding, and exact diagnostic parity remain incomplete. This supersedes earlier
notes listing exception syntax and ordinary catch/finally execution as unsupported.


The throwable-trace slice adds final `getTrace()` and `getTraceAsString()` methods
for Exception, Error, and their subclasses. Traces capture at object creation or
runtime-error materialization, retain live fixed-parameter values and coerced
variadic arguments, preserve named extras, and expose copy-on-write array
snapshots. Frames distinguish instance/static methods, nested lexical closures,
internal callbacks, eval, and file inclusion. Rejected arguments remain visible;
completed calls and rethrows do not add frames. Direct built-ins compiled as PHP
opcodes omit their frames, while dynamic calls and unoptimized aliases retain
them. String traces use PHP's default 15-byte string-argument truncation and
escaping without invoking object conversion.
Twelve regression tests and one new PHPT cover these behaviors. All **709 Rust
tests** and **53 project PHPT fixtures** pass; the latter also match PHP 8.5.0.
A 220-case trace differential matrix and the previous 286 exception cases match
stdout, exit status, and diagnostic presence. Closure argument handles and trace
strings are compared without claiming complete Closure var_dump metadata.
Refreshed exception, throw, try/finally, closure, arrow-function, and first-class
callable reports lose no passing fixtures; the closure report now records 28
passes, including gains from the preceding exception work. The official
[getTrace](https://www.php.net/manual/en/exception.gettrace.php) and
[getTraceAsString](https://www.php.net/manual/en/exception.gettraceasstring.php)
manuals, PHP 8.5.0 Zend source, and local reference probes establish the semantics.
Throwable `__toString()`, property layouts, configurable trace argument capture
and truncation, sensitive-parameter redaction, and the other exception gaps above
remain incomplete. This supersedes the earlier stack-trace limitation.
