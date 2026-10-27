--TEST--
GLOBALS provides live variable access and independent snapshots
--FILE--
<?php
$x=1;
$a=['n'=>2];
$copy=$GLOBALS;
function update() {
    $x=99;
    $GLOBALS['x']+=2;
    $GLOBALS['a']['n']=9;
    echo $x, ':', $GLOBALS['x'], "\n";
}
update();
echo $copy['x'], $copy['a']['n'], ':', $x, $a['n'], "\n";
function detach() {
    global $x;
    unset($GLOBALS['x']);
    $GLOBALS['x']=7;
    echo $x, ':', $GLOBALS['x'], "\n";
}
detach();
$GLOBALS[1.5]=4;
$GLOBALS[null]=5;
echo ${'1.5'}, ${''}, "\n";
$n='GLOBALS';
$$n=6;
echo $GLOBALS['GLOBALS'], ':', ${'GLO'.'BALS'}['x'], "\n";
var_dump(isset($GLOBALS), isset($GLOBALS['missing']), empty($GLOBALS['missing']));
$GLOBALS['ga']=1;
$gb=2;
$ga=3;
foreach($GLOBALS as $key=>$value) {
    if($key==='ga'||$key==='gb') echo $key;
}
echo "\n";
function change() {
    $GLOBALS['x']=8;
    return $GLOBALS;
}
echo match($GLOBALS) {change()=>'bad', default=>'snapshot'}, "\n";
--EXPECT--
99:3
12:39
3:7
45
6:7
bool(true)
bool(false)
bool(true)
gbga
snapshot
