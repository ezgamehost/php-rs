--TEST--
Goto resumes branches, loops, and switch cases without repeating dispatch
--FILE--
<?php
$i=0;
again:
echo ++$i;
if($i<3) goto again;
goto inside;
echo 'bad';
if(false) {
    echo 'bad';
    inside: echo "branch\n";
} else {
    echo 'bad';
}
for($i=0;$i<2;$i++) {
    foreach([1,2] as $v) {
        goto tail;
    }
    tail: echo $i,$v;
}
echo "\n";
$n=0;
$i=0;
switch(++$n) {
    case 1:
        start: echo ++$i;
        if($i<2) goto other;
        break;
    case 2:
        other: echo 'x';
        goto start;
}
echo ':',$n,"\n";
function countdown($n) {
    step: echo $n;
    if($n) {--$n; goto step;}
}
countdown(2);
goto end;
?>skipped HTML<?php end: ?>done
--EXPECT--
123branch
0111
1x2:1
210done
