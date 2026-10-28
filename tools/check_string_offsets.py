#!/usr/bin/env python3
"""Compare string-offset conversions with a separately installed PHP 8.5 CLI."""
import argparse
import ast
import itertools
import re
import subprocess
from pathlib import Path


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--reference', required=True, help='PHP 8.5 CLI executable')
parser.add_argument('--runtime', default='target/debug/php-rs', help='Rust runtime executable')
options = parser.parse_args()
php = str(Path(options.reference).resolve())
rust = str(Path(options.runtime).resolve())
root = Path(__file__).resolve().parents[1]
cases = []

# Include the Rust regression programs and vary operands, destinations, and
# writes performed during conversion. Establishing global bindings in advance
# also avoids a reference-engine crash in some otherwise equivalent programs.
for m in re.finditer('output\\(\\s*("(?:[^"\\\\]|\\\\.)*")\\s*\\)', (root / 'tests/string_offset_conversion.rs').read_text()):
    cases.append(ast.literal_eval(m[1]))
for (text, index, ret) in itertools.product(["'abc'", "''", "'abcdef'"], ['0', '1', '5', '-1', '-9', 'true', 'null', '"1"', '"1x"', '[]'], ["'Q'", "'YZ'", "''", '1', 'null']):
    cases.append('class A{function __toString(){echo "C";return ' + ret + ';}}$s=' + text + ';try{var_dump($s[' + index + ']=new A);}catch(Throwable $e){echo get_class($e);}var_dump($s);')
for (setup, effect, ret) in itertools.product([('variable', '$s="abc";', '$s[0]', 'global $s;'), ('array', '$a=["abc",1];', '$a[0][0]', 'global $a;'), ('property', 'class B{public string $s="abc";}$b=new B;', '$b->s[0]', 'global $b;'), ('static', 'class B{public static string $s="abc";}', 'B::$s[0]', '')], ['none', 'replace', 'same', 'offset', 'throw', 'empty'], ['"Q"', '"YZ"', '""']):
    (label, init, lhs, glob) = setup
    parent = lhs[:-3]
    code = {'none': '', 'replace': parent + '="changed";', 'same': parent + '="abc";', 'offset': parent + '[1]="Z";', 'throw': parent + '="changed";throw new Exception;', 'empty': parent + '="";'}[effect]
    cases.append(init + 'class A{function __toString(){' + glob + code + 'return ' + ret + ';}}try{var_dump(' + lhs + '=new A);}catch(Throwable $e){echo get_class($e);}var_dump(' + parent + ');')
for copy in ['', '$before=$s;', '$before=[$s];']:
    for body in ['$during=$s;', '$during=[$s];', 'unset($s);', '$s="changed";', 'unset($s);$s="abc";']:
        cases.append('class A{function __toString(){global $s,$during;' + body + 'return "Q";}}$s="abc";$during=null;' + copy + 'try{var_dump($s[0]=new A);}catch(Throwable $e){echo get_class($e);}var_dump($s,$during);')
cases += ['function prepare_globals(){global $s,$a,$b,$during;}prepare_globals();' + c for c in cases]
for (effect, offset) in itertools.product(['$during=$s;$s="changed";', '$during=$s;$s="abc";', '$during=$s;$s[1]="Z";', '$during=$s;$s="";', '$during=$s;unset($GLOBALS["s"]);'], ['0', '1', '-1', '5']):
    cases.append('function prepare(){global $s;}prepare();class A{function __toString(){global $s,$during;' + effect + 'return "Q";}}$s="abc";$during=null;var_dump($s[' + offset + ']=new A,$s,$during);')
for effect in ['$copy=$a;$a[1]="changed";', '$copy=$a;$a[0]="changed";', '$copy=$a;$a=["changed"];', '$copy=$a[0];$a[0]="changed";', '$copy=$a[0];$a=["changed"];']:
    cases.append('class A{function __toString(){global $a,$copy;' + effect + 'return "Q";}}$a=["abc",1];$copy=null;var_dump($a[0][0]=new A,$a,$copy);')

failed = 0
crashes = 0
for code in cases:
    a = subprocess.run([php, '-n', '-d', 'display_errors=stderr', '-r', code], capture_output=True, timeout=4)
    b = subprocess.run([rust, '-r', code], capture_output=True, timeout=4)
    if a.returncode < 0:
        crashes += 1
        print('REFERENCE CRASH', a.returncode, code, flush=True)
        if b.returncode < 0:
            failed += 1
            print('RUNTIME CRASH', b.returncode, code, flush=True)
        continue
    if (a.stdout, a.returncode, bool(a.stderr)) != (b.stdout, b.returncode, bool(b.stderr)):
        failed += 1
        print('DIFF', code, '\nPHP', a.returncode, a.stdout, a.stderr[:180], '\nRUST', b.returncode, b.stdout, b.stderr[:180], flush=True)
print('Cases:', len(cases), 'Mismatches:', failed, 'Reference crashes:', crashes)
raise SystemExit(bool(failed))
