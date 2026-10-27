--TEST--
Byte string offsets, negative indexes, padding and copy isolation
--FILE--
<?php
$s='abc';
echo "$s[1]/{$s[-1]}\n";
$copy=$s;
$s[1]='X'; $s[5]=7;
var_dump($s,$copy,$s[-1]);
$s[1]??='Y'; $s[7]??='Z';
var_dump($s,isset($s[8]),empty($s[-10]),$s[9]??'missing');
$a=['abc']; $b=$a; $b[0][1]='Q';
var_dump($a[0],$b[0]);
--EXPECT--
b/c
string(6) "aXc  7"
string(3) "abc"
string(1) "7"
string(8) "aXc  7 Z"
bool(false)
bool(true)
string(7) "missing"
string(3) "abc"
string(3) "aQc"
