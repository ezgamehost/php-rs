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
`report_uncaught(&error)` handles an execution error at the request boundary,
including registered exception handlers, user-defined exception `__toString()`,
and any output or exit they cause. A successfully handled exception preserves an
earlier exit status, or sets it to 0 when none was set.
Runtime diagnostics invoke registered PHP error handlers synchronously and
propagate exceptions from them. Some mutation paths still need this integration;
see the coverage notes below.
Parsing happens before execution.
Use `Runtime::execute_file` for file source identity and relative includes, or
`Runtime::execute_code` for code without an opening PHP tag.
Call `Runtime::shutdown()` after reporting any execution error and before consuming
output. It runs shutdown callbacks and object destructors once; errors are reported and returned,
so callers must not report them again. The CLI, PHPT runner, and `run` helper do
this automatically. The `run` helper still returns execution errors even when an
exception handler runs.
`Runtime::set_working_directory` configures relative path resolution for one
request without changing the host process's working directory.
`Runtime::exit_status()` reports request termination. `exit`/`die` return from
execution successfully with `Value::Null`, preserving output and global state.
A terminated request does not execute further PHP; create a new `Runtime` for a
new request. The CLI returns the low eight bits of the requested status.

## Current coverage

- Null, booleans, 64-bit integers, floats, and binary-safe strings.
- `time` returns Unix seconds, `microtime` returns a microsecond-resolution
  string or float, and `hrtime` returns monotonic nanoseconds or a seconds/nanoseconds
  pair. Boolean conversion, named arguments, and optional defaults follow PHP.
- PHP 8.5 NaN conversion warnings for boolean, string, and array conversions;
  NaN comparisons remain silent and unordered against numbers and strings.
- Explicit scalar, array, and object casts, cast-alias deprecations, and PHP 8.5 `(void)`
  result discarding in statements and permitted `for` clauses.
- `intval`, `floatval`/`doubleval`, `boolval`, and `strval` share native conversion
  behavior; `intval` also supports radix prefixes and saturating string parsing.
  `is_numeric` and `is_scalar` inspect values without invoking Stringable code.
- `settype` mutates references using native type aliases and conversion rules,
  enforces typed-property assignments, and preserves conversion-exception effects.
- Global `const` declarations, `define`/`defined`/`constant`, qualified lookup,
  and core numeric, version, separator, and error-level constants.
- Ordered arrays, PHP key coercion, copy-on-write assignment, nested indexed
  writes, append, array union and comparison, `unset`, `isset`, and `empty`.
- Array unpacking with integer-key reindexing and string-key overwrites.
- `ArrayAccess` object offsets support reads, assignment, append, unset, isset,
  empty, coalescing, and destructuring. Nested mutation preserves reference-returning
  getters; increments update returned references with typed-property constraints.
  The interface exposes tentative return types and honors ReturnTypeWillChange.
- `Countable`, `count`/`sizeof`, and `is_countable` support user objects, native
  result conversion, mode validation, tentative return types, and exceptions.
  Recursive array counting protects against cycles and warning-handler reentry;
  nested objects are counted as entries without invoking their count methods.
- `Traversable`, `Iterator`, and `IteratorAggregate` support user-defined `foreach`,
  nested aggregates, callback ordering, exceptions, and reference restrictions.
  Iterable type checks, `is_iterable`, and reflection recognize these objects;
  interface methods expose PHP's tentative return types.
- `iterator_to_array`, `iterator_count`, and `iterator_apply` support user iterators,
  preserved references and keys, native callback argument rules, and early stopping.
  Array conversion/counting also accept arrays. `SeekableIterator`,
  `RecursiveIterator`, and `OuterIterator` extend the native Iterator interface.
- `ArrayIterator` provides native array/object storage, cursor methods, offsets,
  append, count, array copies, debug output, and shared-storage cloning with
  independent cursors. Native iteration supports references and typed backing
  properties; storage participates in destruction and cycle collection.
  Six sorting methods use the array sort engine with callback mutation guards;
  `ARRAY_AS_PROPS` routes property access and references to backing offsets.
  Native comparisons inspect backing storage and ordinary properties.
  `__serialize`/`__unserialize` preserve backing storage, flags, ordinary members,
  and iterator classes. Legacy `Serializable` methods encode and restore the SPL
  format with shared reference tables and deferred object hooks.
- `ArrayObject` shares native storage, offsets, flags, sorting, and property routing,
  with storage exchange, independent storage clones, and configurable iterator
  factories. Its iterators share live storage and skip user constructors.
- `RecursiveArrayIterator` provides child detection, `CHILD_ARRAYS_ONLY`, and
  subclass child construction with parent storage links. Parent writes and child
  sorting can separate storage. Native child writes also reach captured nested-array
  copies; ordinary PHP writes retain copy-on-write behavior.
- `IteratorIterator` wraps traversable objects with cached values/keys and inner
  method forwarding. FilterIterator, callback and recursive filters, ParentIterator,
  LimitIterator, NoRewindIterator, InfiniteIterator, and EmptyIterator provide
  filtering, bounded seeking, restart control, and recursive child construction.
  Callback filters retain callable access scope; native wrapper storage participates
  in cycle collection. Additional SPL iterator classes and full lifecycle timing
  remain incomplete.
- `CachingIterator` and `RecursiveCachingIterator` provide one-element lookahead,
  string-conversion modes, full-cache array access, and recursive child caching.
  Returned cache arrays observe native writes and resets; recursive caches can
  suppress child-fetch exceptions. Cached values participate in cycle collection,
  whose counts and complete lifecycle behavior remain incomplete.
