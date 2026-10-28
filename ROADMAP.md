# PHP 8.5 implementation and conformance plan

The goal is a PHP 8.5 runtime in Rust, with all project unit tests passing and
upstream conformance measured explicitly. Each completed slice is tested,
committed, and pushed. This roadmap records unfinished scope, not a reduced
definition of completion.

1. **Interpreter foundation (initial slice implemented):** byte strings, scalar
   values, source locations, parsing, variables, expression evaluation, basic
   control flow, CLI, and regression tests. Continue tightening scalar and syntax
   edge cases with the reference interpreter.
2. **Conformance infrastructure (initial harness implemented):** PHPT runner with
   exact output, EXPECTF/EXPECTREGEX through reference PCRE, explicit unsupported
   reports, before/after upstream comparisons, and CI.
   Local reference is PHP 8.5.0. Supported fixtures execute as real files with
   a selectable suite working directory and generated-file cleanup. Reference
   comparisons include process exit status, including intentional nonzero exits.
   Remaining: full upstream sections, reproducible reference provisioning in CI,
   complete diagnostic fidelity, and process-tree resource limits. Per-fixture workers
   isolate crashes and enforce configurable process timeouts. Nonfatal warnings,
   notices, and deprecations retain source filenames and output positions, and
   the harness compares their display text in emission order. Uncaught errors
   render exception chains/traces and run custom __toString methods; expected
   fatal output and reference exit status can now pass. ParseError/CompileError
   have distinct display forms, but complete compile-error classification,
   parser wording, and exception-property integration remain unfinished.
   Never count skipped tests as passing.
3. **Core language (arrays started):** ordered arrays, key coercion, copy-on-write
   assignment, nested indexed writes, array union/comparison, `foreach` by value,
   `isset`, `empty`, and `unset` implemented. Named functions, local scopes,
   recursion, defaults, named arguments, variadics, and call unpacking implemented.
   Function argument inspection supports live parameters, omitted defaults,
   named arguments, and extra positional arguments.
   First-class function callables support identity, normal argument binding,
   callable/Closure/object types, and PHP 8.5 constant initializers. Pipe chains
   accept these objects and function-name strings with normal caller strictness.
   Anonymous functions, explicit value/reference captures, automatic arrow
   captures, and per-instance closure statics implemented. Static anonymous
   functions support PHP 8.5 constant/default initializers.
   Basic string interpolation, Unicode escapes, heredoc/nowdoc, and byte string
   offset reads/writes implemented. Deprecated ${...} interpolation supports
   named variables, single array offsets, and expression-selected variable names.
   Deprecations follow compilation and constant short-circuit pruning; heredoc
   scanning handles nested interpolations with repeated delimiter names.
   All loop forms, alternative loop syntax, and numbered break/continue implemented.
   Switch selection/fall-through, alternative syntax, and control-flow diagnostics implemented.
   Exit/die support PHP 8.5 callable syntax, scalar coercion, and request-local
   termination with CLI exit status.
   Goto and labels support block/branch jumps, loop exits, and active-loop/switch
   jumps, with compile-time scope and entry validation.
   Match expressions with strict identity, lazy arms, and unmatched errors implemented.
   Alternative if/elseif/else syntax and nested templates implemented.
   Print expressions with PHP precedence and return values implemented.
   Array unpacking with integer and string keys implemented.
   Value destructuring with nested/keyed/skipped entries and foreach implemented.
   Explicit scalar/array casts and PHP 8.5 void result discarding implemented.
   NaN boolean/string/array conversion warnings, implicit integer diagnostics,
   and silent unordered numeric/string comparisons implemented. Logical
   constant folding preserves compile-time warnings and short-circuit behavior.
   `error_reporting` filters diagnostics at emission time; `@` scopes preserve
   fatal error levels and restore masks across exceptions.
   Global constants, dynamic definition/lookup, and core predefined constants implemented.
   Parameter/return types for available values, nullable/union types, typed variadics,
   default validation, void/never returns, and strict_types implemented.
   Live global bindings, persistent static locals with dynamic initializers,
   and variable variables implemented.
   `$GLOBALS` element access, mutation, snapshots, and symbol ordering implemented.
   Eval, local-file include/require and once-only variants, source/line/function
   magic constants, and included-file tracking implemented. Bracketed and
   unbracketed namespaces, separate class/function/constant imports, aliases,
   grouped imports, and static name resolution with function/constant fallback
   are implemented. Namespace context is lexical and does not rewrite dynamic strings.
   Variable reference assignment shares cells across named/dynamic variables,
   individual GLOBALS entries, closure captures, and global/static bindings.
   Rebinding preserves other aliases; undefined reference sources initialize to null.
   Fixed by-reference parameters retain source cells across argument evaluation,
   apply entry coercion to the caller, and distinguish callback value warnings
   from ordinary by-reference argument errors. Signatures preserve reference mode.
   Array elements and reference literals share the variable-cell representation.
   Copy-on-write drops unshared reference containers; recursive array writes and
   unset release owners before following aliases. Fixed reference parameters and
   internal callback array entries retain existing reference bindings.
   Array foreach by reference retains live source cells and element aliases.
   Weak iterator tokens keep independent positions in array copies; insertion,
   deletion, nested loops, source replacement, and final aliases are supported.
   Reference variadics retain positional/named caller cells and apply typed entry
   coercion. Array argument unpacking separates copies before binding selected
   elements, and argument inspection snapshots live referenced values.
   Reference destructuring supports nested/keyed/skipped entries and foreach,
   preserving source cells across rebinding and side effects in key expressions.
   Calls returning values produce temporary reference targets with notices.
   Functions, methods, closures, and arrows can return variable/array references.
   Typed returns coerce the referent; ordinary expression reads dereference it,
   while reference consumers retain identity. Getter and string-conversion paths
   handle returned references, and method overrides preserve reference contracts.
   Untyped instance/static property references preserve aliases across ordinary
   writes, rebinding, returns, arguments, destructuring, and array iteration.
   Magic getters retain references, and readonly/asymmetric access is checked.
   Object snapshots drop singleton reference wrappers and dumps detect cycles.
   Typed property references retain live type sources across variables, arrays,
   calls, returns, and clones, with coercion/conflict checks and increment bounds.
   Unset/rebound properties release their constraints; catch assignment preserves
   aliases. Nullable reference initialization and promoted references are supported.
   Object foreach by reference binds visible property cells with typed constraints,
   skips uninitialized slots, and rejects initialized readonly properties.
   Remaining: complete reference coercion diagnostics, cycle collection,
   advanced interpolation, complete
   operators, complete class-context magic constants, other superglobals,
   Traversable unpacking,
   Closure methods and complete object context handling,
   intersection/DNF types and class resolution,
   complete namespace validation and error/exception behavior.
