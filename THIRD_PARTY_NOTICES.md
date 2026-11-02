# Third-party notices

Environment functions in `src/runtime/environment.rs` follow PHP 8.5.0's
`ext/standard/basic_functions.c` and `main/php_variables.c`, copyright (c) The PHP
Group. PHPT environment handling follows `run-tests.php`. The PHP license below
applies to these adaptations.

INI parsing in `src/runtime/ini.rs` and `src/runtime/ini/scanner.rs` adapts
PHP 8.5.0's `Zend/zend_ini_parser.y` and `Zend/zend_ini_scanner.l`, copyright (c)
Zend Technologies Ltd., by Zeev Suraski and Jani Taskinen. The Zend license below
applies. The native function entry points and array/section callbacks follow
`ext/standard/basic_functions.c` and `basic_functions.stub.php`, copyright (c)
The PHP Group, under the PHP license below.

JSON encoding, decoding, validation, constants, and error handling in
`src/runtime/json.rs` and `src/runtime/json/` adapt PHP 8.5.0's `ext/json/json.c`,
`json_encoder.c`, `json_parser.y`, `json_scanner.re`, `php_json.h`, and
`json.stub.php`, copyright (c) The PHP Group. The sources credit Omar Kilani and
Jakub Zelenka. The PHP license below applies to these adaptations. Native JSON
signature metadata is extracted by the existing reflection metadata generator.

Lazy-object state and property integration in `src/runtime/lazy_objects.rs`,
`src/object.rs`, and the runtime object handlers adapt PHP 8.5.0's
`Zend/zend_lazy_objects.c` and `Zend/zend_object_handlers.c`, copyright (c) Zend
Technologies Ltd., including Arnaud Le Blanc's lazy-object implementation. The Zend
license below applies. Lazy-object reflection APIs in `src/runtime/reflection.rs`,
`src/runtime/reflection/properties.rs`, and `src/runtime/reflection/declarations.rs`
adapt `ext/reflection/php_reflection.c` and its stubs under the PHP license below.

Class/function/parameter reflection rendering in `src/runtime/reflection/class_display.rs`
and `src/runtime/reflection/display.rs` follows PHP 8.5.0's `ext/reflection/php_reflection.c`, copyright (c) The PHP Group,
by Timm Friebe, George Schlossnagle, Andrei Zmievski, Marcus Boerger, and Johannes
Schlueter. `native_signatures.rs` contains PHP 8.5.0 function signature metadata
extracted with `tools/generate_native_signatures.py`. `class_modules.rs` contains
extension ownership extracted with `tools/generate_class_modules.py`. The PHP license
below applies to these adaptations.

SPL class queries in `src/runtime/class_queries.rs` adapt PHP 8.5.0's
`ext/spl/php_spl.c`, `ext/spl/spl_functions.c`, and `ext/spl/php_spl.stub.php`,
copyright (c) The PHP Group, including work by Marcus Boerger. The PHP license
below applies to these adaptations.

Interface enumeration in `src/runtime/reflection.rs` follows PHP 8.5.0's
`Zend/zend_inheritance.c`, copyright (c) Zend Technologies Ltd. The Zend license
below applies to this adaptation.

Default-expression rendering in `src/runtime/reflection/expressions.rs` adapts
PHP 8.5.0's `Zend/zend_ast.c`, copyright (c) Zend Technologies Ltd., by Bob Weinand
and Dmitry Stogov. The Zend license below applies to this adaptation.

`src/runtime/sorting/algorithm.rs` is a Rust adaptation of PHP 8.5.0's
`Zend/zend_sort.c`, by Xinchen Hui and Sterling Hughes, including its libc++
derived sorting algorithm. This product includes the Zend Engine, freely
available at http://www.zend.com. The Zend Engine is freely available at
http://www.zend.com.

Native scanner reference assignment in `src/runtime/scanf.rs` follows
PHP 8.5.0's `Zend/zend_API.h`, `zend_API.c`, `zend_execute.h`, and
`zend_variables.c`, copyright (c) Zend Technologies Ltd. The Zend license below
also applies to this adaptation.

`src/runtime/path_parts.rs` adapts `zend_dirname` from PHP 8.5.0's
`Zend/zend_compile.c`, copyright (c) Zend Technologies Ltd. The source lists
Andi Gutmans, Zeev Suraski, and Nikita Popov as authors. The Zend license below
also applies to this adaptation.

Contextual callback resolution in `src/runtime/method_visibility.rs` and
`src/runtime/callbacks.rs` adapts PHP 8.5.0's `Zend/zend_API.c`, copyright (c)
Zend Technologies Ltd. The Zend license below also applies to these adaptations.

`src/runtime/exception_handlers.rs` and `src/runtime/error_handlers.rs` adapt
handler registration and dispatch from
PHP 8.5.0's `Zend/zend_builtin_functions.c` and `Zend/zend.c`, copyright (c)
Zend Technologies Ltd., by Andi Gutmans and Zeev Suraski. The Zend license below
also applies to these adaptations.

`src/runtime/reference_paths.rs` and `src/runtime/string_offsets.rs` adapt
write diagnostic ordering and storage guards from PHP 8.5.0's
`Zend/zend_execute.c`, copyright (c) Zend Technologies Ltd. The Zend license
below also applies to these adaptations.

