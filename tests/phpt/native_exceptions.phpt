--TEST--
ErrorException severity and location, repeated construction, and SPL inheritance
--FILE--
<?php
$previous = new RuntimeException('previous');
$e = new ErrorException(previous: $previous, line: -9, filename: 'virtual.php', severity: E_USER_WARNING, code: 7, message: 'broken');
echo $e->getMessage(), ':', $e->getCode(), ':', $e->getSeverity(), ':', $e->getFile(), ':', $e->getLine(), "\n";
var_dump($e->getPrevious() === $previous);
$e->__construct();
echo $e->getMessage(), ':', $e->getCode(), ':', $e->getSeverity(), ':', $e->getFile(), ':', $e->getLine(), "\n";
$e->__construct(filename: 'next.php');
var_dump($e->getMessage(), $e->getLine(), $e->getPrevious() === $previous);
class Custom extends BadMethodCallException {}
try { throw new Custom('bad method'); }
catch (RuntimeException) { echo "wrong\n"; }
catch (BadFunctionCallException $caught) { echo get_class($caught), ':', $caught->getMessage(), "\n"; }
foreach (['LogicException', 'DomainException', 'InvalidArgumentException', 'LengthException', 'OutOfRangeException', 'RuntimeException', 'OutOfBoundsException', 'OverflowException', 'RangeException', 'UnderflowException', 'UnexpectedValueException'] as $name) {
    $e = new $name;
    echo get_class($e), ':', get_parent_class($e), "\n";
}
?>
--EXPECT--
broken:7:512:virtual.php:-9
bool(true)
broken:7:1:virtual.php:-9
string(0) ""
int(0)
bool(true)
Custom:bad method
LogicException:Exception
DomainException:LogicException
InvalidArgumentException:LogicException
LengthException:LogicException
OutOfRangeException:LogicException
RuntimeException:Exception
OutOfBoundsException:RuntimeException
OverflowException:RuntimeException
RangeException:RuntimeException
UnderflowException:RuntimeException
UnexpectedValueException:RuntimeException
