# Conformance baseline

Measurements through 2026-11-02 with the PHP **8.5.0** release source and a locally built
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

The [string translation/escaping slice](conformance/php-8.5.0-string-transform-hillclimb.txt)
adds **24 upstream string passes**, with no regressions across strings, Zend, reflection,
and SPL. `strtr` supports byte translation tables and longest-match replacement maps.
Translation tables stop at the shorter mapping string, with later duplicate source bytes
winning. Replacement maps use decimal numeric keys, skip empty keys with warnings, and
do not rescan replacement text. The single-entry path converts its value once even when
it does not match; larger maps convert values on each match and preserve live references.
Empty input bypasses replacement-map processing after argument validation.

`addslashes`/`stripslashes` support quotes, backslashes, NUL, and PHP's ordinary slash
removal rules. `addcslashes` reuses binary character masks and range validation, emits
named control escapes or three-digit octal escapes, and preserves unselected bytes.
`stripcslashes` decodes controls, up to two hex digits or three octal digits, unknown
escapes, and byte truncation. Ordinary stripping removes a trailing backslash; C-style
stripping retains it. Empty addcslashes input bypasses character-mask warnings.

The full string suite reaches **258 / 320 / 152**. Zend remains **1662 / 3186 / 408**,
reflection **244 / 211 / 37**, and SPL **18 / 666 / 94**. All 797 existing Rust tests and
57 reference-checked project fixtures pass; formatting and Clippy pass. No custom tests
were added. PHP 8.5.0 `ext/standard/string.c` and upstream fixtures establish the behavior.
The remaining focused failures need streams or diagnostic-output matching; other string
APIs, native reflection metadata, and INI configuration remain incomplete.

The [tag-stripping slice](conformance/php-8.5.0-strip-tags-hillclimb.txt) adds
**26 upstream passes**: 25 standard string fixtures and one Zend fixture, with no
regressions across strings, Zend, reflection, and SPL. `strip_tags` follows PHP's text,
HTML tag, PHP, declaration, and comment states, preserving its handling of nested angle
brackets, quoted text, PHP parentheses, XML/doctype transitions, malformed tags, and NUL
bytes. Whitespace recognition includes vertical tabs as required by PHP's C parser.

Allowed tags accept null, a string, or an array whose values convert in insertion order.
Matching normalizes tag names and closing/self-closing syntax, retains allowed tag text,
and follows the allowed-list NUL boundary. Nullable union argument errors share the native
coercion helper without changing its existing callers. Parsing begins only after allowed
array elements have been converted, including when the input string is empty.

The full string suite reaches **283 / 295 / 152**, and Zend **1663 / 3185 / 408**.
Reflection remains **244 / 211 / 37**, and SPL **18 / 666 / 94**. All 797 existing Rust
tests and 57 reference-checked project fixtures pass; formatting and Clippy pass. No
custom tests were added. PHP 8.5.0 `php_strip_tags_ex`/`php_tag_find`, native signatures,
and upstream fixtures establish these behaviors. Remaining related fixtures require
other APIs; native reflection metadata, INI configuration, streams, broader string APIs,
and diagnostic matching remain incomplete.

The [byte-string helper slice](conformance/php-8.5.0-byte-helpers-hillclimb.txt) adds
**17 upstream passes**: 16 standard string fixtures and one Zend fixture, with no
regressions across strings, Zend, reflection, and SPL. It implements `hex2bin`, `ucfirst`,
`lcfirst`, `ucwords`, `nl2br`, `count_chars`, `strpbrk`, and `str_pad`, plus the three
`STR_PAD_*` constants with standard-extension metadata. Hex decoding rejects odd lengths
and invalid digits with warnings/false. Case conversion is ASCII-only; word separators
use PHP character masks, and each separator check observes earlier capitalization.
Newline insertion preserves original CR/LF bytes and treats opposite newline pairs as one.

Character counts return ordered byte counts or byte sets according to mode; character-set
search returns the first matching suffix. Padding supports repeated byte patterns on the
left, right, or both sides, assigning an odd extra byte to the right and restarting the
pattern on each side. Parameter conversion precedes the no-padding return; empty patterns
and invalid pad modes are checked only when padding is needed.

The full string suite reaches **299 / 279 / 152**, and Zend **1664 / 3184 / 408**.
Reflection remains **244 / 211 / 37**, and SPL **18 / 666 / 94**. All 797 existing Rust
tests and 57 reference-checked project fixtures pass; formatting and Clippy pass. The two
upstream padding fixtures requesting `precision=14` also match PHP output/exit without
diagnostics when executed directly; they remain unsupported in runner counts. Direct
execution of the INI-bearing ucfirst/lcfirst fixtures remains blocked by deprecated
interpolation syntax. No custom tests were added. PHP 8.5.0 string implementations,
signatures, constants, and upstream fixtures establish these rules. Diagnostic matching,
INI/memory settings, deprecated interpolation, other APIs, and complete native reflection
metadata remain unfinished.

The [diagnostic display slice](conformance/php-8.5.0-diagnostics-hillclimb.txt) adds
**173 upstream passes**: 134 Zend, 21 reflection, 17 standard string, and one SPL
fixture, with no regressions. Diagnostics now record their source filename and byte
position in ordinary output at emission time, after error-reporting filtering.
Warnings, notices, and deprecations render with PHP's labels and source locations;
function, include, eval, parameter-conversion, and reflected-attribute source contexts
restore together. Compile diagnostics retain the compiled file's identity.

The public Rust output and warning vectors remain separate. The CLI writes output
and diagnostics to their respective streams in recorded order, flushing at boundaries.
The PHPT harness interleaves displayed diagnostics with output and matches EXPECT,
EXPECTF, or EXPECTREGEX. Reference checks use `display_errors=1` and `log_errors=0`,
requiring matching expectations and process status. This replaces the previous blanket
rejection of nonfatal diagnostics; unexpected warnings still fail output comparisons.
Buffered print/export output and direct dumps retain their different emission ordering,
including diagnostic positions relocated from temporary closure-dump buffers.

Full suites reach **1798 / 3050 / 408** for Zend, **265 / 190 / 37** for reflection,
**19 / 665 / 94** for SPL, and **316 / 262 / 152** for standard strings. All 797 existing
Rust tests and 57 reference-checked project fixtures pass; formatting and Clippy pass.
No custom tests were added. Existing upstream fixtures drive validation of diagnostic
text, ordering, source locations, suppression, and reference behavior. These are gains
in measured conformance from rendering existing diagnostics, not newly implemented
language features. Fatal/uncaught diagnostic rendering, error handlers, configuration,
and some diagnostic text/ordering edge cases remain incomplete; fatal errors still
fail in the harness. Unsupported fixture counts are unchanged.

The [uncaught-error display slice](conformance/php-8.5.0-fatal-hillclimb.txt) adds
**238 upstream passes**: 226 Zend, nine reflection, and three SPL fixtures, with no
regressions across the four full suites. The CLI and PHPT harness explicitly report
execution errors at the request boundary. Ordinary execution APIs continue returning
errors without implicitly reporting them. Uncaught errors render exception text,
previous-exception chains, captured stack traces, and the final thrown location.
Exact internal ParseError and CompileError classes use PHP's separate parse/fatal forms;
subclasses follow normal throwable rendering. Preparation failures retain file identity.

Reporting invokes custom exception `__toString()` methods through an internal call
frame. Their output precedes fatal display, a nested throw becomes the new uncaught
error, and `exit()` preserves its requested status and stops reporting. Display filtering
uses the request's error-reporting mask after conversion; suppressing the fatal display
does not suppress the method's side effects. Diagnostic formatting preserves byte data,
and the public warning vector continues to contain nonfatal messages only.

Expected fatal fixtures can pass when rendered output matches EXPECT/EXPECTF/EXPECTREGEX
and PHP's process status agrees. Two class-alias fixtures that matched Rust's output
still fail reference validation because PHP 8.5.0 emits additional stack traces absent
from their expectations. Unsupported cases remain uncounted as passes.

Full suites reach **2024 / 2824 / 408** for Zend, **274 / 181 / 37** for reflection,
**22 / 662 / 94** for SPL, and **316 / 262 / 152** for standard strings. All 797 existing
Rust tests and 57 reference-checked project fixtures pass; formatting and Clippy pass.
No new tests or generated matrices were added. One existing CLI assertion now expects
PHP's `Parse error:` label. Upstream fixtures, PHP 8.5.0 exception-reporting code, and
short reference probes establish the behavior. Remaining gaps include compile-fatal
classification/catchability, parser wording, Throwable property synchronization,
exception/error handlers, shutdown/destructor behavior, and configurable diagnostics.

The [deprecated interpolation slice](conformance/php-8.5.0-interpolation-hillclimb.txt)
adds **six upstream Zend passes** with no regressions across Zend, reflection, SPL,
and standard strings. `${name}` and `${name[index]}` use named variables, while
`${expression}` evaluates the expression to select a variable name. Immediate lexical
boundaries distinguish the forms, including whitespace and constants. Named-array syntax
retains PHP's single-offset restriction. Expression forms reuse ordinary dynamic-variable
parsing, preserving namespace resolution, constant names, calls, and nested strings.

Interpolation parts retain their syntax category and source lines until compilation.
The compiler emits PHP's two distinct deprecations before compiling each affected part,
including uncalled function bodies, while constant short-circuit pruning suppresses
warnings from discarded expressions. Scope reservation, capture analysis, validation,
and runtime evaluation traverse the revised interpolation representation.

Heredoc scanning skips interpolated PHP expressions when searching for the closing label.
Indentation is removed from string text while PHP source inside expressions retains its
own indentation. This supports nested heredocs sharing the same delimiter and delimiter
names used as constants inside multiline interpolation; all four upstream complex
flexible-heredoc fixtures now pass. Escaped interpolation markers and nowdocs retain
literal scanning.

Zend reaches **2030 / 2818 / 408**; reflection remains **274 / 181 / 37**, SPL
**22 / 662 / 94**, and standard strings **316 / 262 / 152**. All 797 existing Rust tests
and 57 reference-checked project fixtures pass; formatting and Clippy pass. No custom
tests or generated matrices were added. The upstream ucfirst/lcfirst FILE programs also
match reference output, diagnostics, and exit status with PHP precision=14; their INI
sections remain unsupported and those results are not counted as runner passes.
PHP 8.5.0 scanner/parser/compiler code, upstream fixtures, and short reference probes
establish these behaviors. Octal-escape overflow warnings, full string-expression
ordering, some diagnostic ordering/location edge cases, INI settings, and error handlers
remain incomplete.

The [array slicing/construction slice](conformance/php-8.5.0-array-segments-hillclimb.txt)
adds **67 upstream passes**: 61 standard array fixtures and six Zend fixtures, with no
regressions across arrays, Zend, reflection, SPL, and standard strings. The newly measured
full array suite starts at **75 / 713 / 52** and reaches **136 / 652 / 52**.

`array_slice` clamps positive/negative offsets and nullable lengths by insertion order,
retaining string keys and optionally integer keys. `array_reverse` reverses insertion
order with the same key-preservation choice. `array_chunk` validates positive lengths,
splits in insertion order, and either preserves every key or renumbers every chunk.
These copies preserve shared references while dropping wrappers with no other owner,
matching PHP's copy helper. Existing argument coercion and named-parameter metadata
apply before empty/no-op returns.

`array_fill` validates count and index overflow and creates consecutive keys, including
negative starting indexes. `array_fill_keys` converts non-integer keys through ordinary
PHP string conversion, including Stringable objects and array-conversion warnings, then
applies numeric-string key rules. Duplicate keys retain insertion position. `array_pad`
returns the original array value when no padding is needed; otherwise it pads left or
right, preserves string keys, renumbers integer keys, and retains element references.
Repeated arrays retain copy-on-write behavior and repeated objects retain handles.
Array construction reserves capacity fallibly and rejects capacities beyond PHP's
64-bit hash-table limit before requesting storage.

Zend reaches **2036 / 2812 / 408**. Reflection remains **274 / 181 / 37**, SPL
**22 / 662 / 94**, and standard strings **316 / 262 / 152**. All 797 existing Rust tests
and 57 reference-checked project fixtures pass; formatting and Clippy pass. No new custom
tests or generated matrices were added. PHP 8.5.0 array implementations and copy helpers,
upstream fixtures, and short reference probes establish these behaviors. Remaining
related failures require streams, range, and internal array-pointer functions. Exact
allocation-fatal formatting/catchability, configurable memory limits, complete native
reflection metadata, and further array APIs remain unfinished. Unsupported fixture
counts are unchanged.

The [range slice](conformance/php-8.5.0-range-hillclimb.txt) adds **19 upstream passes**:
15 standard array fixtures, three Zend fixtures, and one standard string fixture, with
no regressions across arrays, Zend, reflection, SPL, and strings. `range` supports integer,
floating-point, and binary character sequences, including decreasing ranges, equal
endpoints, and non-dividing steps. Native union conversion preserves numeric strings for
endpoint interpretation, accepts weak bool/null and Stringable endpoints where PHP does,
and applies strict types. The step is converted and validated before endpoint processing.

Single-byte digit strings retain PHP's numeric/character ambiguity. Empty strings,
multibyte nonnumeric strings, mixed numeric/character endpoints, and fractional character
steps emit the corresponding warnings and use PHP's fallback numeric values. Integral
floating steps can produce integer or character sequences; fractional/out-of-integer-range
steps select floating-point generation. Nonfinite endpoints/steps, zero steps, negative
steps for increasing ranges, and steps wider than unequal endpoints produce ValueErrors.

Integer distance/count calculations avoid signed overflow. Float values derive from the
original endpoint plus/minus each index times the step, preserving PHP's rounding rather
than accumulating increments. Range-size checks precede allocation and include PHP's
reported excess/count bounds. Parameter metadata includes the default step of one.

Zend reaches **2039 / 2809 / 408**, standard arrays **151 / 637 / 52**, and strings
**317 / 261 / 152**. Reflection remains **274 / 181 / 37** and SPL **22 / 662 / 94**.
All 797 existing Rust tests and 57 reference-checked project fixtures pass; formatting and
Clippy pass. No new custom tests or generated matrices were added. Five additional upstream
INI-bearing range programs match reference output/exit when run directly with their PHP
settings. Two others differ only in float output under unsupported serialize_precision=14;
both match PHP at the runtime's default serialize_precision=-1. They remain unsupported
and are not counted as passes. PHP 8.5.0 range and native argument-conversion code, upstream
fixtures, and direct reference checks establish these behaviors. Remaining executable
range fixtures require pow(); INI precision settings, native reflection type completeness,
error handlers, and memory-limit/fatal-allocation behavior remain unfinished.

The [core math slice](conformance/php-8.5.0-math-hillclimb.txt) adds **15 upstream passes**:
eight standard math fixtures, four Zend fixtures, and three array fixtures. No previously
passing cases regress across Zend, reflection, SPL, strings, arrays, or the newly measured
full math suite. `abs`, `ceil`, and `floor` preserve PHP's int|float conversion rules;
`sqrt`, `fmod`, `fdiv`, `fpow`, `is_finite`, `is_infinite`, and `is_nan` use native float
conversion, including strict integer-to-float widening and weak null deprecations.
`intdiv` truncates toward zero and raises the native zero-divisor and minimum-integer
overflow exceptions. IEEE operations retain infinities, NaN, and signed zero.

`pow` and `**` share operator conversions and integer exponentiation with promotion at
the same multiplication step as PHP. They emit PHP 8.5's zero-base/negative-exponent
deprecation; `fpow` retains its distinct warning-free float behavior. This also fixes
the remaining executable range-size fixtures. Native function names and parameter names
support callbacks and named calls. Complete reflection type metadata is still pending.

Zend reaches **2043 / 2805 / 408**, arrays **154 / 634 / 52**, and math **10 / 44 / 117**.
Reflection, SPL, and strings retain their previous totals. All 797 existing Rust tests,
57 reference-checked project fixtures, formatting, and Clippy pass. No custom tests or
generated matrices were added. Nine existing INI-bearing upstream math programs also
match reference output and exit status when run directly with their requested precision
settings; these remain unsupported in the harness and are not counted as passes.
PHP 8.5.0 math implementations, operator code, stubs, and upstream fixtures establish
the behavior. Rounding modes, number formatting, base conversion, trigonometric and
logarithmic functions, streams, configurable precision, and extension object arithmetic
remain unfinished.

The [base conversion slice](conformance/php-8.5.0-base-conversion-hillclimb.txt) adds
**10 upstream passes**: eight math fixtures and two string fixtures, with no regressions
across the six measured upstream suites. `base_convert`, `bindec`, `octdec`, and `hexdec`
trim C whitespace, recognize the appropriate binary/octal/hexadecimal prefix, ignore
invalid digits with one PHP deprecation per call, and promote integer accumulation to
float at overflow. `base_convert` validates bases 2–36 after native parameter conversions,
preserves PHP's float division and 64-digit output-buffer behavior, and rejects infinite
results. `decbin`, `decoct`, and `dechex` convert native integers to unsigned 64-bit digit
strings, including negative inputs. Parameter names and native coercions are registered
for all seven functions.

Math reaches **18 / 36 / 117** and strings **319 / 259 / 152**; Zend, arrays, reflection,
and SPL are unchanged. All 797 existing Rust tests, 57 reference-checked project fixtures,
formatting, and Clippy pass. Thirteen additional existing upstream conversion programs,
whose INI/SKIPIF sections are unsupported by the harness, match PHP directly on the
64-bit host with their requested settings. A short reference probe also matches float
output truncation and infinite conversion errors. These direct comparisons do not change
the harness totals, and no custom test files or generated matrices were added. Behavior
follows PHP 8.5.0 `ext/standard/math.c`, native stubs, and upstream fixtures. Stream-based
variation fixtures, INI/SKIPIF harness support, complete native reflection type metadata,
rounding, number formatting, and transcendental math remain unfinished.