`src/runtime/printing.rs` and `src/runtime/diagnostic_display.rs` adapt debug
hook results and pending-exception fatal reporting from PHP 8.5.0's
`Zend/zend_object_handlers.c` and `Zend/zend.c`, copyright (c) Zend Technologies
Ltd. The Zend license below also applies to these adaptations.

Deprecated and NoDiscard attribute diagnostics in `src/runtime/attributes.rs`,
`src/runtime/static_members.rs`, and `src/runtime/objects.rs` adapt PHP 8.5.0's
`Zend/zend_execute.c`, `Zend/zend_constants.c`, `Zend/zend_ast.c`, and
`Zend/zend_inheritance.c`, and `Zend/zend_vm_def.h`, copyright (c) Zend
Technologies Ltd. The Zend license
below also applies to these adaptations.

Assertion descriptions in `src/parser/assertions.rs`, call compilation in
`src/parser.rs`, and numeric interpolation offsets in `src/lexer/strings.rs`
adapt PHP 8.5.0's `Zend/zend_ast.c`, `Zend/zend_compile.c`, and
`Zend/zend_language_scanner.l`, copyright (c) Zend Technologies Ltd. The Zend
license below also applies to these adaptations.

Override validation in `src/runtime/inheritance.rs` and promoted attribute
filtering in `src/parser.rs`, `src/parser/attributes.rs`, and
`src/parser/promotion.rs` adapt PHP 8.5.0's `Zend/zend_inheritance.c` and
`Zend/zend_compile.c`, copyright (c) Zend Technologies Ltd. The Zend license
below also applies to these adaptations.

Object destruction in `src/object.rs` and `src/runtime/destructors.rs`, with
integration in the evaluator, shutdown, constructor, reflection, and inheritance
paths, adapts PHP 8.5.0's `Zend/zend_objects.c`, `Zend/zend_objects_API.c`,
`Zend/zend_execute_API.c`, `Zend/zend_compile.c`, and `Zend/zend_vm_def.h`,
copyright (c) Zend Technologies Ltd. The Zend license below also applies to
these adaptations.

The Countable and iterator declarations in `src/runtime/throwables.rs` adapt
PHP 8.5.0's `Zend/zend_interfaces.stub.php`. User iteration and interface checks
in `src/runtime/iterators.rs` adapt `Zend/zend_interfaces.c`,
`Zend/zend_execute.c`, and `Zend/zend_vm_def.h`. These sources are copyright (c)
Zend Technologies Ltd. The Zend license below also applies to these adaptations.

Key handling in `src/runtime/iterator_helpers.rs` adapts PHP 8.5.0's
`Zend/zend_API.c`, copyright (c) Zend Technologies Ltd. The Zend license below
also applies to this adaptation.

Serializable interface declarations, dispatch, and deprecations in
`src/runtime/serialization.rs`, `src/runtime/throwables.rs`, and
`src/runtime/inheritance.rs` adapt PHP 8.5.0's `Zend/zend_interfaces.c` and
`.stub.php`, copyright (c) Zend Technologies Ltd. Throwable serialization property
layouts and anonymous-class trace names follow `Zend/zend_exceptions.stub.php`
and `Zend/zend_exceptions.c`. The Zend license below also applies to these
adaptations.

Native Throwable property declarations, construction, getters, wakeup checks,
trace formatting, cached string conversion, clone restrictions, and exception
chain handling in `src/runtime/throwable_properties.rs`,
`src/runtime/throwables.rs`, and related object/reporting paths adapt PHP 8.5.0's
`Zend/zend_exceptions.c`, `.stub.php`, and `.h`, copyright (c) Zend Technologies
Ltd., by Andi Gutmans, Marcus Boerger, Sterling Hughes, and Zeev Suraski.
The Zend license below also applies to these adaptations.

--------------------------------------------------------------------
                The Zend Engine License, Version 2.00
Copyright (c) 1999-2006 Zend Technologies Ltd. All rights reserved.
--------------------------------------------------------------------

Redistribution and use in source and binary forms, with or without
modification, is permitted provided that the following conditions
are met:

  1. Redistributions of source code must retain the above copyright
     notice, this list of conditions and the following disclaimer.

  2. Redistributions in binary form must reproduce the above
     copyright notice, this list of conditions and the following
     disclaimer in the documentation and/or other materials provided
     with the distribution.

  3. The names "Zend" and "Zend Engine" must not be used to endorse
     or promote products derived from this software without prior
     permission from Zend Technologies Ltd. For written permission,
     please contact license@zend.com.

  4. Zend Technologies Ltd. may publish revised and/or new versions
     of the license from time to time. Each version will be given a
     distinguishing version number.
     Once covered code has been published under a particular version
     of the license, you may always continue to use it under the
     terms of that version. You may also choose to use such covered
     code under the terms of any subsequent version of the license
     published by Zend Technologies Ltd. No one other than Zend
     Technologies Ltd. has the right to modify the terms applicable
     to covered code created under this License.

  5. Redistributions of any form whatsoever must retain the following
     acknowledgment:
     "This product includes the Zend Engine, freely available at
     http://www.zend.com"

  6. All advertising materials mentioning features or use of this
     software must display the following acknowledgment:
     "The Zend Engine is freely available at http://www.zend.com"

