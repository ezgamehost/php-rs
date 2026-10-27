--TEST--
Abstract classes, inherited implementations, static dispatch, and constructor contracts
--FILE--
<?php
abstract class Base {
    abstract public function __construct(int $value);
    abstract public function value(): int;
    abstract public static function label(): string;
    public function show() {
        echo static::label(), ':', $this->value(), "\n";
    }
}
abstract class Middle extends Base {
    public static function label(): string { return static::class; }
}
class Leaf extends Middle {
    public function __construct(public int|float $stored = 7) {}
    public function value(): int { return (int)$this->stored; }
}
$leaf = new Leaf;
$leaf->show();
var_dump($leaf instanceof Base, class_exists('Base'), method_exists('Base', 'value'));
$label = Leaf::label(...);
echo $label(), "\n";
abstract readonly class Record {
    public function __construct(public int $id) {}
    abstract public function getId(): int;
}
readonly class Entry extends Record {
    public function getId(): int { return $this->id; }
}
echo (new Entry(8))->getId(), "\n";
?>
--EXPECT--
Leaf:7
bool(true)
bool(true)
bool(true)
Leaf
8
