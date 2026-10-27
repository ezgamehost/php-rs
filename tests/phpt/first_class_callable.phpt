--TEST--
First-class function callables retain identity and normal argument semantics
--FILE--
<?php
function add($x, $y = 2, ...$extra) { return $x + $y + count($extra); }
$a = add(...);
$b = add(...);
var_dump($a === $b, $a == $b, $a === $a(...));
var_dump($a(y: 4, x: 3), $a(...[2, 3, 4]));
function keep(callable $f): Closure { return $f; }
echo gettype($a), '/', get_debug_type($a), "\n";
echo keep(strlen(...))('abcd'), "\n";
const LENGTH = strlen(...);
echo (LENGTH)('abc'), "\n";
?>
--EXPECT--
bool(false)
bool(true)
bool(true)
int(7)
int(6)
object/Closure
4
3
