--TEST--
Private and protected methods, factory construction, saved callables, and cloning
--FILE--
<?php
class Vault {
    private function __construct(public int $number) {}
    private function value(): int { return $this->number; }
    protected function label(): string { return 'vault'; }
    public static function make(int $number): static { return new static($number); }
    public function reader(): callable { return $this->value(...); }
    protected function __clone() { $this->number++; }
    public function copy(): static { return clone $this; }
}
class ChildVault extends Vault {
    public function value(): int { return 99; }
    public function show(): string { return $this->label(); }
}
$v = ChildVault::make(3);
$reader = $v->reader();
echo $reader(), ':', $v->value(), ':', $v->show(), "\n";
$copy = $v->copy();
$copyReader = $copy->reader();
echo $reader(), ':', $copyReader(), "\n";
var_dump(method_exists('Vault', 'label'), method_exists('ChildVault', '__construct'), method_exists($v, '__construct'));
?>
--EXPECT--
3:99:vault
3:4
bool(true)
bool(false)
bool(true)
