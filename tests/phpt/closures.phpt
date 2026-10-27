--TEST--
Anonymous functions capture values and references with independent static storage
--FILE--
<?php
$x = 2;
$value = function() use ($x) { return ++$x; };
$reference = function() use (&$x) { return ++$x; };
$x = 5;
var_dump($value(), $value(), $reference(), $x);
function counter() {
    return function() { static $n = 0; return ++$n; };
}
$a = counter();
$b = counter();
var_dump($a(), $a(), $b(), $a == $b);
$factorial = function($n) use (&$factorial) {
    return $n < 2 ? 1 : $n * $factorial($n - 1);
};
var_dump($factorial(5));
const ADD = static function($n) { return $n + 1; };
var_dump((ADD)(4));
?>
--EXPECT--
int(3)
int(3)
int(6)
int(6)
int(1)
int(2)
int(1)
bool(false)
int(120)
int(5)
