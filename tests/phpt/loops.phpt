--TEST--
For, do-while, numbered loop control, and alternative loop syntax
--FILE--
<?php
function mark($s) { echo $s; return false; }
for($i=0,mark('I');mark('C'),$i<2;$i++,mark('S')) echo $i;
echo "\n";
for($i=0;$i<3;$i++) {
    foreach([1,2] as $j) {
        echo $i,$j;
        continue 2;
    }
    echo 'bad';
}
echo "\n";
$i=0;
do {
    $i++;
    while(true) { echo $i; continue 2; }
} while($i<3);
echo "\n";
for($i=0;$i<2;$i++):
    foreach(['a','b'] as $x): echo $x; endforeach;
endfor;
echo "\n";
foreach([1,2] as $x) {
    do { while(true) { echo $x; break ((3)); } } while(true);
}
echo "\n";
--EXPECT--
IC0SC1SC
011121
123
abab
1
