--TEST--
Typed parameters and returns, union coercion, nullable and literal types
--FILE--
<?php
function number(int|float $value): int|float { return $value; }
function collect(int ...$values): array { return $values; }
function optional(?int $value = null): ?int { return $value; }
function flag(false|null $value): false|null { return $value; }
function invoke(callable $fn, string $value): int { return $fn($value); }
function identity(object|iterable $value): iterable { return $value; }
function as_int($value): int { return $value; }
var_dump(number('42'), number('42.0'), number('2e1'));
var_dump(collect('1', second: '2'));
var_dump(optional(), optional('3'), flag(false), flag(null));
var_dump(invoke('strlen', 1234), identity([5]), as_int('6'));
?>
--EXPECT--
int(42)
float(42)
float(20)
array(2) {
  [0]=>
  int(1)
  ["second"]=>
  int(2)
}
NULL
int(3)
bool(false)
NULL
int(4)
array(1) {
  [0]=>
  int(5)
}
int(6)
