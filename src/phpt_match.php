// Invoked with php -n -r by the PHPT harness, not by the Rust runtime.
// Input: decimal expected-byte length, newline, expected bytes, actual bytes.
$length = (int) fgets(STDIN);
$payload = stream_get_contents(STDIN);
$pattern = substr($payload, 0, $length);
$actual = substr($payload, $length);

if ($argv[1] === 'EXPECTF') {
    // Quote literal spans while retaining paired %r PCRE fragments. An
    // unmatched %r is literal, as in PHP 8.5's run-tests.php.
    $parts = explode('%r', $pattern);
    $last = count($parts) - 1;
    if ($last % 2 !== 0) {
        $parts[$last - 1] .= '%r' . array_pop($parts);
    }
    foreach ($parts as $index => &$part) {
        $part = $index % 2 === 0 ? preg_quote($part, '/') : '(' . $part . ')';
    }
    unset($part);
    $pattern = strtr(implode('', $parts), [
        '%e' => preg_quote(DIRECTORY_SEPARATOR, '/'),
        '%s' => '[^\r\n]+',
        '%S' => '[^\r\n]*',
        '%a' => '.+?',
        '%A' => '.*?',
        '%w' => '\s*',
        '%i' => '[+-]?\d+',
        '%d' => '\d+',
        '%x' => '[0-9a-fA-F]+',
        '%f' => '[+-]?(?:\d+|(?=\.\d))(?:\.\d+)?(?:[Ee][+-]?\d+)?',
        '%c' => '.',
        '%0' => '\x00',
    ]);
}
$matched = @preg_match('/^' . $pattern . '$/s', $actual);
if ($matched === false) {
    fwrite(STDERR, preg_last_error_msg());
    exit(2);
}
echo $matched;
