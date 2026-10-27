--TEST--
Scalar values, numeric literals, arithmetic and concatenation
--FILE--
<?php
echo true, false, null, '|', 0xff, ',', 0o17, ',', 0b101, ',', 1_000, "\n";
echo 2 + 3 * 4, ',', 2 ** 3 ** 2, ',', -2 ** 2, ',', 9 / 2, ',', -9 % 4, "\n";
echo 'sum=' . 2 + 3, ',', -0.0, ',', 1e-5, ',', 1e14, "\n";
--EXPECT--
1|255,15,5,1000
14,512,-4,4.5,-1
sum=5,-0,1.0E-5,1.0E+14
