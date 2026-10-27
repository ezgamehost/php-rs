--TEST--
File inclusion, source identity, and once-only loading
--FILE--
<?php
$base = __DIR__ . '/tests/fixtures/loading';
$config = require $base . '/config.php';
var_dump($config['loads'], $config['directory'] === $base);
var_dump(loaded_source() === $base . '/config.php', load_child());
var_dump(include_once $base . '/config.php');
var_dump(require_once $base . '/child.php');
var_dump(count(get_included_files()), get_included_files() === get_required_files());
?>
--EXPECT--
int(1)
bool(true)
bool(true)
int(42)
bool(true)
bool(true)
int(2)
bool(true)
