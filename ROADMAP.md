# PHP 8.5 implementation and conformance plan

The goal is a PHP 8.5 runtime in Rust, with all project unit tests passing and
upstream conformance measured explicitly. Each completed slice is tested,
committed, and pushed. This roadmap records unfinished scope, not a reduced
definition of completion.

1. **Interpreter foundation (initial slice implemented):** byte strings, scalar
   values, source locations, parsing, variables, expression evaluation, basic
   control flow, CLI, and regression tests. Continue tightening scalar and syntax
   edge cases with the reference interpreter.
2. **Conformance infrastructure (initial harness implemented):** exact-output PHPT
   runner, fifteen differential fixtures, explicit unsupported reports, and CI.
   Local reference is PHP 8.5.0. Remaining: full upstream sections, reproducible
   reference provisioning in CI, diagnostic comparison, isolation, and timeouts.
   Never count skipped tests as passing.
3. **Core language (arrays started):** ordered arrays, key coercion, copy-on-write
   assignment, nested indexed writes, array union/comparison, `foreach` by value,
   `isset`, `empty`, and `unset` implemented. Untyped named functions, local scopes,
   recursion, defaults, named arguments, variadics, and call unpacking implemented.
   Basic string interpolation, Unicode escapes, heredoc/nowdoc, and byte string
   offset reads/writes implemented.
   All loop forms, alternative loop syntax, and numbered break/continue implemented.
   Switch selection/fall-through, alternative syntax, and control-flow diagnostics implemented.
   Match expressions with strict identity, lazy arms, and unmatched errors implemented.
   Alternative if/elseif/else syntax and nested templates implemented.
   Print expressions with PHP precedence and return values implemented.
   Remaining: references, array unpacking,
   destructuring, goto, advanced interpolation, casts, complete
   operators, constants, includes, typed/by-reference functions, closures, types,
   namespaces, and error/exception behavior.
4. **Objects and execution semantics:** classes, inheritance, interfaces, traits,
   visibility, properties and hooks, magic methods, reflection, attributes, enums,
   generators, fibers, garbage collection, and weak references.
5. **Standard runtime and extensions (core builtins started):** type inspection,
   `var_dump`, `strlen`, array keys/values/key existence, counting, and function
   existence implemented. Remaining: the other core and standard builtins, streams and
   filesystem, output buffering, serialization, date/time, JSON, PCRE, SPL,
   encoding, networking, sessions, and remaining bundled extension surfaces.
6. **PHP 8.5 specifics:** pipe operator, clone-with, URI extension, new APIs and
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
