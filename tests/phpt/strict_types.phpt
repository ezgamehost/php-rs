--TEST--
Strict function types, defaults, recursion, void and mixed returns
--FILE--
<?php
declare(strict_types=(1));
function fact(int $n): int { return $n < 2 ? 1 : $n * fact($n - 1); }
function floating(float $n = 2): float { return $n; }
function anything(mixed $value): mixed { return $value; }
function nothing(): void { return; }
function fallthrough(): void {}
function only_null(null $value): null { return $value; }
function truth(true $value): true { return $value; }
function run_callback(callable $f, int $n): int { return $f($n); }
var_dump(fact(5), floating(), floating(3));
var_dump(anything('abc'), nothing(), fallthrough(), only_null(null), truth(true));
var_dump(run_callback('fact', 4), strlen('abc'));
?>
--EXPECT--
int(120)
float(2)
float(3)
string(3) "abc"
NULL
NULL
NULL
bool(true)
int(24)
int(3)
