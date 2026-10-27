<?php
$loads = ($loads ?? 0) + 1;
function loaded_source() { return __FILE__; }
function load_child() { return include 'child.php'; }
return ['loads' => $loads, 'directory' => __DIR__];