The [rounding and number formatting slice](conformance/php-8.5.0-rounding-hillclimb.txt)
adds **29 upstream passes**: 25 math fixtures and four string fixtures. No passes regress
across Zend, reflection, SPL, arrays, strings, or math. `round` implements eight modes
through the native `RoundingMode` enum or integer arguments, with the four legacy
`PHP_ROUND_HALF_*` constants. Decimal boundary correction, exact small powers of ten,
large-precision decimal conversion, signed zero, and nonfinite inputs follow PHP 8.5.0.
All parameters are converted before mode validation; large precision arguments clamp to
PHP's integer precision bounds. The default mode is represented as an enum-case expression
and resolves to the same singleton exposed by `RoundingMode::cases()` and reflection.

`number_format` preserves exact integer input, including minimum/maximum integer values
and negative decimal-place rounding. Large integral floats use the integer path when
they fit PHP integers. Other floats use half-away-from-zero decimal rounding followed by
fixed conversion, with PHP's 318-place conversion limit and zero padding. Grouping and
decimal separators support multiple bytes, empty strings, and nullable defaults; rounded
negative zero loses its minus sign. Nonfinite results use PHP's lowercase `inf`/`nan`
spelling. Checked sizes and fallible allocation retain the runtime's existing allocation
error handling; PHP memory limits and exact fatal allocation semantics remain incomplete.

An upstream rounding program exposed a shared native integer-coercion issue: fractional
numeric strings now report PHP's `float-string` deprecation with the original string,
rather than reporting a float. This also fixes three base-conversion fixtures. The full
math suite reaches **43 / 11 / 117**, and strings **323 / 255 / 152**. Zend remains
**2043 / 2805 / 408**, arrays **154 / 634 / 52**, reflection **274 / 181 / 37**, and
SPL **22 / 662 / 94**. All 797 existing Rust tests, 57 reference-checked project fixtures,
formatting, and Clippy pass. Five additional existing INI/SKIPIF-bearing upstream programs
match PHP directly with their settings; these remain unsupported in the harness and do
not count as passes. A short reference probe also matches enum identity/default reflection,
named calls, and special floating-point formatting. No custom tests or generated matrices
were added. Evidence comes from PHP 8.5.0 math/argument-conversion code, stubs, upstream
fixtures, and the reference executable. Remaining math failures require streams, execution
timeouts, trigonometric/logarithmic functions, constants, random functions, or serialization.
Configurable precision and complete native reflection types remain unfinished.

The [transcendental math slice](conformance/php-8.5.0-transcendental-hillclimb.txt) adds
**11 upstream passes**: six math fixtures, four Zend fixtures, and one string fixture.
There are no regressions across the six measured suites. It registers 22 functions:
trigonometric and inverse trigonometric functions, hyperbolic and inverse hyperbolic
functions, `atan2`, `hypot`, `exp`, `expm1`, `log`, `log10`, `log1p`, `deg2rad`,
`rad2deg`, and `pi`. Native float conversion applies caller strictness and argument
diagnostics. Degree/radian conversion preserves PHP's operation order; `log` uses
dedicated base-two/base-ten paths, returns NaN for base one, and rejects nonpositive
bases. The default base retains the `M_E` constant expression for reflection. Zero-argument
math dispatch handles `pi` before accessing required arguments.

Seventeen standard mathematical constants are registered with standard-extension metadata.
Rust's floating math operations provide the host math-library behavior; no extra dependency
was added. Local reference comparisons confirm domain/NaN/infinity behavior and signed
zero for the checked fixtures. Transcendental last-bit results may depend on the host math
library, as they do in PHP.

Zend reaches **2047 / 2801 / 408**, math **49 / 5 / 117**, and strings **324 / 254 / 152**.
Arrays, reflection, and SPL retain their previous totals. All 797 existing Rust tests,
57 reference-checked project fixtures, formatting, and Clippy pass. Sixty additional
upstream INI/SKIPIF-bearing programs were compared directly: 43 match with their requested
settings, and 17 differ only under unsupported precision/serialize_precision settings and
match PHP with the runtime's current defaults. They remain unsupported and are excluded
from the harness pass count. A short reference probe also matches logarithm base special
cases, default reflection, and named atan2/hypot arguments. No custom tests or generated
matrices were added. PHP 8.5.0 math code, constant definitions, native stubs, fixtures,
and reference execution establish these behaviors. The five remaining executable math
fixtures require streams, execution timeouts, random functions, or serialization.
INI/SKIPIF support, complete native reflection types, and resource limits remain incomplete.

The [array search and numeric reduction slice](conformance/php-8.5.0-array-search-hillclimb.txt)
adds **28 upstream passes**: 25 array fixtures and three Zend fixtures. No passes regress
across arrays, Zend, reflection, SPL, strings, or math. `in_array` and `array_search`
validate their array and optional strict flag before iteration, reuse the runtime's
loose/strict PHP comparisons, and inspect entries in insertion order. Search returns
the first matching integer or binary string key, including key zero, or false. Referenced
entries and `$GLOBALS` entries retain their values during comparison. Native parameter
names and a false strict default support named calls and reflection defaults.

`array_sum` and `array_product` start with integer zero and one, respectively, apply
arithmetic in entry order, preserve numeric strings and referenced variables, and promote
overflow to float. Unsupported arrays and ordinary objects produce PHP warnings and are
skipped. Direct nonnumeric strings retain PHP's zero fallback; references to nonnumeric
strings are warned about and skipped, matching the native entry-type check. Leading-numeric
strings retain arithmetic conversion warnings. Resource IDs and extension-specific numeric
object casts remain unsupported because those runtime value/extension types are unfinished.

Arrays reach **179 / 609 / 52**, and Zend **2050 / 2798 / 408**; all other suite totals
are unchanged. All 797 existing Rust tests, 57 reference-checked project fixtures,
formatting, and Clippy pass. The existing INI-bearing large-array sum program matches PHP
when run directly, but remains unsupported and is not counted as a pass. A short reference
probe also matches direct/reference string fallbacks, invalid entries, warning order, and
unchanged input strings. No custom tests or generated matrices were added. PHP 8.5.0
array implementations, native signatures, upstream fixtures, and reference execution
establish these behaviors. Remaining targeted failures require streams/resources and
extension numeric types; further array APIs, error handlers, complete native reflection
types, and INI/SKIPIF harness support remain unfinished.

The [array inspection and construction slice](conformance/php-8.5.0-array-inspection-hillclimb.txt)
adds **19 upstream passes**: 18 array fixtures and one Zend fixture, with no regressions
across the six measured suites. PHP 8.5 `array_first` and `array_last` return dereferenced
values, while `array_key_first` and `array_key_last` return insertion-order keys; all four
return null for empty arrays and leave array state unchanged. `array_is_list` checks for
ordered consecutive integer keys starting at zero, including empty arrays.

`array_flip` and `array_count_values` accept integer/string entries, apply numeric-string
key conversion, preserve the first insertion position of duplicate keys, and warn/skip
other value types. Flipping retains the final source key for duplicate values; counting
increments frequency. `array_combine` validates equal counts, ignores the input arrays'
keys, converts key values using ordinary PHP string conversion where needed, and copies
values with PHP's shared/unshared reference rules. Duplicate keys replace their value
without moving their position. Empty input arrays produce an empty result.

Arrays reach **197 / 591 / 52**, and Zend **2051 / 2797 / 408**; other suite totals are
unchanged. All 797 existing Rust tests, 57 reference-checked project fixtures, formatting,
and Clippy pass. A short reference comparison confirms shared and unshared reference
copying, dereferenced first/last results, empty arrays, list order, Stringable keys, and
duplicate overwrite behavior. The additional general-functions `array_is_list` fixture
still fails because it requires `json_encode`; it adds no claimed pass. No custom tests or
generated matrices were added. PHP 8.5.0 array implementations, native signatures, upstream
fixtures, and reference execution establish these behaviors. Remaining targeted fixtures
require streams, JSON, or internal array-pointer functions. Complete native reflection
types, configurable memory limits, and INI/SKIPIF harness support remain unfinished.

The [array merge and replacement slice](conformance/php-8.5.0-array-join-hillclimb.txt)
adds **27 upstream passes**: 26 array fixtures and one Zend fixture, with no regressions
across Zend, reflection, SPL, strings, arrays, or math. `array_merge` renumbers top-level
numeric keys and overwrites duplicate string keys without changing their insertion
position. `array_replace` preserves keys and replaces matching entries. Both validate
all array arguments before processing; merging zero arrays returns an empty array.

Recursive merging combines duplicate string-key values, casts objects to their property
arrays, retains null values, and preserves existing nested numeric keys and append
positions. Recursive replacement descends only when both colliding values are arrays.
Colliding reference containers separate from their inputs, while untouched entries use
PHP's shared/unshared reference copying rules. Recursion guards follow the distinct PHP
merge and replacement rules; unrelated shared siblings and permitted cross-input sharing
remain valid. Upstream cases cover recursive inputs, orphan references, input preservation,
and next-index overflow. Native variadic type errors now omit a parameter label, also
fixing an existing `array_map` fixture. Unknown named variadic arguments no longer count
toward a native function's minimum arity.

Arrays reach **223 / 565 / 52**, and Zend **2052 / 2796 / 408**. All 797 existing Rust
tests, 57 reference-checked project fixtures, formatting, and Clippy pass. Direct PHP
comparisons verify argument errors, shared references, and overlapping array inputs.
The existing `array_merge.phpt` FILE program matches PHP with `precision=14`, but its
INI-bearing fixture remains unsupported and is not counted as a pass. No custom tests
or generated matrices were added. PHP 8.5.0 array implementations, native signatures,
upstream fixtures, and reference execution establish the behavior. Remaining targeted
fixtures depend on streams, internal array cursors, or array walking. Packed/mixed table
metadata, complete native reflection types, memory limits, and INI/SKIPIF harness support
remain unfinished.

The [internal array cursor slice](conformance/php-8.5.0-array-cursor-hillclimb.txt)
adds **44 upstream passes**: 33 array fixtures and 11 Zend fixtures, with no regressions
across Zend, reflection, SPL, strings, arrays, or math. `current` and its `pos` alias return
the dereferenced current value; `key` returns its key. Missing positions return false
or null respectively. `reset`, `end`, `next`, and `prev` take references and move the
internal cursor independently of foreach iterators. Empty arrays, exhaustion, deletion,
appending at the old end, indirect temporary arguments, and recursive arrays follow the
upstream cases. Array duplication preserves valid positions and resets exhausted ones;
cursor mutation separates actual value aliases without treating the native argument
snapshot as an additional PHP alias.

Object arguments emit PHP's deprecation and traverse property storage, including encoded
non-public keys and stable declared slots. Uninitialized/unset declared properties are
skipped in the appropriate direction when fetching a value; dynamic-property deletion
adjusts later positions. Closure property traversal is empty. Moving calls whose return
value is discarded do not fetch/skip an uninitialized property. The evaluator now carries
return-use information through expression statements, for-loop sequences, and directly
inlined callbacks. Void casts still consume the operand result, as PHP does. Direct
reference comparisons confirm these distinctions, array sharing/exhaustion, declared
property order, dynamic deletion/reinsertion, and native property traversal.

Arrays reach **256 / 532 / 52**, and Zend **2063 / 2785 / 408**. All 797 existing Rust
tests, 57 reference-checked project fixtures, formatting, and Clippy pass. No custom tests
or generated matrices were added. Direct execution of the existing `end_64bit.phpt` FILE
program matches through its array cases but fails at missing `fopen`; its INI/SKIPIF
fixture remains unsupported and is not counted as a pass. PHP 8.5.0 array/hash-table
implementations, native signatures, upstream fixtures, and reference execution establish
the behavior. Streams, complete native-object property handlers, packed/mixed table
metadata, native reflection types, and INI/SKIPIF harness support remain unfinished.

The [array walking slice](conformance/php-8.5.0-array-walk-hillclimb.txt) adds **43 upstream
passes**: 41 array fixtures and two Zend fixtures, with no regressions across Zend,
reflection, SPL, strings, arrays, or math. `array_walk` and `array_walk_recursive` validate
the array/object input and callback, pass live value references and original keys, and
include userdata only when supplied. Callback return values are ignored. Recursive walks
descend into arrays, preserve input copy-on-write separation, and reject recursive array
cycles. The internal array cursor is independent of the walk position.

Walk positions advance before callback execution and follow deletion, appending, and
replacement of the input. Replacing the input with an unrelated array resumes at its
internal cursor; replacement with a scalar raises `Iterated value is no longer an array
or object`. Object traversal includes encoded non-public property names, skips
uninitialized properties, and shares the existing typed-reference constraint machinery.
It follows native walking's reference acquisition behavior, including readonly-property
storage. Weak property-iteration tokens allow dynamic deletion to update active walks
without extending object lifetimes. Generic foreach reference acquisition retains its
own readonly restrictions. Invalid input errors retain PHP's legacy `array` wording even
though objects are accepted.

Arrays reach **297 / 491 / 52**, and Zend **2065 / 2783 / 408**. All 797 existing Rust
tests, 57 reference-checked project fixtures, formatting, and Clippy pass. Direct PHP
comparisons match live mutations, replacement cursor positions, scalar replacement
errors, typed/readonly properties, and recursive inputs when properties are constructed
directly. A separate comparison exposes an outstanding cast-object backing-table gap:
after a walk appends a property to an object cast from an array, PHP retains a shared
reference wrapper on the first visited property while this runtime drops that wrapper.
The values agree, but the reference representation does not; the mismatch is recorded in
the report and is not claimed as a pass. Remaining targeted fixtures require `min`,
`settype`, `flush`, PCRE, array intersection, or PHP 8.5 closure-debug metadata. No custom
tests or generated matrices were added. PHP 8.5.0 array/hash-table implementations,
native signatures, upstream fixtures, and reference execution establish the behavior.

The [minimum/maximum slice](conformance/php-8.5.0-extrema-hillclimb.txt) adds **11 upstream
passes**: eight array fixtures and three Zend fixtures, with no regressions across Zend,
reflection, SPL, strings, arrays, or math. `min` and `max` accept a single nonempty array
or multiple mixed values and return the selected value with its original type. Array
entries are dereferenced on return. Empty arrays, invalid single inputs, missing arguments,
and unknown named variadic arguments follow PHP's errors.

Array extrema compare the incumbent against each candidate; generic variadic extrema
compare in the opposite order. PHP's uncomparable result is retained, including object
and NaN cases. Numeric variadic scans use exact integer comparison, switch to floating
comparison only when the integer round-trips exactly, and remain in generic comparison
after a fallback. Direct positional two-argument calls preserve PHP's optimized behavior:
`min` selects the right argument on a tie while `max` keeps the left, and numeric NaN
selects the right argument. Dynamic and unpacked calls use the ordinary variadic path.
Stringable comparison exceptions retain the native function frame.

Arrays reach **305 / 483 / 52**, and Zend **2068 / 2780 / 408**. All 797 existing Rust
tests, 57 reference-checked project fixtures, formatting, and Clippy pass. Six existing
INI/SKIPIF fixture FILE programs match reference PHP when executed directly on the
64-bit host, with `precision=14` for the two INI cases. Their PHPT fixtures remain
unsupported and add no claimed passes. A short reference comparison confirms tie result
types, direct/dynamic/unpacked calls, NaN, uncomparable objects, and exception frames.
No custom tests or generated matrices were added. PHP 8.5.0 array/hash-table implementations,
native signatures, upstream fixtures, and reference execution establish the behavior.
Resource/extension-object comparisons, complete native reflection metadata, and
INI/SKIPIF harness support remain unfinished.

The [array-sorting slice](conformance/php-8.5.0-sorting-hillclimb.txt) adds **99 upstream
passes**: 92 array fixtures, three Zend fixtures, three reflection fixtures, and one
string fixture, with no regressions across the six suites. It implements `sort`,
`rsort`, `asort`, `arsort`, `ksort`, `krsort`, `natsort`, and `natcasesort`, with PHP's
sorting constants and stable ties in either direction. Numeric key sorting compares
integer keys exactly; numeric value sorting uses doubles. String and natural modes
support case folding, and unknown modes fall back to regular comparison. Natural
comparison handles whitespace, leading zeroes, and digit runs. Locale sorting currently
implements the C locale only.

The sorting algorithm follows PHP's bucket comparison order, including small-array
insertion sorting and median-pivot quicksort, so non-transitive comparisons and conversion
warnings do not depend on Rust's total-order sorting contract. Reordering preserves
reference cells and live foreach offsets; key-preserving sorts retain the append counter,
while renumbering sorts reset it. Internal cursors reset only when PHP actually reorders
buckets. Active call frames retain by-reference argument cells instead of artificial
array snapshots, fixing unnecessary copy-on-write and exhausted-cursor changes. Failed
string conversions record the exception and finish sorting with empty strings while
suppressing subsequent user conversions. Regular object/string comparisons retain PHP's
failed-cast ordering. The translated algorithms' original notices and licenses are in
[THIRD_PARTY_NOTICES.md](../THIRD_PARTY_NOTICES.md).

Arrays reach **397 / 391 / 52**, Zend **2071 / 2777 / 408**, reflection **277 / 178 / 37**,
and strings **325 / 253 / 152**. All 797 existing Rust tests, 57 reference-checked project
fixtures, formatting, and Clippy pass. Direct reference execution matches stable numeric
ties, copied/shared references, one-element cursors, active foreach, NaN string warnings,
regular/string conversion exceptions, and replacement of the input during conversion.
Two existing INI fixture FILE programs match with `precision=14`; their fixtures remain
unsupported and add no claimed passes. A separate reference probe confirms an outstanding
reentrant-copy limitation: a copy captured inside `__toString()` retains its original
order here, while PHP exposes its live sorting table and updates that copy's order too.
This mismatch is recorded rather than counted as a pass. Callback sorting, `array_multisort`,
locale configuration, resource values, live-table observation during conversions, and
complete native reflection metadata remain unfinished. No custom tests or generated
matrices were added. PHP 8.5.0 source, native signatures, upstream PHPT, and reference
execution establish the implemented behavior.

