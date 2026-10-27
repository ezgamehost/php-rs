--TEST--
Switch fall-through, default order, live variables, and loop control
--FILE--
<?php
switch('2') {
    default: echo 'bad'; break;
    case 2: echo 'two';
    case 3: echo 'three';
}
echo "\n";
function mark($v) { echo $v; return $v; }
switch(mark(2)) {
    case mark(1): echo 'bad'; break;
    case mark(2): echo 'yes';
    case mark(3): echo 'tail';
}
echo "\n";
for($i=0;$i<3;$i++) {
    switch($i) {
        case 0: continue 2;
        case 1: echo 'one'; break;
        case 2: break 2;
    }
    echo 'tail';
}
echo $i,"\n";
$a=1;
switch($a) { case ($a=2): echo 'live'; }
echo "\n";
switch(7):
    case 1: echo 'bad'; break;
    default: echo 'default';
    case 2: echo 'tail';
endswitch;
echo "\n";
--EXPECT--
twothree
212yestail
onetail2
live
defaulttail
