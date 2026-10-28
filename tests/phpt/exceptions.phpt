--TEST--
Throwable hierarchy, catch unions, finally unwinding, and previous exceptions
--FILE--
<?php
class Problem extends Exception {}
function fail($message) { throw new Problem($message, 7); }
try {
    fail('problem');
} catch (Missing|Exception $e) {
    echo get_class($e), ':', $e->getMessage(), ':', $e->getCode(), "\n";
} finally {
    echo "finally\n";
}
try { 1 / 0; } catch (ArithmeticError $e) {
    echo get_class($e), ':', $e->getMessage(), "\n";
}
try {
    try { throw new Error('first'); }
    finally { throw new Exception('second'); }
} catch (Throwable $e) {
    echo $e->getMessage(), ':', $e->getPrevious()->getMessage(), "\n";
}
$f = fn($x) => $x ?? throw new Exception('missing');
echo $f(3), "\n";
try { $f(null); } catch (Exception) { echo "caught\n"; }
function choice() { try { return 1; } finally { return 2; } }
echo choice(), "\n";
?>
--EXPECT--
Problem:problem:7
finally
DivisionByZeroError:Division by zero
second:first
3
caught
2