The [callback-sorting slice](conformance/php-8.5.0-user-sort-hillclimb.txt) adds **32
upstream array passes**, with no regressions across Zend, reflection, SPL, strings,
arrays, or math. `usort`, `uasort`, and `uksort` validate the callback even for empty
inputs and sort a duplicate table, leaving the evolving order invisible to callbacks.
Value comparators retain shared reference arguments; key comparators receive integer
or string keys. Internally invoked callbacks retain PHP's weak argument coercion and
by-reference argument warnings. The existing PHP-order sorting algorithm retains stable
ties; `usort` renumbers keys, while the associative forms preserve keys and append state.

Comparator results are converted using PHP's integer-cast semantics, including fractional
truncation and out-of-range warnings. Direct boolean returns emit the comparison
deprecation, and `false` retries the callback with reversed operands. Reference returns
are dereferenced only for integer conversion, matching PHP's separate boolean handling.
The deprecation flag follows request state: native entry resets it even for an invalid
argument count, nested calls may change it, and outer sorts do not restore it. After a
callback throws, remaining comparisons retain stable tie ordering without calling user
code; the sorting copy is installed before the exception propagates. Input mutation or
replacement inside a callback does not replace the sorting copy.

Arrays reach **429 / 359 / 52**. All 797 existing Rust tests, 57 reference-checked project
fixtures, formatting, and Clippy pass. Short reference comparisons match stable key
ordering, boolean retries, reference returns, shared reference arguments, exceptions,
input replacement, active reference foreach, nested deprecation state, and return-value
overflow/object conversion warnings. The existing INI `user_sort_basics` FILE program
matches when executed directly with `precision=14`; its PHPT remains unsupported and
adds no claimed pass. No custom tests or generated matrices were added. PHP 8.5.0 source,
native signatures, upstream fixtures, and reference execution establish the behavior.
The remaining named `usort` fixture requires `shuffle`; a Zend fixture stops at missing
`set_error_handler`. `array_multisort`, resource comparisons, complete native reflection
metadata, and reentrant live-table observation in built-in comparison modes remain
unfinished.

The [multisort slice](conformance/php-8.5.0-multisort-hillclimb.txt) adds **24 upstream
passes**: 19 array fixtures and five Zend fixtures, with no regressions across the six
suites. `array_multisort` parses per-array direction and comparison flags, rejects repeated
or invalid flags, requires equal row counts, and compares columns lexicographically.
Stable ties use original row positions only after all columns compare equal. It reuses
PHP's comparison order and the existing regular, numeric, string, natural, case-folded,
and C-locale comparison modes. Nested sorting calls keep their own column configuration.

Native preferred-reference binding accepts literal arrays, flags, and expression results
without by-reference errors or notices, while writable inputs and actual references
retain their cells. This behavior also applies to indirect calls, unpacking, and pipe
calls. Array separation happens while arguments are parsed, before comparison snapshots
are retained. Repeated aliases to the same input are reordered once. Output preserves
string keys, renumbers only integer keys, resets the cursor even for one-element inputs,
and sets the next append index from the complete row count. If a comparison throws,
multisort propagates the exception without installing the permutation.

Arrays reach **448 / 340 / 52**, and Zend **2076 / 2772 / 408**. All 797 existing Rust
tests, 57 reference-checked project fixtures, formatting, and Clippy pass. A short
reference comparison matches multiple columns, aliases and copies, mixed-key append
behavior, one-element cursors, literal and indirect arguments, unpacking, invalid flags,
named-argument rejection, and comparison exceptions. No custom tests or generated matrices
were added. PHP 8.5.0 source, native signatures, upstream PHPT, and reference execution
establish the behavior. Remaining multisort fixtures stop at missing `set_error_handler`
or `DateTime`; resources, locale configuration, complete native reflection metadata,
and reentrant observation/mutation of native sorting tables remain unfinished.

The [variable-extraction slice](conformance/php-8.5.0-extract-hillclimb.txt) adds **35
upstream passes**: 28 array fixtures, six Zend fixtures, and one reflection fixture,
with no regressions across the six suites. `extract` and the `EXTR_*` constants support
all seven extraction modes and the reference modifier. Key/prefix identifiers follow
PHP's byte rules, numeric keys are considered only by the applicable prefix modes, and
empty names, `$this`, and an existing `GLOBALS` symbol retain their mode-specific rules.
The low flag byte selects the mode; the reference bit controls separation, and unrelated
high bits do not change the mode. Argument conversion, separation, flag validation,
prefix validation, and dynamic-call rejection follow PHP's ordering.

Value extraction retains its source table while overwriting locals and assigns through
existing references with PHP's weak type checks, even in strict files. Reference
extraction separates the input, creates references only for accepted entries, and rebinds
locals instead of overwriting their previous reference targets. Input aliases and
self-overwriting source names retain the native argument's cell. Function scopes now
reserve compiled local slots before executing their bodies, as top-level scopes already
did; prefix-if-existing extraction can distinguish an unset compiled local from a name
absent from the symbol table. The preferred-reference argument binder accepts literal
arrays and expression results without by-reference notices.

Arrays reach **476 / 312 / 52**, Zend **2082 / 2766 / 408**, and reflection
**278 / 177 / 37**. All 797 existing Rust tests, 57 reference-checked project fixtures,
formatting, and Clippy pass. Every array fixture named `extract*` passes. A short reference
comparison matches prefix modes with unset/later locals, copy-on-write reference extraction,
source-variable overwrite, typed-reference coercion and rebinding, byte identifiers,
`GLOBALS` rules, high flag bits, and dynamic-call error precedence. No custom tests or
generated matrices were added. PHP 8.5.0 source, native signatures, upstream PHPT, and
reference execution establish the behavior. General variable introspection, destructors,
error handlers, resources, and complete native reflection metadata remain unfinished.

The [variable-inspection slice](conformance/php-8.5.0-variable-inspection-hillclimb.txt)
adds **12 upstream passes**: nine array fixtures and three Zend fixtures, with no
regressions across the six suites. `compact` accepts mixed variadic arguments, recursively
visits nested name arrays in order, keeps the first insertion position for duplicate
names, and returns dereferenced values. Invalid entries report their original argument
position, including nested entries; missing variables warn, while an unavailable `this`
name is silently omitted. A bound object context can be selected with `compact('this')`.
Numeric variable names remain string keys in the result.

Recursive name arrays raise errors while the remaining scan continues, retaining PHP's
warning order. Repeated recursion errors chain the previously pending exception.
`get_defined_vars` follows current symbol-table order, omits undefined compiled slots and
the implicit `$this` context value, copies ordinary values, and preserves reference cells
with other owners. Both functions reject dynamic calls; `compact` also rejects unknown
named variadic arguments. They inspect the active PHP scope rather than introducing
synthetic variables inside the builtins.

Arrays reach **485 / 303 / 52**, and Zend **2085 / 2763 / 408**. All 797 existing Rust
tests, 57 reference-checked project fixtures, formatting, and Clippy pass. A short reference
comparison matches alias-preserving versus value-copy snapshots, undefined locals, numeric
names, method contexts, nested invalid names, recursion/warning order, chained errors,
and dynamic/named-argument failures. The separate upstream
`general_functions/get_defined_vars_basic` fixture still fails solely because no CLI
startup globals are present at its initial global-scope snapshot; its subsequent local
function snapshots match the expected output. `Zend/tests/get_defined_vars` also requires
CLI `$argc`. These failures remain recorded and are not counted as passes. CLI startup
globals, error handlers, resources, and complete native reflection metadata remain
unfinished. No custom tests or generated matrices were added. PHP 8.5.0 source, native
signatures, upstream PHPT, and reference execution establish the implemented behavior.

The [array-splice slice](conformance/php-8.5.0-array-splice-hillclimb.txt) adds
**8 upstream passes**: six array fixtures and two Zend fixtures, with no regressions
across the six suites. `array_splice` separates its array argument before parsing
offset/length, clamps positive and negative ranges, treats omitted/null length as the
remaining tail, and casts replacement values to arrays. It preserves string keys and
reference containers in retained/removed entries, ignores replacement keys, renumbers
numeric keys, and resets the internal cursor and next append index.

Active reference iterators follow the tested packed and mixed table paths, including
PHP's different tail-position updates when the removed array is consumed versus discarded,
forward remapping, final-bucket clamping, and removal of expired iterator registrations.
A short reference probe matches aliases, copy-on-write, self replacement, object/closure/
scalar replacements, named arguments, invalid index coercion after separation, numeric
append keys, cursors, and dense numeric/string-key iterator cases. A separate sparse-array
probe exposes an existing representation limitation: after deleting the first bucket,
splicing during reference iteration can visit an extra retained value. The runtime does
not retain PHP's physical bucket history or packed/mixed transitions. That mismatch is
recorded, and sparse iterator compatibility is not claimed. Destructor-driven mutation
fixtures and resource replacements also remain failing.

Arrays reach **491 / 297 / 52**, and Zend **2087 / 2761 / 408**. All 797 existing Rust
tests, 57 reference-checked project PHPT cases, formatting, and Clippy pass. No custom
tests or generated matrices were added. PHP 8.5.0 `ext/standard/array.c`, native signatures,
upstream PHPT, and reference execution establish the implemented behavior.

The [array-stack slice](conformance/php-8.5.0-array-stack-hillclimb.txt) adds
**44 upstream passes**: 35 array fixtures and nine Zend fixtures, with no regressions
across the six suites. `array_push` and `array_unshift` accept zero or more values and
return the resulting element count. Push preserves existing keys and cursor position;
completed appends survive a later exhausted-index error. Unshift rebuilds the array,
preserves string keys/reference cells, renumbers numeric keys, advances live iterators,
and resets the internal pointer even when no values are prepended.

`array_pop` and `array_shift` return null for an empty array and otherwise return the
removed value without its reference binding. Pop preserves the remaining keys and restores
the next append index only when the removed integer key immediately precedes it, including
negative indices and PHP's minimum-integer sentinel. Shift renumbers remaining numeric keys
and resets the next append index. Both reset the internal cursor on removal. Dense packed
shift traversal retains PHP's distinction between an exhausted iterator and one pointing
at a retained element; mixed-array traversal follows its corresponding tested behavior.
All four operations separate the referenced array before mutation. Push/unshift validate
and separate the first argument before rejecting unknown named variadic arguments.

Arrays reach **526 / 262 / 52**, and Zend **2096 / 2752 / 408**. All 797 existing Rust
tests, 57 reference-checked project PHPT cases, formatting, and Clippy pass. A short
reference probe matches reference aliases, copy-on-write, self arguments, empty prepends,
minimum/maximum and negative append keys, partial append failures, cursor resets, live
reference loops, detached return values, callback arguments, and named-error precedence.
The remaining four supported fixtures specifically named for these operations require
`fopen`/stream resources; the INI-dependent unshift fixture remains unsupported. Sparse
physical bucket history, native storage transitions, resources, and complete reflection
metadata remain incomplete. No custom tests or generated matrices were added. Behavior
is grounded in PHP 8.5.0 `ext/standard/array.c`/`Zend/zend_hash.c`, native signatures,
upstream PHPT, and reference execution.

The [array-column slice](conformance/php-8.5.0-array-column-hillclimb.txt) adds
**11 upstream array passes**, with no regressions across the six suites. `array_column`
accepts nullable string/integer selectors with PHP's strict/weak argument coercion,
including Stringable objects in weak mode and float-to-string fallback outside the integer
range. Array lookup normalizes integer-string keys; object lookup preserves property-name
spelling and respects the caller's visibility. Initialized null properties are included,
missing or never-initialized typed properties are skipped, and inaccessible/missing
properties use `__isset` before `__get`. Explicitly unset typed properties retain their
existing getter type checks.

A null column selector copies whole rows. Selected values and index values are dereferenced;
missing index values append, duplicate index keys overwrite without changing their original
position, and null/float index keys produce PHP's conversion diagnostics. An exhausted
append index silently drops a later unindexed row, matching the native function. Pending
exceptions suppress subsequent user property calls while later ordinary rows continue to
produce native diagnostics; subsequent key errors chain the earlier exception. Exit
signals still stop immediately.

Arrays reach **537 / 251 / 52**; the other suite totals are unchanged. All 797 existing Rust
tests, 57 reference-checked project PHPT cases, formatting, and Clippy pass. A short reference
probe matches missing versus null columns, reference detachment, whole scalar rows, caller
visibility, magic access, unset typed-property failures, Stringable/float selectors, key
collisions, exhausted append indices, warning order after errors, and chained exceptions.
`array_column_basic` still fails at its stream-resource setup (`fopen`); the runner does
not count its earlier matching output as a pass. Resources, property hooks, native object
handlers, and complete reflection metadata remain incomplete. No custom tests or generated
matrices were added. PHP 8.5.0 `ext/standard/array.c`, `Zend/zend_API.c`, object handlers,
upstream PHPT, and reference execution establish the implemented behavior.

The [resource/stream slice](conformance/php-8.5.0-resources-hillclimb.txt) adds
**59 upstream passes**: 30 array, 17 Zend, ten string, and two math fixtures, with no
regressions across the six suites. A distinct resource value preserves handle identity and
shared closed state. `is_resource`, `get_resource_id`, `get_resource_type`, `gettype`,
`get_debug_type`, dumps, print/export output, and trace formatting recognize open/closed
handles. Explicit integer/float casts use the handle ID; string casts retain the resource
label. PHP 8.5 arithmetic rejects resources, native scalar parameters reject implicit
resource conversion, resource array keys warn and convert to IDs, and numeric sorting and
loose resource/string comparison follow their separate native conversion paths.

Basic streams implement `fopen`, `fclose`, `fread`, `fwrite`/`fputs`, `feof`, `ftell`,
`fseek`, `rewind`, and `fflush`, with `SEEK_SET`, `SEEK_CUR`, and `SEEK_END`. Local files
use native file handles and byte paths. Memory streams retain permissions, binary content,
positions, and EOF state; temporary streams own a distinct registered backing resource.
Default contexts allocate lazily, and handle allocation reserves the CLI standard-stream
IDs. Closing any alias closes the handle for all aliases while retaining its ID.
Logical stream positions preserve PHP's append-mode behavior instead of exposing the OS
append offset. Stream validation precedes later argument coercion, without holding a
resource borrow while user string conversion runs. `file_exists` and `unlink` support
local file lifecycle checks, including continued use of an unlinked open file.

The focused stream/resource run covers **75 fixtures: 12 passed / 24 failed / 39
unsupported**, independently of the six-suite gain count. It includes existing read/write
roundtrip, seek-boundary, closed-resource, and unlink-with-open-handle cases. A short
reference probe matches casts, identity/comparisons, array-key diagnostics, weak parameter
rejection, file and memory I/O, append positions, EOF/seek behavior, invalid modes/paths,
closed aliases, temporary backing-resource IDs, and unlink behavior. Remaining focused
failures require other APIs such as `fgets`, `chmod`, `basename`, configurable include
paths, context creation, and wrapper registration. Temporary files left by failing fixture
setup were removed after verification.

Arrays reach **567 / 221 / 52**, Zend **2113 / 2735 / 408**, strings **335 / 243 / 152**,
and math **51 / 3 / 117**. All 797 existing Rust tests, 57 reference-checked project PHPT
cases, formatting, and Clippy pass. No custom tests or generated matrices were added.
Standard-stream constants and I/O, resource enumeration, configurable contexts, general
URL/user wrappers, socket behavior, temporary-file spill thresholds, buffering parity,
and PHP memory limits remain incomplete. PHP 8.5.0 stream/operator source, native
signatures, upstream PHPT, and reference execution establish the implemented behavior.

The [built-in array set slice](conformance/php-8.5.0-array-sets-hillclimb.txt) adds
**60 upstream passes**: 58 array and two Zend fixtures, with no regressions across
the six suites. `array_diff`, `array_intersect`, and their `_key` and `_assoc`
variants preserve first-array keys and shared reference cells. Value comparisons
use PHP byte strings, including numeric, null, boolean, resource, array-warning,
and Stringable conversions. Key variants use exact normalized-key lookup.

`array_diff` follows PHP's empty, singleton, and exclusion-map paths, including
conversion before later argument validation for singleton input and early candidate
termination after a match. `array_intersect` sorts each input with Zend's comparison
order and scans the sorted lists, preserving duplicate values and the first array's
original order. It validates each next argument after sorting the previous array.
Pending exceptions suppress further object conversions, retain the first exception,
and allow subsequent scalar-conversion warnings. Empty-array append indices and
cursor behavior follow each function's native copy/rebuild path.

Arrays reach **625 / 163 / 52** and Zend **2115 / 2733 / 408**; reflection, SPL,
strings, and math retain their prior totals. All 797 existing Rust tests and 57
reference-checked project PHPT cases pass, along with formatting and Clippy. A short
reference probe matches key/reference preservation, cursor and append-index state,
conversion order, singleton and empty arrays, invalid/named arguments, and pending
exception behavior. No custom tests or generated matrices were added. User-comparator
variants, error handlers, destructors, physical sparse-table history, PHP memory
limits, and complete native metadata remain incomplete. PHP 8.5.0 array/operator
source, native signatures, upstream PHPT, and reference execution establish these
behaviors; the adapted set algorithms retain PHP's notices and license.