THIS SOFTWARE IS PROVIDED BY ZEND TECHNOLOGIES LTD. ``AS IS'' AND
ANY EXPRESSED OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL ZEND
TECHNOLOGIES LTD.  BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF
USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT
OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
SUCH DAMAGE.

--------------------------------------------------------------------

 * Derived from LLVM's libc++ implementation of std::sort.
 *
 * ===========================================================================
 * libc++ License
 * ===========================================================================
 *
 * The libc++ library is dual licensed under both the University of Illinois
 * "BSD-Like" license and the MIT license. As a user of this code you may
 * choose to use it under either license. As a contributor, you agree to allow
 * your code to be used under both.
 *
 * Full text of the relevant licenses is included below.
 *
 * ===========================================================================
 *
 * University of Illinois/NCSA
 * Open Source License
 *
 * Copyright (c) 2009-2012 by the contributors listed at
 * http://llvm.org/svn/llvm-project/libcxx/trunk/CREDITS.TXT
 *
 * All rights reserved.
 *
 * Developed by:
 *
 *     LLVM Team
 *
 *     University of Illinois at Urbana-Champaign
 *
 *     http://llvm.org
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal with the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 *     * Redistributions of source code must retain the above copyright notice,
 *       this list of conditions and the following disclaimers.
 *
 *     * Redistributions in binary form must reproduce the above copyright
 *       notice, this list of conditions and the following disclaimers in the
 *       documentation and/or other materials provided with the distribution.
 *
 *     * Neither the names of the LLVM Team, University of Illinois at
 *       Urbana-Champaign, nor the names of its contributors may be used to
 *       endorse or promote products derived from this Software without
 *       specific prior written permission.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * CONTRIBUTORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
 * WITH THE SOFTWARE.
 *
 * ===========================================================================
 *
 * Copyright (c) 2009-2012 by the contributors listed at
 * http://llvm.org/svn/llvm-project/libcxx/trunk/CREDITS.TXT
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to
 * deal in the Software without restriction, including without limitation the
 * rights to use, copy, modify, merge, publish, distribute, sublicense, and/or
 * sell copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in
 * all copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
 * FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS
 * IN THE SOFTWARE.
 */

`src/runtime/sorting/natural.rs` is an altered Rust adaptation of PHP's
`ext/standard/strnatcmp.c`. Its original notice follows:

/*

  Modified for PHP by Andrei Zmievski <andrei@ispi.net>

  strnatcmp.c -- Perform 'natural order' comparisons of strings in C.
  Copyright (C) 2000 by Martin Pool <mbp@humbug.org.au>

  This software is provided 'as-is', without any express or implied
  warranty.  In no event will the authors be held liable for any damages
  arising from the use of this software.

  Permission is granted to anyone to use this software for any purpose,
  including commercial applications, and to alter it and redistribute it
  freely, subject to the following restrictions:

  1. The origin of this software must not be misrepresented; you must not
     claim that you wrote the original software. If you use this software
     in a product, an acknowledgment in the product documentation would be
     appreciated but is not required.
  2. Altered source versions must be plainly marked as such, and must not be
     misrepresented as being the original software.
  3. This notice may not be removed or altered from any source distribution.
*/

`src/runtime/array_sets.rs`, `src/runtime/array_user_sets.rs`, and
`src/runtime/array_transform.rs` contain altered Rust adaptations of operations in
PHP 8.5.0's `ext/standard/array.c`, copyright (c) The PHP Group. The source lists
Andi Gutmans, Zeev Suraski, Rasmus Lerdorf, Andrei Zmievski, Stig Venaas, and
Jason Greene as authors.

`src/runtime/file_contents.rs`, `src/runtime/file_writing.rs`, and
`src/runtime/csv.rs` contain altered Rust
adaptations of PHP 8.5.0's `ext/standard/file.c` and `main/streams/streams.c`,
copyright (c) The PHP Group.
Their authors include Rasmus Lerdorf, Stig Bakken, Andi Gutmans, Zeev Suraski,
Thies C. Arntzen, Wez Furlong, and Jim Winstead.

`src/runtime/base64.rs` adapts decoding rules from PHP 8.5.0's
`ext/standard/base64.c`, copyright (c) The PHP Group. The source lists
Jim Winstead and Xinchen Hui as authors.

`src/runtime/wordwrap.rs`, `src/runtime/string_token.rs`, `src/runtime/csv.rs`,
and `src/runtime/path_parts.rs`
contain altered Rust adaptations of PHP 8.5.0's `ext/standard/string.c`, copyright
(c) The PHP Group. The source lists
Rasmus Lerdorf, Stig Sæther Bakken, and Zeev Suraski as authors.

`src/runtime/scalar_values.rs` adapts integer-prefix and `settype` rules from PHP 8.5.0's
`ext/standard/type.c`, copyright (c) The PHP Group. The source lists
Rasmus Lerdorf as author.

