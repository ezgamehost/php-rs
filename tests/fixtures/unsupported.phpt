--TEST--
Explicitly unsupported section must fail the runner gate
--INI--
precision=14
--FILE--
<?php echo 42;
--EXPECT--
42