The [user-comparator array set slice](conformance/php-8.5.0-array-user-sets-hillclimb.txt)
adds **68 upstream array passes**, with no regressions across the six suites. All ten
callback variants are implemented: `_ukey`, `_uassoc`, `array_udiff`/`array_uintersect`,
and their `_assoc` and `_uassoc` forms. Sorted variants follow Zend's unstable
comparison order and native list scans; key-comparator differences restart their
search in each other array. Separate key and value callbacks retain their native
argument order. Exact-key associative value-callback variants use direct lookup and
cast comparator returns without the boolean retry/deprecation used by sorted variants.

Callback validation precedes array validation, and unknown named arguments precede
minimum-arity checks. Shared element references remain visible to callback reference
parameters and returned arrays. Pending exceptions suppress later callbacks while
preserving scalar-conversion diagnostics. The request-wide boolean-comparator
deprecation flag follows nested-call resets, including ordinary `array_intersect`.

Arrays reach **693 / 95 / 52**; Zend remains **2115 / 2733 / 408**, and reflection,
SPL, strings, and math retain their prior totals. All 797 existing Rust tests, 57
reference-checked project PHPT cases, formatting, and Clippy pass. A short reference
probe matches all ten variants, comparison order, reference mutations, argument-error
ordering, boolean and reference-returned comparator results, nested resets, pending
exceptions, and distinct key/data callbacks. No custom tests or generated matrices
were added. Error handlers, destructors, physical sparse-table history, PHP memory
limits, and complete native metadata remain incomplete. PHP 8.5.0 array source,
native signatures, upstream PHPT, and reference execution establish these behaviors.

The [array transformation slice](conformance/php-8.5.0-array-transform-hillclimb.txt)
adds **23 upstream passes**: 19 array, three Zend, and one reflection fixture, without regressions
across the six suites. `array_unique` retains first occurrences using PHP's string
hash-set path or Zend's unstable comparison order followed by original-index
selection. It supports regular, numeric, string, natural, case-insensitive, and
C-locale comparison. Empty/singleton inputs return early, and each path preserves
its native cursor and append-index behavior. Shared reference cells survive.

`array_change_key_case` converts ASCII string keys, preserves numeric keys and
reference bindings, and overwrites colliding keys without moving their original
slots. Any nonzero integer case argument selects uppercase. `CASE_LOWER` and
`CASE_UPPER` are available as standard-extension constants.

Arrays reach **712 / 76 / 52**, Zend **2118 / 2730 / 408**, and reflection
**279 / 176 / 37**; SPL, strings, and math retain their prior totals. All 797 existing Rust tests, 57
reference-checked project PHPT cases, formatting, and Clippy pass. A short reference
probe matches comparison modes, first-occurrence selection, reference sharing,
key collisions, non-ASCII bytes, cursor/append-index state, conversion order,
pending exceptions, and invalid arguments. `array_unique_variation3` still fails
at its missing `STDERR` setup. No custom tests or generated matrices were added.
Locale configuration beyond C, standard-stream constants, error handlers,
destructors, physical sparse-table history, PHP memory limits, and complete native
metadata remain incomplete. PHP 8.5.0 array source, native signatures, upstream
PHPT, and reference execution establish these behaviors.

The [array predicate search slice](conformance/php-8.5.0-array-find-hillclimb.txt)
adds **six upstream array passes**, with no regressions across the six suites.
`array_find`, `array_find_key`, `array_any`, and `array_all` invoke the predicate
with each value and key, stopping at the first matching or failing result as
appropriate. Empty inputs return null for find operations, false for any, and
true for all. Callback validation still runs for empty arrays.

Callback arguments preserve existing reference cells. Found values are read after
the callback and dereferenced on return; value arguments without reference bindings
retain their native reference-parameter warnings and do not mutate the input.
Callbacks may replace the caller's array while iteration retains the passed array.
Exceptions propagate immediately without invoking later predicates.

Arrays reach **718 / 70 / 52**; the other five suite totals remain unchanged.
All 797 existing Rust tests, 57 reference-checked project PHPT cases, formatting,
and Clippy pass. A short reference probe matches all four functions, empty and
short-circuit behavior, argument-error ordering, reference mutation and return
separation, callback input replacement, and exceptions. No custom tests or generated
matrices were added. Error handlers, destructors, property hooks, PHP memory limits,
and complete native metadata remain incomplete. PHP 8.5.0 array source, native
signatures, upstream PHPT, and reference execution establish these behaviors.

The [standard-stream slice](conformance/php-8.5.0-stdio-hillclimb.txt) adds
**seven upstream passes**: three Zend, three array, and one reflection fixture,
with no regressions across the six suites. `STDIN`, `STDOUT`, and `STDERR` are
stream resources with IDs 1, 2, and 3; their `php://` wrappers create independent
handles. Aliases share closed state, existing duplicates survive a standard handle
close, and reopening a closed original channel fails. Host descriptors are safely
duplicated so a request cannot close the embedding application's handles.

Input drains buffered data and reads at most one additional 8192-byte chunk, allowing
`fread` to return available pipe data without waiting for the entire requested length.
Pipe positions start unknown and advance after I/O. Explicit stderr writes retain
byte order alongside echo, stdout writes, and diagnostics without adding diagnostic
prefixes or newlines. Closing stderr suppresses later displayed diagnostics. Closing
stdout terminates later ordinary output with status 255, while preserving earlier
captured bytes and writes through still-open duplicates.

Arrays reach **721 / 67 / 52**, Zend **2121 / 2727 / 408**, and reflection
**280 / 175 / 37**; SPL, strings, and math retain their totals. All 797 existing
Rust tests, 57 reference-checked project PHPT cases, formatting, and Clippy pass.
Reference probes match stream identities, positions, buffered reads, EOF, I/O errors,
merged output order, wrapper modes, duplicate/alias closes, and exit status. A separate
live-pipe probe verifies prompt reads while the writer remains open. The upstream
`std_streams.phpt` remains unsupported for `CAPTURE_STDIO` and `SKIPIF`, and is not
counted as a pass. No custom tests or generated matrices were added.

Redirected stdout/stderr seeking, full buffer-relative seeking, nonblocking settings,
filters, descriptor inheritance, PHP memory limits, and complete wrapper/context
support remain unfinished. Captured output handles currently remain nonseekable even
when their host descriptor is redirected to a regular file. PHP 8.5.0 CLI and stream
source, upstream PHPT, and reference execution establish these behaviors.

The [formatted-stream/file-read slice](conformance/php-8.5.0-formatted-streams-hillclimb.txt)
adds **14 upstream string passes**, without regressions across the six suites.
`fprintf` and `vfprintf` validate streams before later arguments, share the existing
binary-safe formatter, and return the formatted byte length even when the underlying
write fails. String-conversion exceptions retain the completed partial format buffer,
which is printed or written before the exception propagates. A subsequent format
error aborts the write and chains the earlier conversion error.

`file_get_contents` supports local files and implemented wrappers, include-path
lookup, positive/negative offsets, and nullable length limits. Argument parsing and
context validation follow native order. Reads use PHP's stat-based buffer sizing,
return partial or empty strings after read errors, return false on open/seek failure,
and release their temporary stream. These reads complete the existing formatted-file
round-trip fixtures, including float, integer, character, octal, and hex formats.

Strings reach **349 / 229 / 152**; the other five suite totals remain unchanged.
All 797 existing Rust tests, 57 reference-checked project PHPT cases, formatting,
and Clippy pass. A short reference probe matches binary output, positional/array
formats, readonly and closed streams, argument-error ordering, conversion side effects,
pending/chained exceptions, file offsets and limits, and open/read/seek diagnostics.
The dedicated file-read selection reports **0 / 1 / 21** across 22 fixtures: its
failure needs stream contexts/custom URI parsing, and unsupported cases are not
counted as passes. No custom tests or generated matrices were added. General URL/user
wrappers, configurable contexts/include paths, memory limits, complete filtering and
buffering parity, and native metadata remain incomplete. PHP 8.5.0 formatting/file/
stream source, native signatures, upstream PHPT, and reference execution establish
these behaviors; adapted read code retains PHP's notices and license.

The [Base64/ROT13 slice](conformance/php-8.5.0-base64-hillclimb.txt) adds
**11 upstream passes**: eight string fixtures and three dedicated Base64 fixtures.
`base64_encode` produces padded byte encodings; `base64_decode` supports permissive
character skipping and strict validation. Strict mode permits omitted padding,
accepts only space/tab/CR/LF as whitespace, rejects data after padding and malformed
padding lengths, and rejects a single trailing alphabet character. Unused final
bits are accepted as in PHP. `str_rot13` transforms ASCII letters and preserves
all other bytes. Native parameter binding handles named arguments, scalar/Stringable
coercion, and nullable-scalar deprecations.

Strings reach **357 / 221 / 152**. The other five tracked suites are unchanged,
with zero regressions across all six. The six dedicated Base64 fixtures improve
from **0 / 6 / 0** to **3 / 3 / 0**; the remaining failures depend on missing
`pack` or `json_encode`, and are not counted as passes. All 797 existing Rust tests
and 57 reference-checked project PHPT pass. A short reference probe matches padding,
non-alphabet bytes, whitespace, binary output, coercion side effects, argument-error
order, and named arguments. No custom tests or generated matrices were added.
Encoding/decoding uses scalar Rust loops; SIMD acceleration and PHP memory-limit
configuration remain unimplemented. PHP 8.5.0 source, native signatures, upstream
PHPT, and reference execution establish these behaviors; the decoding adaptation
retains PHP's notices and license.

The [wordwrap slice](conformance/php-8.5.0-wordwrap-hillclimb.txt) adds
**six upstream string passes**, with no regressions across the six tracked suites.
`wordwrap` follows PHP's separate single-byte/no-cut and general wrapping paths,
including spaces at boundaries, long words, existing separators, zero/negative
widths, and native argument-validation order. The general path preserves PHP's
NUL-terminated separator comparison and its treatment of separators ending exactly
at EOF. Empty input returns after argument parsing but before empty-break and
zero-width/cut validation.

Strings reach **363 / 215 / 152**; the other five suite totals remain unchanged.
The focused selection reports **6 / 0 / 2**; its two memory-limit fixtures require
unsupported INI/SKIPIF handling and are not counted as passes. All 797 existing
Rust tests, 57 reference-checked project PHPT, formatting, and Clippy pass. A short
reference probe matches binary separators, negative widths, EOF boundaries,
Stringable side effects, named arguments, null deprecations, and error ordering.
No custom tests or generated matrices were added. Rust storage grows as needed;
PHP allocation sizes, memory-limit configuration, and allocation-failure diagnostics
are not reproduced. The wrapping adaptation retains PHP's notices/license.

The [strtok slice](conformance/php-8.5.0-strtok-hillclimb.txt) adds
**seven upstream passes**: six string fixtures and one Zend regression fixture.
`strtok` retains a copied source and cursor in each `Runtime`, resets them when a
nonnull second argument is supplied, and continues tokenization when that argument
is omitted or null. Each call uses its current byte delimiter set, including NUL
and high bytes. Consuming a delimiter-only remainder releases the saved source;
subsequent continuation warns. Reaching an already exhausted cursor instead returns
false without clearing the source. Parameter conversion finishes before the outer
call updates or borrows state, preserving nested `strtok` calls from Stringable code.

Zend reaches **2122 / 2726 / 408**, strings **369 / 209 / 152**; the other four
suite totals remain unchanged, with zero regressions across all six. The focused
selection reports **6 / 0 / 0**. All 797 existing Rust tests, 57 reference-checked
project PHPT, formatting, and Clippy pass. A short reference probe matches initial
warnings, both exhaustion paths, delimiter changes, explicit null, binary input,
named arguments, validation failures that preserve state, and nested coercion calls.
No custom tests or generated matrices were added. As with other string operations,
PHP memory limits and allocation-failure diagnostics remain incomplete. The native
string.c adaptation retains PHP's notices/license.

The [CSV string slice](conformance/php-8.5.0-csv-hillclimb.txt) adds
**seven upstream string passes**, with zero regressions across the six tracked suites.
`str_getcsv` handles quoted/unquoted fields, doubled enclosures, retained escape
bytes, whitespace before opening enclosures, trailing CR/LF removal, empty records,
and unterminated fields. The parser preserves the native terminating-NUL behavior
when NUL is the escape character, including GH-11982/GH-12151 and oss-fuzz #57392.
UTF-8 scanning preserves multi-byte characters and resets on invalid bytes;
trailing-line-ending detection follows the native remembered-character behavior.

All arguments are converted before separator/enclosure/escape length validation.
Omitting escape emits PHP 8.5's deprecation; an explicit empty escape disables
escaping. Strings reach **376 / 202 / 152**; the other five suite totals are
unchanged. The focused selection reports **7 / 1 / 0**: bug65947 now reaches its
missing `basename` dependency. All 797 existing Rust tests, 57 reference-checked
project PHPT, formatting, and Clippy pass. A short reference probe matches line
endings, binary/UTF-8 fields, invalid bytes, quoting, named arguments, Stringable
side effects, validation order, and deprecations. No custom tests or generated
matrices were added. Configurable character locales, stream CSV APIs, PHP memory
limits, and allocation-failure diagnostics remain unfinished. The parser adaptation
retains PHP's notices/license.

The [path-component slice](conformance/php-8.5.0-path-parts-hillclimb.txt) adds
**17 upstream passes**: nine strings, four Zend fixtures, and four dedicated file
fixtures. `basename`, `dirname`, and `pathinfo` implement POSIX path-string handling,
including trailing/repeated separators, suffix removal, empty/root paths, repeated
parent traversal, and dotfile extensions. `dirname` stops when additional levels
cannot shorten the result, including enormous requested levels. `pathinfo` returns
an array only for exactly `PATHINFO_ALL`; other flag combinations return the first
available selected component in native order. Its dirname field retains the native
C-string NUL truncation while other components preserve their byte lengths.

All five `PATHINFO_*` constants are registered under the standard extension;
`pathinfo`'s reflected default retains the `PATHINFO_ALL` constant expression.
Zend reaches **2126 / 2722 / 408**, strings **385 / 193 / 152**, with the other
four tracked suites unchanged and zero regressions across all six. The dedicated
file selection improves from **0 / 4 / 16** to **4 / 0 / 16**. The focused string
selection reports **9 / 0 / 4**. Unsupported fixtures are not counted as passes.
All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. A short reference probe matches embedded NULs, suffix boundaries,
large levels, flag combinations, named arguments, reflected defaults, coercion
side effects, and deprecations. No custom tests or generated matrices were added.
Windows path rules, non-ASCII-compatible locales, PHP memory limits, and allocation
failure diagnostics remain unfinished. The adaptations retain PHP/Zend notices
and licenses.

The [scalar-value slice](conformance/php-8.5.0-scalar-values-hillclimb.txt) adds
**14 upstream passes**: three Zend, one array, five strings, and five dedicated
conversion fixtures. `intval`, `floatval`/`doubleval`, `boolval`, and `strval` reuse
native cast behavior, including Stringable calls, object/array/resource conversion,
NaN and overflow warnings, and negative zero. `intval` parses non-decimal strings
with sign/prefix/base handling, saturates at signed integer limits, validates the
base before conversion, and preserves the native long-to-C-int base narrowing.
`is_numeric` and `is_scalar` inspect values without invoking Stringable conversion.
The shared numeric parser now includes vertical tabs in leading and trailing
whitespace, also correcting ordinary casts and numeric-string classification.

Zend reaches **2129 / 2719 / 408**, arrays **722 / 66 / 52**, and strings
**390 / 188 / 152**. Reflection, SPL, and math are unchanged; no regressions occur
across the six tracked suites. The dedicated conversion selection improves from
**0 / 5 / 3** to **5 / 0 / 3**. The two predicate fixtures remain failing because
they require `opendir`; they are not counted as passes. All 797 existing Rust tests,
57 reference-checked project PHPT, formatting, and Clippy pass. A short reference
probe matches radix overflow, exponent/base differences, whitespace, NaN, negative
zero, named arguments, validation order, Stringable side effects, and open/closed
resources. No custom tests or generated matrices were added. PHP configuration,
unsupported internal-object conversion handlers, memory limits, and 32-bit platform
behavior remain unfinished. Prefix-rule adaptations retain PHP notices/license.

The [settype slice](conformance/php-8.5.0-settype-hillclimb.txt) adds
**ten upstream passes**: eight Zend fixtures, one array fixture, and the dedicated
typed-property fixture. `settype` mutates its reference with native case-insensitive
type names and aliases, rejects resource/invalid target names, and preserves
PHP 8.5 NaN warnings, including conversion to null. Type-name coercion completes
before the current referenced value is read. Typed assignments use the caller's
strictness, allowing weak coercion back to a property's type and rejecting
incompatible conversions without replacing its value.

Failed string conversion still attempts assignment of an empty string before
propagating the exception. If typed assignment also fails, the new error chains
the conversion exception. Reference probes match this behavior, mutation during
Stringable calls, aliases, named arguments, and weak/strict typed-property handling.
Zend reaches **2137 / 2711 / 408**, arrays **723 / 65 / 52**; the other four
tracked suite totals are unchanged and no regressions occur. The focused selection
reports **9 / 18 / 1**: its failures require `set_error_handler`, and its unsupported
fixture requires INI handling. Neither group is counted as passing. All 797 existing
Rust tests, 57 reference-checked project PHPT, formatting, and Clippy pass. No custom
tests or generated matrices were added. Error-handler callbacks, destruction timing,
unsupported internal-object handlers, PHP configuration, and memory-limit behavior
remain unfinished. Adapted conversion rules retain PHP notices/license.

