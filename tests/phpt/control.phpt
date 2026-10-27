--TEST--
Variables, loops, branch control, and lazy expressions
--FILE--
<?php
$i = 0; $sum = 0;
while ($i < 10) {
    $i++;
    if ($i == 2) continue;
    if ($i == 5) break;
    $sum += $i;
}
echo $sum, ',', $i, '\n';
echo $missing ?? 'default', ',', true ? 7 : 1 / 0, ',', 'x' ?: 1 / 0;
$x = 0; false && ($x = 1); true || ($x = 2); echo ',', $x;
return;
echo 'unreachable';
--EXPECT--
8,5\ndefault,7,x,0
