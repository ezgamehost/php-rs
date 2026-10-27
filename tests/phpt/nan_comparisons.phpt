--TEST--
NaN comparisons are silent and unordered against numbers and strings
--FILE--
<?php
$nan = NAN;
foreach ([0, 1.0, '1', 'NAN', 'text', NAN] as $value) {
    var_dump($nan == $value, $nan < $value, $nan > $value,
             $nan <=> $value, $value <=> $nan);
}
var_dump($nan == true, $nan == false, $nan == null, [NAN] == [true]);
?>
--EXPECT--
bool(false)
bool(false)
bool(false)
int(1)
int(1)
bool(false)
bool(false)
bool(false)
int(1)
int(1)
bool(false)
bool(false)
bool(false)
int(1)
int(1)
bool(false)
bool(false)
bool(false)
int(1)
int(1)
bool(false)
bool(false)
bool(false)
int(1)
int(1)
bool(false)
bool(false)
bool(false)
int(1)
int(1)
bool(true)
bool(false)
bool(false)
bool(true)
