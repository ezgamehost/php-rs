--TEST--
Array unpacking reindexes integers and overwrites string keys in place
--FILE--
<?php
$a=[...[9=>'a',-2=>'b'],...[4=>'c'],'d'];
foreach($a as $k=>$v)echo $k,$v;
echo "\n";
$a=[...['x'=>1,'y'=>2],'x'=>3,...['x'=>4,'z'=>5]];
foreach($a as $k=>$v)echo $k,$v;
echo "\n";
$a=[-3=>'a',...[12=>'b',15=>'c']];
foreach($a as $k=>$v)echo $k,$v;
echo "\n";
function f($x=[...[1,2],...[]]) { return $x; }
$a=array(...[],...f(),3,);
foreach($a as $v)echo $v;
echo "\n";
--EXPECT--
0a1b2c3d
x4y2z5
-3a-2b-1c
123
