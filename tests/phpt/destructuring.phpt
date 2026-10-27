--TEST--
Value destructuring with nested lists, keys, skips and foreach
--FILE--
<?php
[$a,$b]=[1,2];
list($c,$d)=[3,4];
echo $a,$b,$c,$d,"\n";
[,,$a,]=[2=>'x'];
list(,$b,)=[1=>'y'];
echo $a,$b,"\n";
['x'=>[$a,[$b]],'y'=>$c]=['x'=>[1,[2]],'y'=>3];
echo $a,$b,$c,"\n";
$a=1;$b=2;$r=([$a,$b]=[$b,$a]);
echo $a,$b,$r[0],$r[1],"\n";
foreach([[1,2],[3,4]] as $k=>[$a,$b])echo $k,$a,$b;
echo "\n";
$a=[10,20];[0=>$x,($a[1]=1)=>$y]=$a;
echo $x,':',$y,':',$a[1],"\n";
--EXPECT--
1234
xy
123
2121
012134
10:20:1