The [directory-stream slice](conformance/php-8.5.0-directories-hillclimb.txt) adds
**twelve upstream passes**: five directory fixtures, three array fixtures, two
scalar-predicate fixtures, one Zend fixture, and one string fixture. It implements
`opendir`, `readdir`, `rewinddir`, `closedir`, and `scandir`, with native resource
validation, default-directory lifetime, sorting constants, and stream behavior.
A small POSIX adapter owns an open directory handle, so reading and rewinding
continue after the directory is renamed. Raw directory records initialize unused
bytes and preserve PHP's platform-specific record size and type byte.

Zend reaches **2138 / 2710 / 408**, arrays **726 / 62 / 52**, and strings
**391 / 187 / 152**. Reflection, SPL, and math are unchanged. The directory suite
records **5 / 2 / 66**; its remaining executable failures need working-directory
and disk-space functions. The two dedicated scalar-predicate fixtures both pass.
There are zero regressions across these comparisons. All 797 existing Rust tests,
57 reference-checked project PHPT, formatting, and Clippy pass. Reference probes
match directory renames, raw records, seek positions, default-handle replacement,
coercion/validation order, named arguments, and reflection defaults. No custom
tests or generated matrices were added. Directory objects, non-POSIX platforms,
configurable locale collation, general stream wrappers, and unsupported PHPT
sections remain unfinished; unsupported fixtures are not counted as passes.
The adapter supports macOS and 64-bit Linux; native execution was checked on macOS.
Adapted directory rules retain PHP notices/license.

The [natural string comparison slice](conformance/php-8.5.0-natural-comparison-hillclimb.txt)
adds **five upstream string passes**, bringing that suite to **396 / 182 / 152**.
`strnatcmp` and `strnatcasecmp` use the same comparator as natural array sorting,
with native string argument conversion, named arguments, callback dispatch, and
normalized -1/0/1 results. The comparator handles numeric runs, leading zeros,
whitespace, embedded NULs, and case folding without parsing numbers into bounded
integers. Its existing source and license attribution are retained.

All five focused fixtures pass. Weak and strict reference probes match binary
strings, digit ordering, Stringable conversion order, argument errors, nullable
scalar deprecations, named calls, first-class callables, and use with `usort`.
The other five tracked suites retain their totals, with zero regressions across
all six. All 797 existing Rust tests and 57 reference-checked project PHPT pass;
formatting and Clippy pass. No custom tests or generated matrices were added.
Configurable locale case conversion remains unfinished; the comparison uses the
default C-locale byte rules. PHP 8.5.0's `strnatcmp.c`, the wrappers in `string.c`,
native signatures, upstream fixtures, and reference execution establish behavior.

The [exception-handler slice](conformance/php-8.5.0-exception-handlers-hillclimb.txt)
adds **sixteen upstream Zend passes**, bringing Zend to **2154 / 2694 / 408**.
It implements `set_exception_handler`, `get_exception_handler`, and
`restore_exception_handler`, retaining original callable values and null entries
in the handler stack. Registration validates before changing the active handler.
Dispatch resolves the saved callable again after the caller's scope has unwound.
The active handler is temporarily cleared and stacked during invocation, so a
newly installed handler can catch an exception thrown by the previous handler.

Uncaught file/stdin exceptions dispatch through `Runtime::report_uncaught` after
execution has unwound. Handled exceptions return process status 0; handler throws
and explicit exits preserve native fatal/exit behavior. CLI `-r` and initial
compilation failures bypass the handler; parse errors from runtime `eval` remain
eligible. Reference probes match these boundaries, handler replacement/restoration,
callback validation, private-method re-resolution, reference-bearing callback arrays,
by-reference parameter warnings, and error/exit statuses.

The focused directory plus getter fixture records **11 / 0 / 0**; all 29 Zend
fixtures mentioning these APIs record **16 / 13 / 0**. These overlap the full Zend
suite and are not counted twice. The other five tracked suites retain their totals,
with zero regressions across all six. All 797 existing Rust tests and 57 project
PHPT fixtures pass; formatting and Clippy pass. No custom tests or generated matrices
were added. Shutdown callbacks, destructor timing, error handlers, output buffering,
deprecation attributes, and closure-debug metadata still block related fixtures.
Adapted rules from PHP 8.5.0 `Zend/zend_builtin_functions.c` and `Zend/zend.c`
retain Zend license attribution.

The [shutdown callback slice](conformance/php-8.5.0-shutdown-hillclimb.txt) adds
**twelve upstream passes**: eight Zend fixtures and four dedicated shutdown fixtures.
Zend reaches **2162 / 2686 / 408**; the other five tracked suites retain their totals.
`register_shutdown_function` validates and retains a resolved callback, its class
scope/object, and argument values. Callbacks run in registration order, including
callbacks appended while shutdown is running. A handled exception continues the
queue; an exit or unhandled exception stops it. Successful exception handling
preserves an earlier exit status, including a main-script fatal status.

`Runtime::shutdown` performs this phase once after any execution error is reported.
The CLI, PHPT runner, and `run` helper invoke it before returning output. Embedded
callers can invoke it explicitly; unhandled callback errors are reported and
returned, and must not be reported twice. Execution errors retain the `run` helper's
existing error-return contract. No-active-frame argument inspection now preserves
PHP's dynamic-call errors separately from global-script errors. Internal callback
warnings use `Unknown:0`, while native callback exceptions use `[no active file]:0`.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. There are zero regressions across six tracked suites. The dedicated
selection is **4 / 1 / 0**; its remaining failure needs `forward_static_call`.
Reference probes match live queue order, private callbacks, saved targets versus
referenced callback-name mutation, scalar/array snapshots and shared objects,
by-reference parameter warnings, named-argument rejection, eval exceptions,
fatal/exit paths, and stdin/CLI `-r` shutdown. No custom tests or generated matrices
were added. Destructor and output-buffer shutdown phases, error handlers, and
other missing builtins remain unfinished. Adapted shutdown rules retain PHP
license attribution.

The [forwarding callback slice](conformance/php-8.5.0-forward-static-call-hillclimb.txt)
adds **sixteen upstream passes**: eleven Zend, one SPL, and four dedicated
forwarding fixtures. Zend reaches **2173 / 2675 / 408** and SPL **23 / 661 / 94**;
array, reflection, string, and math totals are unchanged. All six tracked suites
have zero regressions. The focused selection records **5 / 1 / 0**, including one
Zend fixture already counted in the full suite. Its remaining shutdown fixture
now reaches the still-missing `hash_hkdf` callback.

`forward_static_call` and `forward_static_call_array` reuse native callback
validation and invocation, forwarding the called class only within the callback's
inheritance chain. Object-bound calls retain the object's class. The variadic
function requires class scope and rejects unknown named arguments after callback
validation; the array function also works globally and forwards named/reference
array entries. Calls use weak parameter coercion even from strict callers, preserve
internal trace frames, and unwrap reference returns. Shared native callback
resolution now supports deprecated `self`, `parent`, and `static` names, including
inherited called scope, referenced array members, and context-specific errors.

All 797 existing Rust tests, 57 project PHPT, formatting, and Clippy pass. Reference
probes match ancestor/sibling calls, object and closure callbacks, first-class
callables, global-scope differences, argument-validation order, strict callers,
reference argument/return behavior, and contextual callbacks across exception and
shutdown boundaries. No custom tests or generated matrices were added. Missing
builtins, complete qualified callback forms, error handlers, and other documented
runtime gaps remain unfinished. Adapted rules retain PHP/Zend license attribution.

The [fallible-warning prerequisite](conformance/php-8.5.0-fallible-warnings-regression.txt)
makes warning sinks return `Result`, propagating failures through numeric and
boolean conversions, casts, operators, array keys, offsets, recursive counting,
base decoding, and character masks. Weak scalar coercion no longer discards
conversion errors. Compiler-only diagnostics retain an infallible sink and
constant-folding path. Reference reads in numeric/NaN conversions release their
borrow before warning emission.

This is internal preparation for synchronous PHP error-handler callbacks; it
does not enable those callbacks or claim new passes. All six full upstream suites
retain exactly the prior pass/fail/unsupported totals, with zero regressions.
All 797 existing Rust tests, 57 reference-checked project PHPT fixtures, formatting,
and Clippy pass. No custom tests or generated matrices were added. Runtime
callback dispatch and reentrant writes still need implementation.

The [error-handler dispatch slice](conformance/php-8.5.0-error-handlers-hillclimb.txt)
adds **111 upstream passes**: 103 Zend, six array, and two reflection. Zend reaches
**2276 / 2572 / 408**, array **732 / 56 / 52**, and reflection **282 / 173 / 37**;
SPL, string, and math totals are unchanged. All six suites have zero regressions.
The 195 handler-related fixtures selected from those same suites improve from
**1 / 176 / 18** to **98 / 79 / 18**, without double-counting their gains.

`set_error_handler`, `get_error_handler`, and `restore_error_handler` retain
original callable values and a stack of handlers and signed 32-bit masks.
Validation precedes registration. `trigger_error` and its `user_error` alias
validate user levels, deprecate E_USER_ERROR, and preserve fatal shutdown and
backtraces when default reporting applies. Handler masks are independent of
`error_reporting` and `@`. Dispatch temporarily clears the active handler,
re-resolves the callable in the current scope, invokes it with weak argument
coercion and four diagnostic arguments, then restores it unless replaced.
Only a literal false return falls back to default display. Handler exceptions
and exits propagate through the existing fallible operation paths.

Runtime conversions, operators, builtin diagnostics, property reads, and callable
warnings now dispatch synchronously. Compile-time E_WARNING/deprecation messages
from eval/includes can dispatch; E_COMPILE_WARNING remains excluded. Compiler
state and source identity are restored across callback execution and failure.
Failed includes retain their call frame and native OS error text. Stream failure
paths release resource borrows before calling handlers. NaN-to-array/object
`settype` conversion wraps the live slot after its warning, observing handler
changes. Diagnostic text now preserves quoted string array keys and PHP's
increment/decrement warning/deprecation messages.

All 797 existing Rust tests, 57 reference-checked project PHPT fixtures,
formatting, and Clippy pass. Four temporary reference probes match exact output
and exit status for registration stacks, masks, nested/replaced handlers, fallback,
strict callers, referenced/invalid callbacks, binary messages, eval warnings,
reentrant stream closure, thrown exceptions, handler exits, fatal errors, and
shutdown. No custom tests or generated matrices were added. Warning-producing
array/reference write traversal, string-offset preparation, and malformed-property
var_dump notices still bypass dispatch. Compound assignment and increment mutation
semantics, last-error APIs, destructor/attribute diagnostics, and other documented
runtime gaps remain unfinished. This supersedes earlier notes listing all error
handler APIs as absent; PHP/Zend attribution covers the adapted dispatch rules.

The [write-diagnostic slice](conformance/php-8.5.0-write-warnings-hillclimb.txt)
adds **7 upstream Zend passes**, reaching **2283 / 2565 / 408**, with no
regressions across all 8,267 fixtures in the six tracked suites. Other full-suite
counts are unchanged. Array-key conversion and false-to-array diagnostics now
invoke PHP handlers after releasing storage borrows. Traversal caches evaluated
keys and append positions while suspended; invalidated destinations abandon the
pending write. False conversion installs the array before notifying the handler.
Instance and static property dimensions use live storage rather than overwriting
handler changes with a detached snapshot.

String writes retain their buffer across offset conversion, replacement conversion,
and truncation warnings, propagating handler exceptions and abandoning invalidated
writes. All 797 existing Rust tests, 57 reference-checked project PHPT, formatting,
and Clippy pass. A temporary reference probe matches exact output and status for
null-key replacement, false conversion, property/static storage, string invalidation,
nested append resumption, and array copies made by handlers. No custom tests or
generated matrices were added. PHP 8.5.0 `Zend/zend_execute.c` and upstream fixtures
establish the behavior; the notices file covers the adapted storage guards.

This supersedes the earlier write-traversal limitation above. Nested lvalue
execution order, copied ancestor arrays during callbacks, compound assignment,
increment/decrement mutation, malformed-property dump notices, and other documented
runtime gaps remain incomplete. Unsupported fixtures remain unsupported.

The [last-error API slice](conformance/php-8.5.0-last-error-hillclimb.txt) adds
**3 upstream passes**: one Zend fixture and both targeted standard API fixtures.
Zend reaches **2284 / 2564 / 408**, with no regressions across the six full suites;
all other full-suite counts remain unchanged. `error_get_last` exposes the latest
default-reported diagnostic independently of suppression, preserving binary
messages, source file, line, and severity. Handled diagnostics leave the prior
record intact, and returned arrays can be modified without changing the record.
`error_clear_last` clears both the record and separately retained fatal trace.
User fatal traces are captured before dispatch, available to handlers and shutdown
callbacks, and cleared by subsequent diagnostics.

All 797 existing Rust tests, 57 project PHPT fixtures, formatting, and Clippy pass.
Two temporary reference probes match exact output and exit status for suppression,
handler fallback, nested errors, independent snapshots, binary messages, fatal
traces, and shutdown clearing (statuses 0 and 255). No custom tests or generated
matrices were added. PHP 8.5.0 `ext/standard/basic_functions.c`, `main/main.c`,
`Zend/zend.c`, and the upstream fixtures establish the behavior. This supersedes
earlier notes listing the last-error APIs as absent. Configurable fatal backtraces,
compile-fatal trace capture, and the other documented gaps remain incomplete;
the INI-based fatal-trace fixture is still unsupported.

The [offset diagnostic slice](conformance/php-8.5.0-offset-diagnostics-hillclimb.txt)
adds **30 upstream passes**: 26 Zend and 4 array fixtures, with no regressions
across the six full suites. Zend reaches **2310 / 2538 / 408** and array reaches
**736 / 52 / 52**; the other full-suite counts are unchanged. Invalid array/string
offsets identify the actual type, with distinct unset and isset/empty errors.
Scalar offset warnings distinguish null, true, false, and numeric/resource types.
String coalescing rejects object and resource keys. `array_key_exists` uses its
own null-key deprecation and preserves the different invalid-key diagnostics of
optimized positional calls and native calls.

All 797 existing Rust tests, 57 project PHPT fixtures, formatting, and Clippy pass.
A temporary reference probe matches exact output and status for direct, dynamic,
alias, named, reordered named, and unpacked calls, including second-argument type
validation before key conversion. No custom tests or generated matrices were
added. PHP 8.5.0 `Zend/zend.c`, `Zend/zend_execute.c`, `ext/standard/array.c`, and
upstream fixtures establish the distinctions. Nested lvalue timing, intermediate
string probes, mutation during compound operations, shortest float formatting in
precision diagnostics, and other documented limitations remain incomplete.

The [float diagnostic precision slice](conformance/php-8.5.0-float-diagnostics-hillclimb.txt)
adds **3 upstream passes**: two Zend string-offset probe fixtures and one array
fixture. Zend reaches **2312 / 2536 / 408** and array **737 / 51 / 52**, with no
regressions across the six full suites. Float-to-int precision and overflow
warnings use the shortest round-tripping decimal, sharing the existing formatter
with `var_dump`. Ordinary string conversion retains its default precision of 14
significant digits. All 797 existing Rust tests, 57 project PHPT, formatting, and
Clippy pass; no custom tests or generated matrices were added. PHP 8.5.0
`Zend/zend_operators.c` and upstream fixtures establish the diagnostic format.
Configurable precision and the other documented runtime gaps remain incomplete.

The [debug-info slice](conformance/php-8.5.0-debug-info-hillclimb.txt) adds
**22 upstream Zend passes**, reaching **2334 / 2514 / 408**, with no regressions
across the six full suites. Fourteen gains implement `var_dump` debug hooks;
eight correct the PHPT environment for native fatal backtraces (seven class-alias
errors and one user fatal). The harness now disables native fatal backtraces in
both Rust and reference PHP, matching PHP 8.5.0 `run-tests.php`. Ordinary CLI
requests keep PHP's enabled default. INI sections remain unsupported.

`var_dump` invokes `__debugInfo` using internal call frames and recursion guards
that span nested dump calls. It handles returned references, integer/mangled keys,
null deprecations, partial output, and malformed-key notices without retaining
storage borrows across PHP callbacks. Dump-time handler exceptions are deferred
while rendering finishes; subsequent handler or hook entry observes the pending
exception. Invalid returns and hook exceptions trigger native fatal reporting,
including shutdown, last-error records, and backtraces. `print_r` shares the
hook-result and deferred-error handling.

All 797 existing Rust tests, 57 project PHPT, formatting, and Clippy pass. The
focused debug-info set is **14 / 0 / 1**; the current `print_r` set is **6 / 0 / 7**.
Five temporary reference probes match exact output and statuses (0 or 255) for
reentry, deferred exceptions, malformed keys, fatal errors, and shutdown. No custom
tests or generated matrices were added. PHP 8.5.0 `ext/standard/var.c`,
`Zend/zend_object_handlers.c`, `Zend/zend.c`, and the official test runner establish
the behavior; the notices file records the adaptations. Later snapshotted-property
mutation, `print_r` callback/output-buffer interleaving and cross-call recursion,
arbitrary INI configuration, lazy objects, and other documented gaps remain.

