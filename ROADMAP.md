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
   diagnostic comparison, and process-tree resource limits. Per-fixture workers
   isolate crashes and enforce configurable process timeouts.
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
   offset reads/writes implemented.
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
   Remaining: references,
   advanced interpolation, complete
   operators, complete class-context magic constants, other superglobals, by-reference functions,
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
   Typed public instance/static properties support uninitialized state, strict
   defaults, assignment coercion, and invariant inherited declarations. Readonly
   properties/classes enforce initialization scope and mutation restrictions.
   Public and readonly constructor promotion initializes properties before the
   constructor body, with separate parameter defaults and property storage.
   Shallow object/closure cloning, inherited `__clone()`, readonly reinitialization,
   and PHP 8.5 clone-with property updates are implemented.
   Abstract classes and public/protected abstract methods enforce implementation requirements,
   signature compatibility, and inherited abstract constructor contracts.
   User-defined interfaces support multiple inheritance, method contracts,
   constants, type checks, and interface_exists.
   Private/protected methods support access checks, lexical private dispatch,
   constructor/clone visibility, and saved method callables.
   Remaining: built-in interfaces, interface properties, traits, abstract properties, intersection/DNF constant types,
   complete constant/default resolution,
   property/constant visibility, property hooks, remaining magic methods, reflection, attributes, enums,
   generators, fibers, garbage collection, and weak references.
5. **Standard runtime and extensions (core builtins started):** type inspection,
   `var_dump`, `strlen`, array keys/values/key existence, counting, and function
   existence implemented. Array map/filter/reduce support available callables,
   key preservation, zipped arrays, filter modes, initial carry values, and
   internal callback argument coercion. Remaining: configurable include paths, stream wrappers,
   the other core and standard builtins, streams and
   filesystem, shutdown callbacks, output buffering, serialization, date/time, JSON, PCRE, SPL,
   encoding, networking, sessions, and remaining bundled extension surfaces.
6. **PHP 8.5 specifics:** pipe operator implemented for available callable kinds;
   clone-with implemented for available object kinds;
   remaining: other callable kinds, URI extension, new APIs and
   attributes, and migration-guide compatibility changes.
7. **Deployment and completion audit:** configuration, environment and SAPI
   behavior, resource limits, supported-platform validation, upstream unit and
   PHPT suites, diagnostic fidelity, real-application integration, performance,
   and requirement-by-requirement review of remaining unsupported behavior.

## Validation gates for every slice

- Add failing behavioral regressions before implementation.
- Run `cargo fmt --check`, `cargo test`, and
  `cargo clippy --all-targets -- -D warnings`.
- Compare relevant behavior against PHP 8.5; record reference version and scope.
- Inspect changes, check disk/build-cache usage, commit, and push to `origin`.
- Keep this document and the README honest about incomplete implementation.