- `AppendIterator` chains iterators, exposes its queued inputs through ArrayIterator,
  skips exhausted inputs, and can append inputs during traversal. Explicit current()
  reads follow changes to the active inner cursor. Native iterator handles and cached
  values are released when switching inputs; generator integration remains unfinished.
- `MultipleIterator` combines iterator keys and values, with any/all validity modes,
  numeric or associated keys, duplicate-key checks, and attachment management.
  It shares native object storage for cloning, debug output, and cycle collection;
  native dimension access follows its flags. Complete mutation timing remains
  unfinished.
- `RecursiveIteratorIterator` traverses recursive iterators in leaf-only, self-first,
  or child-first order, with depth limits, subclass hooks, child-error catching,
  inner-method forwarding, and native iterator lifetime handling.
- `RecursiveTreeIterator` renders recursive inputs with configurable prefix parts
  and a postfix, with flags to bypass key/current formatting. It wraps inputs in
  RecursiveCachingIterator for lookahead, accepts aggregates, preserves formatting
  across reinitialization, and retains its current depth after a foreach break.
- `SplDoublyLinkedList`, `SplQueue`, and `SplStack` provide native list storage,
  offsets, insertion/removal, count, debug output, and independent storage clones.
  Foreach cursors retain their starting mode and support FIFO/LIFO and delete/keep
  traversal. Both magic serialization hooks and legacy `Serializable` methods
  are available. Legacy restoration replaces list contents and preserves references.
- `SplObjectStorage` stores values by object identity or overridden `getHash`,
  with offsets, set operations, seekable iteration, debug output, comparisons,
  cloning, and both magic and legacy serialization. Native dimension access
  preserves PHP's null-entry probes and iterator behavior. Storage participates
  in cycle collection; `SplObserver` and `SplSubject` interfaces are available.
- `SplHeap`, `SplMinHeap`, `SplMaxHeap`, and `SplPriorityQueue` provide native
  heap ordering, comparison callbacks, destructive iteration, extraction flags,
  cloning, debug output, and serialization hooks. Failed comparisons mark heaps
  as corrupted; mutation guards and recovery follow native heap behavior.
  Stored values participate in cycle collection. Complete reentrant mutation
  and destructor timing remain unfinished.
- `SplFileInfo` provides path components, metadata and access queries, symlink
  targets, canonical paths, cloning, debug output, and file-info class factories.
  Native stat/lstat caches are cleared by `clearstatcache()` and ordinary file I/O.
  File-info objects cannot be serialized. `openFile()` creates the selected file
  class with its mode, include-path setting, and stream context. Custom stream
  wrappers and PHP's realpath cache remain unfinished.
- [`SplFileObject` and `SplTempFileObject`](docs/conformance/php-8.5.0-file-objects-hillclimb.txt)
  provide line and CSV iteration, seeking, read-ahead and empty-line flags,
  configurable CSV controls, scanning into references, byte I/O, metadata,
  truncation, flushing, and file locking. Temporary files spill to disk at their
  memory threshold. Constructors, subclass line readers, debug output, clone
  restrictions, and shutdown handle cleanup follow the native file objects.
  Custom stream-wrapper integration remains unfinished.
- `DirectoryIterator`, `FilesystemIterator`, and `RecursiveDirectoryIterator`
  provide native directory cursors, seeking, key/current modes, dot filtering,
  recursive children, subpaths, and independent cloned handles. Native filesystem
  iteration caches current values; explicit current() calls create fresh file-info
  objects. Recursive wrappers expose the source iterator for child methods and
  forwarding. `GlobIterator` and `glob://` directory streams expand byte-oriented
  patterns with wildcards, bracket ranges, POSIX character classes, and escapes.
  Match counts, per-entry paths, empty streams, and rewind snapshots follow PHP.
  GlobIterator cloning is forbidden. Custom stream wrappers, open_basedir filtering,
  and complete mutation/destructor timing remain unfinished.
- `SplFixedArray` provides dense storage, resizing, offsets, array conversion,
  independent storage clones, and native iteration through `InternalIterator`.
  Resizing preserves destructor order and deferred size changes. Property inspection,
  serialization hooks, and `JsonSerializable` are available, including JSON encoding.
  `get_mangled_object_vars` exposes ordinary object properties without visibility filtering.
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
- `random_bytes` and `random_int` use operating-system randomness on macOS and
  Linux. Integer ranges include both endpoints, cover the full signed 64-bit
  range, and avoid modulo bias. Entropy-source failures throw
  `Random\RandomException`; other operating systems remain unsupported.
- `mt_rand`/`rand` share seeded MT19937 state with `mt_srand`/`srand`, including
  PHP's deprecated legacy mode, 32/64-bit ranges, and maximum-value functions.
  `array_rand` selects unique keys in input order using the same generator;
  exact seeded single-key sequences for arrays with deleted bucket history
  remain unfinished. `lcg_value` implements the deprecated combined LCG.
- `shuffle` and `str_shuffle` use the shared MT generator with unbiased
  Fisher–Yates swaps. Array shuffling renumbers keys, preserves reference values,
  resets the internal pointer, and updates active iteration positions; string
  shuffling operates on bytes, including embedded NULs.
- Trigonometric/inverse and hyperbolic/inverse functions, `atan2`, `hypot`,
  `exp`/`expm1`, `log`/`log10`/`log1p`, degree/radian conversion, `pi`, and
  PHP's standard mathematical constants.
- `array_map`, `array_filter`, and `array_reduce`, including multiple-array zip
  behavior, null callbacks where allowed, filter key modes, initial reduction
  values, and PHP's weak scalar coercion for internally invoked callbacks.