The [Deprecated attribute slice](conformance/php-8.5.0-deprecated-hillclimb.txt)
adds **37 upstream Zend passes**, reaching **2371 / 2477 / 408**, with no
regressions across all six suites (8,267 fixtures). The focused attribute set
records **40 / 7 / 0**. Calls, global/class constant reads, enum cases, and trait
use emit E_USER_DEPRECATED with binary-safe messages and optional `since` text.
Attribute construction keeps caller strictness and declaration class scope;
magic-call diagnostics retain the requested method name. Handler exceptions
abort the call before entering its body, and internal callbacks retain internal
trace frames. Constant guards span suffix evaluation and handler invocation,
including aliases, nested messages, and throwing handlers. `defined` and
reflection reads do not emit a warning for the queried constant. Deprecated
traits defer binding so execution-time error handlers observe their warnings.
Invalid class/interface/enum targets report compile errors, with delayed target
validation handled by reflection.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. One temporary reference probe matches exact output and exit status
for nested constant diagnostics and recursive handlers, including caller versus
scoped constant-expression locations. No custom tests or generated matrices
were added. PHP 8.5.0 `Zend/zend_execute.c`, `Zend/zend_constants.c`,
`Zend/zend_ast.c`, `Zend/zend_attributes.c`, and `Zend/zend_inheritance.c` establish
the behavior. The seven focused failures require destructors (two), `random_int`
(three), `Closure::fromCallable` (one), or `Random\IntervalBoundary` (one).
General constant-expression diagnostic overrides outside attribute deprecations
and other documented runtime gaps remain unfinished.

The [NoDiscard attribute slice](conformance/php-8.5.0-nodiscard-hillclimb.txt)
adds **14 upstream Zend passes**, reaching **2385 / 2463 / 408**, with no
regressions across all six suites (8,267 fixtures). Thirteen gains are focused
attribute fixtures; one covers `(void)` casts in `for` expressions. The focused
NoDiscard set is **17 / 4 / 4**, and Deprecated remains **40 / 7 / 0**.
NoDiscard emits E_USER_WARNING before entering a call whose result is discarded.
It shares attribute construction, binary-safe messages, declaration scope,
caller strictness, and error-handler dispatch with Deprecated. Deprecation runs
first; throwing handlers prevent subsequent diagnostics and function execution.
Magic methods retain the requested name. Nullsafe calls, pipes, and `@` preserve
whether the result is consumed, while `(void)`, assignments, and intermediate
chain calls suppress the warning. Inline callback calls propagate result use;
ordinary internal callback wrappers retain PHP's consumed-result behavior.
Unsupported void/never declarations and forbidden magic methods report compile
errors.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. One temporary reference probe matches exact output and exit status
for nullsafe chains, pipes, suppressed calls, casts, conditional/coalescing
expressions, and callback wrappers. No custom tests or generated matrices were
added. PHP 8.5.0 `Zend/zend_execute.c`, `Zend/zend_vm_def.h`,
`Zend/zend_attributes.c`, and `Zend/zend_compile.c` establish the behavior.
Remaining focused failures require DateTimeImmutable, Random\IntervalBoundary,
and property hooks; four extension/INI fixtures remain unsupported.

The [assertion slice](conformance/php-8.5.0-assert-hillclimb.txt) adds **20
upstream Zend passes**, reaching **2405 / 2443 / 408**, with no regressions across
all six suites (8,267 fixtures). The focused Zend/assert-dependent set records
**16 / 1 / 26**; standard assertion fixtures record **2 / 0 / 18**. Assertions
are enabled by default. `assert` validates descriptions, throws supplied
Throwable objects unchanged, invokes configured callbacks, and creates
AssertionError with code 1, preserving callback exceptions as previous errors.
`assert_options` controls activation, callbacks, warnings, exceptions, and
bailout; it and the ASSERT_* constants emit PHP 8.5 deprecations. Direct calls
receive compiler-generated descriptions for supported expressions, closures,
and class declarations. Source name spelling and numeric interpolation offsets
are preserved, and constructor promotion now accepts final properties.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No new custom tests or generated matrices were added. Extracted
FILE sections from 19 upstream INI fixtures whose settings match the implemented
defaults were also compared directly: 15 match exact output and exit status;
four require `preg_match`, `ini_get`, `ini_set`, or generator syntax. These manual
checks do not change their unsupported PHPT classification. PHP 8.5.0
`ext/standard/assert.c`, `Zend/zend_ast.c`, `Zend/zend_compile.c`, and
`Zend/zend_language_scanner.l` establish the behavior. Assertion INI configuration,
disabled assertion compilation, complete AST description coverage (including
complex statements, traits, and original magic-constant spelling), binary warning
messages, and the remaining operator type errors are unfinished.

The [Override attribute slice](conformance/php-8.5.0-override-hillclimb.txt)
adds **23 upstream Zend passes**, reaching **2428 / 2420 / 408**, with zero
regressions across all six suites (8,267 fixtures). Two gains cover
DelayedTargetValidation. It validates methods and properties after inheritance
compatibility checks. Methods
can satisfy non-private parent declarations, interface declarations, or distinct
abstract trait requirements. Constructors require an abstract or interface
prototype. Concrete trait methods hidden by a class declaration do not qualify.
Trait declarations defer validation until composition; inherited declarations
remain validated in their original class. Static and promoted properties use
the same inherited-member requirement. Failed declarations emit E_COMPILE_ERROR,
terminate with status 255, and bypass catch/finally and user handlers. Method
locations retain their defining source, including imported trait methods.
Promoted internal attributes are assigned to their supported target: Override
to the property and SensitiveParameter to the parameter. Assertion descriptions
retain the original attribute list.

The focused set improves from **19 / 31 / 0** to **40 / 10 / 0**. Its remaining
ten failures require generator syntax (one) or interface property hooks (nine).
All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or generated matrices were added. PHP 8.5.0
`Zend/zend_inheritance.c`, `Zend/zend_compile.c`, and `Zend/zend.c` establish the
behavior. Property-hook override checks and other documented inheritance gaps
remain unfinished.

The [secure random function slice](conformance/php-8.5.0-csprng-hillclimb.txt)
adds **11 Zend passes** and **one array pass**, reaching **2439 / 2409 / 408**
and **738 / 50 / 52**, respectively. There are zero regressions across all six
suites (8,267 fixtures). Four upstream random-function fixtures improve from
**0 / 4 / 0** to **4 / 0 / 0**; all 15 focused random-function/dependent cases pass.
Three gains unblock existing Deprecated attribute tests.

`random_bytes` and `random_int` validate native integer parameters and use
operating-system entropy: CCRandomGenerateBytes on macOS and blocking getrandom
on Linux. Integer sampling uses rejection to avoid modulo bias, includes both
endpoints, covers the full signed 64-bit range, and returns equal endpoints
without requesting entropy. Failed entropy requests throw Random\RandomException,
which inherits Exception. Linux handles short reads and interruptions; other
operating systems and kernels lacking getrandom remain unsupported.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or generated matrices were added. One direct CLI
probe matches reference construction, inheritance, message/code, catch behavior,
and exit status for Random\RandomException. Local runtime validation uses macOS;
Linux compilation is covered by CI. Entropy-source failures and allocation
failures were not injected. PHP 8.5.0 `ext/random/random.c`, `ext/random/csprng.c`,
`ext/random/random.stub.php`, and the installed CommonCrypto header establish
the implementation. Mersenne Twister, rand/mt_rand, Random engines, Randomizer,
and general internal reflection metadata remain unfinished.

The [legacy random-function slice](conformance/php-8.5.0-mt-hillclimb.txt)
adds **15 passes** across the six full suites (8,267 fixtures), with zero
regressions: Zend **2442 / 2406 / 408**, array **748 / 40 / 52**, reflection
**283 / 172 / 37**, and math **52 / 2 / 117**. The complete upstream random
function suite improves from **5 / 12 / 0** to **17 / 0 / 0**. Including seven
focused array_rand fixtures, the focused run records **24 / 0 / 0**.

`mt_rand` and `rand` share request-local MT19937 state with `mt_srand` and `srand`.
Explicit seeds truncate to 32 bits; default/null seeds use OS entropy with a
noncryptographic fallback. PHP's legacy MT_RAND_PHP sequence and range scaling
are preserved, with both constant and seeding deprecations. The standard engine
uses 32-bit or 64-bit rejection sampling as appropriate, preserves draw order
for full and singleton ranges, and enforces PHP's retry limit. `rand` accepts
inverted bounds; `mt_rand` rejects them. Maximum-value aliases return 2147483647.
Native zero-or-two arity and missing named arguments retain PHP diagnostics.
`array_rand` shares the generator while using unbiased range selection even in
legacy mode, returns unique keys in source order, and uses complement selection
for large requests. `lcg_value` implements the separately seeded combined LCG
and its PHP 8.5 deprecation. Random\RandomError and
Random\BrokenRandomEngineError provide the random-error hierarchy.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or generated matrices were added. The upstream
fixtures check exact sequences across MT reloads and range bias; one direct CLI
probe also matches PHP output and exit status for shared alias state, singleton
ranges, 32/64-bit boundaries, full signed ranges, and named-argument errors.
PHP 8.5.0 `ext/random/engine_mt19937.c`, `ext/random/random.c`,
`ext/random/random.stub.php`, and `ext/standard/array.c` establish the behavior;
the underlying Mersenne Twister license is retained in THIRD_PARTY_NOTICES.md.
The array representation lacks Zend's physical bucket history: single-key
sampling is uniform but exact seeded sequences may differ for sparse/deleted
arrays. The entropy-failure seed mixer differs from PHP's fallback, and failure
injection was not exercised. Random engine objects, Randomizer, shuffling,
and full internal reflection metadata remain unfinished.

The [shuffle slice](conformance/php-8.5.0-shuffle-hillclimb.txt) adds **12
upstream passes**, with zero regressions across all six suites (8,267 fixtures).
Array reaches **757 / 31 / 52** and strings **399 / 179 / 152**. All eight
focused shuffle fixtures pass; the expanded set records **12 / 1 / 0**, with
the remaining Zend backtrace failure requiring destructor execution.

`shuffle` and `str_shuffle` apply Fisher–Yates swaps through the shared MT19937
state. They use unbiased range selection even in legacy mode. Array shuffling
validates a reference argument, separates shared storage, renumbers keys,
preserves reference containers, updates active foreach positions, and resets
the internal pointer and next append index. Empty arrays preserve their table
state. String shuffling is binary-safe and does not seed or advance the generator
for zero- or one-byte strings. Nonempty array calls initialize the generator;
singleton arrays renumber without consuming a draw.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or generated matrices were added. PHP's statistical
fixture checks 90,000 array and 90,000 string permutations. One direct CLI probe
also matches reference values, copy-on-write, active reference iteration, cursors,
append indices, binary strings, singleton arrays, legacy RNG sharing, and exit
status. A transient file-not-found runner error cleared on a full math rerun.
PHP 8.5.0 `ext/standard/array.c` and `ext/standard/string.c` establish the
implementation. Physical bucket history and destructor execution remain
unfinished; other previously documented runtime gaps still apply.

The [object destruction slice](conformance/php-8.5.0-destructors-hillclimb.txt)
adds **62 upstream passes** across the six suites, with **zero regressions**.
Zend reaches **2502 / 2346 / 408** and array **759 / 29 / 52**; reflection,
SPL, strings, and math retain their previous counts. The source-selected
destructor fixtures, complete Zend GC directory, and reflection constructor
failure fixture improve from **41 / 168 / 65** to **101 / 108 / 65**.

Destructors execute on reference release, discarded results, local cleanup, and
shutdown. A request queue transfers dying objects out of Rust Drop so PHP code
can safely run and resurrect them. Object handles and typed-property reference
constraints survive resurrection. Calls use inherited methods, visibility,
attribute diagnostics, and exception chaining. Failed constructors suppress
automatic destruction through both ordinary and reflection instantiation.
Invalid static, parameterized, and return-typed destructor declarations produce
compile diagnostics. Shutdown callbacks run first, followed by unique globals
in reverse order and remaining objects in handle order; reference-bound globals
remain available during the final sweep.

`gc_collect_cycles` implements trial deletion over tracked object properties,
arrays, and reference cells. External references protect live roots, reentrant
collection returns zero, and reachability is recomputed after destructors to
protect resurrection. Array-only cycles, closure/native payload cycles,
automatic collection thresholds, and GC status/configuration APIs remain
unfinished. Some temporary cleanup locations, reentrant assignment behavior,
complex collection counts, and shutdown exception chaining also remain incomplete.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or matrices were added. Direct CLI reference probes
also match resurrection, cleanup order, typed aliases, live cycle roots,
reentrant collection, basic collected counts, output, and exit status.
PHP 8.5.0's Zend object lifecycle, executor shutdown, and VM sources establish
the destruction behavior; other previously documented runtime gaps still apply.

The [splice destructor slice](conformance/php-8.5.0-splice-destructors-hillclimb.txt)
adds **nine upstream array passes**, with **zero regressions** across all six
suites. Array reaches **768 / 20 / 52**; the other five suites retain their
previous counts. The focused set improves from **9 / 11 / 1** to **18 / 2 / 1**.
Seven gains cover destructor-driven splice mutations, and two remove Rust borrow
panics when `array_pop` or `array_shift` removes a self-reference.

Discarded splice removals now delete entries and invoke destructors one at a
time before rebuilding the table. The native operation retains the original
array while user writes separate it. An unretained original produces PHP's
modification error; retained aliases and exception trace arguments observe the
completed table. Later deletions continue after a destructor throws, preserving
exception chains and internal callback trace sites. Scalar-only removals retain
the linear bulk path. Iterator mapping, key renumbering, and pointer reset remain
shared with ordinary splicing. Pop/shift dereference their removed values only
after releasing the source variable's mutable borrow.

The private splice guard uses a narrowly scoped unsafe write to implement PHP's
native exception to copy-on-write. Its input is separated first, and its two
mutation methods execute no PHP or retain borrowed table views. The aliasing
requirements follow Rust's [Rc mutable access contract](https://doc.rust-lang.org/std/rc/struct.Rc.html#method.get_mut_unchecked).
Eleven affected upstream FILE extracts also execute under Miri and match reference
output/status. Miri requires an isolated copy that skips its unsupported stdin
seek probe; splice code is unchanged, and leak checking is disabled because the
runtime still has incomplete cycle collection. This is additional validation,
not additional PHPT passes or a general memory-safety proof.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or matrices were added. Direct reference probes
also match partial-array visibility during destruction, retained aliases,
multiple throwing destructors, exception-chain getters, and trace arguments.
Sparse physical bucket history, complete iterator behavior under reentrancy,
coercion callback timing, and previously documented runtime gaps remain.

The [Countable slice](conformance/php-8.5.0-countable-hillclimb.txt) adds **six
passes across the six suites**, plus **one general-functions pass**, with
**zero regressions**. Array reaches **772 / 16 / 52** and SPL **25 / 659 / 94**;
the other suites retain their counts. The focused Countable/count/sizeof/
is_countable set improves from **7 / 19 / 3** to **14 / 12 / 3**.

The internal Countable interface exposes a tentative `int` return type, using
existing interface compatibility and ReturnTypeWillChange handling. `count` and
`sizeof` validate the optional mode before checking the value, dispatch object
methods with native trace context, convert results using PHP's non-strict integer
rules, and propagate exceptions and result destructors. `is_countable` recognizes
arrays and Countable instances without invoking user code. Recursive array
counting preserves recursion protection across warning-handler calls, removes
it on exceptions or exit, and names the invoked alias in its diagnostics.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or matrices were added. A direct CLI reference probe
also matches object counting, aliases, tentative-return reflection, mode-validation
order, and recursion reentry through an error handler, including output/status
with equivalent CLI diagnostic settings. PHP 8.5.0's standard array/type functions,
Zend interface declarations, and scalar conversion code establish the behavior.
Extension-native counting handlers, SPL containers/iterators, full reflection
strings, intersection/DNF types, and other documented runtime gaps remain.

The [user iterator slice](conformance/php-8.5.0-iterators-hillclimb.txt) adds
**15 upstream passes** across the six full suites, with **zero regressions**:
12 Zend, two reflection, and one SPL. Zend reaches **2514 / 2334 / 408**,
reflection **285 / 170 / 37**, and SPL **26 / 658 / 94**. The array, string, and
math counts are unchanged. All **14 additional upstream iteration fixtures** in
`tests/classes/iterators_*.phpt` and `tests/lang/foreachLoopIterator*.phpt` pass;
these are supplementary measurements, not counted in the 15-pass increase.

Traversable, Iterator, and IteratorAggregate use native interface metadata,
including tentative return types and ReturnTypeWillChange. Concrete classes must
implement Traversable through Iterator or IteratorAggregate and cannot implement
both. User-defined foreach resolves nested aggregates, calls rewind/valid/current/
key/next in PHP order, obtains keys only when requested, preserves current-result
references until assignment, and propagates exceptions and loop control. Invalid
aggregate returns and iterator iteration by reference raise PHP's diagnostics.
Temporary aggregates are released after initial rewind/valid, with the resolved
iterator and cached current value kept alive for their required lifetimes.
Iterable declarations, is_iterable, and ReflectionClass::isIterable/isIterateable
recognize user iterators without invoking their methods.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No custom tests or generated matrices were added. Direct reference
CLI checks also match temporary aggregate destruction, reference-returning current
values changed by key(), reference restrictions, iterable declarations, tentative
return reflection, and iterable enum reflection. The implementation follows PHP
8.5.0's Zend interface/foreach source and native type/reflection functions.
Generators, fibers, iterator argument/array unpacking, SPL iterator containers and
helpers, property hooks, deep recursion limits, and prior reflection gaps remain.

The [iterator helper slice](conformance/php-8.5.0-iterator-helpers-hillclimb.txt)
adds **two upstream passes**, with **zero regressions** across the six full suites.
SPL reaches **28 / 656 / 94**; the other suite counts remain unchanged. The focused
33-fixture helper set reaches **2 / 27 / 4**. Most remaining focused cases require
SPL containers, generator execution, or property hooks; unsupported section
combinations remain unsupported.

