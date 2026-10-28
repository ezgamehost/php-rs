--TEST--
Throwable traces preserve live arguments, call identity, and creation snapshots
--FILE--
<?php
function make($x) {
    $x = 7;
    $e = new Exception('x');
    $x = 9;
    return $e;
}
$e = make(1);
$t = $e->getTrace();
var_dump($t[0]['file'] === __FILE__, $t[0]['line'], $t[0]['function'], $t[0]['args']);
$t[0]['args'][0] = 99;
var_dump($e->getTrace()[0]['args'][0]);
class A { function make($x) { return new Error; } }
class B extends A {}
$e = array_map([new B, 'make'], [3])[0];
$t = $e->getTrace();
var_dump(isset($t[0]['file']), $t[0]['class'], $t[0]['type'], $t[0]['function'], $t[0]['args']);
var_dump($t[1]['function']);
echo (new Exception)->getTraceAsString(), "\n";
?>
--EXPECT--
bool(true)
int(8)
string(4) "make"
array(1) {
  [0]=>
  int(7)
}
int(7)
bool(false)
string(1) "A"
string(2) "->"
string(4) "make"
array(1) {
  [0]=>
  int(3)
}
string(9) "array_map"
#0 {main}
