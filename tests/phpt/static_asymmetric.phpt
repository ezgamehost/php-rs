--TEST--
PHP 8.5 asymmetric visibility and final static properties
--FILE--
<?php
class Registry {
    private(set) static int $count = 0;
    public protected(set) static array $items = [];
    final public static string $label = 'registry';
    public static function add($value) {
        self::$count++;
        self::$items[] = $value;
    }
    public static function counter() { return static fn() => ++self::$count; }
}
class ChildRegistry extends Registry {
    public static function replace($value) { parent::$items[0] = $value; }
}
Registry::add('a');
ChildRegistry::add('b');
ChildRegistry::replace('A');
echo Registry::$count, ':', Registry::$items[0], ':', ChildRegistry::$items[1], "\n";
$counter = ChildRegistry::counter();
echo $counter(), ':', (Registry::$count ??= 99), "\n";
Registry::$label = 'changed';
echo ChildRegistry::$label, "\n";
var_dump(isset(Registry::$items[0]), empty(Registry::$items[3]));
class SharedBox { public $number = 1; }
class Holder {
    public private(set) static object $box;
    public static function init() { self::$box = new SharedBox; }
}
Holder::init();
Holder::$box->number++;
echo Holder::$box->number, "\n";
?>
--EXPECT--
2:A:b
3:3
changed
bool(true)
bool(true)
2
