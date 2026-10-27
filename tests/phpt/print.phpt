--TEST--
Print result values, precedence, and lazy expression contexts
--FILE--
<?php
$r=print $a=2;
echo ':',$a,':',$r,"\n";
print(1+2)*3;
echo "\n";
print 'a' and print 'b';
print 'x' && print 'c';
print 'z' && false;
echo "\n";
print print print 'x';
echo "\n";
match(1) {1=>print 'yes', default=>print 'bad'};
echo "\n";
--EXPECT--
2:2:1
9
abc1
x11
yes