4. **Objects and execution semantics (instance foundation started):** public
   classes, constructors, instance properties and methods, bound `$this` and
   method callables, invokable objects, basic property iteration, comparisons,
   array casts, and object inspection implemented. Public inheritance, `final`
   restrictions, compatible method overrides, descendant type checks, and
   `instanceof` implemented. Public static methods, parent/scoped calls, late
   static binding, contextual class types, scoped callables, and `::class` are
   implemented. Public static properties share inherited storage unless redeclared;
   public class constants support inheritance, final restrictions, dynamic names,
   lazy dependency resolution, strict type validation, and covariant type overrides.
   Typed instance/static properties support uninitialized state, strict
   defaults, assignment coercion, and invariant inherited declarations. Readonly
   properties/classes enforce initialization scope and mutation restrictions.
   Constructor promotion with visibility/readonly modifiers initializes properties before the
   constructor body, with separate parameter defaults and property storage.
   Shallow object/closure cloning, inherited `__clone()`, readonly reinitialization,
   and PHP 8.5 clone-with property updates are implemented.
   Abstract classes and public/protected abstract methods enforce implementation requirements,
   signature compatibility, and inherited abstract constructor contracts.
   User-defined interfaces support multiple inheritance, method contracts,
   constants, type checks, and interface_exists.
   Private/protected methods support access checks, lexical private dispatch,
   constructor/clone visibility, and saved method callables.
   Instance properties support private storage per declaring class, protected
   prototype access, separate read/set visibility, scoped iteration/inspection,
   mangled cast keys, final inheritance checks, and clone-with access checks.
   Magic property methods handle inaccessible/unset reads, writes, isset/empty,
   and unset, with per-property guards, typed getter checks, temporary indirect
   writes, caller trace locations, and restoration after exceptions.
   Method overloading dispatches missing/inaccessible calls through __call and
   __callStatic, including named arguments, first-class callables, callbacks,
   inherited handlers, late static binding, and autoload callback identity.
   Anonymous classes reuse ordinary class linking with constructor arguments,
   readonly declarations, inheritance, interfaces, traits, and declaration identity.
   Enclosing function and anonymous class scopes remain distinct for magic constants.
   Class aliases share linked identity and storage, including aliases of interfaces,
   traits, anonymous classes, and implemented native classes. Inheritance queries
   support aliases and distinguish source autoloading from target lookup.
   Traits support nested composition before inheritance, method precedence,
   `insteadof`, aliases, visibility/final adaptations, abstract requirements,
   property/constant compatibility checks, and lexical magic constants.
   Class reflection supports names/source locations, flags, ancestry, interface/trait
   metadata, constants, property defaults, static property access, and construction.
   Class attribute reflection preserves deferred arguments and validates instantiation
   in the declaration's source, scope, and strict-types mode. Function, method, and
   parameter reflection includes source/signature metadata, attributes, defaults,
   named/union types, closure creation/capture inspection, and reflective invocation.
   Property reflection exposes declaration-ordered instance/static and dynamic
   properties, metadata, types/defaults/attributes, initialization checks, and
   visibility-bypassing reads/writes with type/reference/readonly checks.
   Promoted and trait properties retain declaration order; ReflectionObject keeps
   its receiver for live dynamic-property discovery.
   Class-constant reflection supports individual/filtered discovery, deferred values,
   types/flags/attributes, declaring classes, and declaration strings. Ordinary
   property/constant documentation comments survive lexing and composition.
   Global constants retain declaration names/source/attributes/strictness for reflection.
   Their reflectors expose values, namespace names, deprecation flags, attributes,
   extension ownership, and declaration strings. Basic extension name/version/persistence
   reflection is available for Core, standard, Reflection, and SPL.
   Unit and backed enum cases retain singleton identities and readonly properties.
   Native cases/from/tryFrom methods validate backing values and honor scalar coercion;
   enum/case reflection exposes cases, types, values, and declaration metadata.
   Enum declarations compose methods/traits/interfaces and enforce enum restrictions.
   Native reflector metadata fields use the reflection write handler rather than
   PHP readonly-property flags. Native methods share argument-count validation
   with other internal functions.
   Remaining: built-in interfaces, interface properties, abstract properties, intersection/DNF constant types,
   direct static trait-access deprecations and exact composition diagnostics,
   complete constant/default resolution,
   complete property/constant diagnostics, property hooks, remaining magic methods, complete extension reflection/inventories,
   promoted-property/class/function doc comments, raw property access and expression
   stringification, complete internal
   property/builtin signature metadata, prototypes/static variables, lazy objects,
   complete attribute instantiation and runtime effects, enum serialization and
   full reflection strings,
   generators, fibers, garbage collection, and weak references.
