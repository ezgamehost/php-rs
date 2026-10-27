--TEST--
Typed class constants validate lazy values and covariant overrides
--FILE--
<?php
class Base {
    public const int FIRST = 2, SECOND = 3;
    public const float SCALE = FACTOR;
    public const int|string VALUE = 5;
    public const iterable ITEMS = [1, 2];
    public const ?self NONE = null;
    public const Closure FACTORY = static function () { return self::FIRST; };
    final public const string LABEL = 'base';
}
class Child extends Base {
    public const int VALUE = 6;
    public const array ITEMS = [3];
    public const ?self NONE = null;
}
define('FACTOR', 4);
var_dump(Base::FIRST, Base::SECOND, Child::SCALE);
var_dump(Child::VALUE, Child::ITEMS, Child::NONE);
echo Child::LABEL, ':', (Child::FACTORY)(), "\n";
var_dump(Base::FACTORY === Child::FACTORY);
var_dump(defined('Child::SCALE'), constant('Child::SCALE'));
--EXPECT--
int(2)
int(3)
float(4)
int(6)
array(1) {
  [0]=>
  int(3)
}
NULL
base:2
bool(true)
bool(true)
float(4)
