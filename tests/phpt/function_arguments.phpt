--TEST--
Function argument inspection with defaults, named arguments, and variadics
--FILE--
<?php
function show($a=1,$b=2,$c=3) {
    echo func_num_args(), ':';
    foreach(func_get_args() as $v) echo $v, ',';
    echo "\n";
}
show();
show(7);
show(c:9,a:8);
show(4,5,6,7);
function variadic(int $a=1,int ...$v) {
    echo func_num_args(), ':';
    foreach(func_get_args() as $x) echo $x, ',';
    echo ':',count($v),"\n";
}
variadic(z:9);
variadic('2','3','4',z:'5');
function mutate($a,...$v) {
    $copy=func_get_args();
    $a=7;
    $v[0]=8;
    echo func_get_arg(position:0),func_get_arg(1),':',$copy[0],$copy[1],"\n";
    unset($a);
    var_dump(func_get_arg(0));
    $a=9;
    echo func_get_args()[0],"\n";
}
mutate(1,2);
function inner($x) {
    echo func_num_args(),func_get_arg(0),"\n";
}
function outer($x) {
    inner(func_get_arg(0));
    eval('$x=8;');
    echo func_num_args(),func_get_arg(0),"\n";
}
outer(7);
--EXPECT--
0:
1:7,
3:8,2,9,
4:4,5,6,7,
0::1
3:2,3,4,:3
72:12
NULL
9
17
18
