--TEST--
Readonly initialization scopes, classes and interior object mutation
--FILE--
<?php
readonly class Base {
    public int $id;
    const LABEL = 'base';
    function __construct($id) { $this->id = $id; }
}
final readonly class Child extends Base {
    public string $name;
    function __construct($id, $name) {
        parent::__construct($id);
        $this->name = $name;
    }
}
$c = new Child('2', 'child');
echo $c->id, ':', $c->name, ':', Child::LABEL, "\n";
class Holder {
    public readonly object $object;
    public readonly array $items;
    public readonly ?int $optional;
    function init() {
        $this->object = new stdClass;
        $this->items = [new stdClass];
        $this->optional = null;
    }
}
$h = new Holder;
$h->init();
$h->object->value = 3;
$items = $h->items;
$items[0]->value = 4;
echo $h->object->value, ':', $h->items[0]->value, "\n";
var_dump(isset($h->optional), property_exists($h, 'optional'));
class Lazy {
    public readonly int $value;
    function setter() { return fn($v) => $this->value ??= $v; }
}
$l = new Lazy;
$set = $l->setter();
echo $set('5'), ':', $set('6'), ':', $l->value, "\n";
--EXPECT--
2:child:base
3:4
bool(false)
bool(true)
5:5:5
