--TEST--
Core type, array, string and function inspection builtins
--FILE--
<?php
$a = ['x'=>1, 7=>'1', 'n'=>null];
var_dump(array_keys($a,1,true),array_values($a));
echo count([1,[2,[3]],[]], COUNT_RECURSIVE), ':', strlen("a\0b"), "\n";
echo gettype(1.5), ':', get_debug_type(1.5), ':', is_float(1.5), "\n";
echo function_exists('STRLEN'), ':', array_key_exists('n',$a), ':', isset($a['n'])?1:0;
--EXPECT--
array(1) {
  [0]=>
  string(1) "x"
}
array(3) {
  [0]=>
  int(1)
  [1]=>
  string(1) "1"
  [2]=>
  NULL
}
6:3
double:float:1
1:1:0
