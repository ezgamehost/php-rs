--TEST--
Alternative if syntax, nested templates, and conditional loop control
--FILE--
<?php
foreach([0,1,2] as $n):
    if($n): ?><<?php
        if($n===1): ?>one<?php else: ?>two<?php endif;
        ?>><?php
    else: ?>zero<?php endif;
endforeach;
echo "\n";
for($i=0;$i<5;$i++):
    if($i===1): continue;
    elseif($i===3): break;
    else: echo $i;
    endif;
endfor;
echo "\n";
if(false): echo 'bad';
elseif(true): echo 'yes';
elseif(1/0): echo 'bad';
else: echo 'bad';
endif;
echo "\n";
--EXPECT--
zero<one><two>
02
yes
