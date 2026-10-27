--TEST--
Exit string messages unwind evaluated code without evaluating later expressions
--FILE--
<?php
function stop() {
    echo 'before:';
    eval('echo "inside:"; exit("12"); echo "bad";');
    echo 'bad';
}
stop();
echo 'bad';
--EXPECT--
before:inside:12
