--TEST--
Typed properties preserve uninitialized state, assignment types and inherited storage
--FILE--
<?php
class Base {
    public int $count;
    public ?string $name;
    public array $items;
    public static int $shared;
    public float $scale = 2;
}
class Child extends Base { public int $count = 3; }
$a = new Base;
var_dump($a);
var_dump(isset($a->count), empty($a->items), $a->name ?? 'none');
var_dump($a->count = '4', $a->name = 5);
$a->items[] = 6;
Child::$shared = '7';
echo Base::$shared, ':', (new Child)->count, ':', count($a->items), "\n";
unset($a->count);
var_dump(isset($a->count), property_exists($a, 'count'));
var_dump($a->count ??= '8');
var_dump(count(get_object_vars($a)), count((array)$a));
class Counter { public string $value = '9'; public static array $values; }
$c = new Counter;
var_dump(++$c->value);
Counter::$values[] = 1;
unset(Counter::$values[0]);
var_dump(Counter::$values);
--EXPECT--
object(Base)#1 (1) {
  ["count"]=>
  uninitialized(int)
  ["name"]=>
  uninitialized(?string)
  ["items"]=>
  uninitialized(array)
  ["scale"]=>
  float(2)
}
bool(false)
bool(true)
string(4) "none"
int(4)
string(1) "5"
7:3:1
bool(false)
bool(true)
int(8)
int(4)
int(4)
string(2) "10"
array(0) {
}
