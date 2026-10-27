--TEST--
Private and protected class constants retain scope, inheritance, and independent types
--FILE--
<?php
interface Label { const NAME = 'interface'; }
class Base {
    private const int SECRET = 7;
    private const NAME = 'base';
    protected const int VALUE = self::SECRET + 1;
    public $value = self::VALUE;
    public static function reader() { return static fn() => self::SECRET; }
    public static function inspect() {
        echo self::SECRET, ':', static::SECRET, ':', constant('self::VALUE'), "\n";
        var_dump(defined('self::SECRET'));
    }
}
class Child extends Base implements Label {
    public const string SECRET = 'child';
    public const int VALUE = 9;
    public static function show() { echo parent::VALUE, ':', self::VALUE, ':', self::NAME, "\n"; }
}
Child::inspect();
Child::show();
$reader = Child::reader();
echo $reader(), ':', (new Child)->value, "\n";
var_dump(defined('Base::SECRET'), defined('Base::VALUE'), defined('Child::NAME'));
echo Child::NAME, ':', constant('Child::SECRET'), "\n";
?>
--EXPECT--
7:child:8
bool(true)
8:9:interface
7:8
bool(false)
bool(false)
bool(true)
interface:child