- `array_find`, `array_find_key`, `array_any`, and `array_all` short-circuit on
  predicate results, preserve callback reference behavior, and detach found values.
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
- `min` and `max` accept arrays or variadic values, preserve result types, and
  follow PHP's numeric comparison paths, tie-breaking, and NaN behavior.
- `sort`/`rsort`, `asort`/`arsort`, `ksort`/`krsort`, `natsort`, and `natcasesort`
  support stable ordering, numeric/string/natural modes, keys, references, and
  cursor resets. Locale sorting currently uses the C locale; reentrant copies
  captured during conversion do not yet observe the live sorting table.
- `usort`, `uasort`, and `uksort` invoke comparators on a separate sorting copy,
  preserve stable ties, support PHP's return coercion and boolean retry behavior,
  and install the resulting array even when a callback throws or replaces the input.
- `array_multisort` applies stable lexicographic row ordering with independent
  comparison/direction flags, preserves string keys, renumbers numeric keys, and
  supports PHP's preferred-reference arguments and repeated input aliases.
- `extract` supports overwrite, skip, conditional/prefixed extraction, and
  reference rebinding in the current scope, including typed-reference assignments,
  compiled-but-unset locals, name validation, and dynamic-call restrictions.
- `compact` traverses nested variable-name lists with ordered results and recursion
  diagnostics. `get_defined_vars` snapshots the current scope, preserving shared
  references and omitting undefined locals and the implicit `$this` context value.
- `array_splice` edits arrays by reference, preserves string keys and reference cells,
  renumbers numeric keys, converts replacements, and resets the internal cursor.
  Discarded removals run destructors before rebuilding the table, preserve retained
  aliases and exception traces, and detect destructor-driven array replacement.
  Sparse-table iterator history and some coercion/reentrant timing remain incomplete.
- `array_push`, `array_pop`, `array_shift`, and `array_unshift` support reference
  targets, copy-on-write, numeric append-key restoration, and cursor/iterator updates.
  Pop/shift return values without retaining the removed element’s reference binding.
  Self-referencing removals read the updated array after releasing its write borrow.
- `array_column` extracts array entries or visible/magic object properties, supports
  optional index columns, preserves null values, and returns dereferenced values.
- `array_diff` and `array_intersect`, including their `_key` and `_assoc` variants,
  preserve input keys/references and follow PHP's string comparison and conversion
  order. Their user-comparator variants support value/key callbacks, separate
  comparators, reference arguments, return coercion, and boolean retry semantics.
- `array_unique` retains the first duplicate under PHP's string, numeric, regular,
  natural, or case-insensitive comparisons. `array_change_key_case` converts ASCII
  keys, preserving reference bindings and the position of colliding keys.
- Resource values preserve handle identity and closed state across aliases, with
  `is_resource`, `get_resource_id`, `get_resource_type`, inspection, casts, and key conversion.
- `md5`, `sha1`, `md5_file`, and `sha1_file` produce hexadecimal or binary digests
  using incremental Rust implementations. File hashing uses supported streams,
  preserves native diagnostics and resource lifetimes, and distinguishes MD5's
  EOF requirement from SHA-1's partial-read result. General hash-extension APIs
  and data/custom/network stream wrappers remain unfinished.
- `crc32` computes the standard 32-bit checksum of binary strings and returns
  a nonnegative PHP integer. `convert_uuencode` and `convert_uudecode` support
  PHP's 45-byte encoding lines, termination and malformed-input behavior.
- `levenshtein` compares bytes with configurable insertion, replacement and
  deletion costs, including strings longer than 255 bytes. `similar_text` follows
  PHP's matching order and writes its percentage by reference, respecting typed
  properties. Both support native argument conversion and named arguments.
- `soundex` and `metaphone` implement PHP's phonetic rules, including duplicate
  handling, initial-letter exceptions and Metaphone output limits. Their letter
  classification follows the C locale; general locale changes remain unfinished.
- `str_increment` and `str_decrement` step through ASCII alphanumeric strings,
  preserving letter case and applying PHP's carry, borrow and underflow rules.
  Numeric-looking inputs retain string semantics, including scientific notation.
- `quoted_printable_encode` and `quoted_printable_decode` follow PHP's byte
  escaping, line wrapping and soft-break rules. Decoding retains invalid hex
  escapes and stops at input NUL bytes, while decoded `=00` bytes remain intact.
- `utf8_encode` and `utf8_decode` convert between Latin-1 and UTF-8, including
  PHP's malformed-sequence consumption and `?` substitution. They emit PHP's
  deprecation diagnostics; their `Deprecated` metadata is available in reflection.
- `parse_url` follows PHP's component parsing, including scheme/port ambiguity,
  credentials, bracketed hosts, file paths and empty query/fragment components.
  `urlencode`, `urldecode`, `rawurlencode` and `rawurldecode` support binary input,
  PHP's space/plus and tilde rules, and permissive malformed-escape handling.
- [Query-string parsing](docs/conformance/php-8.5.0-query-strings-hillclimb.txt)
  supports `parse_str` with nested arrays, PHP name normalization, permissive
  URL decoding, typed output references and native destructor/handler ordering.
  It uses the default `&` separator, 1,000-field limit and 64-level nesting limit;
  configuring these through INI remains unfinished.
- `htmlspecialchars`, `htmlentities`, their decode functions and
  `get_html_translation_table` support PHP's HTML 4.01, XML, XHTML and HTML5 rules,
  named and numeric entities, multi-codepoint entities, quote/substitution flags,
  double-encoding control and legacy charsets. The default charset remains UTF-8;
  request-level charset INI settings are unfinished.
