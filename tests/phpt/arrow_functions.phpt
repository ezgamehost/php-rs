--TEST--
Arrow functions automatically capture free variables and compose in pipe chains
--FILE--
<?php
$x = 3;
$add = fn(int $n): int => $n + $x;
$nested = fn() => fn($n) => $n + $x;
$x = 9;
var_dump($add(2), $nested()(4));
var_dump(2 |> $add |> (fn($n) => $n * 2));
$shadow = static fn($x) => $x + 1;
var_dump($shadow(6));
$live = fn() => $GLOBALS['x'];
$x = 10;
var_dump($live());
?>
--EXPECT--
int(5)
int(7)
int(10)
int(7)
int(10)
