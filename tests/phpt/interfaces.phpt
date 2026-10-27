--TEST--
User-defined interfaces, multiple inheritance, constants, and inherited implementations
--FILE--
<?php
interface Named {
    const string PREFIX = 'item';
    public function name(): string;
}
interface Valued {
    public function value(): int;
}
interface Item extends Named, Valued {}
abstract class Base implements Item {
    public function name(): string { return self::PREFIX; }
}
class Entry extends Base {
    public function __construct(public int $number = 7) {}
    public function value(): int { return $this->number; }
}
function show(Item $item): Named {
    echo $item->name(), ':', $item->value(), "\n";
    return $item;
}
$entry = new Entry;
var_dump(show($entry) === $entry);
var_dump($entry instanceof Item, $entry instanceof Named, $entry instanceof Valued);
var_dump(interface_exists('Item'), class_exists('Item'), method_exists('Item', 'name'));
echo Entry::PREFIX, ':', Item::PREFIX, "\n";
?>
--EXPECT--
item:7
bool(true)
bool(true)
bool(true)
bool(true)
bool(true)
bool(false)
bool(true)
item:item
