--TEST--
Non-public static properties preserve declaring scope, inherited storage, and private redeclarations
--FILE--
<?php
class Store {
    private static int $count = 1;
    protected static array $items = [];
    public static function countReader() { return static fn() => ++self::$count; }
    public static function add($value) { static::$items[] = $value; }
    public static function inspect() { echo self::$count, ':', static::$count, ':', count(self::$items), "\n"; }
}
class ChildStore extends Store {
    public static string $count = 'child';
    public static function take() { $value = parent::$items[0]; unset(parent::$items[0]); return $value; }
}
Store::add('a');
ChildStore::add('b');
ChildStore::inspect();
$reader = ChildStore::countReader();
echo $reader(), ':', ChildStore::take(), "\n";
ChildStore::inspect();
var_dump(isset(Store::$count), empty(Store::$items), Store::$count ?? 'hidden');
var_dump(property_exists('Store', 'count'), property_exists('ChildStore', 'items'));
class Counter extends Store {
    public static function next() { return self::$count ?? 'hidden'; }
}
echo Counter::next(), "\n";
var_dump(property_exists('Counter', 'count'));
?>
--EXPECT--
1:child:2
2:a
2:child:1
bool(false)
bool(true)
string(6) "hidden"
bool(true)
bool(true)
hidden
bool(false)
