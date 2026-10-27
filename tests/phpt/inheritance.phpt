--TEST--
Public inheritance, compatible overrides, class binding order, and instanceof
--FILE--
<?php
class Base {
    public $label = 'base';
    public $value = 0;
    function __construct(int $value) { $this->value = $value; }
    function name(): string { return $this->label; }
    function show(): string { return $this->name() . ':' . $this->value; }
    function sequence(): int { static $n = 0; return ++$n; }
}
class Child extends Base {
    public $label = 'child';
    function name(): string { return 'derived'; }
}
final class Leaf extends Child {}
function accept(Base $value): Child { return $value; }
$object = accept(new Leaf(7));
echo $object->show(), "\n";
var_dump($object instanceof Leaf, $object instanceof Child, $object instanceof Base);
$name = 'base';
var_dump($object instanceof $name, $object instanceof Missing, !$object instanceof Base);
echo (new Base(0))->sequence(), ':', $object->sequence(), ':', (new Child(0))->sequence(), "\n";
$bound = $object->show(...);
echo $bound(), "\n";
class Producer {
    function produce(Child $input): Base { return $input; }
}
class LeafProducer extends Producer {
    function produce(Base $input): Leaf { return new Leaf(9); }
}
echo (new LeafProducer)->produce(new Base(0))->show(), "\n";
var_dump(class_exists('LaterChild'));
class LaterChild extends LaterBase {}
class LaterBase {}
var_dump(class_exists('LaterChild'));
var_dump(null instanceof (missing_function()));
--EXPECT--
derived:7
bool(true)
bool(true)
bool(true)
bool(true)
bool(false)
bool(false)
1:2:3
derived:7
derived:9
bool(false)
bool(true)
bool(false)
