--TEST--
Ordered arrays, nested copy-on-write, append indexes, and foreach
--FILE--
<?php
$a = ['x'=>1, 2, 'y'=>3, 'x'=>4];
foreach ($a as $k=>$v) echo $k, ':', $v, ';';
echo "\n";
$a = ['x'=>[1,2]]; $b=$a; $b['x'][0]=9; $b['x'][]=3;
foreach ($a['x'] as $v) echo $v, ':';
foreach ($b['x'] as $v) echo $v, ':';
echo "\n";
$a = [-5=>1]; $a[]=2; unset($a[-4]); $a[]=3;
foreach ($a as $k=>$v) echo $k, ':', $v, ';';
echo "\n";
$i=0; $a=[]; $a[$i++]['x']=4; $a[0]['x']+=3;
echo $i, ',', $a[0]['x']++, ',', $a[0]['x'], "\n";
echo $a[0]['missing'] ?? 'fallback', ',', isset($a[0]['x']), ',', empty($a[0]['missing']);
--EXPECT--
x:4;0:2;y:3;
1:2:9:2:3:
-5:1;-3:3;
1,7,8
fallback,1,1
