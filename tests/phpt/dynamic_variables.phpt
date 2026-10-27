--TEST--
Variable variables, computed global names, and single evaluation of names
--FILE--
<?php
function name() { echo "name\n"; return 'value'; }
$value = 3;
function update() {
    global ${name()};
    ++$value;
}
update();
${name()} += 2;
echo $value, "\n";
$name = 'items';
$$name = [1, 2];
$$name[1]++;
${'items'}[] = 4;
echo $items[1], ':', $items[2], "\n";
$a = 'b'; $b = 'c'; $c = 7;
echo $$$a, "\n";
unset($$name);
var_dump(isset($$name), empty($$name));
$$name ??= 9;
echo $items, "\n";
?>
--EXPECT--
name
name
6
3:4
7
bool(false)
bool(true)
9
