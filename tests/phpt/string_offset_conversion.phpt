--TEST--
Stringable offset assignment retains string buffers across reentrant writes
--FILE--
<?php
function prepare() { global $s, $before, $during; }
prepare();
class Text {
    function __toString() { return 'Q'; }
}
$s = 'abc';
var_dump($s[-1] = new Text, $s);
class CopyText {
    function __toString() { global $s, $during; $during = $s; return 'Q'; }
}
$s = 'abc';
$before = $s;
var_dump($s[0] = new CopyText, $s, $before, $during);
class ReplaceText {
    function __toString() { global $s; $s = 'changed'; return 'Q'; }
}
$s = 'abc';
var_dump($s[0] = new ReplaceText, $s);
class ExtendText {
    function __toString() { global $s, $during; $during = $s; $s = 'changed'; return 'Q'; }
}
$s = 'abc';
var_dump($s[5] = new ExtendText, $s, $during);
?>
--EXPECT--
string(1) "Q"
string(3) "abQ"
string(1) "Q"
string(3) "Qbc"
string(3) "abc"
string(3) "Qbc"
NULL
string(7) "changed"
string(1) "Q"
string(6) "abc  Q"
string(6) "abc  Q"
