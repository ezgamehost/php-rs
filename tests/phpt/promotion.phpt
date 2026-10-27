--TEST--
Constructor promotion initializes properties before the body and preserves parameter defaults
--FILE--
<?php
class Point {
    function __construct(public int $x = 0, public int $y = 0) {
        echo $this->x, ':', $this->y, ':', func_num_args(), "\n";
        $x = 9;
    }
}
$p = new Point(y: 2, x: '1');
echo $p->x, ':', $p->y, "\n";
class Child extends Point {
    function __construct(public string $label = 'child') {
        parent::__construct(3, 4);
    }
}
$c = new Child;
echo $c->label, ':', $c->x, ':', $c->y, "\n";
readonly class Record {
    function __construct(public int $id = 5, public ?string $name = null) {}
}
$r = new Record;
var_dump($r->id, $r->name);
class Skipped extends Point { function __construct() {} }
$s = new Skipped;
var_dump(isset($s->x), property_exists($s, 'x'));
class Untyped { function __construct(public $value = 6) {} }
class SkippedUntyped extends Untyped { function __construct() {} }
var_dump((new SkippedUntyped)->value);
--EXPECT--
1:2:2
1:2
3:4:2
child:3:4
int(5)
NULL
bool(false)
bool(true)
NULL