`src/runtime/stream_contexts.rs` and context storage in `src/resource.rs` adapt
PHP 8.5.0's `ext/standard/streamsfuncs.c`, `main/streams/streams.c`, and
`Zend/zend_API.c`, copyright (c) The PHP Group. This includes context options,
parameter parsing, and notification callback representation.

Temporary-file creation and directory selection in `src/runtime/temporary_files.rs`,
with lifecycle integration in `src/resource.rs`, adapt PHP 8.5.0's
`main/php_open_temporary_file.c`, `main/streams/plain_wrapper.c`, and
`ext/standard/file.c`, copyright (c) The PHP Group. Metadata and memory/temporary
stream modes in `src/runtime/streams.rs` follow `ext/standard/streamsfuncs.c`,
`main/streams/memory.c`, and `ext/standard/php_fopen_wrapper.c`.

CSV stream reading/writing in `src/runtime/csv.rs` extends the existing CSV
adaptation from PHP 8.5.0's `ext/standard/file.c`. Line and remainder readers in
`src/runtime/streams.rs`, and file() line-array loading in
`src/runtime/file_contents.rs`, follow `ext/standard/file.c`,
`ext/standard/streamsfuncs.c`, and `main/streams/streams.c`, copyright (c) The PHP
Group. These operations reuse the native string-conversion and stream helpers.

`src/runtime/filesystem.rs` implements directory creation/removal, permission changes, timestamps,
request-local working directories, and metadata/access queries from PHP 8.5.0's
`ext/standard/dir.c`, `file.c`, `filestat.c`, and `main/streams/plain_wrapper.c`,
copyright (c) The PHP Group. These operations reuse the native access adapter and
metadata conversion in `src/runtime/file_info.rs`.

`src/runtime/file_stat.rs` adapts stat-array construction and open-stream metadata
from PHP 8.5.0's `ext/standard/file.c`, `filestat.c`, and
`main/streams/{memory,plain_wrapper}.c`, copyright (c) The PHP Group. Metadata
wrapper selection in `src/runtime/filesystem.rs` follows `main/streams/streams.c`;
platform constants in `src/runtime/constants.rs` follow `main/php.h`,
`main/main.stub.php`, and `main/main_arginfo.h`.

`src/runtime/directories.rs` and the directory branches in `src/resource.rs` and
`src/runtime/streams.rs` adapt directory operations from PHP 8.5.0's
`ext/standard/dir.c`, `main/streams/plain_wrapper.c`, and `main/streams/streams.c`,
copyright (c) The PHP Group. Their authors include Thies C. Arntzen and Wez Furlong.

`src/runtime/shutdown.rs` adapts shutdown callback registration and invocation
from PHP 8.5.0's `ext/standard/basic_functions.c`, copyright (c) The PHP Group.
Its authors are Andi Gutmans and Zeev Suraski.

Forwarding callback operations in `src/runtime/callbacks.rs` adapt PHP 8.5.0's
`ext/standard/basic_functions.c`, copyright (c) The PHP Group, by Andi Gutmans
and Zeev Suraski.

`src/runtime/dumping.rs` adapts dump traversal and diagnostic ordering from
PHP 8.5.0's `ext/standard/var.c`, copyright (c) The PHP Group, by Jani Lehtimäki,
Thies C. Arntzen, and Sascha Schumann.

`src/runtime/assertions.rs` adapts assertion checks, options, callbacks, and
bailout behavior from PHP 8.5.0's `ext/standard/assert.c`, copyright (c) The PHP
Group, by Thies C. Arntzen.

`src/runtime/random.rs` adapts random-function validation and unbiased integer
range sampling from PHP 8.5.0's `ext/random/random.c` and `ext/random/csprng.c`,
copyright (c) The PHP Group, by Tim Düsterhus and Go Kudo.

`src/runtime/mersenne_twister.rs` adapts MT19937 generation, range selection,
legacy scaling, seeding, and combined LCG behavior from PHP 8.5.0's
`ext/random/engine_mt19937.c` and `ext/random/random.c`, and array key selection
from `ext/standard/array.c`, copyright (c) The PHP Group. MT19937 implementation
authors include Rasmus Lerdorf, Zeev Suraski, Pedro Melo, Sterling Hughes, and
Go Kudo. The underlying Mersenne Twister license is reproduced below.

Array and binary-string shuffling in `src/array.rs` and
`src/runtime/mersenne_twister.rs` adapts PHP 8.5.0
`ext/standard/array.c` and `ext/standard/string.c`, copyright (c) The PHP Group.

Splice deletion, destructor timing, and mutation checks in `src/array.rs` and
`src/runtime/array_splice.rs` adapt PHP 8.5.0's `php_splice` in
`ext/standard/array.c`, copyright (c) The PHP Group.

Counting in `src/runtime/counting.rs` adapts PHP 8.5.0's `ext/standard/array.c`
and `ext/standard/type.c`, copyright (c) The PHP Group.

Iterable inspection in `src/runtime/builtins.rs` and `src/runtime/reflection.rs`
adapts PHP 8.5.0's `ext/standard/type.c` and `ext/reflection/php_reflection.c`,
copyright (c) The PHP Group.

