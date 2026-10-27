--TEST--
Eval scope, return values, independent strictness and lexical magic constants
--FILE--
<?php
declare(strict_types=1);
function typed(int $x) { return $x; }
function outer($x) {
    $value = eval('$x += 2; return typed("3");');
    echo $x, ':', $value, ':', __FUNCTION__, ':', __METHOD__, "\n";
    echo eval('return __FUNCTION__;'), "\n";
}
outer(4);
eval('function declared($name=__FUNCTION__) { return $name; }');
echo declared(), "\n";
var_dump(eval(''), eval(' '), eval('return [1,2];'));
echo eval('?>text<?php return 7;'), "\n";
var_dump(function_exists('eval'), __CLASS__, __NAMESPACE__, __TRAIT__);
?>
--EXPECT--
6:3:outer:outer

declared
bool(false)
NULL
array(2) {
  [0]=>
  int(1)
  [1]=>
  int(2)
}
text7
bool(false)
string(0) ""
string(0) ""
string(0) ""
