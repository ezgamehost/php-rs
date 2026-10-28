#!/usr/bin/env python3
"""Compare nullsafe chain evaluation and side effects with PHP 8.5."""
import argparse
import ast
import itertools
import re
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--reference', required=True)
parser.add_argument('--runtime', default='target/debug/php-rs')
options = parser.parse_args()
root = Path(__file__).resolve().parents[1]
cases = []
for match in re.finditer(r'output\(\s*("(?:[^"\\]|\\.)*")\s*\)', (root / 'tests/nullsafe.rs').read_text()):
    cases.append(ast.literal_eval(match[1]))
setup = 'class A{public $p=7;public $n=null;public $a=[8];public static $s=9;const C=10;function f($x=null){echo "f;";return $this;}function none(){echo "none;";return null;}static function g(){echo "g;";return 3;}}function side(){echo "side;";return "p";}$a=new A;$n=null;'
expressions = ['?->p', '?->missing', '?->f(side())->p', '?->none()->p', '?->none()?->p', '?->p[side()]', '?->a[0]', '?->f()::$s', '?->f()::g()', '?->{side()}', '?->f(...[])', '?->f(x:side())->p', '?->f()->missing[side()]', '?->missing?->p', '?->f()::C', '?->p::class', '?->f()()', '?->f($n?->f())->p']
for receiver, suffix, context in itertools.product(['$a','$n','$missing','false','[]'], expressions, ['var_dump({});', 'var_dump({}??"fallback");', 'var_dump(empty({}));']):
    code = receiver + suffix
    cases.append(setup+'try{'+context.format(code)+'}catch(Throwable $e){echo get_class($e);}')
for expression in ['$missing?->f()', '($missing?->f())', '$missing?->f()->p', '($missing?->f())->p', '$missing?->p', '($missing?->p)', '$missing?->a[side()]', '(null?->p)()', '(null?->p)->f()', '((null?->p)->x)[side()]']:
    for context in ['var_dump({});','var_dump({}??7);','var_dump(empty({}));']:
        cases.append(setup+'try{'+context.format(expression)+'}catch(Throwable $e){echo get_class($e);}')
for receiver, suffix in itertools.product(['$a','$n','$missing'],['?->p','?->f()->p','?->f()[0]','?->a[side()]','?->f()::$s','?->{side()}']):
    cases.append(setup+'try{var_dump(isset('+receiver+suffix+'));}catch(Throwable $e){echo get_class($e);}')
for code in ['$n?->p=1;','$n?->p[0]=1;','$n?->f()->p=1;','$n?->f()::$s=1;','$n?->p??=1;','$n?->p++;','--$n?->p;','unset($n?->p);','foreach([1] as $n?->p){}','[$n?->p]=[1];','$n?->f(...);','$n?->f()::g(...);','($n?->p)->f(...);']:
    cases.append('echo "before";'+code)


for receiver, name in itertools.product(['$a','$n'], ['null','true','0','[]','new A','"f"','$n?->p']):
    for form in [receiver+'?->{'+name+'}()', receiver+'?->f()::{'+name+'}()']:
        cases.append(setup+'try{'+form+';}catch(Throwable $e){echo $e->getMessage();}')
for expression in ['$missing?->f()::g()->p', '$missing?->f()::$s', '$missing?->f()::g(...)', '($missing?->f())::$s', '$a?->f()::g(...)']:
    for context in ['var_dump(empty({}));','var_dump({}??7);']:
        cases.append(setup+'try{'+context.format(expression)+'}catch(Throwable $e){echo get_class($e);}')
for receiver, property, context in itertools.product(['$a','$n'], ['p','n'], ['var_dump({});','var_dump({}??7);','var_dump(isset({}));','var_dump(empty({}));']):
    cases.append('class A{public int $p;public ?A $n=null;}$a=new A;$n=null;try{'+context.format(receiver+'?->'+property)+'}catch(Throwable $e){echo get_class($e);}')
for expression in ['new $a?->p','new $n?->p','$a instanceof $a?->p','$a instanceof $n?->p']:
    cases.append('class A{public $p="A";}$a=new A;$n=null;try{var_dump('+expression+');}catch(Throwable $e){echo get_class($e);}')

failed = 0
for code in cases:
    reference = subprocess.run([options.reference,'-n','-d','display_errors=stderr','-r',code], capture_output=True, timeout=5)
    runtime = subprocess.run([options.runtime,'-r',code], capture_output=True, timeout=5)
    if reference.returncode < 0 or runtime.returncode < 0 or (reference.stdout,reference.returncode,bool(reference.stderr)) != (runtime.stdout,runtime.returncode,bool(runtime.stderr)):
        failed += 1
        print('DIFF',code,'\nPHP',reference.returncode,reference.stdout,reference.stderr[:250],'\nRUST',runtime.returncode,runtime.stdout,runtime.stderr[:250],flush=True)
print('Cases:',len(cases),'Mismatches:',failed)
raise SystemExit(bool(failed))
