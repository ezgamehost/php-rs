--TEST--
Stringable conversion and throwable string representations
--FILE--
<?php
$e = new Exception('boom');
$expected = "Exception: boom in ".__FILE__.":2\nStack trace:\n#0 {main}";
var_dump((string)$e === $expected, $e instanceof Stringable);
class Text {
    function __toString() { return 'text'; }
}
function use_text(string $value): string { return new Text; }
$t = new Text;
echo $t, '|', use_text($t), '|', strlen($t), "\n";
var_dump($t == 'text', [$t] == ['text']);
class Custom extends Exception {
    function __toString() { return 'custom'; }
}
echo new Custom, "\n";
class Broken {
    function __toString() { throw new Exception('inner'); }
}
function broken(): string { return new Broken; }
try { broken(); } catch (Throwable $e) {
    echo get_class($e), ':', $e->getPrevious()->getMessage(), "\n";
}
?>
--EXPECT--
bool(true)
bool(true)
text|text|4
bool(true)
bool(true)
custom
TypeError:inner
