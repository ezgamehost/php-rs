--TEST--
Integer and string bitwise operators, shifts, and precedence
--FILE--
<?php
echo 8 | 4 & 8, ',', 7 ^ 3 & 1, ',', ~2 ** 3, ',', 4 & 4 == 4, "\n";
echo -8 >> 2, ',', 1 << 63, ',', 1 << 64, ',', -1 >> 64, ',', 1 >> 64, "\n";
echo 'AB' & 'a', '|', 'AB' | 'a', '|', 'AB' ^ 'a', "|end\n";
$x=5; $x |= 2; $x &= 6; $x ^= 3; $x <<= 2; $x >>= 1; echo $x;
--EXPECT--
8,6,-9,0
-2,-9223372036854775808,0,-1,0
A|aB| |end
10
