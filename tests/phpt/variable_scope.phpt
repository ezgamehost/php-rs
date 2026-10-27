--TEST--
Live global bindings and persistent static variables
--FILE--
<?php
$counter = 10;
function bump() { global $counter; ++$counter; }
function report() {
    global $counter;
    static $calls = 0, $values = [];
    bump();
    $values[] = ++$calls;
    echo $counter, ':', $calls, ':', count($values), "\n";
    unset($counter);
    $counter = -1;
}
report(); report();
echo $counter, "\n";
function capture($value) {
    static $saved = $value;
    return ++$saved;
}
echo capture(3), ':', capture(99), "\n";
function recursive($n) {
    static $count = 0;
    ++$count;
    if ($n) recursive($n - 1);
    echo $count;
}
recursive(2);
echo "\n";
?>
--EXPECT--
11:1:1
12:2:2
12
4:5
333
