--TEST--
Scalar and array casts, precedence, constant defaults and void expressions
--FILE--
<?php
echo (int)'12tail',':',(int)'bad',':',(int)-1.9,':',(float)'2.5tail',"\n";
echo (int)2.5**2,':',(int)2.5*2,':',(int)(float)'3.5',"\n";
$a=(array)null; $b=(array)'x'; $c=(array)['y'=>2];
echo count($a),$b[0],$c['y'],"\n";
function f($a=(int)'2.9',$b=(array)null) { echo $a,count($b),"\n"; }
f();
(void) print 's';
for((void)$i=0;(void)print 'c',$i<2;(void)$i++)echo $i;
echo "\n";
--EXPECT--
12:0:-1:2.5
6:4:3
0x2
20
sc0c1c
