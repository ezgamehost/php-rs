--TEST--
Strict match expressions, lazy grouped arms, nested results and live subjects
--FILE--
<?php
foreach([1,'1',1.0,true,null] as $v) {
    echo match($v) {1=>'i','1'=>'s',1.0=>'f',true=>'b',null=>'n'};
}
echo "\n";
function mark($v) { echo $v; return $v; }
echo match(mark(2)) {
    default => 1/0,
    mark(1),mark(2),mark(3), => 'yes',
};
echo "\n";
echo (match(1) {1 => [match(2) {2 => 'nested'}]})[0],"\n";
$v=1;
echo match($v) {($v=2) => 'live', default => 'bad'},"\n";
echo match(['x'=>1,'y'=>2]) {
    ['y'=>2,'x'=>1] => 'bad',
    ['x'=>'1','y'=>2] => 'bad',
    ['x'=>1,'y'=>2] => 'exact',
},"\n";
--EXPECT--
isfbn
212yes
nested
live
exact
