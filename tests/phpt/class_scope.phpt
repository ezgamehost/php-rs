--TEST--
Static and parent calls preserve declaring and late-bound class contexts
--FILE--
<?php
class Base {
    public $n;
    public $factory = static function () { return static::class; };
    function __construct(int $n) { $this->n = $n; }
    function describe(): string { return 'base:' . $this->n; }
    static function make(): static { return new static(4); }
    static function called(): string { return static::class; }
    static function scale(int $n): int { return $n * 2; }
    function reader() { return static fn() => self::class . ':' . static::class; }
}
class Child extends Base {
    function __construct(int $n) { parent::__construct($n + 1); }
    function describe(): string { return parent::describe() . ':child'; }
    static function forward() { return parent::called() . ':' . Base::called(); }
}
$child = Child::make();
echo $child->describe(), "\n";
echo Child::forward(), "\n";
echo ($child->reader())(), "\n";
echo ($child->factory)(), "\n";
$first = Base::called(...);
$second = Child::called(...);
var_dump($first == $second);
echo $first(), ':', $second(), "\n";
echo $child::class, ':', get_parent_class($child), ':', UnknownClass::class, "\n";
$name = 'Child';
$method = 'scale';
echo $name::$method(3), "\n";
var_dump(array_map('Child::scale', [1, 2]));
$a = new Base(0);
$b = new Base(1);
var_dump($a->factory === $b->factory, $a->factory === $child->factory);
--EXPECT--
base:5:child
Child:Base
Base:Child
Base
bool(false)
Base:Child
Child:Base:UnknownClass
6
array(2) {
  [0]=>
  int(2)
  [1]=>
  int(4)
}
bool(true)
bool(false)
