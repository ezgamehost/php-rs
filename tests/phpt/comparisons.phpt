--TEST--
PHP 8 scalar comparisons and truthiness
--FILE--
<?php
echo 0 == 'foo' ? 'wrong' : 'ok', ',', 12 == '12', ',', 12 === '12' ? 'wrong' : 'ok', "\n";
echo '10' < '2' ? 'wrong' : 'ok', ',', 1 <=> 2, ',', (1 < 2) < 3 ? 'wrong' : 'ok', "\n";
echo !'0', !'', !null, !false, !0, !0.0, '|', !!'false', !!'00', !!-1;
--EXPECT--
ok,1,ok
ok,-1,ok
111111|111
