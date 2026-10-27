--TEST--
PHP documents, short echo tags, comments and escapes
--FILE--
header<?php echo 'body'; // comment ?>
footer<?= 42 ?>
<?php echo "\x41\101", '\\', '\'';
--EXPECT--
headerbodyfooter42AA\'
