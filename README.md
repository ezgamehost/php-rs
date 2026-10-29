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

`Value::String` stores a `PhpString`; Rust callers can construct it with
`Value::String(bytes.into())` and read an owned snapshot with `bytes()` or
`to_vec()`. Cloning shares byte storage until an ordinary write separates it.

The library exposes `run`, `lint`, and a request-local `Runtime`. `Runtime` retains
output and warnings when execution fails. `output_with_diagnostics()` interleaves
displayed nonfatal diagnostics with ordinary output; `write_output()` writes separate
output/error streams in emission order. Source filenames and diagnostic levels are
retained while the existing `output` and `warnings` fields remain separate.
`report_uncaught(&error)` renders an execution error at the request boundary,
including a user-defined exception `__toString()` and any output or exit it causes.
Parsing happens before execution.
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
- Explicit scalar, array, and object casts, cast-alias deprecations, and PHP 8.5 `(void)`
  result discarding in statements and permitted `for` clauses.
- Global `const` declarations, `define`/`defined`/`constant`, qualified lookup,
  and core numeric, version, separator, and error-level constants.
- Ordered arrays, PHP key coercion, copy-on-write assignment, nested indexed
  writes, append, array union and comparison, `unset`, `isset`, and `empty`.
- Array unpacking with integer-key reindexing and string-key overwrites.
- `ArrayAccess` object offsets support reads, assignment, append, unset, isset,
  empty, coalescing, and destructuring. Nested mutation preserves reference-returning
  getters; increments update returned references with typed-property constraints.
  The interface exposes tentative return types and honors ReturnTypeWillChange.
- `array_slice`, `array_reverse`, and `array_chunk`, including key preservation,
  numeric-key reindexing, clamped slices, and reference-aware element copying.
  `array_fill`, `array_fill_keys`, and `array_pad` support repeated values, converted
  keys, signed padding, and native size validation.
- `range` generates integer, floating-point, and byte-string sequences, with native
  endpoint/step coercion, direction checks, conversion diagnostics, and size bounds.
- `abs`, `ceil`, `floor`, `sqrt`, `intdiv`, `fdiv`, `fmod`, `fpow`, and the
  finite/infinite/NaN predicates support native numeric argument conversion.
  `pow` and `**` share integer overflow promotion and PHP 8.5 zero-base deprecations.
- `base_convert`, `bindec`, `octdec`, `hexdec`, `decbin`, `decoct`, and `dechex`
  support prefixes, invalid-character diagnostics, overflow, and negative integer encoding.
- `round` supports all eight `RoundingMode` enum cases, legacy integer modes,
  signed zero, and decimal precision correction. `number_format` supports exact
  integer rounding, negative decimal places, and binary string separators.
- Trigonometric/inverse and hyperbolic/inverse functions, `atan2`, `hypot`,
  `exp`/`expm1`, `log`/`log10`/`log1p`, degree/radian conversion, `pi`, and
  PHP's standard mathematical constants.
- `array_map`, `array_filter`, and `array_reduce`, including multiple-array zip
  behavior, null callbacks where allowed, filter key modes, initial reduction
  values, and PHP's weak scalar coercion for internally invoked callbacks.
- `in_array` and `array_search` support loose/strict comparisons, referenced entries,
  and the first matching key. `array_sum` and `array_product` preserve numeric types,
  overflow promotion, and warnings/fallbacks for unsupported entries.
- `array_first`/`array_last`, first/last keys, and list detection inspect insertion
  order. `array_flip`, `array_count_values`, and `array_combine` support PHP key
  conversion, duplicate handling, and reference-aware value copying.
- `array_merge`/`array_replace` and their recursive forms support numeric-key
  handling, nested collisions, reference separation, and recursion errors.
- `current`/`pos`, `key`, `reset`, `end`, `next`, and `prev` support internal array
  cursors, copy-on-write separation, and deprecated object-property traversal.
- `array_walk` and `array_walk_recursive` invoke callbacks on live entry references,
  follow input mutations/replacement, and enforce typed-property constraints.