- [JSON encoding, decoding, and validation](docs/conformance/php-8.5.0-json-hillclimb.txt)
  support formatting and Unicode flags, associative decoding, large integers,
  depth checks, recursion detection, partial output, and `JsonException`.
  `json_last_error` and `json_last_error_msg` preserve PHP's exception-mode rules.
  `JsonSerializable` callbacks, backed enums, lazy-object initialization, live
  declared properties, property getters, and dynamic-property snapshots are supported. Validation
  uses the decoder's grammar without building decoded arrays or objects.
  Configurable serialization precision remains unfinished;
  encoding currently limits native traversal to 1,024 container levels.
- [Environment functions](docs/conformance/php-8.5.0-environment-hillclimb.txt)
  support `getenv` and `putenv`, including enumeration, deletion, native name
  filtering and reassignment order. Environment changes are local to the request;
  INI interpolation and temporary-file creation use that same state. The CLI has
  no separate SAPI environment for `getenv`'s `local_only` argument.
- [INI parsing](docs/conformance/php-8.5.0-ini-hillclimb.txt) supports
  `parse_ini_file` and `parse_ini_string`, normal/raw/typed scanner modes,
  sections, array entries, constants, bitwise expressions, quoted strings,
  environment substitutions and fallback values. File parsing searches the
  include path. Configuration-directive substitutions remain unfinished;
  nested expressions and substitutions currently have a 512-level guard.
- `pack` and `unpack` support PHP's binary integer, float, string, hexadecimal,
  padding and cursor formats, including explicit byte order, repeat counts,
  named unpacked fields and input offsets. Packing preserves conversion and
  warning order, including pending exceptions and `*` string-argument replacement.
- Basic file streams support `fopen`, `fclose`, `fread`, `fgetc`, `fwrite`/`fputs`, `feof`,
  `ftell`, `fseek`, `rewind`, and `fflush`, plus `file_exists` and `unlink`.
  `php://memory` and `php://temp` are supported, including temporary spill to disk
  and `/maxmemory:` thresholds. General wrapper registration remains unfinished.
  Supported `php://` names are case-insensitive and retain their original URI spelling.
- `stream_is_local` classifies existing streams and supported wrapper names with
  native string-conversion and unknown-wrapper diagnostics. `stream_supports_lock`
  queries the backend capability without acquiring a lock; memory and directory
  streams report false. General wrapper registration and the standalone `flock`
  function remain unfinished; `SplFileObject::flock` is available.
- `ftruncate` shrinks or extends file, memory, and temporary streams with native
  read-only behavior. It preserves logical position, EOF, and stat caches; memory
  streams clamp their internal cursor on shrink and zero-fill newly added bytes.
- `stream_copy_to_stream` copies from the current position or a positive absolute
  offset, with optional byte limits and native resource/seek diagnostics. Shared
  transfers preserve mapped-file source advancement on write failure and the
  differing EOF/stat-cache effects of mapped and ordinary reads. Mapped ranges
  currently use owned buffers of up to 512 MiB; native memory mapping and Linux
  `copy_file_range` optimizations remain unfinished.
- `stream_get_line` reads bounded records with binary delimiters, including
  delimiters crossing buffer boundaries. A zero length uses PHP's 8 KiB default.
  Buffered data is shared with later reads, writes, seeks, copies, and metadata;
  unread bytes remain available after truncation, and buffer-only reads retain
  stat caches. Custom/nonblocking wrappers and complete native buffering outside
  record reads remain unfinished.
- `stream_context_*` APIs create contexts, merge options, manage notification
  callbacks, and share a request's default context. Returned option arrays retain
  snapshots across later updates. Plain files and memory/directory streams lazily
  allocate independent contexts when queried. Network/custom wrapper behavior,
  notification delivery, and complete resource-cycle collection remain unfinished.
- `copy` transfers supported file/stream contents with directory checks and
  same-inode protection. `rename` moves local files and directories, clears stat
  caches, and falls back to a private copy plus ownership/permission restoration
  across filesystems. Both preserve native filesystem effects when context or
  wrapper exceptions are pending. File URLs and unknown-wrapper diagnostics are
  supported; data/custom/network wrappers and Windows rename behavior remain unfinished.
- `file_put_contents` writes scalars, Stringable objects, arrays, and stream data,
  with append mode, exclusive locking before truncation, and PHP's continued
  scalar-array writes after a string conversion throws. `readfile` sends file bytes to
  output and returns the transferred count. FILE_APPEND and LOCK_* constants are
  available; custom wrappers and complete native buffering remain unfinished.
  `fpassthru` outputs the remainder of an open stream, preserving native position,
  EOF, and read-error behavior without closing it.
- `tmpfile`, `tempnam`, and `sys_get_temp_dir` provide temporary streams and named
  files, with atomic exclusive creation, private permissions, PHP prefix handling,
  cached TMPDIR selection, and fallback notices. Temporary stream files survive
  resource aliases and are removed on close or final release. `stream_get_meta_data`
  exposes supported wrapper types, modes, URI, EOF, seekability, and buffered input
  counts; memory and temporary streams honor append mode. Configurable
  sys_temp_dir/open_basedir and complete stream metadata/buffering remain unfinished.
- `stat`, `lstat`, and `fstat` expose PHP's 26-entry numeric/named metadata arrays.
  Path calls share the stat/lstat caches; handle calls read fresh metadata without
  moving the stream position. Memory streams use native synthetic fields, and
  directory handles return false. File URLs and unknown-wrapper diagnostics are
  supported; custom stat wrappers and non-Unix metadata remain unfinished.
  `PHP_OS` and `PHP_OS_FAMILY` identify the runtime platform.