Iterator helpers in `src/runtime/iterator_helpers.rs` and SPL interface
declarations in `src/runtime/throwables.rs` adapt PHP 8.5.0's
`ext/spl/spl_iterators.c` and `ext/spl/spl_iterators.stub.php`, copyright (c)
The PHP Group.

ArrayIterator and ArrayObject storage, cursor, sorting, and property routing in `src/runtime/array_iterator.rs`,
with integration in object lifetime, cloning, offsets, iteration, and property
export, adapts PHP 8.5.0's `ext/spl/spl_array.c` and
`ext/spl/spl_array.stub.php`, copyright (c) The PHP Group.

Native array-object comparison in `src/runtime/comparison.rs` also adapts
`ext/spl/spl_array.c`. Iterator wrapper handle lifetime in `src/runtime/iterators.rs`
and `src/runtime/iterator_helpers.rs` follows `Zend/zend_iterators.c`, copyright
(c) The PHP Group.

RecursiveArrayIterator child construction and storage links in
`src/runtime/recursive_array_iterator.rs` adapt `ext/spl/spl_array.c`, copyright
(c) The PHP Group. Shared child-table mutation in `src/runtime/array_iterator.rs`
and `src/runtime/reference_paths.rs` follows the same source. Checked shared
storage and callback-safe reads in `src/array.rs` support those operations and
PHP's native splice mutation rules without unchecked shared-Rc writes.

RecursiveIteratorIterator traversal, native frames, hooks, and lifecycle behavior,
and RecursiveTreeIterator construction and prefix/postfix rendering
in `src/runtime/recursive_iterator.rs` adapt PHP 8.5.0's
`ext/spl/spl_iterators.c` and `ext/spl/spl_iterators.stub.php`, copyright (c) The
PHP Group. Related iterator dispatch, method forwarding, cloning restrictions,
and garbage-collection edges follow those native handlers. Missing native method
fatal diagnostics follow `Zend/zend_interfaces.c` and the existing PHP diagnostic
adaptation.

IteratorIterator, filter/recursive/callback wrappers, ParentIterator, LimitIterator,
NoRewindIterator, InfiniteIterator, EmptyIterator, CachingIterator,
RecursiveCachingIterator, and AppendIterator in
`src/runtime/dual_iterator.rs` adapt PHP 8.5.0's `ext/spl/spl_iterators.c` and
`.stub.php`, copyright (c) The PHP Group. Cached user-iterator values and callable
handling also follow `Zend/zend_interfaces.c` and `Zend/zend_API.c`; integration
in method dispatch, argument binding, lifetime handling, and cycle collection
follows the corresponding native handlers. These sources include work by Marcus
Boerger and Etienne Kneuss.

SplFixedArray storage, resizing, offsets, property export, serialization hooks,
and InternalIterator handling in `src/runtime/fixed_array.rs` adapt PHP 8.5.0's
`ext/spl/spl_fixedarray.c`, `ext/spl/spl_fixedarray.stub.php`, and
`Zend/zend_interfaces.c`/`.stub.php`, copyright (c) The PHP Group. The
JsonSerializable declaration follows `ext/json/json.stub.php`; native member
loading follows `Zend/zend_API.c`. Property inspection and
`get_mangled_object_vars` follow `Zend/zend_builtin_functions.c`.

SplHeap, SplMinHeap, SplMaxHeap, and SplPriorityQueue in `src/runtime/heap.rs`
adapt PHP 8.5.0's `ext/spl/spl_heap.c` and `ext/spl/spl_heap.stub.php`, copyright
(c) The PHP Group, including work by Etienne Kneuss. Sift operations, comparison
callbacks, corruption and locking rules, native iteration, cloning, debug output,
serialization, and cycle-collection integration follow those native handlers.

SplFileInfo path handling, metadata, class factories, cloning, debug output,
and serialization restrictions in `src/runtime/file_info.rs` and its integration
adapt PHP 8.5.0's `ext/spl/spl_directory.c` and `.stub.php`, copyright (c) The PHP
Group, including work by Marcus Boerger. Stat/lstat caching, access checks, and
`clearstatcache` follow `ext/standard/filestat.c`; invalidation during file I/O
follows `main/streams/plain_wrapper.c`. Path components reuse the string and
Zend dirname adaptations described above.

SplFileObject and SplTempFileObject in `src/runtime/file_object.rs`, their
FileInfo factories and shutdown handling adapt the same `ext/spl/spl_directory.c`
and `.stub.php` sources. Locking follows `ext/standard/file.c`; typed reference
assignment reuses the Zend API adaptation described above. Memory EOF propagation
and temporary stream spill thresholds in `src/resource.rs` and
`src/runtime/streams.rs` follow `main/streams/memory.c` and
`ext/standard/php_fopen_wrapper.c`, copyright (c) The PHP Group. The PHP license
below applies to these stream and SPL adaptations.

DirectoryIterator, FilesystemIterator, and RecursiveDirectoryIterator in
`src/runtime/directory_iterator.rs`, including native cursor wrappers, independent
directory clones, cached current values, and recursive child construction, adapt
the same `ext/spl/spl_directory.c` and `.stub.php` sources. RecursiveIteratorIterator
source-object dispatch in `src/runtime/recursive_iterator.rs` follows
`ext/spl/spl_iterators.c`; native directory handles use the existing platform adapter.

