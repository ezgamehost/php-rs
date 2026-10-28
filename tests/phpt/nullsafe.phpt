--TEST--
Nullsafe member chains, skipped side effects, quiet access, and static callables
--FILE--
<?php
class A {
    public $p = 'yes';
    public static $s = 7;
    function next($value = null) { echo "next\n"; return $this; }
    function none() { return null; }
    static function result() { return 9; }
}
$a = new A;
$n = null;
var_dump($n?->next(print 'bad')->p[print 'bad']);
var_dump(($n?->p)->next());
var_dump($a?->next()->p);
var_dump($a?->none()?->p);
var_dump($n?->next()::$s, $a?->next()::$s);
var_dump(isset($missing?->next()->p), empty($missing?->p), $missing?->p ?? 'fallback');
var_dump($n?->next()::result(...));
$f = $a?->next()::result(...);
var_dump($f());
echo "[$a?->p][$n?->p]\n";
const EMPTY_VALUE = null?->p->q;
var_dump(EMPTY_VALUE);
?>
--EXPECT--
NULL
NULL
next
string(3) "yes"
NULL
next
NULL
int(7)
bool(false)
bool(true)
string(8) "fallback"
NULL
next
int(9)
[yes][]
NULL