- `link`, `symlink`, `readlink`, and `linkinfo` support local Unix links with
  native diagnostics, literal symbolic-link targets, fresh device metadata, and
  preserved stat caches. `realpath` resolves existing paths, including an empty
  path for the working directory. Link path expansion enforces native path limits
  and a 32-symlink traversal limit. Realpath caching, open_basedir enforcement,
  and Windows-specific link/metadata behavior remain unfinished.
- `chmod` changes Unix file/directory permissions, follows symlinks, and clears
  cached metadata on success. It preserves native path/wrapper diagnostics and
  permission changes that occur while a wrapper warning exception is pending.
  Custom metadata wrappers, open_basedir checks, and Windows permissions remain
  unfinished.
- `mkdir`, `rmdir`, and `touch` create directories, remove empty directories, and
  update file timestamps without truncating existing contents. Recursive mkdir
  resolves dot components and applies native permission/umask behavior. `chdir`
  and `getcwd` use request-local paths. File metadata and access queries share
  the SplFileInfo stat/lstat caches; chdir clears cached relative paths, and
  touch/rmdir clear metadata caches. Custom wrappers, open_basedir, realpath-cache
  behavior, and tracking a renamed working directory remain unfinished.
- `fgetcsv` reads quoted records across physical lines, preserving embedded line
  endings and respecting the first-line length limit. `fputcsv` writes escaped
  fields with configurable separators, enclosures, escapes, and line endings;
  field-conversion exceptions preserve PHP's write-before-propagation behavior.
  `fgets`, `stream_get_contents`, and `file` provide line, remainder, and line-array
  reads with length/offset controls and FILE_* flags. Configurable encodings,
  automatic legacy line-ending detection, custom wrappers/filters, and complete
  stream-buffering behavior remain unfinished.
- `file_get_contents` reads local files and supported wrappers, with include-path
  lookup, positive/negative offsets, length limits, and native read diagnostics.
- `opendir`, `readdir`, `rewinddir`, `closedir`, and `scandir` support local
  directories on macOS and 64-bit Linux. Handles survive directory renames;
  default-handle lifetime, sorting constants, and directory stream operations
  follow PHP. `Directory` objects, other platforms, configurable collation,
  and general stream wrappers remain unfinished.
- `STDIN`, `STDOUT`, `STDERR`, and their `php://` wrappers preserve resource identity,
  alias close state, pipe positions, and interleaved output. Input uses bounded
  buffered reads; closing stdout terminates later ordinary output. Redirected output
  seeks and nonblocking stream configuration remain unfinished.
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
  expose those handles.
- Object destructors run on reference release and after shutdown callbacks, with
  visibility checks, resurrection, constructor-failure suppression, and exception
  propagation. Shutdown visits unique globals before remaining live objects.
  `gc_collect_cycles` collects unreachable object cycles through properties,
  arrays, and reference cells, protecting live roots and reentrant collection.
  Array-only and closure cycles, automatic GC thresholds, and some temporary
  cleanup and mutation timing remain incomplete.
- Anonymous classes, including readonly declarations, constructor arguments,
  inheritance, interfaces, and traits. Repeated executions share the declaration's
  class and static state; separately compiled declarations have distinct identities.
- Unit and int/string-backed enums with shared case identities, readonly name/value
  properties, constants, methods, traits, interfaces, `cases`, `from`, and `tryFrom`.
  Native UnitEnum/BackedEnum contracts, `enum_exists`, enum comparisons/dumps, and
  enum/case reflection are supported. Full reflection strings remain unfinished.
- `var_export` prints or returns PHP expressions for scalars, arrays, objects,
  closures, and enum cases. It preserves byte strings, floating-point fractions,
  nested indentation, and detects circular references across getter callbacks.
  Property getters run during export; configurable serialization precision remains unfinished.
- `serialize` encodes scalars, byte strings, arrays, objects, and enum cases,
  preserving shared references and object identity. It supports `__serialize`,
  `__sleep`, and the user-defined `Serializable` interface, including nested hook
  reference tables and legacy-interface deprecations. Configurable precision,
  deep nesting beyond 1,024 levels, and complete mutation/destructor
  timing remain unfinished.
- `unserialize` restores scalars, byte strings, arrays, objects, enum cases, and
  shared references. It validates typed properties, supports `allowed_classes`
  and `max_depth`, and defers magic restoration hooks until the graph is parsed.
  Unknown/disallowed classes become `__PHP_Incomplete_Class`, with restricted
  property/method access and preservation of their original serialized class name.
  Native legacy formats are supported for ArrayObject/ArrayIterator and linked
  lists, as well as SplObjectStorage. Other SPL classes, INI configuration, and
  complete lifecycle compatibility
  remain unfinished.
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
- [`ReflectionClass`/`ReflectionObject`](docs/conformance/php-8.5.0-class-rendering-hillclimb.txt)
  class names, source locations, documentation, extension ownership, flags,
  inheritance/interface/trait queries, constants, defaults, static property access,
  and construction with or without constructors. Class, interface, trait, enum,
  and object declaration strings include members in reflection order, enum cases,
  inherited visibility, and dynamic property names. Property strings show declaration
  defaults, including after instance/static values change. Promoted properties retain
  their documentation comments.
- [Lazy objects](docs/conformance/php-8.5.0-lazy-objects-hillclimb.txt) support
  reflection-created ghosts and proxies, explicit initialization, initializer queries,
  and resetting existing instances. Property reads, writes, references, iteration,
  comparison, cloning, and serialization trigger initialization where PHP requires it.
  Ghost initializer exceptions restore property state; proxy properties delegate to the
  factory's compatible instance. Reflection can skip individual properties or assign
  their raw values without initialization. Array casts, mangled property queries,
  and ordinary debug output preserve deferred initialization. Uninitialized objects
  suppress destructors, and reset options support skipping destruction or serialization
  initialization. Raw reflection writes can bypass property hooks with or without
  lazy initialization. Some initialization lifetime and compound-assignment edge
  cases remain unfinished.
