--TEST--
Pipe operator ordering, chaining, precedence, and optional parameters
--FILE--
<?php
function input() { echo "input\n"; return 3; }
function choose() { echo "choose\n"; return 'twice'; }
function twice($x, $extra = 0) { return $x * 2 + $extra; }
var_dump(input() |> choose() |> twice(...));
var_dump(1 + 2 |> twice(...));
var_dump(6 == 3 |> twice(...));
var_dump(false && 3 |> twice(...));
var_dump('a' . 'bc' |> strlen(...));
?>
--EXPECT--
input
choose
int(12)
int(6)
bool(true)
bool(false)
int(3)