5. **Standard runtime and extensions (core builtins started):** type inspection,
   `var_dump`, `print_r`, `var_export`, `strlen`, array keys/values/key existence, counting, and function
   existence implemented. Class method/default-property inspection respects caller
   visibility, declaration/inheritance order, private ancestors, and name shadowing.
   Exporting values supports escaped byte strings, shortest round-tripping floats,
   arrays, ordinary object properties, enum cases, and circular-reference warnings.
   Objects and closures share a request-local handle allocator, recycling released
   handles in PHP's last-freed-first order after contained values are released.
   spl_object_id and spl_object_hash expose the handle without affecting ownership.
   User-function argument-count/type diagnostics retain declaration locations,
   PHP calling locations, and the distinction between named gaps and positional
   omissions. Internal callback/reflection invocations omit PHP calling locations.
   ArrayAccess dispatches object offsets through offsetExists/Get/Set/Unset, including
   append, probes/coalescing, destructuring, nested reference mutations, and typed
   increments through reference-returning getters. Object roots retain identity through
   restricted properties and reentrant callbacks. Native abstract method metadata and
   tentative return-type reflection/compatibility warnings are supported for this interface.
   Complete nested compound-assignment caching and tentative-return inheritance/formatting
   remain unfinished. Local scopes release values in PHP slot order for stable handle reuse.
   Trait composition installs aliases before the original method, matching Zend.
   Formatted printing/string construction supports printf,
   sprintf, vprintf, and vsprintf, including positional arguments, star width/precision,
   byte padding, integer/float conversions, and validation errors. Locale configuration
   and resource formatting remain incomplete. Byte-string repetition/reversal and chr/ord conversion
   include PHP 8.5 deprecations; common native argument diagnostics include parameter
   names, received types, and argument counts. ASCII case conversion, bin2hex, and the
   trim family support binary strings, default whitespace, custom character ranges, and
   malformed-range warnings. String replacement supports scalar/array inputs, ordered
   searches, ASCII case-insensitive matching, and typed reference count output.
   Substring extraction/replacement supports negative/clamped byte bounds, integer
   limits, null lengths, and insertion-ordered array inputs with missing-entry defaults.
   Position/suffix searches and contains/starts-with/ends-with predicates support binary
   strings, ASCII case folding, positive/negative offsets, empty needles, and prefix results.
   Binary comparisons and bounded/insensitive variants, substring comparison, literal
   character spans, and substring counting preserve their distinct range/error rules.
   Locale-sensitive substring comparison beyond the default C locale remains unfinished.
   String splitting/joining supports explode limits, fixed byte chunks, chunk separators,
   insertion-ordered array conversion, aliases, and implode direct-call argument parsing.
   Byte translation and longest-match replacement maps preserve PHP replacement conversion
   timing. Slash escaping/unescaping supports binary character masks, C controls, octal/hex
   escapes, and distinct trailing-backslash rules.
   Tag stripping follows PHP’s HTML/PHP/declaration/comment states, preserves allowed tags
   from string or array lists, and handles nested brackets, quotes, XML/doctype, and NUL.
   Hex decoding, first-byte/word capitalization, newline insertion, character counts,
   character-set suffix search, and left/right/both padding are implemented. Padding
   constants retain standard-extension reflection metadata.
   Array map/filter/reduce support available callables,
   key preservation, zipped arrays, filter modes, initial carry values, and
   internal callback argument coercion. Callable inspection supports syntax-only
   checks, autoloading, scoped/abstract/magic methods, descriptive output names,
   and typed reference output assignments. Native dispatch retains argument cells.
   Protected method access follows the original non-private prototype. Debug backtrace
   capture/printing shares exception trace machinery, with live arguments, optional
   receiver objects, omission flags, frame limits, and both backtrace constants.
   SensitiveParameter redaction covers fixed and positional variadic parameters in
   debug/exception traces, with native SensitiveParameterValue storage, access,
   cloning, and hidden exported properties. Extra named variadic behavior follows
   PHP 8.5.0. Configurable trace settings, serialization, and
   uninitialized wrapper reads remain incomplete.
   SPL autoload queues support callback
   identity, prepend/removal, recursive lookup guards, exceptions, default file
   extensions, and runtime class/interface/trait/member resolution. Remaining:
   inheritance-variance autoloading, configurable include paths, stream wrappers,
   the other core and standard builtins, streams and
   filesystem, shutdown callbacks, output buffering, serialization, date/time, JSON, PCRE, SPL,
   encoding, networking, sessions, and remaining bundled extension surfaces.
6. **PHP 8.5 specifics:** pipe operator implemented for available callable kinds;
   clone-with implemented for available object kinds. Attribute syntax and metadata
   cover declaration targets, with internal target/repetition checks, delayed target
   validation, inherited AllowDynamicProperties, and core attribute constructors.
   NoDiscard checks void/never and magic method declaration restrictions.
   Remaining: other callable kinds, URI extension, new APIs and
   complete attribute reflection/instantiation and runtime effects, and migration-guide compatibility changes.
7. **Deployment and completion audit:** configuration, environment and SAPI
   behavior, resource limits, supported-platform validation, upstream unit and
   PHPT suites, diagnostic fidelity, real-application integration, performance,
   and requirement-by-requirement review of remaining unsupported behavior.

## Validation gates for every slice

- Use failing upstream PHPT cases to drive implementation; run existing Rust tests
  as regression checks and compare full Zend results before and after each slice.
- Run `cargo fmt --check`, `cargo test`, and
  `cargo clippy --all-targets -- -D warnings`.
- Compare relevant behavior against PHP 8.5; record reference version and scope.
- Inspect changes, check disk/build-cache usage, commit, and push to `origin`.
- Keep this document and the README honest about incomplete implementation.