- [Function/method/parameter reflection](docs/conformance/php-8.5.0-function-rendering-hillclimb.txt)
  exposes names, source lines, visibility,
  signature metadata, defaults, attributes, named/union types, and closure captures.
  Reflective invocation supports named arguments, argument-array references, weak
  coercion, non-public methods, and closure creation.
  Function, method, and parameter declaration strings include defaults, documentation,
  source locations, closure captures, and inheritance/prototype flags. Native function
  metadata covers all 430 implemented function names, including types, symbolic defaults,
  extension ownership, SensitiveParameter attributes, and by-reference modes.
  Method prototype queries and function/method documentation queries are supported.
  Complete native method signatures, static-variable queries, full reflector
  interfaces, complete internal class layouts, closure-object reflection, and advanced
  default-expression folding remain unfinished.
- [`Reflection::getModifierNames`](docs/conformance/php-8.5.0-modifier-names-hillclimb.txt)
  reports abstract, final, virtual, visibility, asymmetric setter, static, and
  readonly flags in PHP's order. Conflicting visibility bits and unknown flags
  follow the native mask rules.
- [`ReflectionReference`](docs/conformance/php-8.5.0-reference-reflection-hillclimb.txt)
  detects shared array references and self-referential arrays, retains reference
  lifetime, and provides stable opaque IDs within a runtime. Numeric string keys
  use literal string lookup. Construction, cloning, and serialization follow the
  native restrictions.
- Property reflection supports declared/inherited/dynamic discovery, filters, types,
  defaults, attributes, promoted/readonly/asymmetric flags, mangled names, and
  initialization checks. Reflective reads and writes bypass visibility while retaining
  typed-reference and readonly constraints. [Hook reflection](docs/conformance/php-8.5.0-hook-reflection-hillclimb.txt)
  exposes get/set methods, final/virtual flags, settable types, attributes, prototypes,
  invocation, and closures. Raw access bypasses available hooks; declaration strings
  show hooks and ordinary defaults. Unevaluated expression rendering and complete
  internal property metadata remain unfinished.
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
  Native exception fields use ordinary declared property storage, including typed
  private trace/previous slots, protected fields, and ErrorException severity.
  Getters, reflection, serialization, references, and property writes observe the
  same values. `__wakeup` validates legacy message/code values; native cloning is
  forbidden. Error reporting observes location changes made by `__toString`.
- `set_error_handler`, `get_error_handler`, and `restore_error_handler` preserve
  callable identity, handler stacks, and signed 32-bit level masks. `trigger_error`
  and `user_error` dispatch user notices, warnings, deprecations, and fatal errors.
  Handlers run independently of display suppression, including `@`; only a literal
  false return requests default reporting. Active handlers are disabled during
  invocation, with replacement/restored handlers honored. Exceptions and exits
  propagate synchronously. Handleable compile warnings from eval/includes also
  dispatch; compile-only warnings bypass user handlers. Array-key and string-offset
  write diagnostics release storage borrows before dispatch and observe handler
  replacement of live variables/properties. Compound writes and some nested
  mutation cases remain incomplete. Float-to-int
  precision/overflow diagnostics preserve the shortest round-tripping value.
- `error_get_last` returns the latest default-reported diagnostic, including
  suppressed warnings and uncaught errors, as a fresh type/message/file/line array.
  Handled diagnostics preserve the prior record. `error_clear_last` clears the
  record and fatal backtrace; native fatal errors retain a trace for shutdown code.
  The PHPT harness disables fatal backtraces on both Rust and reference PHP,
  matching PHP's official test-runner default. CLI requests retain them.
- `set_exception_handler`, `get_exception_handler`, and `restore_exception_handler`
  preserve callable identity and handler stacks, including null entries. Uncaught
  file/stdin exceptions dispatch at the request boundary; replacement handlers can
  catch exceptions from an active handler. CLI `-r` and initial compilation errors
  bypass handlers, following PHP.
- `register_shutdown_function` retains resolved callbacks and argument values,
  runs them in registration order, and includes callbacks added during shutdown.
  Handled exceptions continue the queue; exits and unhandled exceptions stop it.
  Shutdown runs after normal completion, exit, or a reported fatal error and
  preserves prior exit status. Destructor and output-buffer shutdown phases
  remain unfinished.
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
- [Weak references](docs/conformance/php-8.5.0-weak-references-hillclimb.txt)
  support `WeakReference::create()` and `get()` for objects and closures. Repeated
  creation returns the same live wrapper without keeping its target alive. Object
  identity survives destructor-queue moves and resurrection; reused numeric handles
  do not revive expired references. Debug output exposes the current target.
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
- [Object initializers](docs/conformance/php-8.5.0-object-initializers-hillclimb.txt)
  allow `new` in parameter defaults and global constants, including nested objects,
  named constructor arguments, and promoted defaults. Omitted arguments and
  reflection default queries construct fresh objects; global constants retain their
  initialized object. Constructors run at evaluation time, with declaring scope
  preserved for `self` and `parent`.
- Live `global` bindings and function-local `static` storage, including dynamic
  initializers, recursive calls, and local `unset` behavior. Variable variables
  (`$$name` and `${expression}`) work with reads, writes, offsets, and probes.
- `$GLOBALS` element reads/writes, nested mutation, probes, and unsetting, with
  independent whole-array snapshots and compile-time mutation restrictions.
  Symbol-table ordering preserves compiled variable slots and dynamic names.
