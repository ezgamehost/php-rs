--TEST--
Array mapping, filtering, and reduction use PHP callbacks and preserve key rules
--FILE--
<?php
$a = ['a' => 1, 'b' => 2, 'c' => 3];
$mapped = array_map(fn($x) => $x * 2, $a);
$filtered = array_filter($mapped, fn($v, $k) => $v > 2 && $k !== 'c', ARRAY_FILTER_USE_BOTH);
var_dump($filtered);
var_dump(array_reduce($mapped, fn($sum, $v) => $sum + $v, 10));
var_dump(array_map(null, ['x' => 1, 'y' => 2], [3]));
var_dump(array_filter([null, false, 0, '0', '', [], 'yes']));
?>
--EXPECT--
array(1) {
  ["b"]=>
  int(4)
}
int(22)
array(2) {
  [0]=>
  array(2) {
    [0]=>
    int(1)
    [1]=>
    int(3)
  }
  [1]=>
  array(2) {
    [0]=>
    int(2)
    [1]=>
    NULL
  }
}
array(1) {
  [6]=>
  string(3) "yes"
}
