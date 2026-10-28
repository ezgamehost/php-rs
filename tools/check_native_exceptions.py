#!/usr/bin/env python3
"""Compare native exception constructors and inheritance with PHP 8.5."""
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
for match in re.finditer(r'output\(\s*("(?:[^"\\]|\\.)*")\s*\)', (root / 'tests/native_exceptions.rs').read_text()):
    cases.append(ast.literal_eval(match[1]))

setup = 'class S{function __toString(){echo "convert;";return "12";}}'
inspect = '$p=$e->getPrevious();var_dump($e->getMessage(),$e->getCode(),$e->getFile(),$e->getLine(),$p===null?null:$p->getMessage());if($e instanceof ErrorException)var_dump($e->getSeverity());'
classes = ['Exception', 'Error', 'ErrorException', 'LogicException', 'BadFunctionCallException', 'BadMethodCallException', 'DomainException', 'InvalidArgumentException', 'LengthException', 'OutOfRangeException', 'RuntimeException', 'OutOfBoundsException', 'OverflowException', 'RangeException', 'UnderflowException', 'UnexpectedValueException']
for cls in classes:
    for args in ['', '"hello"', 'message:"hello",code:9', 'previous:new Error("old")', 'null,null', 'new S,3', 'message:[]', 'code:[]', 'previous:3', 'unexpected:1']:
        cases.append(setup + 'try{$e=new ' + cls + '(' + args + ');' + inspect + '}catch(Throwable $e){echo get_class($e),":",$e->getMessage();}')
    for args in ['', '""', 'code:0', 'previous:null', '"changed",-2', 'previous:new Exception("new")']:
        initial = '"old",7,3,"saved",-8,new Exception("prior")' if cls == 'ErrorException' else '"old",7,new Exception("prior")'
        cases.append('$e=new ' + cls + '(' + initial + ');$e->__construct(' + args + ');' + inspect)
    cases.append('$e=new '+cls+';var_dump($e instanceof Exception,$e instanceof Error,$e instanceof Throwable,$e instanceof Stringable,get_parent_class($e));try{clone $e;}catch(Error){echo "uncloneable";}')
for strict, parameter, value in itertools.product([False, True], ['message', 'code', 'severity', 'filename', 'line', 'previous'], ['null', 'true', 'false', '0', '-9', '1.5', '"12"', '"bad"', '[]', 'new S', 'new Exception("prior")']):
    cases.append(('declare(strict_types=1);' if strict else '') + setup + 'try{$e=new ErrorException(' + parameter + ':' + value + ');' + inspect + '}catch(Throwable $e){echo get_class($e),":",$e->getMessage();}')
for filename, line in itertools.product(['null', '""', '"file"'], ['null', '0', '-1', '9223372036854775807', '-9223372036854775807-1']):
    cases.append('$e=new ErrorException("old",7,8,"initial",-3);$e->__construct(filename:'+filename+',line:'+line+');'+inspect)
for args in ['1,2,3,4,5,6,7', 'lineno:1', 'severity:[],filename:[]', 'filename:new S,line:[]', 'message:new S,code:[]']:
    cases.append(setup+'try{new ErrorException('+args+');}catch(Throwable $e){echo get_class($e),":",$e->getMessage();}')

failed = 0
for code in cases:
    reference = subprocess.run([options.reference, '-n', '-d', 'display_errors=stderr', '-r', code], capture_output=True, timeout=5)
    runtime = subprocess.run([options.runtime, '-r', code], capture_output=True, timeout=5)
    if reference.returncode < 0 or runtime.returncode < 0 or (reference.stdout, reference.returncode, bool(reference.stderr)) != (runtime.stdout, runtime.returncode, bool(runtime.stderr)):
        failed += 1
        print('DIFF', code, '\nPHP', reference.returncode, reference.stdout, reference.stderr[:300], '\nRUST', runtime.returncode, runtime.stdout, runtime.stderr[:300], flush=True)
print('Cases:', len(cases), 'Mismatches:', failed)
raise SystemExit(bool(failed))
