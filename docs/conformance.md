# Conformance baseline

Measurements through 2026-10-28 with the PHP **8.5.0** release source and a locally built
CLI reference. Upstream fixtures run as real files with the source checkout as
their working directory. These are narrow measurements, not full PHP
compatibility claims.

The primary progress measure is now before/after performance on the upstream
PHP suite, with existing Rust tests used as regression checks. The first full
Zend comparison is [recorded here](conformance/php-8.5.0-object-casts-hillclimb.txt).
The [EXPECTF coverage expansion](conformance/php-8.5.0-expectf-hillclimb.txt)
executes 2,354 additional cases and verifies 44 additional passes, with no
regressions. This measures existing runtime behavior; it is not a runtime feature
gain. All 7,777 upstream EXPECTF patterns translate identically to PHP 8.5.0
`run-tests.php`. Workers isolate crashes and enforce per-process timeouts.
The [namespace implementation](conformance/php-8.5.0-namespaces-hillclimb.txt)
then adds 100 passing Zend cases with zero regressions. It supports lexical
namespace declarations, class/function/constant imports and grouped imports,
qualified and relative names, and global fallback for unqualified functions and
constants. Dynamic strings remain literal. Resolution follows the
[PHP namespace rules](https://www.php.net/manual/en/language.namespaces.rules.php).
The [callback invocation slice](conformance/php-8.5.0-call-user-func-hillclimb.txt)
adds another 20 passes with zero regressions. It implements `call_user_func`,
`call_user_func_array`, closure `__invoke`, and callback validation diagnostics.
Named array arguments follow the [PHP callback rules](https://www.php.net/manual/en/function.call-user-func-array.php);
direct positional calls follow the compiler path, while wrapper calls retain
internal callback coercion and trace frames.
The [trait composition slice](conformance/php-8.5.0-traits-hillclimb.txt)
adds 85 passing cases without regressions. Nested traits, method precedence,
aliases, visibility/final adaptations, abstract requirements, member compatibility,
and magic constants follow the [PHP trait semantics](https://www.php.net/manual/en/language.oop5.traits.php),
including PHP 8.5 composition before inheritance. Some newly passing fixtures
contain attributes; this does not establish attribute validation support.
The [error-reporting slice](conformance/php-8.5.0-error-reporting-hillclimb.txt)
adds 37 passes without regressions. `error_reporting` queries and updates a
request-local mask, applying nullable integer argument coercion and signed
32-bit mask conversion. Diagnostics are filtered when emitted, including
compile warnings in subsequently loaded files. The `@` operator preserves PHP
8's fatal levels, restores nested masks after exceptions, and preserves explicit
changes that enable nonfatal reporting inside the expression. Constant
initializers reject `@`. Existing Rust tests (797), reference-checked project
PHPT fixtures (57), formatting, and Clippy all pass. No new custom tests or
matrices were added. Error handlers, diagnostic interleaving, and exact fatal
rendering remain incomplete.
The [autoloading slice](conformance/php-8.5.0-autoload-hillclimb.txt) adds
17 passing Zend cases with no regressions. SPL callback queues retain resolved
callables, support duplicate registration, prepend/removal, and inspection, and
observe changes made during dispatch. Runtime class, interface, trait, static
member, and callable lookups load missing classes with a case-insensitive
recursion guard. `class_exists` and related APIs honor autoload flags. Loader
exceptions propagate through callable validation; `instanceof` and catch matching
do not initiate loading. Default autoloading searches lowercased filenames with
configurable extensions, includes each file once, and shares the caller's symbol
table without its lexical class scope. Behavior follows the
[PHP autoloading manual](https://www.php.net/manual/en/language.oop5.autoload.php)
and PHP 8.5.0 `ext/spl/php_spl.c`/`Zend/zend_execute_API.c`.
The focused Zend/autoload plus SPL/autoload runner records 8 passes, 13 failures,
and 11 unsupported fixtures. Separately executing the ten upstream SPL fixtures
that request only `include_path=.` matches reference output and exit status for
nine; the remaining fixture requires object destruction. These manual executions
do not change the runner's unsupported counts. Existing 797 Rust tests, 57
reference-checked project fixtures, formatting, and Clippy pass. Inheritance
variance autoloading, destructor callbacks, configurable include paths, and
exact fatal diagnostics remain incomplete.
The [instance-property visibility slice](conformance/php-8.5.0-property-visibility-hillclimb.txt)
adds 59 Zend passes with zero regressions. Linked property slots retain their
owning class, so private properties with the same name remain distinct across
inheritance. Access uses lexical private scope and protected prototype scope;
reads, writes, probes, unset, typed operations, readonly initialization, string
offsets, and cloning resolve the same slot. Dumps and array casts preserve
mangled names, while foreach and `get_object_vars` filter by scope and expose
visible names. Constructor promotion accepts non-public and asymmetric visibility;
instance declarations accept asymmetric setters and final properties. Behavior is
based on the [PHP visibility rules](https://www.php.net/manual/en/language.oop5.visibility.php)
and PHP 8.5.0 property handlers. All 797 existing Rust tests, 57 reference-checked
project fixtures, formatting, and Clippy pass. Magic property methods, references,
property hooks, reflection, and exact compile-fatal diagnostics remain incomplete.
Passing attribute-containing fixtures do not establish attribute validation.
The [magic-property slice](conformance/php-8.5.0-magic-properties-hillclimb.txt)
adds 36 Zend passes without regressions. All 797 existing Rust tests, 57
reference-checked project fixtures, formatting, and Clippy pass. No new custom
tests or differential matrices were added. It implements `__get`, `__set`, `__isset`, and `__unset`
using guards per object, property name, and operation. Guards survive nested
calls and are cleared on exceptions. Typed properties that have never been
initialized bypass handlers; explicitly unset slots can invoke them. Reads,
isset, empty, coalescing, and intermediate chain fetches preserve their distinct
handler order. Getter values are checked against unset property types where PHP
requires it. Indirect writes use a temporary value and emit the PHP notice,
while object handles returned by getters retain ordinary object identity.
Variadic magic-method signatures and caller locations in exception traces follow
PHP 8.5.0. The implementation is grounded in the
[PHP overloading rules](https://www.php.net/manual/en/language.oop5.overloading.php)
and `Zend/zend_object_handlers.c`. Reference-returning getters, method overloading,
property hooks, destructors, and exact diagnostic rendering remain unfinished.
The [method-overloading slice](conformance/php-8.5.0-magic-methods-hillclimb.txt)
adds 24 passing Zend fixtures with zero regressions. Missing or inaccessible
methods dispatch through `__call` and `__callStatic`; accessible methods retain
ordinary visibility, abstract-method, and static-call checks. Resolved callables
retain the requested method name independently of the handler, preserving named
argument keys, inherited handler scope, late static binding, saved closures, and
autoload callback identity. Closure dumps expose the virtual method name,
variadic arguments, and bound object. Objects still require a declared `__invoke`
to be callable. Literal static constructor calls and magic callables in constant
expressions retain PHP's restrictions. Parser checks cover handler arity,
staticness, and parameter types. Behavior follows PHP 8.5.0
`Zend/zend_object_handlers.c`, `Zend/zend_API.c`, and the upstream fixtures.
All 797 existing Rust tests, 57 reference-checked project fixtures, formatting,
and Clippy pass. No new custom tests or differential matrices were added.
General references, closure reflection/binding APIs, destructor callbacks, and
exact diagnostic rendering remain incomplete.
The [anonymous-class slice](conformance/php-8.5.0-anonymous-classes-hillclimb.txt)
adds 24 Zend passes with no regressions. Anonymous and readonly anonymous class
expressions accept constructor arguments, parents, interfaces, and traits, using
ordinary class linking and member storage. One declaration reuses its class and
static state across executions, while separately compiled declarations receive
distinct names. Names retain source identity and the resolved parent/first-interface
prefix; dumps and debug types omit the internal source suffix. Magic constants
distinguish the enclosing function's scope from the anonymous class's scope.
Typed and readonly property diagnostics use display names, and anonymous `self`
types retain their contextual spelling in property errors. The implementation
follows PHP 8.5.0 `Zend/zend_compile.c` and upstream anonymous-class fixtures.
All 797 existing Rust tests, 57 reference-checked project fixtures, formatting,
and Clippy pass, without new custom tests. Object-handle reuse, reflection,
serialization, attribute validation, and complete diagnostic display remain gaps.
The [class-alias and inheritance-query slice](conformance/php-8.5.0-class-aliases-hillclimb.txt)
adds 17 Zend passes without regressions. `class_alias` maps aliases to an existing
linked class and records the alias for type checks on existing objects and their
descendants. Aliases share methods, static storage, and canonical class names;
interfaces, traits, anonymous classes, and implemented native classes are supported.
`stdClass` now has stable linked identity within a request. Alias source lookup
honors its autoload flag, alias targets are case insensitive, and reserved names,
duplicates, and missing sources produce diagnostics. `is_a` and `is_subclass_of`
autoload permitted string sources but never their target names. PHP 8.5.0
`Zend/zend_builtin_functions.c` and `Zend/zend_API.c` establish these rules.
All 797 existing Rust tests, 57 reference-checked project fixtures, formatting,
and Clippy pass. The focused class-alias plus anonymous-alias run records
12 passes, 14 failures, and 1 unsupported case; six standard inheritance-query
fixtures record 2 passes and 4 failures. The FILE section of upstream
`Zend/tests/is_a.phpt` also matches expected output and reference exit status when
executed directly; its INI section remains unsupported by the runner and is not
counted as a pass. Closure class aliases, declaration-list APIs, complete native
class coverage, and exact warning/compile-fatal behavior remain incomplete.
The [byte-string builtin slice](conformance/php-8.5.0-byte-strings-hillclimb.txt)
adds 28 Zend passes with zero regressions. `str_repeat` handles binary input,
zero/negative counts, checked length arithmetic, and fallible allocation;
`strrev` reverses bytes. `chr` constrains integers to one byte and emits PHP 8.5's
out-of-range deprecation; `ord` returns the first byte (zero for empty input),
deprecating strings whose length differs from one. Common scalar coercion preserves
strictness and Stringable behavior. Native argument diagnostics now include
parameter names and supplied types, and distinguish missing named arguments from
argument-count errors. PHP 8.5.0 `ext/standard/string.c` and upstream fixtures
establish these behaviors. All 797 existing Rust tests, 57 reference-checked
project fixtures, formatting, and Clippy pass, without new custom tests.
The focused standard string suite records 7 passes, 4 failures, and 2 unsupported
fixtures. The FILE section of upstream `str_repeat.phpt` also matches reference
output and status directly with its requested precision setting; its INI section
remains unsupported by the runner and is not counted as passing. Expected-warning
interleaving, complete memory-limit semantics, and other string builtins remain
unfinished.
The [variable-reference assignment slice](conformance/php-8.5.0-variable-references-hillclimb.txt)
adds 18 Zend passes with zero regressions. `=&` binds named/dynamic variables and
individual `$GLOBALS` entries to shared variable cells. Undefined sources become
null without read warnings; rebinding replaces the target binding without
overwriting its former aliases. Value reads preserve ordinary copy-on-write
behavior. Existing closure reference captures and global/static bindings use the
same cells. Parsing retains reference-assignment precedence and rejects whole
`$GLOBALS`, `$this`, and nullsafe reference sources where required. All 797
existing Rust tests, 57 reference-checked project fixtures, formatting, and Clippy
pass. No new custom tests were added. The upstream language subset records
97 passes, 137 failures, and 60 unsupported fixtures. Array-element and property
references, references stored in arrays, by-reference parameters/returns,
destructuring references, and reference-aware iteration remain incomplete;
unsupported storage references still fail explicitly.
The [fixed reference-parameter slice](conformance/php-8.5.0-reference-parameters-hillclimb.txt)
adds 15 Zend passes without regressions. Parameters retain reference mode in
function, method, closure, and inherited signatures. Fixed arguments bind the
source variable/global cell before later arguments run, preserving identity
across rebinding; entry coercion updates the referenced value. Named arguments,
optional defaults, assignment-by-reference expressions, and saved callables use
the same path. Ordinary invalid arguments fail, value-returning calls/new objects
emit the PHP notice, and internal callbacks warn while operating on temporary
values. `$this` is passed through a separate temporary cell. Closure dumps label
reference parameters, implemented magic handlers reject fixed reference arguments,
and method compatibility keeps reference mode invariant. All 797 existing Rust
tests, 57 reference-checked project fixtures, formatting, and Clippy pass, with
no new custom tests. Array/property references, reference variadics/unpacking,
reference returns, and promoted property reference storage remain unsupported.
The [array-reference storage slice](conformance/php-8.5.0-array-references-hillclimb.txt)
adds 24 Zend passes with zero regressions. Array elements and reference literals
share variable cells, including nested/append reference assignment, fixed
reference arguments, rebinding, and unset. Copy-on-write unwraps reference
containers with no other owners while preserving shared bindings. Traversal
releases each containing value before following a reference, so recursive array
writes/unsets do not retain overlapping Rust borrows. String-offset writes follow
the same reference boundaries before invoking conversion callbacks. Ordinary
reads, foreach values, callback lookup, dumps, comparisons, recursive count, and
array unpacking account for reference entries. Internal callbacks preserve
already-referenced array arguments, and clone-with rejects shared reference
updates. The implementation follows PHP 8.5.0 `Zend/zend_hash.c`,
`Zend/zend_vm_def.h`, `ext/standard/array.c`, and upstream fixtures.
All 797 existing Rust tests, 57 reference-checked project fixtures, formatting,
and Clippy pass; no new custom tests were added. Property references, reference
iteration/destructuring, variadics/unpacking, reference returns, cycle collection,
and complete recursive comparison/diagnostic behavior remain unfinished.
The [array reference-iteration slice](conformance/php-8.5.0-reference-iteration-hillclimb.txt)
adds 21 Zend passes with zero regressions. `foreach ($array as &$value)` binds
each live element and leaves the final alias intact. Iterator tokens retain
independent positions across array copies, update positions after removals, and
observe appended entries. Mutable bucket storage lets iteration establish an
element reference without prematurely separating an array copied inside the
loop; ordinary writes still perform copy-on-write separation. Bucket indirection
is distinct from a PHP reference and is transparent to value reads and dumps.
Nested loops, source replacement/rebinding, temporary
arrays containing references, and reference-assignment expressions use the same
storage. Tokens are weakly retained by arrays and expire when loops exit.
Behavior is grounded in PHP 8.5.0 `Zend/zend_hash.c` and `Zend/zend_vm_def.h`
and the upstream foreach fixtures. The focused foreach suite records 27 passes,
30 failures, and one unsupported fixture. All 797 existing Rust tests, 57
reference-checked project fixtures, formatting, and Clippy pass; no new custom
tests were added. Object/property reference iteration, reference destructuring,
internal array-pointer APIs, mutating array builtins, and diagnostic formatting
remain incomplete.
The [reference variadic/unpacking slice](conformance/php-8.5.0-reference-variadics-hillclimb.txt)
adds six Zend passes with zero regressions. Reference variadics retain caller
cells for positional and named arguments, including typed entry coercion and
callback temporary references. Array argument unpacking binds selected entries
to reference parameters after separating copied source arrays, retaining named
keys and supporting temporary arrays. Later argument evaluation cannot rebind
already-acquired references. `func_get_arg`/`func_get_args` and captured traces
read referenced values into snapshots; subsequent argument inspection still
sees mutations through the original cells. Variadic reference errors use the
actual positional argument number and omit a fixed parameter name. Invalid
unpacking diagnostics report the supplied type. All 797 existing Rust tests,
57 reference-checked project fixtures, formatting, and Clippy pass; no new custom
tests were added. Traversable unpacking, property reference sources, reference
returns, and complete diagnostic rendering remain unfinished.
The [reference destructuring slice](conformance/php-8.5.0-reference-destructuring-hillclimb.txt)
adds six Zend passes with zero regressions. `list()` and short-array patterns
accept nested and keyed reference targets, initialize missing referenced entries,
and keep the original source cell while destinations rebind variables. Key
expressions may mutate the containing array without invalidating a fetched nested
source. Foreach patterns containing references select live reference iteration,
and nested patterns bind only their requested targets. Constant/scalar source
expressions are rejected at compile time; value-returning calls and the GLOBALS
snapshot produce per-fetch notices and temporary reference targets. Pipe calls
use the same value-return path. The implementation follows PHP 8.5.0
`zend_compile_list_assign`, `zend_propagate_list_refs`, and `ZEND_FETCH_LIST_W`.
All 797 existing Rust tests, 57 reference-checked project fixtures, formatting,
and Clippy pass; no new custom tests were added. Property reference storage,
ArrayAccess integration, by-reference returns, and exact fatal/notice rendering
remain incomplete.
The [reference-return slice](conformance/php-8.5.0-reference-returns-hillclimb.txt)
adds 25 Zend passes with zero regressions. Named functions, methods, closures,
and arrows retain return-reference mode. Return expressions can expose variable
or array-element cells, including calls forwarding existing references; return
coercion updates the referent. Reference assignment, array reference literals,
fixed/variadic parameters, argument unpacking, destructuring, foreach, saved
callables, and pipes retain returned cells, while ordinary expression reads copy
values. Return mode is restored across nested calls and reset for include/eval
units. Invalid reference returns and assignments emit notices; void reference
returns emit the PHP 8.5 compile deprecation. Method overrides preserve inherited
reference-return contracts. String conversion reads referenced strings, and
magic getter results retain reference identity during indirect writes and typed
result checks. All 797 existing Rust tests, 57 reference-checked project fixtures,
formatting, and Clippy pass; no new custom tests were added. Property reference
storage, ArrayAccess, complete magic-property reference behavior, recursion
resource limits, and exact diagnostic rendering remain unfinished.
The [property-reference slice](conformance/php-8.5.0-property-references-hillclimb.txt)
adds 37 Zend passes with zero regressions. Untyped instance/static properties share
reference cells with variables and array elements. Assignment, rebinding, reference
returns and parameters, foreach targets, array unpacking, and destructuring use the
same property reference path. Reference-returning magic getters expose their cells;
value-returning getters produce temporary cells and the indirect-modification notice.
Direct reference assignment to overloaded properties reports PHP's error. Reference
acquisition checks readonly and asymmetric visibility, including PHP's temporary
object-handle behavior for restricted object properties. Object snapshots unwrap
singleton references, and recursive object dumps suppress the reference marker on
recursion lines. All 797 existing Rust tests, 57 reference-checked project fixtures,
formatting, and Clippy pass; no new custom tests were added. Typed property reference
constraints, object reference iteration, cycle collection, ArrayAccess, and exact
diagnostic rendering remain unfinished.

The [typed-reference slice](conformance/php-8.5.0-typed-references-hillclimb.txt)
adds 41 Zend passes with zero regressions. Shared cells track live typed property
sources through weak object/property identities. Writes through variables, array
elements, other properties, parameters, and returned references validate all live
constraints before committing a value. Weak conversion must agree across property
types; incompatible bindings and inconsistent conversions report the applicable
property types. Unset/rebound properties and destroyed objects release constraints,
while clones inherit them. Nullable uninitialized properties initialize to null when
referenced; array auto-initialization and integer increment limits respect property
constraints. Constructor-promoted references work. Catch variables now assign through
existing aliases with strict validation, including finally handling on failure.
Static assignment expressions return the converted value, and uninitialized static
property diagnostics include the static qualifier. All 797 existing Rust tests,
57 reference-checked project fixtures, formatting, and Clippy pass; no new custom
tests were added. Object reference iteration, internal by-reference builtins,
complete parameter/return coercion diagnostics, destructor/reentrant edge cases,
cycle collection, and exact diagnostic rendering remain unfinished.

The [object reference iteration slice](conformance/php-8.5.0-object-reference-iteration-hillclimb.txt)
adds 3 Zend passes with zero regressions. Foreach by reference over ordinary objects
binds visible property cells and registers their typed constraints, including private
properties in the declaring scope. Uninitialized properties are skipped; initialized
readonly properties report PHP's reference-acquisition error. Property snapshots are
released before executing loop bodies so they do not artificially retain reference
sharing. Existing object handles, reference destructuring, and property loop targets
use the same binding path. All 797 existing Rust tests, 57 reference-checked project
fixtures, formatting, and Clippy pass; no new custom tests were added. Iterator and
ArrayAccess objects, deletion/reinsertion cursor edge cases, object destructors, and
cycle collection remain unfinished.

The [print_r slice](conformance/php-8.5.0-print-r-hillclimb.txt) adds 16 Zend
passes with zero regressions. `print_r` writes formatted output or returns a string,
with binary-safe scalar/key rendering, nested array/object indentation, private and
protected property labels, and recursion detection through shared references.
Object rendering invokes `__debugInfo`, including reference-returning methods, and
reports PHP 8.5's null-result deprecation. Scalar NaN rendering emits PHP's warning.
Closure property construction is shared with the existing dump formatter. Four of
13 dedicated upstream `print_r` cases pass; two fail on dynamic-property attributes
or resource support, and seven require unsupported INI/whitespace sections. All 797
existing Rust tests, 57 reference-checked project fixtures, formatting, and Clippy
pass; no new custom tests were added. Custom debug properties in `var_dump`, complete
closure metadata, object identity reuse, callback reentrancy edge cases, and resource
printing remain unfinished.

The [attribute foundation slice](conformance/php-8.5.0-attributes-hillclimb.txt)
adds 9 Zend passes with zero regressions. The lexer recognizes `#[...]`; the parser
retains attributes on classes, functions/methods, properties, parameters, constants,
closures/arrows, and anonymous classes. Deferred argument parsing uses declaration
scope for names and magic constants, validates constant expressions (including new
expressions), and keeps argument evaluation deferred. Internal target/repetition
checks support `DelayedTargetValidation`; class-specific restrictions and invalid
`NoDiscard` return/magic-method declarations are checked. `AllowDynamicProperties`
sets inherited object behavior. Core attribute classes expose target flags, native
constructor argument checks, readonly payloads, and internal dynamic-property
restrictions. The dedicated upstream `print_r` suite improves to 5 passes, 1 failure,
and 7 unsupported cases. All 797 existing Rust tests, 57 reference-checked project
fixtures, formatting, and Clippy pass; no new custom tests were added. Attribute
reflection/instantiation, full Override/Deprecated/NoDiscard runtime behavior,
property hooks and fatal diagnostic rendering remain
unfinished.

The [class reflection slice](conformance/php-8.5.0-class-reflection-hillclimb.txt)
adds 12 Zend passes with zero regressions and raises the separate upstream reflection
suite from 0 to 72 passes (383 failed, 37 unsupported). `ReflectionClass` and
`ReflectionObject` expose class identity, names, source locations, flags, ancestry,
interfaces in Zend order, traits/aliases, constants, defaults, static property access,
and construction. Reflection construction uses weak argument coercion and preserves
references supplied in argument arrays. Static writes retain property type/reference
constraints. Native variadic methods accept extra arguments; native scalar parameters
use PHP's weak null deprecation behavior.

`ReflectionAttribute` supports class-attribute names, targets, repetition, filtering,
deferred arguments, and instantiation. Evaluation restores declaration scope/source
and strict-types mode even on failure. Attribute marker flags are validated without
constructing temporary PHP objects; delayed internal class validation runs when the
attribute is instantiated. Class declarations retain source identity, strictness,
and end lines. All 797 existing Rust tests, 57 reference-checked project fixtures, formatting, and Clippy pass; no new custom tests were added. Member,
function, parameter, type, extension, and lazy-object reflection, complete Reflector
interfaces/string formatting, dynamic properties in ReflectionObject, object-handle
reuse, and exact diagnostics remain unfinished. Other declaration targets still need
attribute reflection, and runtime Override/Deprecated/NoDiscard effects remain open.

The [function reflection slice](conformance/php-8.5.0-function-reflection-hillclimb.txt)
adds 11 Zend passes and 66 reflection-suite passes, with zero regressions in either
suite. `ReflectionFunction`, `ReflectionMethod`, and `ReflectionParameter` expose
source/signature metadata, names, flags, parameter defaults, and declaration ownership.
Named and union types support nullable metadata, string conversion, and type lists.
Method discovery respects visibility/filtering, and `createFromMethodName` constructs
the called reflection subclass. Function end lines and static closure flags are retained.

Reflective invocation handles named arguments, argument-array references, weak scalar
coercion, private/protected methods, late static binding, abstract-method errors, and
copied reference returns. Closure creation preserves the callable, receiver, and scope;
capture inspection preserves reference captures. Attribute inspection now applies to
functions, methods, closures, and parameters using their declaration scope/source,
strictness, target, and repetition metadata. Native method argument-count checks share
the internal-function binder. The reflection implementation is split into declaration,
attribute, function/type, and parameter modules.

All 797 existing Rust tests, 57 reference-checked project fixtures, formatting, and
Clippy pass; no new custom tests were added. Property/class-constant reflection,
complete builtin parameter/return signatures, tentative return metadata, prototypes,
static-variable inspection, doc comments/declaration formatting, intersection types,
Reflector integration, and lazy objects remain unfinished. Closure APIs, generators,
and diagnostic rendering also block upstream cases.

The [formatted printing slice](conformance/php-8.5.0-formatted-print-hillclimb.txt)
adds 7 Zend passes, 2 reflection passes, and 51 formatting-suite passes, with zero
regressions. The 101 formatting fixtures now contain 51 passes, 10 failures, and
40 unsupported cases. Ten additional upstream formatting bug fixtures pass; one
requires `metaphone`, and four have unsupported sections.

`printf`, `sprintf`, `vprintf`, and `vsprintf` share binary-safe format parsing,
sequential/positional arguments, star width and precision, custom padding, string
truncation, signed/unsigned/base integers, and fixed/exponential/general floats.
General float formatting includes shortest precision (`-1`) and PHP's exponent
spelling. Argument arrays use insertion order, references are dereferenced without
mutation, and printing is deferred until formatting succeeds. Missing arguments,
invalid widths/precisions/specifiers, and unknown named parameters produce PHP's
exception classes and messages. Existing conversion diagnostics are preserved.

All 797 existing Rust tests, 57 reference-checked project fixtures, formatting,
and Clippy pass. No new custom tests were added. Locale configuration, resource
arguments, error handlers, and matching expected diagnostics remain unfinished.

The [class inspection slice](conformance/php-8.5.0-class-inspection-hillclimb.txt)
adds 18 Zend passes, 5 standard class/object passes, and 1 reflection pass, with
zero regressions. `get_class_methods` accepts class names or objects, autoloads
classes, filters by caller visibility, and preserves declaration/inheritance order.
`get_class_vars` converts and validates its class-name argument, returns visible
instance defaults before static declaration defaults, includes null for uninitialized
typed properties, and respects name shadowing across storage kinds. Private inherited
declarations are available in their declaring scope. Mutable static values are not
substituted for declaration defaults.

Trait composition now inserts each method's aliases before its original, in adaptation
order, including aliases of excluded methods. This fixes both ordinary method lists
and reflection ordering. All 797 existing Rust tests and 57 reference-checked project
fixtures pass, as do formatting and Clippy; no new custom tests were added. Expected
diagnostic matching and error handlers still block two targeted fixtures. Closure
method names are inspectable; their binding/call APIs remain unimplemented. Property
hooks and native-class metadata remain incomplete.

The [callable inspection slice](conformance/php-8.5.0-callable-inspection-hillclimb.txt)
adds 16 Zend passes and 4 standard-library passes, with zero Zend or reflection
regressions. `is_callable` supports available function names, class/object method
arrays, closures, and invokable objects. Syntax-only checks validate callback shape
without autoloading. Semantic checks preserve caller visibility, active object context,
abstract-method rejection, magic handlers, and autoloader exceptions. Deprecated
self/parent/static and qualified-method callback forms emit diagnostics.

The optional `callable_name` output preserves supplied string/array names and reports
closure declaration names, including first-class methods and anonymous class names.
Native dispatch now retains reference argument cells, so named output arguments and
typed-property reference constraints use the shared reference machinery. `syntax_only`
has its correct false default in parameter metadata. Protected overrides now use their
original non-private prototype for access checks, fixing ordinary calls, magic method
fallback, and trait cases as well as callable inspection.

All 797 existing Rust tests and 57 reference-checked project fixtures pass, along with
formatting and Clippy. No new custom tests were added. Three standard fixtures remain
blocked by expected diagnostic matching, JSON, and file/resource functions. Remaining
native callable APIs and complete callable compatibility remain unfinished.

The [debug backtrace slice](conformance/php-8.5.0-debug-backtrace-hillclimb.txt)
adds 18 Zend passes with zero regressions. All 12 runnable backtrace-cluster cases
that reach the implemented functionality pass; four others fail on missing string
functions, stream wrappers, or expression assignment, and four have unsupported sections.

`debug_backtrace` and `debug_print_backtrace` share existing exception trace capture
and rendering. They exclude their own frame, support argument omission, optional
receiver objects, and zero/unlimited or positive frame limits. Both option constants
are available. Receiver objects appear in PHP's field order; argument snapshots retain
live fixed parameter values and positional/named extra arguments. Printed backtraces
omit the exception formatter's main-script terminator and are empty at top level.

All 797 existing Rust tests and 57 reference-checked project fixtures pass, as do
formatting and Clippy. No new custom tests were added. Complete closure debug properties, attribute construction trace details, generators,
fibers, stream wrappers, and configurable exception trace settings remain unfinished.

The [sensitive-parameter slice](conformance/php-8.5.0-sensitive-parameters-hillclimb.txt)
adds 11 Zend passes with zero Zend or reflection regressions. Backtraces and exception
traces wrap sensitive fixed parameters and positional variadic arguments in native
`SensitiveParameterValue` objects. This includes named fixed arguments, closures, arrow
functions, nested calls, and eval. Extra non-variadic arguments remain ordinary values;
PHP 8.5.0 also leaves extra named variadic arguments unwrapped, and this is preserved.

Wrappers retain captured values in private readonly storage, expose `getValue`, and
support cloning. Their exported properties are hidden for dumps, array casts, and
`get_object_vars`; `__debugInfo` returns an empty array. They reject dynamic properties
and are not themselves attributes. Trace capture snapshots active frames before wrapper
allocation, and throwable allocation retains its reserved object identity. Nowdoc and
heredoc literals carry their body line; eval uses its argument's source line, matching
the newly passing eval trace fixture.

All 797 existing Rust tests and 57 reference-checked project fixtures pass, with
formatting and Clippy also green. No new custom tests were added. Seven of the 18
sensitive-parameter fixtures remain blocked by assert/debug/export/serialization APIs,
property reflection, destructors, or expected fatal diagnostics. Uninitialized wrapper
reads and complete trace configuration remain unfinished.

The [property-reflection slice](conformance/php-8.5.0-property-reflection-hillclimb.txt)
adds 11 Zend and 55 reflection passes with zero regressions in either suite.
`ReflectionProperty` and class/object property discovery now cover declared, inherited,
static, and dynamic properties. Filtering uses PHP modifier flags, enumeration retains
source order across static/instance and promoted declarations, and trait composition
preserves each trait's property order. Qualified ancestor lookups and private ancestor
exclusion follow PHP's reflection rules. `ReflectionObject` retains its receiver so
subsequent discovery sees current dynamic properties.

Metadata includes declaring classes, mangled storage names, visibility/asymmetric-set,
readonly/final/promoted flags, types, defaults, and attributes evaluated in declaration
scope. Reflective reads/writes bypass visibility through the existing property machinery,
retaining magic access, weak scalar coercion, typed reference constraints, and readonly
initialization checks. Initialization queries inspect storage without invoking magic
getters. Ordinary property declaration strings include defaults and byte escaping.
The sensitive-value reflection fixture also passes, bringing that subset to 12/18.

All 797 existing Rust tests and 57 reference-checked project fixtures pass, with formatting
and Clippy green. No new custom tests were added. Property hooks/raw access, lazy objects,
doc comments, unevaluated default-expression rendering, and complete internal property
metadata (including throwable properties) remain unfinished. Expected diagnostic matching
still blocks deprecation/error fixtures; unsupported sections are not counted as passes.

The [class-constant reflection slice](conformance/php-8.5.0-class-constant-reflection-hillclimb.txt)
adds 5 Zend and 10 reflection passes with zero regressions in either suite.
`ReflectionClassConstant` and class-level individual/filtered discovery expose deferred
values, declared types, visibility/final/deprecated flags, declaring classes, attributes,
and declaration strings. Deferred values retain existing constant-expression type checks
and propagate errors through both value access and string conversion. Attribute evaluation
uses the constant's declaring scope, including constants composed from traits.

The lexer preserves documentation comments for ordinary properties and class constants,
including separate comments within grouped declarations. Their reflection APIs and
string representations expose those comments. Native reflector `name`/`class` fields now
use PHP's reflection-specific write exception while retaining ordinary typed-property
unsetting/reference behavior. Missing constant errors preserve the written class name
(such as `self`) and check existence before rejecting direct trait-constant access.

All 797 existing Rust tests and 57 reference-checked project fixtures pass, with formatting
and Clippy green. No new custom tests were added. Global-constant reflection, enums,
class/function/promoted-property documentation comments, complete reflector interfaces
and internal metadata, and class/function declaration rendering remain unfinished.
Expected diagnostic matching still blocks fatal/deprecation fixtures; unsupported sections
are not counted as passes.

The [global-constant reflection slice](conformance/php-8.5.0-global-constant-reflection-hillclimb.txt)
adds 8 Zend and 13 reflection passes with zero regressions in either suite.
Global constant records retain the original declaration name, value, source file,
attributes, and strict-types setting. Namespace lookup remains case-insensitive for
its prefix while reflection reports the original spelling. `ReflectionConstant` exposes
names, values, source locations, deprecation metadata, attributes, extension ownership,
and declaration strings. Reflection reads avoid ordinary constant-access deprecations.

Global attributes use the constant target and declaration context for filtering,
argument evaluation, repeatability checks, and instantiation. Native `E_STRICT` metadata
includes its Deprecated attribute and named arguments. `ReflectionAttribute` now exposes
its public name field. Basic `ReflectionExtension` objects provide names, versions, and
persistence for Core, standard, Reflection, and SPL; extension inventories and the rest
of their reflection API remain unfinished.

All 797 existing Rust tests and 57 reference-checked project fixtures pass, with formatting
and Clippy green. No new custom tests were added. Remaining global-constant fixtures are
blocked by object-ID reuse, random/resource APIs, or unsupported extension sections.
Runtime effects of user Deprecated attributes, remaining internal constants, full
reflector interfaces, and diagnostic matching remain incomplete. Unsupported sections
are not counted as passes.

The [enum slice](conformance/php-8.5.0-enums-hillclimb.txt) adds **74 Zend passes**
and **14 reflection passes**, with **zero regressions** in either suite. Unit and
int/string-backed enums retain shared case identities, readonly name/value properties,
methods, constants, trait composition, and implemented interfaces. Native UnitEnum and
BackedEnum contracts support `cases`, `from`, and `tryFrom`; backing values are evaluated
in class scope and checked for type mismatches and duplicates. Native conversion follows
the backing type and caller strictness. Enums cannot be instantiated, cloned, extended,
or given dynamic properties. Disallowed properties, magic methods, and attributes are
validated. The contextual enum keyword remains usable as a label or function name.

Enum values participate in comparisons, constant/default expressions, `enum_exists`,
`var_dump`, and `print_r`. ReflectionEnum and its unit/backed case reflectors expose
cases, backing types/values, attributes, documentation, and declaring enums. Enum class
reflection reports final, non-instantiable, and non-cloneable flags. Behavior is grounded
in PHP 8.5.0's enum/reflection implementation and upstream fixtures.

The enum directory records **67 passed / 77 failed / 3 unsupported**. All **797 existing
Rust tests** and **57 reference-checked project fixtures** pass; formatting and Clippy
pass. No custom tests were added. Remaining gaps include enum serialization, full
reflection declaration strings, object-handle reuse, and dependencies on other missing
APIs. Exact fatal/diagnostic matching and unsupported PHPT sections remain incomplete;
rejected declarations do not count as passing expected-fatal fixtures.

The [var_export slice](conformance/php-8.5.0-var-export-hillclimb.txt) adds **15
upstream passes**: two Zend, two reflection, and eleven standard-library fixtures,
with zero regressions in all three comparisons. It prints or returns scalar, array,
object, closure, and enum representations using PHP's indentation and qualified names.
Strings and array keys escape quotes, backslashes, and embedded NULs; ordinary object
properties retain their order and use unmangled names. Integer minima use a PHP-safe
expression, and default float formatting preserves fractions, signed zero, non-finite
values, and shortest round-tripping precision. Circular references emit warnings and
render NULL; repeated non-recursive values remain exportable.

All 797 existing Rust tests and 57 reference-checked project fixtures pass, with
formatting and Clippy green. No custom tests were added. The standard var_export subset
records **11 passed / 2 failed / 6 unsupported**. Its two failures emit the expected
circular-reference warning but cannot pass until diagnostic matching is implemented.
Configurable serialize_precision, property hooks, resources, and full native object
property representations remain incomplete. The implementation follows PHP 8.5.0
`ext/standard/var.c`, native float formatting rules, and the upstream fixtures.

The [object-handle slice](conformance/php-8.5.0-object-handles-hillclimb.txt) adds
**28 upstream passes**: 22 Zend, five reflection, and one SPL, with zero regressions.
Ordinary objects, closures, clones, enum cases, reflectors, sensitive-value wrappers,
and exceptions now share a request-local allocator. Released handles are reused in
last-freed-first order, after contained fields have released their own references.
Live aliases keep the original handle; objects retained outside the Runtime keep the
allocator alive. spl_object_id and spl_object_hash expose numeric handles and PHP's
32-character hexadecimal representation for objects and closures.

The full Zend suite records **1596 passed / 3252 failed / 408 unsupported**; reflection
records **240 / 215 / 37**, and the newly measured full SPL suite records **17 / 667 / 94**.
All 797 existing Rust tests and 57 reference-checked project fixtures pass, with
formatting and Clippy green. No custom tests were added. The implementation follows
PHP 8.5.0 `Zend/zend_objects_API.c`, `ext/spl/php_spl.c`, and upstream fixtures.
Cyclic garbage collection, destructor callbacks, and complete lifetime parity remain
unfinished; this implements handle recycling for objects whose owning references are
actually released.

The [argument-diagnostic slice](conformance/php-8.5.0-argument-errors-hillclimb.txt)
adds **23 upstream passes**: 19 Zend and four reflection, with zero regressions in
Zend, reflection, and SPL. Missing positional arguments report the supplied/required
counts and distinguish exact from minimum arity. Named argument gaps retain their
parameter-specific message. User-function type mismatches and count errors use the
function declaration's file and line; messages include the calling PHP file/line when
there is a PHP call site. Reflection and internal callback invocations retain their
location-free messages. Existing argument binding and type-check order are preserved.

All 797 existing Rust tests and 57 reference-checked project fixtures pass; formatting
and Clippy pass. No custom tests were added. Behavior follows PHP 8.5.0
`zend_missing_arg_error` and `zend_verify_arg_error`, with upstream fixtures covering
ordinary calls, named arguments, reflection construction, closures, traits, and pipes.
Passing a generator-containing fixture that fails during argument checking does not
establish generator execution support. Uncaught fatal rendering, diagnostic output
interleaving, and remaining type-system behavior are still incomplete.

The [ArrayAccess slice](conformance/php-8.5.0-array-access-hillclimb.txt) adds
**29 upstream passes**: 28 Zend and one SPL, with zero regressions across Zend,
reflection, and SPL. Object offsets dispatch to offsetExists/Get/Set/Unset with the
original, uncoerced offset values. Reads, isset/empty, coalescing, append, unset,
assignment, and destructuring preserve their distinct dispatch rules. Nested writes
and reference acquisition retain reference-returning getters; non-reference scalar
and array results emit indirect-modification notices and mutate temporary storage.
Compound assignment calls offsetSet, while increments mutate returned references and
retain typed-property overflow checks. ArrayAccess access is rejected in constant
expressions.

Runtime callbacks execute after storage borrows are released. Object-valued property
roots allow interior offset mutation without rebinding readonly/asymmetric slots or
restoring a property replaced during a callback. The native interface carries tentative
return types for reflection and compatibility warnings, with ReturnTypeWillChange
suppression. Native abstract methods are marked internal. Full comparisons also exposed
randomized scope-destruction order affecting recycled object IDs; local scopes now drop
values in PHP variable-slot order, making the affected enum/reflection cases stable.

The final Zend result is **1643 passed / 3205 failed / 408 unsupported**; reflection
remains **244 / 211 / 37**, and SPL improves to **18 / 666 / 94**. All 797 existing Rust
tests and 57 reference-checked project fixtures pass; formatting and Clippy pass. No
custom tests were added. Behavior follows PHP 8.5.0 object dimension handlers, the
ArrayAccess declaration, compiled-variable cleanup, and upstream fixtures. Complete
nested compound-assignment caching, tentative-return inheritance/formatting, other
container APIs, destructors/GC, and diagnostic output matching remain unfinished.

The [string case/trim slice](conformance/php-8.5.0-string-trim-hillclimb.txt) adds
**20 upstream passes**: 16 standard string fixtures and four Zend fixtures, with no
regressions across Zend, reflection, and SPL. `strtolower` and `strtoupper` convert
ASCII letters while preserving every other byte. `bin2hex` emits two lowercase hex
digits per input byte. `trim`, `ltrim`, `rtrim`, and `chop` apply PHP's default whitespace
set or a supplied binary character list, including inclusive ranges and malformed-range
warnings. Scalar/Stringable conversion and argument binding use the existing native
function machinery; `chop` retains its own name in diagnostics.

The focused upstream tests exercise all 256 byte values for case conversion, binary
encoding, whitespace, custom lists, ranges, and alias behavior. Results are **16 passed /
3 failed / 0 unsupported**; the three remaining fixtures require diagnostic output
matching. Full Zend improves to **1647 / 3201 / 408**, with reflection **244 / 211 / 37**
and SPL **18 / 666 / 94** unchanged. All 797 existing Rust tests and 57 reference-checked
project fixtures pass, as do formatting and Clippy. No custom tests were added. Behavior
follows PHP 8.5.0 `ext/standard/string.c`, native signatures, and upstream fixtures.
Complete native reflection metadata and diagnostic interleaving/rendering remain gaps.

The [string replacement slice](conformance/php-8.5.0-string-replace-hillclimb.txt) adds
**10 upstream passes**: eight standard string fixtures and two Zend fixtures, including
enum `var_export`, with no regressions across Zend, reflection, and SPL. `str_replace`
and `str_ireplace` accept scalar/string or array searches, replacements, and subjects.
Search arrays apply sequentially; replacement arrays follow insertion order, with empty
replacements after exhaustion. Subject arrays preserve their keys and produce string
values. Matching is binary-safe and non-overlapping; insensitive matching folds ASCII.

Array elements convert when reached, preserving references and Stringable callback order.
The optional count reference receives the total after successful processing, through the
existing typed-reference checks. Empty subjects avoid element conversion; empty searches
consume their matching replacement entry without changing the subject. Scalar search with
array replacement raises PHP's argument error after normal argument conversion.

Full Zend now records **1649 / 3199 / 408**; reflection remains **244 / 211 / 37**, and SPL
**18 / 666 / 94**. The related 27-fixture string replacement group records **8 / 14 / 5**.
Two INI-bearing upstream fixtures (`str_replace_array_refs` and `str_replace_variation2`)
were also executed directly and match reference output/exit without diagnostics; they
remain unsupported in the runner's counts. Other related failures require substr/substring
replacement, serialization, streams, INI handling, deprecated interpolation, or diagnostic
matching. All 797 existing Rust tests and 57 reference-checked project fixtures pass;
formatting and Clippy pass. No custom tests were added. PHP 8.5.0 `ext/standard/string.c`,
reference assignment helpers, and upstream fixtures establish the implemented behavior.

The [substring slice](conformance/php-8.5.0-substrings-hillclimb.txt) adds **14 upstream
passes**: ten in the full standard string suite and four Zend fixtures, with no
regressions across strings, Zend, reflection, and SPL. `substr` extracts byte ranges;
`substr_replace` replaces ranges or inserts at clamped offsets. Both handle negative
bounds, null lengths, offsets beyond the string, and the minimum signed integer without
arithmetic overflow. Substring replacement accepts arrays of subjects, replacement
values, offsets, and lengths, preserving subject keys and consuming other arrays in
insertion order. Missing entries use PHP's defaults, including zero offsets, remaining
lengths, and empty replacements. Scalar subjects reject array offset/length arguments.

Array element conversions occur in subject/offset/length/replacement order. Integer
conversion of elements uses cast semantics even in strict mode; top-level arguments use
native parameter coercion and report nullable/union types in errors. Existing native
integer conversion now shares a helper that accepts the declared diagnostic type.

Full Zend reaches **1653 / 3195 / 408**. The full standard string suite improves from
**96 / 482 / 152** to **106 / 472 / 152** across 730 fixtures; reflection remains
**244 / 211 / 37**, and SPL **18 / 666 / 94**. All 797 existing Rust tests and 57
reference-checked project fixtures pass, as do formatting and Clippy. No custom tests
were added. PHP 8.5.0 `ext/standard/string.c`, native signatures, and upstream fixtures
establish these rules. Other string APIs, complete native reflection metadata, INI
support, and diagnostic matching remain unfinished.

The [string search slice](conformance/php-8.5.0-string-search-hillclimb.txt) adds
**61 upstream passes**: 58 standard string fixtures and three Zend fixtures, with no
regressions across strings, Zend, reflection, and SPL. It implements `strpos`, `stripos`,
`strrpos`, `strripos`, `strstr`, `strchr`, `stristr`, `strrchr`, `str_contains`,
`str_starts_with`, and `str_ends_with`. Searches operate on bytes, preserve original
substring contents, and use ASCII folding for insensitive matches. Position searches
accept positive and negative offsets, validate integer limits, and retain distinct
forward/reverse offset rules. A negative reverse offset bounds the match's starting
position while allowing a multi-byte match to extend beyond it.

Empty needles follow each API's rules; `strrchr` searches for the needle's first byte,
including NUL for an empty needle. Suffix APIs support `before_needle`, and `strchr`
retains its alias name for diagnostics. Parameter metadata carries offset zero and
before-needle false defaults. Conversion and argument binding use existing native rules.

The full string suite improves to **164 / 414 / 152** across 730 fixtures. Zend reaches
**1656 / 3192 / 408**; reflection remains **244 / 211 / 37**, and SPL **18 / 666 / 94**.
All 797 existing Rust tests and 57 reference-checked project fixtures pass; formatting
and Clippy pass. No custom tests were added. PHP 8.5.0 `ext/standard/string.c`, native
signatures, and upstream fixtures establish these behaviors. Other string APIs, complete
native reflection metadata, INI support, and diagnostic matching remain unfinished.

The [string comparison/span/count slice](conformance/php-8.5.0-string-measure-hillclimb.txt)
adds **47 upstream passes**: 44 standard string fixtures and three Zend fixtures, with
no regressions across strings, Zend, reflection, and SPL. It implements `strcmp`,
`strncmp`, `strcasecmp`, `strncasecmp`, `substr_compare`, `strspn`, `strcspn`, and
`substr_count`. Comparisons preserve binary byte ordering, ASCII folding, bounded
lengths, and length ordering when common bytes match. Substring comparison preserves
PHP's zero-length early return, negative-offset clamping, and invalid-bound errors.

Character spans use literal byte membership rather than trim-style character ranges;
their offsets and lengths clamp like substring extraction. Substring counting instead
rejects out-of-range offsets/lengths and empty needles, and counts non-overlapping
matches. All argument conversions occur before those semantic checks. Nullable lengths,
named parameters, and optional defaults use the existing native binding machinery.

The full string suite reaches **208 / 370 / 152**, and Zend **1659 / 3189 / 408**.
Reflection remains **244 / 211 / 37**, and SPL **18 / 666 / 94**. All 797 existing Rust
tests and 57 reference-checked project fixtures pass; formatting and Clippy pass. No
custom tests were added. PHP 8.5.0 string functions, Zend binary comparison helpers,
native signatures, and upstream fixtures establish these behaviors. Locale-sensitive
substring comparison beyond the default C locale, other string APIs, complete native
reflection metadata, INI support, and diagnostic matching remain unfinished.

The [string split/join slice](conformance/php-8.5.0-string-split-hillclimb.txt) adds
**29 upstream passes**: 26 standard string fixtures and three Zend fixtures, with no
regressions across strings, Zend, reflection, and SPL. `explode` supports binary
separators, positive limits with a final remainder, zero-as-one, negative tail removal,
and empty-input rules. `str_split` returns fixed byte chunks; `chunk_split` appends a
separator after each chunk, including the empty-input compatibility case. Non-positive
chunk lengths and empty explode separators raise PHP argument errors after coercion.

`implode` and `join` retain insertion order, convert array elements when reached, preserve
references, and join converted strings without changing source elements. Bound calls now
record whether arguments were supplied directly and positionally. This preserves PHP's
specialized `implode` argument checks while leaving its alias, named/unpacked calls, and
callbacks on the general signature path. A targeted reference probe confirms the distinct
results for direct/dynamic calls and the alias, named, and unpacked forms.

The full string suite reaches **234 / 344 / 152**, and Zend **1662 / 3186 / 408**.
Reflection remains **244 / 211 / 37**, and SPL **18 / 666 / 94**. All 797 existing Rust
tests and 57 reference-checked project fixtures pass; formatting and Clippy pass. No
custom tests were added. PHP 8.5.0 `ext/standard/string.c`, native signatures, upstream
fixtures, and the targeted probe establish these behaviors. Complete native reflection
metadata and frameless-call behavior, INI configuration, streams, other string APIs, and
diagnostic matching remain unfinished.

The full suite rows below include these changes. Subset rows retain the historical
measurements reported by their individual slices.

| Suite | Passed | Failed | Unsupported |
| --- | ---: | ---: | ---: |
| Project `tests/phpt`, Rust and reference PHP | 57 | 0 | 0 |
| PHP 8.5.0 `ext/standard/tests/general_functions/print_r*.phpt`, Rust and reference PHP for passing cases | 5 | 1 | 7 |
| PHP 8.5.0 all `Zend/tests`, Rust and reference PHP for passing cases | 1662 | 3186 | 408 |
| PHP 8.5.0 `ext/reflection/tests`, Rust and reference PHP for passing cases | 244 | 211 | 37 |
| PHP 8.5.0 all `ext/spl/tests`, Rust and reference PHP for passing cases | 18 | 666 | 94 |
| PHP 8.5.0 `ext/standard/tests/{general_functions,array}/var_export*.phpt`, Rust and reference PHP for passing cases | 11 | 2 | 6 |
| PHP 8.5.0 all `ext/standard/tests/strings`, Rust and reference PHP for passing cases | 234 | 344 | 152 |
| PHP 8.5.0 related string replacement fixtures (27), Rust and reference PHP for passing cases | 8 | 14 | 5 |
| PHP 8.5.0 `ext/standard/tests/strings/{strtolower,strtoupper,trim,ltrim,rtrim,chop,bin2hex}*.phpt`, Rust and reference PHP for passing cases | 16 | 3 | 0 |
| PHP 8.5.0 `ext/standard/tests/strings/{printf,sprintf,vprintf,vsprintf}*.phpt`, Rust and reference PHP for passing cases | 51 | 10 | 40 |
| PHP 8.5.0 `ext/standard/tests/class_object/get_class_{methods,vars}*.phpt`, Rust and reference PHP for passing cases | 5 | 1 | 0 |
| PHP 8.5.0 `ext/standard/tests/general_functions/is_callable*.phpt`, Rust and reference PHP for passing cases | 4 | 3 | 0 |
| PHP 8.5.0 `Zend/tests/backtrace`, Rust and reference PHP for passing cases | 12 | 4 | 4 |
| PHP 8.5.0 `Zend/tests/function_arguments/sensitive_parameter*.phpt`, Rust and reference PHP for passing cases | 12 | 6 | 0 |
| PHP 8.5.0 `tests/basic`, Rust | 6 | 5 | 99 |
| PHP 8.5.0 `tests/lang`, Rust and reference PHP for passing cases | 71 | 96 | 127 |
| PHP 8.5.0 `Zend/tests/match`, Rust and reference PHP for passing cases | 17 | 5 | 13 |
| PHP 8.5.0 `Zend/tests/list`, Rust and reference PHP for passing cases | 12 | 16 | 30 |
| PHP 8.5.0 `Zend/tests/array_unpack`, Rust and reference PHP for passing cases | 2 | 5 | 6 |
| PHP 8.5.0 `Zend/tests/return_types`, Rust and reference PHP for passing cases | 11 | 7 | 71 |
| PHP 8.5.0 `Zend/tests/magic_methods`, Rust and reference PHP for passing cases | 16 | 70 | 71 |
| PHP 8.5.0 `Zend/tests/offsets`, Rust | 0 | 10 | 10 |
| PHP 8.5.0 `Zend/tests/numeric_strings`, Rust and reference PHP for passing cases | 1 | 3 | 4 |
| PHP 8.5.0 `Zend/tests/nullsafe_operator`, Rust and reference PHP for passing cases | 15 | 6 | 26 |
| PHP 8.5.0 `Zend/tests/static_variables`, Rust and reference PHP for passing cases | 7 | 5 | 1 |
| PHP 8.5.0 `Zend/tests/restrict_globals`, Rust and reference PHP for passing cases | 2 | 1 | 13 |
| PHP 8.5.0 `Zend/tests/jump`, Rust and reference PHP for passing cases | 6 | 6 | 5 |
| PHP 8.5.0 `Zend/tests/exit`, Rust and reference PHP for passing cases | 3 | 2 | 22 |
| PHP 8.5.0 `Zend/tests/pipe_operator`, Rust and reference PHP for passing cases | 10 | 9 | 12 |
| PHP 8.5.0 `Zend/tests/first_class_callable`, Rust and reference PHP for passing cases | 8 | 13 | 44 |
| PHP 8.5.0 `Zend/tests/closures`, Rust and reference PHP for passing cases | 28 | 48 | 59 |
| PHP 8.5.0 `Zend/tests/arrow_functions`, Rust and reference PHP for passing cases | 2 | 2 | 5 |
| PHP 8.5.0 selected `ext/standard/tests/array` map/filter/reduce cases, Rust and reference PHP for passing cases | 12 | 21 | 6 |
| PHP 8.5.0 `tests/classes`, Rust and reference PHP for passing cases | 56 | 69 | 158 |
| PHP 8.5.0 `Zend/tests/inheritance`, Rust and reference PHP for passing cases | 24 | 10 | 36 |
| PHP 8.5.0 `Zend/tests/lsb`, Rust and reference PHP for passing cases | 11 | 3 | 11 |
| PHP 8.5.0 `Zend/tests/constants`, Rust and reference PHP for passing cases | 35 | 21 | 39 |
| PHP 8.5.0 `Zend/tests/type_declarations/typed_class_constants*.phpt`, Rust and reference PHP for passing cases | 4 | 11 | 14 |
| PHP 8.5.0 `Zend/tests/type_declarations/typed_properties*.phpt`, Rust and reference PHP for passing cases | 17 | 59 | 44 |
| PHP 8.5.0 `Zend/tests/readonly_props`, Rust and reference PHP for passing cases | 7 | 15 | 18 |
| PHP 8.5.0 `Zend/tests/readonly_classes`, Rust and reference PHP for passing cases | 4 | 3 | 16 |
| PHP 8.5.0 `Zend/tests/ctor_promotion`, Rust and reference PHP for passing cases | 1 | 6 | 11 |
| PHP 8.5.0 `Zend/tests/clone`, Rust and reference PHP for passing cases | 6 | 6 | 14 |
| PHP 8.5.0 `Zend/tests/asymmetric_visibility`, Rust (no passing fixtures) | 0 | 21 | 18 |
| PHP 8.5.0 `Zend/tests/exceptions`, Rust and reference PHP for passing cases | 8 | 32 | 54 |
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
and truncation, and the other exception gaps above
remain incomplete. This supersedes the earlier stack-trace limitation.


The Stringable slice adds the native Stringable interface, automatic membership
for `__toString()` declarations, magic signature/return validation, and runtime
object-to-string conversion. Echo, print, interpolation, casts, concatenation,
dynamic names, eval/include arguments, weak string arguments/returns/properties,
native string parameters, and exit use the conversion path. Comparison now runs
inside the runtime so nested arrays/objects, switch cases, and non-strict
`array_keys` searches can invoke PHP methods. Accepted object/callable union
members take precedence over string coercion; strict string parameters reject
objects. Argument conversion preserves thrown exceptions, while return conversion
raises a TypeError with the original exception as previous. Conversion traces
retain user-declaration/return sites and internal-call distinctions.
Native Exception/Error `__toString()` includes saved origins and traces, formats
previous exceptions oldest first, supports subclass overrides and explicit parent
calls, and terminates cycles without invoking previous objects' overrides.
Seventeen regressions, one new PHPT, and a **306-case differential matrix** cover
these behaviors. All **726 Rust tests** and **54 reference-checked project PHPTs**
pass, along with the previous 220 trace and 286 exception differential cases.
Refreshed tracked suites gain four passes and lose none; the new
[magic-method report](conformance/php-8.5.0-magic_methods.txt) records 16 passes,
70 failures, and 71 unsupported cases. Most magic methods remain unfinished.
The [magic-method manual](https://www.php.net/manual/en/language.oop5.magic.php),
[Exception string conversion](https://www.php.net/manual/en/exception.tostring.php),
PHP 8.5.0 Zend source, and local probes establish these rules. Object conversion
in string-offset assignments (including reentrant writes), Throwable property
layouts, complete built-in coverage, exact diagnostics, and the previously noted
configuration/reflection/trait gaps remain incomplete. This supersedes earlier
notes listing Throwable string conversion as unsupported.


The string-offset conversion slice introduces shared PHP string buffers with
independent value slots and copy-on-write separation. This allows `__toString()`
to run between destination preparation and the final byte write without holding
Rust borrows across PHP execution. Invalid offsets and access failures precede
conversion; replacements, nested writes, exception side effects, copies created
during conversion, and extension preserve the reference engine's buffer lifetime
rules. In-bounds writes resume at the surviving destination slot, while extension
restores the retained original buffer. Literal, constant, property-default, and
array copies continue to separate for ordinary writes. The Rust embedding API
now uses `Value::String(PhpString)`, constructed from byte vectors with `.into()`;
`bytes()` and `to_vec()` return owned snapshots.
Eleven regressions and one new PHPT pass, together with all **737 Rust tests** and
**55 reference-checked project PHPT fixtures**. The reproducible
[string-offset differential report](conformance/php-8.5.0-string-offset-conversion.txt)
records **525 cases: 510 matched, zero mismatches, and 15 reference-process
crashes**. The crashes reproduce in the local PHP 8.5.0 build, including cases
that first bind a destination global during conversion; they are excluded from
compatibility pass counts. Equivalent programs with pre-established global
bindings are included separately. The Rust runtime does not crash in those
cases. Run `python3 tools/check_string_offsets.py --reference /path/to/php` after
building to regenerate the comparison. Prior Stringable, throwable-trace, and
static-asymmetric-visibility matrices also pass (306, 220, and 827 cases).
Refreshed tracked suites lose no passes. New [offset](conformance/php-8.5.0-offsets.txt)
and [numeric-string](conformance/php-8.5.0-numeric_strings.txt) reports retain their
unsupported sections and language gaps explicitly. The
[PHP string manual](https://www.php.net/manual/en/language.types.string.php),
PHP 8.5.0 `zend_assign_to_string_offset` implementation, and local probes establish
the tested rules. Reference operations, error-handler reentrancy, and the other
previously documented gaps remain incomplete; crashes or undefined behavior in
the C reference engine are not compatibility targets. This supersedes the earlier
string-offset object-conversion limitation.


The native-exception slice adds `ErrorException` and all 13 SPL exception
classes with their PHP 8.5 inheritance. `ErrorException::__construct()` accepts
message, code, severity, nullable filename/line, and previous Throwable through
positional or named arguments. A supplied filename with a null line resets the
line to zero; otherwise null location arguments preserve creation metadata.
Signed line values survive getters, string conversion, and uncaught diagnostics.
The final `getSeverity()` getter defaults to `E_ERROR`, including subclasses
whose constructors skip the parent. Native constructors now preserve an omitted
message, a zero code, or a null previous exception on repeated calls; named
arguments fill earlier omitted positions with their defaults. Argument conversion
precedes native mutation, and argument-count/type errors retain the declaring
method and boolean value names.

All **756 Rust tests**, including **19 new regressions**, and **56 project PHPT
fixtures** pass; the latter also match the PHP 8.5.0 reference. The reproducible
[native-exception comparison](conformance/php-8.5.0-native-exceptions.txt) matches
**440 cases with zero mismatches**, including weak/strict scalar conversion,
Stringable inputs, invalid arguments, reentry, subclass construction, inherited
methods, and signed integer limits. Previous exception, Stringable, and trace
comparisons also match (286, 306, and 220 cases). Refreshed exceptions,
inheritance, classes, return-types, and magic-method suites lose no passing
fixtures. Formatting and Clippy checks pass.

The [ErrorException manual](https://www.php.net/manual/en/class.errorexception.php),
PHP 8.5.0 `Zend/zend_exceptions.c` and `ext/spl/spl_exceptions.stub.php`, and local
reference probes establish these behaviors. Throwable metadata remains separate
from ordinary object properties: protected exception-property access, severity
property redeclarations, property hooks, reflection/serialization, and error
handler APIs remain incomplete. This supersedes earlier notes listing the
ErrorException and SPL exception classes as wholly unsupported.


The nullsafe-operator slice adds `?->` property and method access with a distinct
short-circuit state, so a skipped receiver differs from a method that returns
null. Member chains propagate the short circuit through parentheses, offsets,
static properties, static calls, and static callable creation. Dynamic function
calls, class constants, and `::class` end the chain. Names, keys, and arguments
retain independent chains and execute only when their containing operation is
reached. Quiet reads preserve PHP's different rules for `isset`, `empty`, and
coalescing across instance/static access. Arrow captures, interpolation, and
constant/default expressions recognize nullsafe properties. Write contexts and
instance callable creation reject nullsafe chains before execution. Dynamic
method names require strings; constant non-string method names are rejected
after expression folding, leaving pruned branches undiagnosed.

All **780 Rust tests**, including **24 new regressions**, and **57 project PHPT
fixtures** pass; project fixtures also match PHP 8.5.0. The reproducible
[nullsafe comparison](conformance/php-8.5.0-nullsafe-differential.txt) matches
**408 cases with zero mismatches**. Prior Stringable, method-visibility, and
native-exception matrices also match (306, 863, and 440 cases). The new
[upstream nullsafe report](conformance/php-8.5.0-nullsafe_operator.txt) records
**14 passed, 7 failed, 26 unsupported**. Remaining executable failures involve
references/by-reference parameters and returns, object casts, and error handlers.
Refreshed classes, closures, first-class-callable, and constants reports lose no
previously recorded passes; the latter two now record 8 and 35 passes.
Formatting and Clippy checks pass.

The [nullsafe RFC](https://wiki.php.net/rfc/nullsafe_operator), PHP 8.5.0 upstream
fixtures, and reference probes establish the tested chain boundaries. General
reference support, enum/property-hook integration, complete constant-expression
coverage, and exact compile-fatal diagnostics remain subject to the previously
recorded implementation gaps.


The object-cast slice was measured against all **5,256 upstream Zend fixtures**,
using an isolated build of parent commit `7f7f490` as the baseline. Results improve
from **660 passed / 1,834 failed / 2,762 unsupported** to **669 passed / 1,825 failed /
2,762 unsupported**: **9 new passes and zero regressions**. Passing cases also
match the PHP 8.5.0 reference, including exit status. The
[comparison report](conformance/php-8.5.0-object-casts-hillclimb.txt) lists each new
pass and the leading failure/unsupported categories. `tools/compare_phpt.py`
compares runner logs and exits unsuccessfully if a previously passing case
regresses. The largest runner gap is `EXPECTF`: 2,354 fixtures require only that
additional section, with further fixtures combining it with INI/other sections.

Object casts preserve existing object/closure identity, create empty stdClass
instances from null, wrap scalar values in a `scalar` property, and convert array
keys to string property names without recursively converting values. Array and
string writes retain copy-on-write behavior; nested object handles remain shared.
Global constants, parameter defaults, and static-variable initializers accept
object casts, while class constants and property initializers reject them.
Encoded property names survive array roundtrips and are decoded for iteration
and object dumps, with notices for malformed names. The behavior is grounded in
the [PHP object manual](https://www.php.net/manual/en/language.types.object.php),
PHP 8.5.0 `convert_to_object`/`zend_unmangle_property_name_ex`, and upstream tests.

All **797 Rust tests** and **57 reference-checked project PHPT fixtures** pass;
formatting and Clippy pass. The object-cast Rust cases were written before the
workflow changed to prioritize upstream tests. No new project PHPT fixtures or
generated differential matrix were added for this slice. Exact object-handle
reuse, general references, full property visibility, and diagnostic formatting
remain incomplete. Upstream unsupported cases remain explicitly uncounted as
passes.
