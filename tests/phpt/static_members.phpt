--TEST--
Class constants and static properties preserve inheritance and declaring scope
--FILE--
<?php
class Base {
    public const VALUE = 2, ORIGINAL = self::VALUE;
    final const LABEL = 'base';
    public static $count = self::VALUE, $items = [];
    public static $factory = static function () { return self::class; };
    public $value = self::VALUE;
    static function show() {
        echo self::VALUE, ':', static::VALUE, ':', self::$count, ':', static::$count, "\n";
    }
}
class Child extends Base { public const VALUE = 3; }
class Separate extends Child { public static $count = 9; }
Child::$count++;
Child::$items['x'] ??= 4;
Separate::$items[] = 5;
unset(Base::$items['x']);
Child::show();
Separate::show();
echo Child::ORIGINAL, ':', Child::LABEL, ':', (new Child)->value, "\n";
$c = 'Child';
$p = 'count';
$n = 'VALUE';
echo $c::$$p, ':', $c::{$n}, ':', (Child::$factory)(), "\n";
echo constant('Child::VALUE'), ':', count(Base::$items), "\n";
var_dump(defined('Child::ORIGINAL'), defined('Child::missing'));
var_dump(property_exists(new Child, 'count'), isset(Child::$missing));
var_dump(Child::$missing ?? 'fallback');
--EXPECT--
2:3:3:3
2:3:3:9
2:base:2
3:3:Base
3:1
bool(true)
bool(false)
bool(true)
bool(false)
string(8) "fallback"