- Value and reference destructuring with `[]` and `list()`, including nested
  patterns, explicit keys, skipped entries, indexed targets, and `foreach`.
  Reference patterns preserve source bindings across self-rebinding and key
  expressions; missing referenced elements initialize without read warnings.
- Variables, assignment, arithmetic, concatenation, scalar comparisons,
  boolean and bitwise operations, signed shifts, null coalescing, ternary expressions, and numeric increments.
- Reference assignment between named/dynamic variables, individual `$GLOBALS`
  entries, and nested array elements, sharing cells with closure captures and
  global/static bindings. Array literals retain references; copy-on-write drops
  unshared reference containers. Rebinding detaches only the destination slot.
  Ordinary reads copy values, and recursive array paths release their containing
  storage before following a reference.
- Array `foreach` by reference binds each live element, retaining the final alias.
  Nested cursors track append/removal and copy-on-write changes; temporary arrays
  and reference-assignment expressions are supported as iteration sources.
- PHP operator precedence, short-circuit evaluation, and integer overflow promotion.
- SPL autoloader registration, removal, inspection, and explicit dispatch, with
  default extension-based file loading, recursion guards, and loader exceptions.
  Runtime class, interface, trait, static member, and callable lookups invoke loaders.
- Request-local `error_reporting` masks and `@` suppression, including nested scopes,
  exception restoration, and explicit mask changes inside suppressed expressions.
- Named functions, recursion, local scopes, constant default arguments,
  named arguments, variadics, argument unpacking, and string-based function calls.
  User-function argument errors distinguish missing positional arguments from
  named gaps, report declaration locations, and include PHP calling locations
  where applicable. Native callback invocations retain their distinct diagnostics.
  Fixed and variadic by-reference parameters bind variable/global/array cells,
  including named calls, entry coercion, and callback temporary-value warnings.
  Array argument unpacking separates copied arrays before binding element references.
  `func_num_args`, `func_get_arg`, and `func_get_args` expose supplied arguments,
  with live parameter values, default gaps, and snapshots of referenced arguments.
- Reference returns from functions, methods, closures, and arrows retain variable
  or array-element cells for reference assignment, arguments, and iteration.
  Return coercion updates the referent; ordinary reads still copy values.
- Instance and static properties retain reference aliases across writes,
  rebinding, function calls, and array-element operations. Magic getters can expose
  existing references; readonly and asymmetric setters restrict reference access.
  Typed properties constrain writes through every shared alias, including weak
  coercion and conflicts between multiple property types. Clones retain constraints;
  unsetting/rebinding properties releases their constraints from old references.
- Instance classes and `stdClass`, constructors, shared object handles,
  dynamic property names, nested property/array writes, and basic live property
  iteration by value or reference. Reference iteration keeps property aliases and
  typed constraints, skips uninitialized properties, and rejects readonly slots.
  Instance methods bind `$this`, support named/typed arguments, and
  expose bound first-class callables. Non-static closures retain `$this`.
  Method-array callbacks and invokable objects work with pipes and array callbacks.
- Request-local object handles are shared by objects and closures and recycled
  after their last owning reference is released. `spl_object_id` and `spl_object_hash`
  expose those handles. Cyclic garbage collection and destructor callbacks remain
  unfinished.
- Anonymous classes, including readonly declarations, constructor arguments,
  inheritance, interfaces, and traits. Repeated executions share the declaration's
  class and static state; separately compiled declarations have distinct identities.
- Unit and int/string-backed enums with shared case identities, readonly name/value
  properties, constants, methods, traits, interfaces, `cases`, `from`, and `tryFrom`.
  Native UnitEnum/BackedEnum contracts, `enum_exists`, enum comparisons/dumps, and
  enum/case reflection are supported. Serialization and full reflection strings
  remain unfinished.
- `var_export` prints or returns PHP expressions for scalars, arrays, objects,
  closures, and enum cases. It preserves byte strings, floating-point fractions,
  nested indentation, and detects circular references. Configurable serialization
  precision and property hooks remain unfinished.
- `class_alias` preserves class identity, inherited types, and shared static state,
  with optional source autoloading. `is_a` and `is_subclass_of` support object and
  permitted string sources, checking target names without autoloading them.
