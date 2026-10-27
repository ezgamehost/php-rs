--TEST--
Modern exit supports callable and named arguments and stops the entire request
--FILE--
<?php
function stop(callable $callback): never {
    echo 'stopping';
    $callback(status:7);
    echo 'bad';
}
echo function_exists('exit'),function_exists('die'),':';
stop('die');
echo 'bad';
--EXPECT--
11:stopping
