--TEST--
Public class instances, bound callbacks, shared state, and live properties
--FILE--
<?php
$counter = new Counter(start: 4);
class Counter {
    public $value = 0;
    public $history = [];
    function __construct(int $start) { $this->value = $start; }
    function add(int $amount = 1): int {
        $this->history[] = $amount;
        return $this->value += $amount;
    }
    function reader() { return fn() => $this->value; }
}
$alias = $counter;
$add = $counter->add(...);
$read = $counter->reader();
echo $add(), ':', $alias->ADD(amount: 3), ':', $read(), "\n";
var_dump($counter === $alias, count($counter->history));
var_dump(array_map([$counter, 'add'], [2, 3]));
$copy = $counter->history;
$copy[] = 100;
echo count($copy), ':', count($counter->history), "\n";
echo get_class($counter), ':', get_debug_type($counter), ':', gettype($counter), "\n";
var_dump(class_exists('COUNTER'), method_exists($counter, 'ADD'), property_exists($counter, 'value'));
unset($counter->value);
var_dump(isset($counter->value), $counter->value ?? 99);
$counter->value = 20;
echo $read(), "\n";
$obj = new stdClass;
$obj->a = 1;
$obj->b = 2;
foreach ($obj as $key => $value) {
    echo $key, '=', $value, ';';
    if ($key === 'a') { $obj->b = 3; $obj->c = 4; }
}
echo "\n";
--EXPECT--
5:8:8
bool(true)
int(2)
array(2) {
  [0]=>
  int(10)
  [1]=>
  int(13)
}
5:4
Counter:Counter:object
bool(true)
bool(true)
bool(true)
bool(false)
int(99)
20
a=1;b=3;c=4;