- `get_include_path`, `set_include_path`, and `stream_resolve_include_path` provide
  request-local include-path configuration and canonical path lookup. Source
  loading, default autoloading, and file functions share search order, explicit
  relative/absolute paths, file URLs, and executing-source-directory fallback.
  New files still go to the working directory when no existing path match is found.
  INI integration, custom/network wrappers, and realpath-cache behavior remain unfinished.
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
- `#[Deprecated]` emits user deprecations on function/method calls, constant and
  enum-case reads, and trait use. Messages retain named arguments, `since`, binary
  strings, caller strictness, and declaration scope. Constant warnings guard
  recursive attribute evaluation and error handlers. Invalid class, interface,
  and enum targets produce compile errors; reflection honors delayed validation.
- `#[NoDiscard]` warns before calls whose results are discarded, including magic
  methods, nullsafe calls, pipes, and suppressed expressions. Assignment and
  `(void)` consume the result. Attribute messages use caller strictness; throwing
  handlers prevent execution, and `Deprecated` diagnostics run first.
- [Property hooks](docs/conformance/php-8.5.0-property-hooks-hillclimb.txt) support
  concrete getters and setters, short bodies, explicit setter parameters,
  virtual and backed properties, inherited hooks, trait scope, and final hooks.
  Reads, writes, compound assignments, reference getters, indirect-mutation
  errors, and lazy-object backing access use the hook call frame. `__PROPERTY__`
  and reflection's `hasHooks`/`isVirtual` queries are available. Hook attributes
  and full hook reflection remain unfinished.
- [Abstract and interface hooks](docs/conformance/php-8.5.0-abstract-hooks-hillclimb.txt)
  enforce required getters/setters and allow plain properties to implement them.
  Readonly properties satisfy getter requirements; missing setters remain abstract.
  Getter references, hook signatures, virtual-property variance, setter parameter
  compatibility, autoloaded types, and abstract reflection flags are supported.
  Complex interface merges and intersection/DNF type handling remain unfinished.
- [Hook-aware enumeration](docs/conformance/php-8.5.0-hook-export-hillclimb.txt)
  invokes visible getters during `foreach`, `get_object_vars`, JSON encoding,
  and `var_export`, and skips virtual write-only properties. Reference iteration
  requires reference getters and preserves aliases. Declared values stay live;
  dynamic values are captured for export and stay live during iteration.
  Dumps and serialization retain backing storage without invoking getters;
  virtual properties have no class defaults and reject unserialized values.
- [Parent-hook calls](docs/conformance/php-8.5.0-parent-hooks-hillclimb.txt)
  dispatch `parent::$prop::get()` and `parent::$prop::set()` in the matching
  property hook, retaining inherited declaration scope, references, and lazy
  backing access. Plain parent properties use internal accessors with strict
  argument counts and named-argument checks. Invalid lexical contexts and
  attempts to create parent-hook closures produce compile errors.
- [Promoted property hooks](docs/conformance/php-8.5.0-promoted-hooks-hillclimb.txt)
  run setters during constructor entry, support implicit public promotion when
  hooks appear without visibility modifiers, and preserve parameter defaults
  separately from backing storage. Trait promotion, parent-hook calls, and
  property/parameter attributes use the promoted declaration; readonly hooks
  are rejected.
- [Property declaration rules](docs/conformance/php-8.5.0-hook-rules-hillclimb.txt)
  reject final-property overrides before accessor merging, identify the original
  owner in diagnostics, and reject hooked-property conflicts during trait
  composition. Class-linking compile errors terminate the request. Reference
  assignment invokes available getters before rejecting an overloaded target;
  missing getters retain write-only and indirect-modification errors.
- `#[Override]` validates methods and properties against inherited declarations,
  including interfaces, trait requirements, static members, and promoted
  properties. Private parent members and concrete constructors do not qualify.
  Hook-level `#[Override]` checks inherited accessors and backing operations.
  Invalid declarations produce fatal diagnostics. Hook attributes retain lexical
  magic constants; `#[NoDiscard]` hook errors support delayed validation.
- Default-enabled `assert` validates descriptions, throws supplied Throwable
  objects unchanged, and creates AssertionError with code 1 on failure. Direct
  calls receive generated descriptions for supported expressions and declarations.
  `assert_options` controls activation, callbacks, warnings, exceptions, and
  bailout, including PHP 8.5 deprecations. Assertion INI configuration and complete
  AST description coverage remain unfinished.
- Lexical `__LINE__`, `__FILE__`, `__DIR__`, `__FUNCTION__`, `__METHOD__`,
  `__CLASS__`, `__NAMESPACE__`, and `__TRAIT__`, including the using class for
  trait `__CLASS__` and the original defining trait for `__TRAIT__`/`__METHOD__`.
- Parameter and return types for available values, nullable/union types, scalar
  coercion, typed variadics, default validation, and `void`/`never` return rules.
  `declare(strict_types=1)` preserves caller and declaration scope across calls;
  strict calls also check scalar arguments to the supported internal functions.
- `print_r` writes or returns binary-safe formatted values, detects array/object
  reference cycles, labels private/protected property keys, and invokes
  `__debugInfo` for custom object output. `var_dump` invokes the same hooks with
  recursion guards across nested calls, numeric/property keys, and null-return
  deprecations. Dump-time handler exceptions are deferred until traversal ends;
  invalid hook results or hook exceptions produce native fatal errors.