- Attribute groups on classes, functions/methods, properties, parameters, constants,
  closures, arrows, and anonymous classes retain resolved names and constant arguments.
  Internal target/repetition checks support delayed target validation.
  `AllowDynamicProperties` is inherited; core attribute classes expose PHP 8.5 target
  flags and constructors. Class, function, method, parameter, and property attribute reflection
  supports deferred argument evaluation, filtering, target/repetition validation,
  and instantiation using the
  declaration's scope and strict-types setting. Other attribute runtime effects remain unfinished.
- `ReflectionClass`/`ReflectionObject` class names, source locations, flags,
  inheritance/interface/trait queries, constants, defaults, static property access,
  and construction with or without constructors.
- Function/method/parameter reflection exposes names, source lines, visibility,
  signature metadata, defaults, attributes, named/union types, and closure captures.
  Reflective invocation supports named arguments, argument-array references, weak
  coercion, non-public methods, and closure creation.
  Complete builtin signatures, prototypes/static variables, lazy objects, full reflector
  interfaces, and declaration string representations remain unfinished.
- Property reflection supports declared/inherited/dynamic discovery, filters, types,
  defaults, attributes, promoted/readonly/asymmetric flags, mangled names, and
  initialization checks. Reflective reads and writes bypass visibility while retaining
  typed-reference and readonly constraints. Declaration strings cover ordinary defaults;
  unevaluated expression rendering, raw/hooked access, lazy objects,
  and complete internal property metadata remain unfinished.
- Class-constant reflection supports individual/filtered discovery, deferred values,
  declared types, visibility/final/deprecated flags, declaring classes, attributes,
  and declaration strings. Ordinary property and constant documentation comments
  survive lexing, grouped declarations, inheritance, and trait composition.
  Reflection metadata field writes raise ReflectionException; fields retain their
  native typed-property behavior for unsetting and references.
- Global-constant reflection retains original names, source files, values, attributes,
  and declaration strictness. It exposes namespace/short names, deprecation metadata,
  extension ownership, and declaration strings. ReflectionAttribute exposes its name
  field. Basic extension reflection provides names, versions, and persistence for Core,
  standard, Reflection, and SPL; complete extension inventories/APIs remain unfinished.
- Public/protected/private instance properties and constructor promotion, with
  separate private slots across inheritance, scoped iteration and `get_object_vars`,
  mangled array-cast keys, and asymmetric setters. Final properties and readonly
  initialization restrictions are checked across direct and indirect writes.
- Magic property methods `__get`, `__set`, `__isset`, and `__unset`, with
  recursion guards, distinct typed-uninitialized/unset states, quiet probes,
  checked getter results, and temporary values for indirect modification.
  Reference-returning getters preserve their referents during indirect writes.
- Missing and inaccessible methods dispatch through `__call` and `__callStatic`,
  preserving requested names, named arguments, inherited handlers, and late static
  binding across direct calls, callbacks, first-class callables, and autoloaders.
- Public class inheritance with inherited constructors/properties/methods,
  virtual dispatch, shared inherited method statics, and `final` classes/methods.
  Overrides validate arity, variadic parameters, parameter contravariance, and
  return covariance for implemented types. Class types accept descendants.
  `instanceof` supports named/dynamic class operands and skips dynamic operands
  for non-object values. Class linking preserves forward-declaration timing.
- Abstract classes and public/protected abstract methods, including static methods and
  constructor contracts that remain binding through intermediate implementations.
  Concrete subclasses must implement all inherited abstract methods with compatible
  signatures; abstract classes cannot be instantiated or their abstract methods called.
- Private/protected instance and static methods, constructor and clone access checks,
  lexical private-method dispatch, and visibility-preserving method callables.
  Private methods have independent override signatures; final private constructors
  still prevent replacement.
- User-defined interfaces with multiple inheritance, method contracts, typed/final
  constants, and implementation checks. Interface types work with arguments, returns,
  properties, and `instanceof`; `interface_exists` distinguishes them from classes.
