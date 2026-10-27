--TEST--
File inclusion, source identity, and once-only loading
--FILE--
<?php
$base = __DIR__ . '/../fixtures/loading';
$config = require $base . '/config.php';
var_dump($config['loads'], get_included_files()[0] === __FILE__);
var_dump(loaded_source() === $config['directory'] . '/config.php', load_child());
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
int(3)
bool(true)
