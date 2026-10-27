--TEST--
Variable interpolation, Unicode escapes, heredoc and nowdoc
--FILE--
<?php
$name='PHP'; $a=['x'=>'array'];
echo "$name/{$name}8.5/$a[x]/\$name/\u{2603}\n";
echo <<<TEXT
    Hello $name
      {$a['x']}
    TEXT;
echo "\n";
echo <<<'TEXT'
    $name \u{41}
    TEXT;
echo "\n";
var_dump("\u{1f602}", b"$name");
--EXPECT--
PHP/PHP8.5/array/$name/☃
Hello PHP
  array
$name \u{41}
string(4) "😂"
string(3) "PHP"