- Public static methods, parent implementation calls, scoped method callables,
  static-method string/array callbacks, and late static binding. `self`, `parent`,
  and `static` resolve in calls, construction, and contextual types; closures
  retain their declaring and called class contexts. `::class`, `get_called_class`,
  and `get_parent_class` expose class names. Static closure defaults use the
  declaring scope; property defaults are cached separately for each class.
- Throw expressions and `try`/`catch`/`finally`, including union catches, rethrowing,
  saved exception identity, runtime-error catches, and unwinding of return/loop
  control. The built-in `Throwable` interface, `Exception`, `Error`, and core Error
  subclasses support constructors, message/code/file/line/previous getters,
  `getTrace()`, and `getTraceAsString()`. Traces capture live arguments at creation,
  including method, closure, internal callback, eval, and include frames.
  Exceptions thrown from `finally` retain the pending exception as their previous.
  Throwable string conversion includes saved traces and previous exceptions,
  honors subclass overrides, and terminates cyclic previous-exception chains.
- `ErrorException` supports severity, custom filenames and signed line numbers,
  named arguments, and the final `getSeverity()` method. The 13 SPL exception
  classes support their native inheritance and constructors. Repeated native
  constructor calls preserve omitted messages, zero codes, and null previous
  exceptions according to PHP behavior.
- `Stringable` and `__toString()` support implicit interface membership, signature
  validation, echo/print/interpolation, string casts and concatenation, weak string
  arguments/returns/properties, native string parameters, and comparisons (including
  nested arrays/objects and switch matching). Strict string types reject objects;
  failed return conversions chain the original exception under a TypeError.
- Nullsafe property access and method calls (`?->`) short-circuit member chains,
  array offsets, and static access while preserving independent argument/key
  expressions. They support `isset`/`empty`/coalescing, interpolation, and
  constant/default expressions; writes through nullsafe chains are rejected.
- Object conversion in string-offset assignments, including negative offsets,
  extension, typed properties, access checks, and reentrant replacement/unsetting.
  Shared string buffers preserve copies created before or during conversion.
- Public, protected, and private static properties with inherited shared storage,
  scope checks, independent private redeclarations, dynamic names, array mutation, and `isset`/`empty`/coalescing. Public, protected, and private
  class constants support scope-aware access, inheritance, `final`, dynamic names,
  lazy dependency resolution, cycle errors, and lookup through `constant`/`defined`. `self`, `parent`, and
  `static` select the appropriate class for both kinds of member access.
  PHP 8.5 asymmetric static-property visibility separates reads from writes with
  `public(set)`, `protected(set)`, and `private(set)`. Explicit and implicit final
  static properties prevent redeclaration; indirect array writes check setter access.
- Typed public instance properties and typed static properties of all visibility
  levels, including uninitialized state, strict defaults, weak/strict assignment checks, invariant inherited types,
  typed array writes, integer increment limits, and reinitialization after `unset`.
  Object dumps show uninitialized slots; casts, iteration, and `get_object_vars`
  omit them until initialized.
- Readonly properties and classes, with one-time initialization from permitted
  class scopes, mutation/unset checks, and readonly inheritance restrictions.
  Readonly classes require typed instance properties and reject dynamic ones.
  Objects stored directly in readonly properties can still be mutated.
- Public and readonly constructor property promotion, including typed parameters,
  argument coercion, inherited constructors, and assignment before the body runs.
  Parameter defaults remain separate from uninitialized property storage.
- Shallow object and closure cloning, inherited `__clone()` methods, and readonly
  reinitialization during cloning. PHP 8.5 `clone($object, $withProperties)` updates
  copied properties after `__clone()`, using the caller's access scope and internal
  weak type coercion. Named arguments, unpacking, and clone callables are supported.
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
- Bracketed and unbracketed namespaces, class/function/constant imports and aliases,
  grouped imports, and lexical name resolution in calls, types, inheritance,
  catches, and class members. Unqualified functions and constants fall back to
  global definitions; dynamic strings keep their literal names.
- Traits support nested composition, method precedence, `insteadof`, aliases,
  visibility/final adaptations, abstract requirements, independent method static
  storage, and property/constant composition before inheritance. `trait_exists`
  distinguishes traits from classes and interfaces.