- [`class_parents`, `class_implements`, and `class_uses`](docs/conformance/php-8.5.0-class-queries-hillclimb.txt)
  return canonical class names as array keys and values. Parent queries walk the
  inheritance chain, interface queries include inherited interfaces in native
  order, and trait queries report only directly used traits. Objects, aliases,
  closures, autoload controls, and native validation/warning behavior are supported.
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
  PHP 8.5.0's extra named variadic behavior and serialization prohibition are
  preserved; trace configuration and uninitialized wrapper reads remain unfinished.
- Core builtins: `var_dump`, `print_r`, `strlen`, `count`/`sizeof`, `gettype`, `get_debug_type`,
  `call_user_func`, `call_user_func_array` with positional/named arguments,
  scalar/array type predicates, `array_keys`, `array_values`, `array_key_exists`,
  `function_exists`, and `get_included_files`/`get_required_files`.
- `forward_static_call` and `forward_static_call_array` preserve late static
  binding within the callback's inheritance chain. Object callbacks keep their
  object's class; the array variant accepts named/reference arguments and works
  at global scope. Native callbacks support deprecated `self`, `parent`, and
  `static` class names with their scope and validation rules.
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
  string offset checks with `isset`, `empty`, and null coalescing. Invalid array
  and string offsets report their actual type and operation; scalar offset
  warnings distinguish null, booleans, and numeric values. `array_key_exists`
  preserves PHP's distinct opcode/native diagnostics and null-key deprecation.
- `printf`, `sprintf`, `vprintf`, `vsprintf`, `fprintf`, and `vfprintf` share byte-oriented formatting,
  positional arguments, dynamic width/precision, custom padding, integer bases,
  floating-point formats, and PHP's format-validation errors. String-conversion
  exceptions preserve PHP's partial formatted output. Stream variants report the
  formatted length even when the underlying write fails.
- Binary-safe `str_repeat`, `strrev`, `chr`, and `ord`, with scalar/Stringable
  coercion where applicable and PHP 8.5 byte-range/length deprecations.
- Binary-safe `base64_encode`, permissive/strict `base64_decode`, and ASCII
  `str_rot13`, with native argument coercion and padding validation.
- Byte-oriented `wordwrap` supports custom breaks, optional long-word cutting,
  existing separators, and PHP's zero/negative-width behavior.
- `strtok` keeps request-local tokenization state, supports changing byte delimiters,
  and preserves PHP's continuation, exhaustion, and restart behavior.
- `sscanf` and `fscanf` share numeric, string, scanset, positional, suppressed,
  and byte-count conversions, with field widths, reference outputs, overflow and
  partial-match behavior. `fscanf` consumes one physical line per call. Typed
  references and destructors follow native assignment ordering; configurable
  character locales remain unfinished.
- `str_getcsv` parses quoted and unquoted fields, doubled quotes, configurable
  single-byte separators/enclosures/escapes, and PHP's unterminated-field behavior.
  It follows UTF-8 character scanning; configurable character locales are unfinished.
- `basename`, `dirname`, and `pathinfo` implement POSIX path-string handling,
  suffix removal, repeated parent traversal, and `PATHINFO_*` flags. Windows path
  rules remain unfinished.
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
- `strnatcmp` and `strnatcasecmp` share the natural-order comparator used by
  array sorting, including digit runs, leading zeros, whitespace, and binary strings.
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
complete property-hook support,
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
and complete cycle collection remain unfinished. Cycles without a tracked object
and cycles through closures are not yet collected.
Configurable trace settings, remaining error-handler edge cases, and complete
compile-time fatal error distinctions remain unfinished.
Other `declare` directives and output handlers are not implemented. Destructor
timing during complex expressions and reentrant mutations remains incomplete.
Automatic initialization and scope handling for the other superglobals are not
implemented; `$GLOBALS` currently exposes the variables created by PHP code or
provided through `Runtime::variables`. Include-path INI integration, general
stream wrappers, and PHP configuration remain incomplete.
Complete object interpolation,
some scalar edge cases, complete
syntax validation, resource limits, and exact PHP diagnostic formatting also
remain to be implemented. Passing the repository tests does not establish PHP
conformance. See [ROADMAP.md](ROADMAP.md) for the full target and validation plan.

The PHPT runner supports `TEST`, `FILE`, and exact `EXPECT` output,
plus descriptive `DESCRIPTION` and `CREDITS` sections. The isolated CLI runner
supports `ENV` assignments and `{PWD}` substitution for workers, reference runs,
and cleanup processes; the ordinary library parser keeps `ENV` unsupported.
With `--reference`, it also
supports `EXPECTF` (including `%r` fragments) and `EXPECTREGEX`, using the reference
PHP binary's PCRE engine to match output. It also supports `CLEAN`, running its
cleanup script with reference PHP after the Rust worker and again after reference
validation. Cleanup runs even after a failure or timeout; cleanup errors, output,
or timeouts fail the fixture. The tested `FILE` program still executes in Rust;
reference cleanup supplies no test output. Other sections are reported
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
of the `.phpt` fixture and runs both interpreters against that filename. A nonempty
`CLEAN` section gets a `.clean.php` sibling with the same directory context. It
refuses to overwrite an existing file and removes generated files after the run.
Use `--cwd /path/to/php-src` to select the suite root; the default is the current
directory. Each fixture runs in a separate worker, so a Rust panic or stack
overflow fails that fixture without aborting the suite. The parent removes the
generated files even after worker failure. Worker, reference, and cleanup executions have a
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

## Third-party notices

The sorting implementation adapts PHP/Zend and libc++ algorithms and Martin Pool's
natural string comparator. This product includes the Zend Engine, freely available
at http://www.zend.com. The Zend Engine is freely available at http://www.zend.com.
This product includes software developed by the University of California,
Berkeley and its contributors.
See [third-party notices and licenses](THIRD_PARTY_NOTICES.md).