GlobIterator and its lifecycle checks in the directory, cloning, reflection, and
method-resolution code follow the same `ext/spl/spl_directory.c` sources.
`src/native_glob.rs` and shared directory stream integration adapt
`main/streams/glob_wrapper.c`, copyright (c) The PHP Group. The pattern compiler,
matcher, and path expansion in `src/native_glob.rs` adapt `main/php_glob.c`,
derived from Berkeley glob contributed by Guido van Rossum. The Berkeley glob
license is retained below.

SplDoublyLinkedList, SplQueue, and SplStack storage, cursor behavior, cloning,
offsets, debug output, serialization hooks, and legacy Serializable methods
in `src/runtime/linked_list.rs`
adapt PHP 8.5.0's `ext/spl/spl_dllist.c`, `.h`, and `.stub.php`, copyright (c)
The PHP Group. Shared iterator wrappers in `src/runtime/native_iterator.rs`
follow `Zend/zend_interfaces.c` and the SPL native handlers. The shared member
loader in `src/runtime/objects.rs` follows `Zend/zend_API.c`.

SplObjectStorage storage, object hashing, iteration, set operations, cloning,
comparison, and serialization, plus MultipleIterator attachment, combined iteration,
and native dimension handling in `src/runtime/object_storage.rs` adapt PHP 8.5.0's
`ext/spl/spl_observer.c` and `.stub.php`, copyright (c) The PHP Group, including
work by Marcus Boerger and Etienne Kneuss. SplObserver/SplSubject declarations follow the same
stubs. Native dimension access, object lifetime, and collection integration in
the runtime follow the corresponding SPL handlers.

Serialization encoding, reference tables, and magic hooks in
`src/runtime/serialization.rs` adapt PHP 8.5.0's `ext/standard/var.c`, copyright
(c) The PHP Group, by Jani Lehtimäki, Thies C. Arntzen, and Sascha Schumann.
ArrayObject/ArrayIterator serialization hooks and legacy Serializable methods
in `src/runtime/array_iterator.rs` follow `ext/spl/spl_array.c` and `.stub.php`, copyright (c) The PHP Group.

Serialized input parsing, reference tables, class filtering, depth limits, typed
property restoration, and deferred hooks in `src/runtime/unserialization.rs`
adapt PHP 8.5.0's `ext/standard/var.c` and `ext/standard/var_unserializer.re`,
copyright (c) The PHP Group. Incomplete-class declarations, access handlers, and
serialization in the runtime follow `ext/standard/incomplete_class.c` and
`ext/standard/basic_functions.stub.php`. These sources include work by Sascha
Schumann, Jani Lehtimäki, and Thies C. Arntzen. Typed reference-source checks reuse
the Zend property implementation described above.

This product includes PHP software, freely available from
<http://www.php.net/software/>. The PHP license for these files follows:

--------------------------------------------------------------------
                  The PHP License, version 3.01
Copyright (c) 1999 - 2024 The PHP Group. All rights reserved.
--------------------------------------------------------------------

Redistribution and use in source and binary forms, with or without
modification, is permitted provided that the following conditions
are met:

  1. Redistributions of source code must retain the above copyright
     notice, this list of conditions and the following disclaimer.

  2. Redistributions in binary form must reproduce the above copyright
     notice, this list of conditions and the following disclaimer in
     the documentation and/or other materials provided with the
     distribution.

  3. The name "PHP" must not be used to endorse or promote products
     derived from this software without prior written permission. For
     written permission, please contact group@php.net.

  4. Products derived from this software may not be called "PHP", nor
     may "PHP" appear in their name, without prior written permission
     from group@php.net.  You may indicate that your software works in
     conjunction with PHP by saying "Foo for PHP" instead of calling
     it "PHP Foo" or "phpfoo"

  5. The PHP Group may publish revised and/or new versions of the
     license from time to time. Each version will be given a
     distinguishing version number.
     Once covered code has been published under a particular version
     of the license, you may always continue to use it under the terms
     of that version. You may also choose to use such covered code
     under the terms of any subsequent version of the license
     published by the PHP Group. No one other than the PHP Group has
     the right to modify the terms applicable to covered code created
     under this License.

  6. Redistributions of any form whatsoever must retain the following
     acknowledgment:
     "This product includes PHP software, freely available from
     <http://www.php.net/software/>".

THIS SOFTWARE IS PROVIDED BY THE PHP DEVELOPMENT TEAM ``AS IS'' AND
ANY EXPRESSED OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO,
THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A
PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE PHP
DEVELOPMENT TEAM OR ITS CONTRIBUTORS BE LIABLE FOR ANY DIRECT,
INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED
OF THE POSSIBILITY OF SUCH DAMAGE.

--------------------------------------------------------------------

This software consists of voluntary contributions made by many
individuals on behalf of the PHP Group.

The PHP Group can be contacted via Email at group@php.net.

For more information on the PHP Group and the PHP project,
please see <http://www.php.net>.

PHP includes the Zend Engine, freely available at
<http://www.zend.com>.

