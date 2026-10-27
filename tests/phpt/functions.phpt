--TEST--
Function hoisting, recursion, local scopes, and named/unpacked arguments
--FILE--
<?php
$n=99;
echo fact(8), ':', $n, "\n";
function fact($n) { if ($n<2) return 1; return $n*fact($n-1); }
function combine($a,$b=2,$c=[10,20][1],...$rest) {
    echo $a, ':', $b, ':', $c, '|';
    foreach ($rest as $key=>$value) echo $key, ':', $value, ';';
    echo "\n";
}
combine(c:3,a:1,extra:4);
$callable='COMBINE';
$callable(...[7=>4,9=>5], z:6);
function mutate($a=[1]) { $a[]=2; return $a; }
echo mutate()[1], mutate()[0];
--EXPECT--
40320:99
1:2:3|extra:4;
4:5:20|z:6;
21