iterator_to_array accepts arrays and user iterators, optionally preserves keys,
retains reference-returning current values, converts keys with PHP diagnostics,
and destroys replaced duplicate-key values at the native boundary. iterator_count
uses rewind/valid/next without requesting values or keys. iterator_apply validates
its Traversable, callback, and optional argument array in order, honors named
arguments and references with native weak callback coercion, counts the final
false callback result, and stops without advancing again. Callback and iterator
exceptions propagate; the shared traversal releases iterators on failure.
SeekableIterator, RecursiveIterator, and OuterIterator extend Iterator with native
method signatures and tentative returns. Aggregate hooks now reject reference
returns, matching PHP's raw return-value check, and keys are detached from returned
references when fetched.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, and
Clippy pass. No new custom tests or generated matrices were added. Direct CLI
reference checks cover method order, reference-preserving conversion, duplicate-key
destruction, named callback references, weak callback coercion, early stopping,
exception/error ordering, native interface inheritance and tentative types, and
reference-returning aggregates. PHP 8.5.0's SPL iterator implementation and stubs,
Zend interface hooks, and native array-key insertion establish these semantics.
SPL iterator classes, generators, serialization, property hooks, deep recursion
limits, and prior reflection gaps remain incomplete.

The [ArrayIterator storage slice](conformance/php-8.5.0-array-iterator-hillclimb.txt)
adds **36 upstream passes**, with **zero regressions** across the six full suites:
ten Zend and 26 SPL. Zend reaches **2524 / 2324 / 408** and SPL **54 / 630 / 94**;
the other full-suite counts are unchanged. This is native ArrayIterator storage
and iteration support, not a claim that its complete API is implemented.

ArrayIterator now constructs array or object storage, exposes cursor movement and
seek errors, reads/writes/unsets offsets, appends, counts, returns array copies,
and provides native debug output. Clones retain their original as backing storage
and have independent cursors. Native iteration bypasses method dispatch until an
Iterator method is overridden, preserves raw reference values for helper calls,
and permits foreach by reference, including typed-property/readonly checks on
object backing. Direct native offset paths support nested writes and references;
isset distinguishes null slots from offsetExists. Object backing emits PHP 8.5's
deprecation, uses string property keys, and skips non-public properties during
iteration/counting. STD_PROP_LIST controls casts/export; constructor reentry
accumulates flags while setFlags replaces them.

The new native payload follows object construction, cloning, destructor transfer,
and cycle collection. Ordinary properties are destroyed before backing storage.
Existing Rust tests (797), reference-checked project PHPT (57), formatting, and
Clippy pass; no custom tests or matrices were added. Direct PHP CLI comparisons
also match storage sharing, cursor independence, offset reference mutation,
null-slot queries, casts, seek failure state, offset errors, reference iteration,
cycle counts, destruction order, constructor reentry flags, and numeric properties
on object backing. The implementation follows PHP 8.5.0's SPL array source/stubs.

Remaining ArrayIterator work includes sorting methods, serialization/Serializable,
ARRAY_AS_PROPS routing, native object comparison, exact overloaded-object rejection,
and some cursor mutation/diagnostic edges. RecursiveArrayIterator and ArrayObject
are not implemented by this slice. Other existing generator, fiber, property-hook,
reflection, and runtime gaps remain.

The [ArrayIterator sorting/property slice](conformance/php-8.5.0-array-iterator-ops-hillclimb.txt)
adds **one upstream pass**, with **zero regressions** across the six full suites.
SPL reaches **55 / 629 / 94**; the other full-suite counts are unchanged.

ArrayIterator now exposes asort, ksort, uasort, uksort, natsort, and natcasesort
through the existing array sort engine. Callback validation belongs to the
underlying function's internal frame. An object-local guard rejects direct writes,
removals, and writable offset references during sorting, while PHP's nested-unset
exception remains allowed. The result table is installed even when comparison
throws. Constructor reentry preserves the active guard and storage slot. Sorting
object backing retains the reordered property table in snapshots and clones.

ARRAY_AS_PROPS routes reads, writes, probes, unsets, nested operations, and references
to backing offsets when no accessible initialized ordinary property occupies the
name. This includes inaccessible and uninitialized declared properties. Existing
offset overrides participate; property_exists handles null entries and inherited
offset hooks. Ordinary initialized properties retain priority over array entries.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, Clippy,
and diff checks pass. No custom tests or matrices were added. Direct PHP CLI
comparisons also cover all six sorting methods, callback errors and traces,
mutation guards and nested unsets, constructor reentry, object backing order and
cloning, property visibility/uninitialized slots, null entries, references, and
offset/magic overrides with flag changes.

Serialization/Serializable, native object comparison, disabled-function support,
precise overloaded-object rejection, and remaining cursor/diagnostic edges are
still incomplete. ArrayObject, RecursiveArrayIterator, and the previously listed
runtime and PHPT-harness gaps remain outside this slice.

The [ArrayObject slice](conformance/php-8.5.0-array-object-hillclimb.txt) adds
**110 upstream passes**, with **zero regressions** across the six full suites:
nine Zend, two reflection, 98 SPL, and one array fixture. Zend reaches
**2533 / 2315 / 408**, reflection **287 / 168 / 37**, SPL **153 / 531 / 94**,
and arrays **773 / 15 / 52**. Strings and math retain their previous counts.

ArrayObject now shares the native array/object backing, offsets, flags, property
routing, sorting, counting, and debug handlers. exchangeArray returns the previous
storage and replaces the live backing; getIterator creates the configured
ArrayIterator subclass with shared storage and flags, bypassing its constructor.
Iterator class names are autoloaded, validated, and retained canonically. Native
allocation also permits abstract iterator subclasses as PHP does. ArrayObject
clones duplicate backing data; ArrayIterator clones continue to share their source.

Native ArrayObject/ArrayIterator comparison examines backing symbol tables before
ordinary properties, including PHP's class-check ordering and self-backed case.
Iterator wrappers now reserve an object-store handle and release it after the
iterator data, matching visible ID reuse. Sorted object dumps retain native
property order. Writes and references through $this offsets are allowed while
reassignment of $this remains rejected. Native array/object parameter errors use
PHP's legacy parser label, and incompatible Closure backing is rejected.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, Clippy,
and diff checks pass. Two additional upstream fixtures outside the six suites pass
(get_object_vars_variation_005 and general_functions/bug47027); they are not counted
in the 110-pass increase. Direct PHP comparisons match factory constructor bypass,
shared iterator storage after exchange, independent clones, class-validation state,
$this offset references, native comparison, and abstract iterator factories. No
custom tests or matrices were added.

Serializable/serialization, RecursiveArrayIterator and other SPL iterator classes,
exact overloaded-object handling beyond Closure/enum checks, cursor mutation edges,
and previously listed runtime/harness limitations remain incomplete.

The [RecursiveArrayIterator slice](conformance/php-8.5.0-recursive-array-iterator-hillclimb.txt)
adds **six upstream SPL passes**, with **zero regressions** across the six full
suites. SPL reaches **159 / 525 / 94**; other full-suite counts are unchanged.

RecursiveArrayIterator extends ArrayIterator and implements RecursiveIterator,
with tentative child-method return types and CHILD_ARRAYS_ONLY. hasChildren reads
the native current entry; getChildren returns null at the end, reuses compatible
object children, filters objects under CHILD_ARRAYS_ONLY, and otherwise invokes
the same subclass constructor with the current value and flags. Scalar child
requests reach the inherited constructor's native type error. Constructor calls
use the internal callback frame, and failed child construction runs the child's
destructor, following this factory's behavior rather than ordinary new.

Array child construction installs an internal parent storage link. Direct child
writes update that parent entry; writes through the parent detach it, repeated
getChildren calls replace the current link, and child sorting separates its result
from the parent. Constructor reentry reconnects array storage. Weak links avoid
keeping parents alive, and child-slot lookup is refreshed when destructor payloads
move to queued objects. Ordinary PHP references remain distinct from these links.

All 797 existing Rust tests, 57 reference-checked project PHPT, formatting, Clippy,
and diff checks pass. Direct PHP CLI comparisons match child writes, parent
separation, repeated children, constructor reentry, sort detachment, subclass
construction, scalar errors/traces, and constructor-failure destruction. No custom
tests or matrices were added.

That slice retained a verified limitation: if a nested array is copied out after getChildren,
PHP's direct native child writes also mutate those captured array copies, while
Rust currently separates them through copy-on-write. For example, after
$p = new RecursiveArrayIterator([[1]]); $c = $p->getChildren(); $copy = $p[0];
$c[0] = 2;, PHP exposes 2 in $copy[0] and Rust retains 1. This slice does not claim
complete child alias semantics. RecursiveIteratorIterator and other SPL iterator
classes, serialization, and prior runtime/harness gaps also remain incomplete.

The [shared native-array storage slice](conformance/php-8.5.0-shared-array-storage-hillclimb.txt)
resolves the captured-array limitation above. Parent and child share the same
hash table after child construction. Native child writes, removals, nested writes,
and writable references reach captured array aliases; ordinary writes still
separate through copy-on-write. Parent writes, repeated children, constructor
reentry, and child sorting retain their separation behavior.

PhpArray now uses checked interior mutability for native shared writes. The same
storage also replaces the unsafe shared-Rc mutation previously used by native
array_splice. Callback-facing reads release their table borrow before running
PHP and track live iteration positions through removals. Snapshot and dump reads
preserve PHP's distinction between shared and unshared reference containers.

All six full-suite counts are unchanged, with **zero regressions** across 8,267
fixtures. All **797 existing Rust tests**, **57 reference-checked project PHPT**,
formatting, Clippy, and diff checks pass. The 35 upstream fixtures affected during
the storage refactor were rerun and recovered. No custom tests or matrices were
added. Direct CLI comparisons also match the captured-alias reproduction and
native writes during foreach, array_map, and __debugInfo. This storage slice does
not claim new full-suite passes. Sorted-set callback mutation, physical bucket
edges, other SPL classes, serialization, and earlier runtime gaps remain.

The [RecursiveIteratorIterator slice](conformance/php-8.5.0-recursive-iterator-hillclimb.txt)
adds native recursive traversal in LEAVES_ONLY, SELF_FIRST, and CHILD_FIRST modes.
The state machine preserves hasChildren/getChildren ordering, depth limits,
begin/end iteration and children hooks, nextElement, and CATCH_GET_CHILD recovery.
Native foreach uses the internal traversal unless an Iterator method is overridden;
subclass Iterator overrides then use normal user dispatch. Inner current results
are cached for user iterators, while native array iterators read live storage.

Construction accepts a RecursiveIterator or a single IteratorAggregate result,
reports invalid aggregates with LogicException, supports reinitialization, and
retains the constructor's constant default in reflection. Inner/subiterator access,
method forwarding, clone restrictions, uninitialized-state errors, and native
wrapper handle allocation are integrated. Child frames are released deepest-first;
loop cleanup and cycle collection include native storage and cached values.

This slice adds **44 upstream passes**, with **zero regressions** across the six
full suites: **43 SPL** and **one Zend**. Zend reaches **2534 / 2314 / 408** and
SPL **202 / 482 / 94**; the other counts remain unchanged. All **797 existing
Rust tests**, **57 reference-checked project PHPT**, formatting, Clippy, and diff
checks pass. No custom tests or matrices were added. Direct CLI comparison also
matches current caching, depth bounds, constructor reentry, and reflection defaults.
The remaining destructor-order fixture is blocked by a reserved-class eval
message mismatch. Other SPL classes, arbitrary callback reentry, generators,
serialization, and earlier runtime/harness gaps remain incomplete.

The [SplFixedArray slice](conformance/php-8.5.0-fixed-array-hillclimb.txt)
adds dense native storage, construction and resizing, offset conversion and bounds
checks, array conversion, cloning, and serialization hooks. Shrinking exposes the
new size before running removed values' destructors, uses PHP's removal order,
and defers nested resize requests. Replacement values are installed before old
values are destroyed. Native storage participates in cycle collection.

Foreach uses independent native cursors over live storage, bypassing offset
method overrides; explicit getIterator returns an InternalIterator with lazy
rewinding, constructor/clone restrictions, and native wrapper handle lifetimes.
Subclass getIterator overrides use ordinary aggregate dispatch. Direct iteration
rejects references. Offset overrides, empty/isset/coalescing, and unsupported
append operations follow the native handlers.

Native elements are included in dumps, array casts, print_r, and var_export,
while ordinary property inspection excludes them. SplFixedArray's get_object_vars
retains mangled property names as PHP does. The new get_mangled_object_vars
builtin exposes ordinary object properties without visibility filtering.
JsonSerializable and method return-type metadata are registered; member loading
preserves native visibility resolution and readonly restrictions.

This slice adds **56 upstream passes**, with **zero regressions** across all six
full suites: **54 SPL** and **two Zend**. Zend reaches **2536 / 2312 / 408** and
SPL **256 / 428 / 94**; other suite totals are unchanged. All **797 existing Rust
tests**, **57 reference-checked project PHPT**, formatting, Clippy, and diff checks
pass. No custom tests or matrices were added. Direct CLI comparisons also match
property loading/inspection, InternalIterator restrictions, and native cycle
collection behavior. Remaining related fixtures need general serialization,
JSON encoding, debug_zval_dump, generators, or other SPL classes. General PHP
compatibility and earlier runtime/harness gaps remain incomplete.

The [linked-list slice](conformance/php-8.5.0-linked-list-hillclimb.txt) adds
SplDoublyLinkedList, SplQueue, and SplStack native storage, insertion/removal,
peek operations, offsets, count/isEmpty, debug output, cloning, and explicit
serialization hooks. Stable node identities preserve cursor behavior when values
are removed. Normal destruction releases ordinary properties before list values,
with list values released from the tail; cycle collection includes stored values.

Each native foreach owns an independent cursor and captures its starting mode.
FIFO/LIFO traversal, delete/keep behavior, nested loops, manual next/prev/rewind,
and frozen queue/stack directions follow the native handlers. Clone allocation
resets the cursor to the head and reapplies subclass direction flags even when
__unserialize changed the original flags. Offset methods retain untyped reflection
metadata while applying PHP's native integer argument coercion. The array-based
__unserialize hook appends storage and restores flags and ordinary members.

Native iterator wrappers are shared with SplFixedArray in a dedicated module.
The native member loader is shared through the object implementation. This slice
adds **46 upstream passes** with **zero regressions** across all six full suites.
SPL reaches **302 / 382 / 94**; the other totals are unchanged. All **797 existing
Rust tests**, **57 reference-checked project PHPT**, formatting, Clippy, and diff
checks pass. Direct CLI comparisons also match cursor behavior, cloning after
flag restoration, offset metadata/coercion, hooks, and cycle collection. No custom
tests or matrices were added. Legacy Serializable methods/interface, general
serialization, other SPL classes, and earlier runtime/harness gaps remain incomplete.

The [serialization encoder slice](conformance/php-8.5.0-serialization-encoder-hillclimb.txt)
adds `serialize` for scalars, binary strings, arrays, ordinary objects, and enum
cases. Reference tables distinguish shared variable bindings from repeated object
handles. `__serialize`, `__sleep`, and user-defined Serializable callbacks follow
PHP's precedence, return validation, and nested serialization context rules.
Serializable declarations emit the legacy-interface deprecation. Closures,
anonymous classes, and implemented native nonserializable objects reject encoding.

ArrayObject and ArrayIterator now expose array-based serialization hooks for
flags, backing storage, ordinary members, and iterator classes. Exception encoding
includes native property names and values. Anonymous-class argument names in
exception traces use their display name.

This slice adds **28 upstream passes**, with **zero regressions** across seven
full suites (**8,430 fixtures**): ten Zend, one SPL, one strings, and sixteen in
the newly tracked standard serialization suite. That suite was baselined at the
preceding commit and reaches **16 / 119 / 28**. Zend reaches **2546 / 2302 / 408**,
SPL **303 / 381 / 94**, and strings **400 / 178 / 152**. All **797 existing Rust
tests**, **57 reference-checked project PHPT**, formatting, Clippy, and diff checks
pass. No custom tests or matrices were added. Direct CLI comparisons also match
ArrayObject hook restoration and exception serialization. `unserialize`, legacy
native Serializable methods, configurable precision, and complete mutation and
destructor timing remain incomplete. Encoding currently has a 1,024-level nesting
guard; this does not establish PHP's full recursion-limit behavior.

The [unserialization slice](conformance/php-8.5.0-unserialization-hillclimb.txt)
adds scalar, binary-string, array, object, enum, and reference decoding. An
explicit parsing stack handles compound values without recursive Rust calls.
Parser slots preserve shared references, repeated object handles, and references
to values parsed earlier. Typed properties enforce strict assignment and retain
constraints when their slots become references.

The decoder supports allowed_classes and max_depth options, autoloading,
constructor-free object allocation, user Serializable payloads, and deferred
__unserialize/__wakeup hooks. Nested legacy callbacks share reference state;
magic callbacks use isolated contexts. Unknown or disallowed classes become
__PHP_Incomplete_Class. Its property reads warn, mutations and method calls fail,
and reserialization preserves the original class name. Malformed input reports
offsets and applicable warnings; deprecated S strings and trailing data are
recognized. Object dumps now preserve non-UTF-8 class-name bytes.

This slice adds **128 upstream passes**, with **zero regressions** across all
seven full suites (**8,430 fixtures**): fourteen Zend, one reflection, thirteen
SPL, four strings, and ninety-six serialization. Totals are **2560 / 2288 / 408**
for Zend, **288 / 167 / 37** for reflection, **316 / 368 / 94** for SPL,
**404 / 174 / 152** for strings, and **112 / 23 / 28** for serialization.
All **797 existing Rust tests**, **57 reference-checked project PHPT**, formatting,
Clippy, and diff checks pass. No custom tests or matrices were added. A direct
CLI comparison also matches PHP for 4,096 nested arrays and an explicit smaller
depth limit. Native exception restoration, legacy SPL Serializable formats,
INI settings/callbacks, property hooks, and complete lifecycle compatibility
remain unfinished, along with the earlier runtime and harness gaps.