Mersenne Twister license

Copyright (C) 1997 - 2002, Makoto Matsumoto and Takuji Nishimura,
Copyright (C) 2000 - 2003, Richard J. Wagner
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:

1. Redistributions of source code must retain the above copyright
   notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright
   notice, this list of conditions and the following disclaimer in the
   documentation and/or other materials provided with the distribution.

3. The names of its contributors may not be used to endorse or promote
   products derived from this software without specific prior written
   permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
"AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
A PARTICULAR PURPOSE ARE DISCLAIMED.  IN NO EVENT SHALL THE COPYRIGHT OWNER OR
CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL,
EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO,
PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR
PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF
LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING
NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

Berkeley glob license

Copyright (c) 1989, 1993
	The Regents of the University of California.  All rights reserved.

This code is derived from software contributed to Berkeley by
Guido van Rossum.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:
1. Redistributions of source code must retain the above copyright
   notice, this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright
   notice, this list of conditions and the following disclaimer in the
   documentation and/or other materials provided with the distribution.
3. Neither the name of the University nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE REGENTS AND CONTRIBUTORS ``AS IS'' AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED.  IN NO EVENT SHALL THE REGENTS OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
SUCH DAMAGE.

`src/runtime/scanf.rs` is an altered Rust adaptation of PHP 8.5.0's
`ext/standard/scanf.c`, `string.c`, and `file.c`, copyright (c) The PHP Group,
under the PHP license above. The scanner lists Clayton Collie as its author
and derives from Tcl 8.3.0. Its additional notice follows verbatim:

This software is copyrighted by the Regents of the University of
California, Sun Microsystems, Inc., Scriptics Corporation,
and other parties.  The following terms apply to all files associated
with the software unless explicitly disclaimed in individual files.

The authors hereby grant permission to use, copy, modify, distribute,
and license this software and its documentation for any purpose, provided
that existing copyright notices are retained in all copies and that this
notice is included verbatim in any distributions. No written agreement,
license, or royalty fee is required for any of the authorized uses.
Modifications to this software may be copyrighted by their authors
and need not follow the licensing terms described here, provided that
the new terms are clearly indicated on the first page of each file where
they apply.

IN NO EVENT SHALL THE AUTHORS OR DISTRIBUTORS BE LIABLE TO ANY PARTY
FOR DIRECT, INDIRECT, SPECIAL, INCIDENTAL, OR CONSEQUENTIAL DAMAGES
ARISING OUT OF THE USE OF THIS SOFTWARE, ITS DOCUMENTATION, OR ANY
DERIVATIVES THEREOF, EVEN IF THE AUTHORS HAVE BEEN ADVISED OF THE
POSSIBILITY OF SUCH DAMAGE.

THE AUTHORS AND DISTRIBUTORS SPECIFICALLY DISCLAIM ANY WARRANTIES,
INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE, AND NON-INFRINGEMENT.  THIS SOFTWARE
IS PROVIDED ON AN "AS IS" BASIS, AND THE AUTHORS AND DISTRIBUTORS HAVE
NO OBLIGATION TO PROVIDE MAINTENANCE, SUPPORT, UPDATES, ENHANCEMENTS, OR
MODIFICATIONS.

GOVERNMENT USE: If you are acquiring this software on behalf of the
U.S. government, the Government shall have only "Restricted Rights"
in the software and related documentation as defined in the Federal
Acquisition Regulations (FARs) in Clause 52.227.19 (c) (2).  If you
are acquiring the software on behalf of the Department of Defense, the
software shall be classified as "Commercial Computer Software" and the
Government shall have only "Restricted Rights" as defined in Clause
252.227-7013 (c) (1) of DFARs.  Notwithstanding the foregoing, the
authors grant the U.S. Government and others acting in its behalf
permission to use and distribute the software in accordance with the
terms specified in this license.

`src/runtime/include_paths.rs` and its loading, autoloading, and stream integration
adapt PHP 8.5.0's `main/fopen_wrappers.c`, `main/streams/streams.c`, and
`ext/standard/basic_functions.c` and `streamsfuncs.c`, copyright (c) The PHP Group.
Stream passthrough in `src/runtime/file_writing.rs` follows `ext/standard/file.c`
and `main/streams/streams.c`, reusing the existing file-transfer adaptation.

`src/runtime/file_operations.rs` adapts copy and rename from PHP 8.5.0's
`ext/standard/file.c` and `main/streams/{streams,plain_wrapper}.c`, copyright (c)
The PHP Group. Its stream-copy integration in `src/runtime/file_writing.rs`,
file-URL normalization and private file creation in `src/runtime/streams.rs`, and
scoped pending-exception warning suppression preserve those native operations.

Character reads and stream truncation in `src/runtime/streams.rs` and
`src/resource.rs` adapt PHP 8.5.0's `ext/standard/file.c` and
`main/streams/{streams,memory,plain_wrapper}.c`, copyright (c) The PHP Group.

`stream_copy_to_stream` and bounded transfers in `src/runtime/file_writing.rs`
adapt PHP 8.5.0's `ext/standard/streamsfuncs.c` and `main/streams/streams.c`,
copyright (c) The PHP Group. Mapped transfer ordering and ordinary-read stat-cache
and error EOF handling follow `main/streams/{streams,plain_wrapper}.c`.

