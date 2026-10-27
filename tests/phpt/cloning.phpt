--TEST--
Shallow cloning, readonly reinitialization, and PHP 8.5 clone-with updates
--FILE--
<?php
class Record {
    public array $tags = [1];
    public object $child;
    public function __construct(public readonly int $id = 1) {
        $this->child = new stdClass;
    }
    public function __clone(): void {
        echo 'clone:', $this->id, "\n";
        $this->id++;
    }
    public function withId($id) {
        return clone($this, ['id' => $id]);
    }
}
$a = new Record;
$b = clone $a;
$b->tags[] = 2;
$b->child->value = 3;
echo $a->id, ':', $b->id, ':', count($a->tags), ':', count($b->tags), ':', $a->child->value, "\n";
$c = $a->withId('4');
echo $a->id, ':', $c->id, "\n";
$f = clone(...);
$d = $f(object: new stdClass, withProperties: ['x' => 5]);
echo $d->x, "\n";
$n = 0;
$counter = function () use (&$n) { static $i = 0; echo ++$i, ':', ++$n, "\n"; };
$counter();
$copy = clone $counter;
$counter();
$copy();
var_dump($a === $b, $a->child === $b->child, $counter === $copy);
?>
--EXPECT--
clone:1
1:2:1:2:3
clone:1
1:4
5
1:1
2:2
2:3
bool(false)
bool(true)
bool(false)