The [Throwable property slice](conformance/php-8.5.0-throwable-properties-hillclimb.txt)
replaces the separate native exception payload with declared PHP properties.
Constructors, getters, references, reflection, serialization, and direct subclass
writes now use the same storage. Native declarations include typed private trace,
previous, and cached-string properties, protected location fields, and
ErrorException severity. Wakeup removes invalid message/code values; exception
objects reject cloning. Trace formatting handles malformed restored frames and
reports their field warnings. Exception chaining rejects shared-ancestor cycles,
and uncaught reporting reads location changes made by custom __toString methods.

This slice adds **23 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**): fifteen Zend, three reflection, one strings, and
four serialization. Totals are **2575 / 2273 / 408** for Zend,
**291 / 164 / 37** for reflection, **405 / 173 / 152** for strings, and
**116 / 19 / 28** for serialization; the other suites are unchanged.
All **797 existing Rust tests**, **57 reference-checked project PHPT**, formatting,
Clippy, and diff checks pass. No custom tests or matrices were added. A direct CLI
comparison also matches uncaught reporting after __toString changes the filename
to an empty string and the line to a negative value. This supersedes the earlier
separate Throwable payload and native exception restoration limitations.
Configurable traces, property hooks, legacy SPL Serializable formats, and complete
exception-handler and lifecycle compatibility remain unfinished.

The [legacy SPL serialization slice](conformance/php-8.5.0-legacy-spl-serialization-hillclimb.txt)
adds the Serializable interface and serialize()/unserialize() methods to
ArrayObject, ArrayIterator, and SplDoublyLinkedList, including their subclasses.
The native formats share the existing encoder and decoder reference tables;
nested decoding inherits allowed_classes and defers magic hooks until the
outermost decode completes. Array storage, flags, self backing, and ordinary
members are restored in native order, with partial changes retained on malformed
input. ArrayObject rejects restoration during sorting. Linked-list restoration
replaces the old contents; serialization stops safely when a callback removes the
next node. Malformed data reports offset exceptions and warnings attributed to
the native method.

This slice adds **16 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**): thirteen SPL and three serialization. Totals are
**329 / 355 / 94** for SPL and **119 / 16 / 28** for serialization; other suites
are unchanged. All **797 existing Rust tests**, **57 reference-checked project
PHPT**, formatting, Clippy, and diff checks pass. No custom tests or matrices were
added. This supersedes earlier limitations on legacy serialization for these
classes. SplObjectStorage, other missing SPL classes, INI configuration, and
complete mutation/destructor timing remain unfinished.

The [SplObjectStorage slice](conformance/php-8.5.0-object-storage-hillclimb.txt)
adds native object-keyed storage, custom getHash callbacks, offset operations,
set operations, seekable iteration, debug output, comparison, cloning, and magic
and legacy serialization. SplObserver and SplSubject interfaces and PHP 8.5
attach/detach/contains deprecations are declared. Native dimension access retains
PHP's null-entry probes and differs from explicit offsetUnset in cursor handling.
Storage keys and information values participate in cycle collection. Cloning
rebuilds hashes while the new object still has default properties, then copies
ordinary properties before invoking the clone hook.

This slice adds **46 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**): four Zend, thirty-nine SPL, and three
serialization. Totals are **2579 / 2269 / 408** for Zend,
**368 / 316 / 94** for SPL, and **122 / 13 / 28** for serialization; other suites
are unchanged. All **797 existing Rust tests**, **57 reference-checked project
PHPT**, formatting, Clippy, and diff checks pass. No custom tests or matrices were
added. Direct CLI comparisons match custom-hash clone initialization order and
collection of a three-object cycle through native storage. The remaining
executable fixture in the SplObjectStorage directory requires gc_enable(); GC
control functions, other missing SPL classes, INI configuration, and complete
mutation/destructor timing remain unfinished.

The [iterator-wrapper slice](conformance/php-8.5.0-iterator-wrappers-hillclimb.txt)
adds IteratorIterator, FilterIterator, RecursiveFilterIterator,
CallbackFilterIterator, RecursiveCallbackFilterIterator, ParentIterator,
LimitIterator, NoRewindIterator, InfiniteIterator, and EmptyIterator. Native
wrappers cache values and keys, forward inner methods, preserve callback access
scope, construct recursive child wrappers, and participate in cycle collection.
LimitIterator supports bounded seeking; NoRewindIterator retains the inner
iterator state; InfiniteIterator restarts exhausted input. Parent method calls
retain their native implementation when a subclass overrides iteration methods.

This slice adds **41 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**): three Zend, thirty-seven SPL, and one
serialization. Totals are **2582 / 2266 / 408** for Zend,
**405 / 279 / 94** for SPL, and **123 / 12 / 28** for serialization; other suites
are unchanged. All **797 existing Rust tests**, **57 reference-checked project
PHPT**, formatting, Clippy, and diff checks pass. No custom tests or matrices were
added. Direct CLI comparisons match private callback access and InfiniteIterator
validity call counts. Other SPL iterator classes, INI configuration, and complete
mutation/destructor timing remain unfinished.

The [caching-iterator slice](conformance/php-8.5.0-caching-iterator-hillclimb.txt)
adds CachingIterator and RecursiveCachingIterator with one-element lookahead,
string-conversion modes, full-cache array offsets and count, flag validation,
recursive child caching, and child-error suppression. Arrays returned by getCache
retain PHP's shared native writes and resets. Native constructor defaults retain
their class-constant metadata. Wrapper handle release order now matches the
upstream fixture, and unresolved forwarded methods report the outer wrapper's
class name. Cache arrays and recursive children are included in cycle tracing.

This slice adds **19 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**), all in SPL. SPL reaches **424 / 260 / 94**;
other suite totals are unchanged. All **797 existing Rust tests**,
**57 reference-checked project PHPT**, formatting, Clippy, and diff checks pass.
No custom tests or matrices were added. Direct CLI comparisons match shared cache
writes, unsets, null-entry probes, lookahead, flag resets, and signature metadata.
For a self-referencing cache with an otherwise unreachable empty ArrayIterator,
gc_collect_cycles reports four collected values where PHP reports two. Collector accounting for native children, additional SPL classes, INI
configuration, and complete mutation/destructor timing remain unfinished.

The [AppendIterator slice](conformance/php-8.5.0-append-iterator-hillclimb.txt)
adds native chained iteration, empty-state handling, queued-input access through
ArrayIterator, iterator-index queries, and appending during active traversal.
Switching inputs releases the prior cached value and iterator handle, rewinds the
selected iterator, and skips exhausted inputs. Explicit current() calls refresh
from the inner cursor. Nested native wrappers retain PHP's user-iterator current
cache, and queued inputs participate in cycle tracing.

This slice adds **13 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**), all in SPL. SPL reaches **437 / 247 / 94**;
other suite totals are unchanged. All **797 existing Rust tests**,
**57 reference-checked project PHPT**, formatting, Clippy, and diff checks pass.
No custom tests or matrices were added. Direct CLI comparisons match empty state,
nested current-value caching, and callback order across multiple and empty inputs.
The remaining AppendIterator fixture requires generators. Other SPL classes,
collector accounting, INI configuration, and complete mutation/destructor timing
remain unfinished.

The [RecursiveTreeIterator slice](conformance/php-8.5.0-recursive-tree-hillclimb.txt)
adds cached recursive tree rendering, key/current bypass flags, six configurable
prefix parts, postfixes, entry rendering, and aggregate input construction.
Prefix generation uses lookahead at each depth. Reinitialization preserves tree
formatting, and breaking foreach retains the tree's traversal depth. Native
constructor defaults expose their class-constant metadata. Missing native method
implementations produce PHP's core fatal diagnostic rather than a Rust panic.

This slice adds **seven upstream passes**, with **zero regressions** across all
seven full suites (**8,430 fixtures**), all in SPL. SPL reaches **444 / 240 / 94**;
other suite totals are unchanged. All **797 existing Rust tests**,
**57 reference-checked project PHPT**, formatting, Clippy, and diff checks pass.
No custom tests or matrices were added. Direct CLI comparisons match custom prefix
parts, repeated construction, default constants, depth after breaking iteration,
base-method current reads, closure argument rejection, and the missing-method
fatal diagnostic. Remaining tree fixtures require INI support; additional SPL
classes, collector accounting, and complete mutation/destructor timing remain
unfinished.

The [SPL heap slice](conformance/php-8.5.0-heap-hillclimb.txt) implements
SplHeap, SplMinHeap, SplMaxHeap, and SplPriorityQueue with native sift operations,
comparison callbacks, corruption checks and recovery, extraction flags, destructive
iteration, cloning, debug output, serialization hooks, and cycle-collection edges.
Priority-queue native iterators cache the current extracted value until advancing;
explicit current() reads inspect the heap directly. Extraction retains the old top
for comparison callbacks until the sift overwrites it.

This slice adds **50 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**). SPL reaches **494 / 190 / 94**; other suite totals
are unchanged. All **797 existing Rust tests**, **57 reference-checked project PHPT**,
formatting, Clippy, and diff checks pass. No custom tests or matrices were added.
All executable heap and priority-queue fixtures pass. Direct CLI comparisons also
match subclass current() overrides and top/count reads from extraction callbacks.
Other SPL classes, collector accounting, and complete reentrant mutation and
destructor timing remain unfinished.

The [MultipleIterator slice](conformance/php-8.5.0-multiple-iterator-hillclimb.txt)
adds combined iterator keys and values, any/all validity modes, numeric/associated
keys, attachment management, and strict duplicate-label checks. It reuses native
object storage for cloning, debug output, and cycle collection. Native dimension
handlers honor PHP's shared flag bits and reject non-Iterator attachments.

This slice adds **three upstream passes**, with **zero regressions** across all
seven full suites (**8,430 fixtures**). SPL reaches **497 / 187 / 94**; other suite
totals are unchanged. All **797 existing Rust tests**, **57 reference-checked project
PHPT**, formatting, Clippy, and diff checks pass. No custom tests or matrices were
added. Direct CLI comparisons match cloning, flag reset, default serialization,
object-keyed dimension access, flag changes during current(), and constructor
default values. Additional SPL classes, collector accounting, and complete mutation
and destructor timing remain unfinished.

The [SplFileInfo slice](conformance/php-8.5.0-file-info-hillclimb.txt) adds
path components, metadata and access queries, symlink targets, canonical paths,
cloning, debug output, info-class factories, and serialization restrictions.
The native payload preserves path spelling separately from filesystem resolution.
Request stat/lstat caches follow PHP's file-I/O invalidation and can be reset by
clearstatcache(). Metadata queries target the supported Unix hosts.

This slice adds **21 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**): 20 in SPL and one in serialization. SPL reaches
**517 / 167 / 94**, and serialization reaches **124 / 11 / 28**; other suite totals
are unchanged. All **797 existing Rust tests**, **57 reference-checked project PHPT**,
formatting, Clippy, and diff checks pass. No custom tests or matrices were added.
Direct CLI comparisons match path spelling, cloning, uninitialized state, factory
restrictions, serialization rejection, metadata/access queries, and stat-cache
invalidation. SplFileObject and openFile(), directory iterator classes, custom
stream wrappers, PHP's realpath cache, and complete mutation/destructor timing
remain unfinished.

The [directory iterator slice](conformance/php-8.5.0-directory-iterator-hillclimb.txt)
implements DirectoryIterator, FilesystemIterator, and RecursiveDirectoryIterator
using native directory handles. It provides key/current modes, dot filtering,
seeking, independent cloned handles, recursive children, and subpaths. Native
filesystem iterators cache current values; explicit current() calls create fresh
file-info objects. Recursive wrappers now dispatch child methods and forwarding
to source objects and return those sources from inner/subiterator accessors.

This slice adds **17 upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**). SPL reaches **534 / 150 / 94**; other suite totals
are unchanged. All **797 existing Rust tests**, **57 reference-checked project PHPT**,
formatting, Clippy, and diff checks pass. No custom tests or matrices were added.
Direct CLI comparisons match clone cursor independence, recursive traversal,
inner/subiterator identity, resource numbering, and native value caching across
flag changes. GlobIterator, SplFileObject, custom stream wrappers, collector
accounting, and complete mutation/destructor timing remain unfinished.

The [glob iterator slice](conformance/php-8.5.0-glob-iterator-hillclimb.txt)
implements GlobIterator and shared `glob://` directory streams. Pattern expansion
runs in Rust and follows PHP's byte-oriented wildcard, range, POSIX class, escape,
and sorting behavior. Iterators track each match's parent path, preserve result
counts across rewinds, handle empty streams, and reject cloning and invalid-state
method calls. DirectoryIterator, opendir(), and scandir() share this backend.

This slice adds **two upstream passes**, with **zero regressions** across all seven
full suites (**8,430 fixtures**). SPL reaches **536 / 148 / 94**; other suite totals
are unchanged. All **797 existing Rust tests**, **57 reference-checked project PHPT**,
formatting, Clippy, and diff checks pass. No custom tests or matrices were added.
Direct PHP comparisons cover matching, relative paths, empty streams, stream
operations, and lifecycle behavior. The upstream dit_002 FILE section also matches
when executed separately; its unsupported SKIPIF remains excluded from pass counts.
Other glob fixtures still require missing touch() and mkdir() functions. Custom
stream wrappers, open_basedir filtering, SplFileObject, and complete lifecycle
and collector behavior remain unfinished.

The [filesystem operations slice](conformance/php-8.5.0-filesystem-operations-hillclimb.txt)
adds mkdir(), rmdir(), touch(), chdir(), getcwd(), file metadata functions, and
file access predicates. Recursive directory creation resolves dot components and
uses native permission/umask behavior. Timestamp updates preserve file contents;
nullable timestamp arguments and failures follow PHP diagnostics. Working-directory
changes remain local to the request. Metadata operations share SplFileInfo's
stat/lstat caches and invalidate them after relevant filesystem changes.

This slice adds **25 upstream passes**, with **zero regressions** across **nine
full suites (9,399 fixtures)**: **22 file**, **two SPL**, and **one directory** case.
The file and directory suites were baselined at the preceding commit. SPL reaches
**538 / 146 / 94**, file reaches **45 / 179 / 672**, and directory reaches
**6 / 1 / 66**. The other seven-suite totals are unchanged. All **797 existing Rust
tests**, **57 reference-checked project PHPT**, formatting, Clippy, and diff checks
pass. No custom tests or matrices were added. The existing bug51068 and gh17067
FILE sections now match PHP output/status in separate executions; unsupported
CLEAN sections remain excluded from runner pass counts. Custom wrappers,
open_basedir, full stat/lstat arrays, permission-changing APIs, complete realpath
caches, and tracking renamed working directories remain unfinished.

The [CSV stream and line reader slice](conformance/php-8.5.0-csv-stream-hillclimb.txt)
adds fgetcsv(), fputcsv(), fgets(), stream_get_contents(), and file(). The shared
CSV parser reads additional physical lines inside quoted fields, preserving line
endings and applying the length limit to the first read. CSV output supports
custom separators, enclosures, escapes, and line endings. A field-conversion
exception leaves native record assembly and writing intact before propagation;
later object conversions are suppressed. Line and remainder reads maintain logical
positions and EOF state, and file() handles FILE_* line-array flags.

This slice adds **54 upstream passes**, with **zero regressions** across **nine
full suites (9,399 fixtures)**: **53 file cases** and **one Zend case**. Zend reaches
**2583 / 2265 / 408**, and file reaches **98 / 126 / 672**. Other suite totals are
unchanged. All **797 existing Rust tests**, **57 reference-checked project PHPT**,
formatting, Clippy, and diff checks pass. No custom tests or matrices were added.
Direct PHP comparisons cover multiline and length-limited reads, escaping,
positions, offsets, and conversion-exception write behavior. The separate
stream_get_contents fixtures still require missing tmpfile() or unsupported
SKIPIF sections. Configurable character locales, automatic legacy line-ending
detection, custom wrappers/filters, and complete buffering/lifecycle semantics
remain unfinished.

The full suite rows below include these changes. Subset rows retain the historical
measurements reported by their individual slices.

| Suite | Passed | Failed | Unsupported |
| --- | ---: | ---: | ---: |
| Project `tests/phpt`, Rust and reference PHP | 57 | 0 | 0 |
| PHP 8.5.0 `ext/standard/tests/general_functions/print_r*.phpt`, Rust and reference PHP for passing cases | 6 | 0 | 7 |
| PHP 8.5.0 all `Zend/tests`, Rust and reference PHP for passing cases | 2583 | 2265 | 408 |
| PHP 8.5.0 `ext/reflection/tests`, Rust and reference PHP for passing cases | 291 | 164 | 37 |
| PHP 8.5.0 all `ext/spl/tests`, Rust and reference PHP for passing cases | 538 | 146 | 94 |
| PHP 8.5.0 `ext/standard/tests/{general_functions,array}/var_export*.phpt`, Rust and reference PHP for passing cases | 11 | 2 | 6 |
| PHP 8.5.0 all `ext/standard/tests/array`, Rust and reference PHP for passing cases | 773 | 15 | 52 |
| PHP 8.5.0 all `ext/standard/tests/math`, Rust and reference PHP for passing cases | 52 | 2 | 117 |
| PHP 8.5.0 all `ext/standard/tests/strings`, Rust and reference PHP for passing cases | 405 | 173 | 152 |
| PHP 8.5.0 all `ext/standard/tests/serialize`, Rust and reference PHP for passing cases | 124 | 11 | 28 |
| PHP 8.5.0 all `ext/standard/tests/file`, Rust and reference PHP for passing cases | 98 | 126 | 672 |
| PHP 8.5.0 all `ext/standard/tests/dir`, Rust and reference PHP for passing cases | 6 | 1 | 66 |
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