- Lexical `__LINE__`, `__FILE__`, `__DIR__`, `__FUNCTION__`, `__METHOD__`,
  `__CLASS__`, `__NAMESPACE__`, and `__TRAIT__`, including the using class for
  trait `__CLASS__` and the original defining trait for `__TRAIT__`/`__METHOD__`.
- Parameter and return types for available values, nullable/union types, scalar
  coercion, typed variadics, default validation, and `void`/`never` return rules.
  `declare(strict_types=1)` preserves caller and declaration scope across calls;
  strict calls also check scalar arguments to the supported internal functions.
- `print_r` writes or returns binary-safe formatted values, detects array/object
  reference cycles, labels private/protected property keys, and invokes
  `__debugInfo` for custom object output.
- `get_class_methods` lists methods in declaration/inheritance order with caller-scope
  visibility. Trait aliases precede their original method. `get_class_vars` returns
  visible instance and static declaration defaults, including null for uninitialized
  typed declarations, with class-name conversion and autoloading.
- `is_callable` validates available function/method/closure/invokable targets,
  supports syntax-only checks without autoloading, and writes descriptive callable
  names through its reference output parameter. Scoped, abstract, and magic method
  checks use caller visibility; deprecated callback forms emit diagnostics.
  Protected overrides retain access through their non-private ancestor prototype.
- `debug_backtrace` returns active frames with live argument snapshots, optional
  receiver objects, argument omission, and frame limits. `debug_print_backtrace`
  shares exception trace formatting and omits the main-script terminator.
  Sensitive fixed parameters and positional variadic arguments are wrapped in
  `SensitiveParameterValue` in backtraces and exception traces. The wrapper supports
  `getValue`, cloning, readonly private storage, and hidden dump/array-cast properties.
  PHP 8.5.0's extra named variadic behavior is preserved; trace configuration,
  serialization and uninitialized wrapper reads remain unfinished.
- Core builtins: `var_dump`, `print_r`, `strlen`, `count`/`sizeof`, `gettype`, `get_debug_type`,
  `call_user_func`, `call_user_func_array` with positional/named arguments,
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
  string prefixes, and indented heredoc/nowdoc strings. Deprecated `${name}`,
  `${name[index]}`, and `${expression}` forms execute with compile-time deprecations.
  Heredoc delimiters and indentation respect nested interpolated PHP expressions.
- Byte string offset reads and writes, negative indexes, space padding, and
  string offset checks with `isset`, `empty`, and null coalescing.
- `printf`, `sprintf`, `vprintf`, and `vsprintf` share byte-oriented formatting,
  positional arguments, dynamic width/precision, custom padding, integer bases,
  floating-point formats, and PHP's format-validation errors. Printing occurs
  after successful formatting and returns the emitted byte count.
- Binary-safe `str_repeat`, `strrev`, `chr`, and `ord`, with scalar/Stringable
  coercion where applicable and PHP 8.5 byte-range/length deprecations.
- ASCII `strtolower`/`strtoupper`, lowercase `bin2hex` encoding, and
  `trim`/`ltrim`/`rtrim`/`chop` with binary character lists and inclusive ranges.
- `str_replace`/`str_ireplace` support binary strings and arrays, sequential searches,
  preserved subject keys, and a reference parameter for the replacement count.
- `substr` and `substr_replace` support byte offsets, negative/clamped bounds, nullable
  lengths, and ordered array replacements with preserved subject keys.
- Binary string searches: `strpos`/`stripos`, `strrpos`/`strripos`, `strstr`/`strchr`/
  `stristr`, `strrchr`, and the contains/starts-with/ends-with predicates.
- Binary `strcmp`/`strncmp` and ASCII insensitive variants, `substr_compare`,
  `strspn`/`strcspn` character spans, and non-overlapping `substr_count`.
- Binary `explode`, `implode`/`join`, `str_split`, and `chunk_split`, including
  split limits, ordered element conversion, and native argument-validation rules.
- `strtr` byte tables and longest-match replacement maps, plus `addslashes`,
  `stripslashes`, `addcslashes`, and `stripcslashes` for binary escaping/unescaping.
