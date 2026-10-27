--TEST--
Global constants, runtime definitions, qualified lookup and array isolation
--FILE--
<?php
const A=2,B=A+3,C=[A,B],D=(int)'4.5';
echo A,B,C[1],D,PHP_EOL;
function read_start($x=A) { define('END',$x+1); return END; }
echo read_start(),END,PHP_EOL;
define(value:[1],constant_name:'VALUES');
$copy=VALUES;$copy[0]=9;
echo VALUES[0],$copy[0],PHP_EOL;
define('Ns\Value',7);
echo \Ns\Value,constant('\ns\Value'),defined('NS\value')?1:0,PHP_EOL;
echo defined('TRUE')?1:0,constant('null')===null?1:0,PHP_EOL;
echo PHP_INT_SIZE,':',PHP_INT_MAX,PHP_EOL;
--EXPECT--
2554
33
19
770
11
8:9223372036854775807