`src/runtime/stream_records.rs`, read-buffer storage in `src/resource.rs`, and
buffer integration in stream reads, writes, seeks, metadata and transfers adapt
PHP 8.5.0's `ext/standard/streamsfuncs.c` and `main/streams/streams.c`, copyright
(c) The PHP Group.

`src/runtime/file_links.rs` adapts link operations and canonical path resolution
from PHP 8.5.0's `ext/standard/{link,file}.c`, `main/fopen_wrappers.c`, and
`Zend/zend_virtual_cwd.c`, copyright (c) The PHP Group.

`src/digest.rs` and `src/runtime/hashes.rs` adapt PHP 8.5.0's
`ext/standard/{md5,sha1}.c`, copyright (c) The PHP Group. The MD5 compression
core follows the OpenSSL-compatible implementation written by Solar Designer
in 2001 and placed in the public domain, without warranty. SHA-1 follows PHP's
implementation by Stefan Esser. Shared wrapper-opening and read helpers preserve
native file hashing diagnostics, EOF handling and stream lifetime.

`src/runtime/clocks.rs` adapts PHP 8.5.0's `ext/standard/{microtime,hrtime}.c`
and `ext/date/php_date.c`, copyright (c) The PHP Group, using Rust's system
and monotonic clocks.

Stream locality and lock-capability queries in `src/runtime/file_operations.rs`,
`src/runtime/streams.rs`, and `src/resource.rs` adapt PHP 8.5.0's
`ext/standard/streamsfuncs.c` and `main/streams/{streams,plain_wrapper,memory}.c`,
copyright (c) The PHP Group.

`src/runtime/string_distance.rs` adapts PHP 8.5.0's
`ext/standard/levenshtein.c` (original author Hartmut Holzgraefe) and the
`similar_text` implementation in `ext/standard/string.c`, copyright (c) The PHP
Group. The latter uses an explicit work stack instead of native recursion.

`src/runtime/packing.rs` adapts PHP 8.5.0's `ext/standard/pack.c`, copyright (c)
The PHP Group, originally authored by Chris Schneider. Its trace integration
preserves native variadic-argument replacement during string conversion.

`src/runtime/phonetics.rs` adapts PHP 8.5.0's `ext/standard/soundex.c` and
`metaphone.c`, copyright (c) The PHP Group. The original authors include
Bjørn Borud and Thies C. Arntzen; the Metaphone implementation derives from
Michael G Schwern's CPAN Text-Metaphone-1.96 code.

`src/runtime/string_step.rs` adapts the `str_increment` and `str_decrement`
functions in PHP 8.5.0's `ext/standard/string.c`, copyright (c) The PHP Group.

`src/runtime/quoted_printable.rs` adapts PHP 8.5.0's `ext/standard/quot_print.c`,
copyright (c) The PHP Group, originally authored by Kirill Maximov.

`src/runtime/utf8.rs` adapts the Latin-1/UTF-8 conversion routines from PHP
8.5.0's `ext/standard/string.c` and the UTF-8 character reader from
`ext/standard/html.c`, copyright (c) The PHP Group.

`src/runtime/urls.rs` adapts PHP 8.5.0's `ext/standard/url.c`, copyright (c)
The PHP Group, originally authored by Jim Winstead.

`src/runtime/html/` adapts PHP 8.5.0's `ext/standard/html.c`, `html.h` and
`html_tables.h`, copyright (c) The PHP Group. Original authors include Rasmus
Lerdorf, Jaakko Hyvätti, Wez Furlong and Gustavo Lopes. The Rust mapping data is
generated from PHP's tables using `tools/generate_html_tables.py`; the generator
uses a temporary C extractor, while the runtime itself uses only Rust.

CRC32 in `src/digest.rs` adapts PHP 8.5.0's `ext/standard/crc32.c` and
`crc32.h`, copyright (c) The PHP Group. Its lookup table is generated at compile
time from the reflected AUTODIN II polynomial.

`src/runtime/uuencoding.rs` adapts PHP 8.5.0's `ext/standard/uuencode.c`,
copyright (c) The PHP Group, originally authored by Ilia Alshanetsky. Portions
are based on Berkeley's uuencode/uudecode implementation under the license below.
This product includes software developed by the University of California,
Berkeley and its contributors.

Berkeley uuencode/uudecode license
---------------------------------

Copyright (c) 1983, 1993
 The Regents of the University of California.  All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions
are met:
1. Redistributions of source code must retain the above copyright
   notice, this list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright
   notice, this list of conditions and the following disclaimer in the
   documentation and/or other materials provided with the distribution.
3. All advertising materials mentioning features or use of this software
   must display the following acknowledgement:
 This product includes software developed by the University of
 California, Berkeley and its contributors.
4. Neither the name of the University nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE REGENTS AND CONTRIBUTORS ``AS IS'' AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
ARE DISCLAIMED.  IN NO EVENT SHALL THE REGENTS OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS
OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT
LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY
OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF
SUCH DAMAGE.