- `strip_tags` handles HTML/PHP/comment states and string or array allowed-tag lists,
  including PHP’s quoted-content, malformed-tag, whitespace, and NUL behavior.
- `hex2bin`, `ucfirst`/`lcfirst`/`ucwords`, `nl2br`, `count_chars`, `strpbrk`, and
  `str_pad` with all three padding constants and binary byte semantics.

Known gaps include intersection/DNF types, most builtins, complete exception APIs,
generators, complete attribute behavior, extensions, and most PHP 8.5 additions.
The object implementation is a foundation: built-in interfaces, interface properties,
property hooks,
intersection/DNF constant types, complete constant/default-expression compatibility,
remaining magic methods, inheritance-variance autoloading, and complete reflection remain unfinished. Closure methods,
direct static trait-access deprecations, complete trait conflict diagnostics,
complete closure debugging and magic-constant context, exact object handle reuse, and iteration edge cases
involving deletion/reinsertion of visited properties also remain unfinished.
Callable checks cover function names, Closure objects, instance-method arrays,
and invokable instances of implemented classes.
Variable and array-element reference assignment, array reference entries, closure
reference captures, reference parameters/returns, and reference destructuring work.
Traversable unpacking, complete reference coercion compatibility and diagnostics,
and cycle collection remain unfinished.
Reference cycles are not yet collected.
Throwable property layouts, configurable trace settings, error/exception
handlers, protected exception-property access (including severity), and complete compile-time fatal
error distinctions remain unfinished.
Other `declare` directives are not implemented. Shutdown callbacks, output
handlers, and object destructor execution remain unimplemented.
Automatic initialization and scope handling for the other superglobals are not
implemented; `$GLOBALS` currently exposes the variables created by PHP code or
provided through `Runtime::variables`. File loading currently
uses the working directory and the executing source directory; configurable
`include_path`, stream wrappers, and PHP configuration are not implemented.
Complete object interpolation,
some scalar edge cases, complete
syntax validation, resource limits, and exact PHP diagnostic formatting also
remain to be implemented. Passing the repository tests does not establish PHP
conformance. See [ROADMAP.md](ROADMAP.md) for the full target and validation plan.

The PHPT runner supports `TEST`, `FILE`, and exact `EXPECT` output,
plus descriptive `DESCRIPTION` and `CREDITS` sections. With `--reference`, it also
supports `EXPECTF` (including `%r` fragments) and `EXPECTREGEX`, using the reference
PHP binary's PCRE engine to match output. Test programs still execute in Rust;
the PHP matcher receives only patterns and output as byte strings. Other sections are reported
as unsupported and make the run exit unsuccessfully. Unexpected diagnostics also
fail a test. `--reference` checks the PHP major/minor version and validates passing
fixtures against both interpreters, including process exit status. Intentional
nonzero exits and expected fatal errors can pass when output and reference status
match. Diagnostics are interleaved and compared with expected output, using
`display_errors=1` and `log_errors=0` for the PHP reference. Uncaught errors include
exception text, stack traces, and source locations. Exact ParseError/CompileError
classes use PHP's distinct display forms. Some parser and compile-error messages
and classifications remain incomplete.
The runner writes each supported `FILE` section to a temporary `.php` sibling
of the `.phpt` fixture and runs both interpreters against that filename. It
refuses to overwrite an existing file and removes generated files after the run.
Use `--cwd /path/to/php-src` to select the suite root; the default is the current
directory. Each fixture runs in a separate worker, so a Rust panic or stack
overflow fails that fixture without aborting the suite. The parent removes the
generated file even after worker failure. Worker and reference executions have a
10-second timeout, adjustable with `--timeout SECONDS`. Test code is not sandboxed.
Use upstream PHPT failures as the primary implementation backlog. Compare the
same suite before and after each slice with `python3 tools/compare_phpt.py
before.log after.log`; the tool lists new passes and fails on regressions.
Keep unsupported cases separate from passes, and run the existing Rust tests as
regression checks. Expanding PHPT-runner coverage is part of this workflow.
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
