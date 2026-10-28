use php_rs::Runtime;

fn output(code: &str) -> Vec<u8> {
    let mut runtime = Runtime::default();
    runtime.execute_code(code.as_bytes()).unwrap();
    runtime.output
}

#[test]
fn object_conversion_assigns_one_byte_and_returns_that_byte() {
    assert_eq!(
        output(
            "class A{function __toString(){return 'Q';}}$s='abc';var_dump($s[1]=new A,$s);$a=['abc'];var_dump($a[0][-1]=new A,$a[0]);"
        ),
        b"string(1) \"Q\"\nstring(3) \"aQc\"\nstring(1) \"Q\"\nstring(3) \"abQ\"\n"
    );
}

#[test]
fn replacement_during_conversion_is_preserved_even_when_bytes_are_equal() {
    assert_eq!(
        output(
            "class A{function __toString(){global $s;$s='abc';return 'Q';}}$s='abc';var_dump($s[0]=new A,$s);"
        ),
        b"NULL\nstring(3) \"abc\"\n"
    );
}

#[test]
fn nested_writes_preserve_siblings_and_detect_replacement_of_the_string() {
    assert_eq!(output("class A{function __toString(){global $a;$a[1]='changed';return 'Q';}}$a=['abc',1];var_dump($a[0][0]=new A,$a);"), b"string(1) \"Q\"\narray(2) {\n  [0]=>\n  string(3) \"Qbc\"\n  [1]=>\n  string(7) \"changed\"\n}\n");
    assert_eq!(
        output(
            "class A{function __toString(){global $a;$a[0]='changed';return 'Q';}}$a=['abc'];var_dump($a[0][0]=new A,$a[0]);"
        ),
        b"NULL\nstring(7) \"changed\"\n"
    );
}

#[test]
fn writes_inside_conversion_separate_the_destination_from_the_pending_buffer() {
    assert_eq!(
        output(
            "class A{function __toString(){global $s;$s[1]='Z';return 'Q';}}$s='abc';var_dump($s[0]=new A,$s);"
        ),
        b"NULL\nstring(3) \"aZc\"\n"
    );
}

#[test]
fn string_copies_before_and_during_conversion_follow_php_separation() {
    assert_eq!(
        output(
            "class A{function __toString(){global $s,$during;$during=$s;return 'Q';}}$s='abc';$before=$s;var_dump($s[0]=new A,$s,$before,$during);"
        ),
        b"string(1) \"Q\"\nstring(3) \"Qbc\"\nstring(3) \"abc\"\nstring(3) \"Qbc\"\n"
    );
}

#[test]
fn properties_and_static_properties_support_reentrant_conversion() {
    assert_eq!(
        output(
            "class B{public string $s='abc';public static string $t='xyz';}class A{function __toString(){return 'Q';}}$b=new B;var_dump($b->s[1]=new A,B::$t[-1]=new A,$b->s,B::$t);"
        ),
        b"string(1) \"Q\"\nstring(1) \"Q\"\nstring(3) \"aQc\"\nstring(3) \"xyQ\"\n"
    );
}

#[test]
fn invalid_offsets_do_not_invoke_conversion_and_exceptions_preserve_side_effects() {
    assert_eq!(
        output(
            "class A{function __toString(){echo 'bad';return 'Q';}}$s='abc';var_dump($s[-9]=new A);try{$s[]=new A;}catch(Throwable){echo 'append';}class B{function __toString(){global $s;$s='changed';throw new Exception('x');}}try{$s[0]=new B;}catch(Throwable $e){echo $e->getMessage();}echo $s;"
        ),
        b"NULL\nappendxchanged"
    );
}

#[test]
fn shared_literals_constants_defaults_and_arrays_separate_before_writes() {
    assert_eq!(
        output(
            "const S='abc';function f(){$s='abc';$s[0]='Z';return $s;}class A{public string $s='abc';}echo f(),f(),S;$a=new A;$b=new A;$a->s[0]='Z';$x=[S];$y=$x;$x[0][0]='Z';echo $b->s,$y[0],S;"
        ),
        b"ZbcZbcabcabcabcabc"
    );
}

#[test]
fn access_checks_precede_offset_conversion() {
    assert_eq!(
        output(
            "class T{function __toString(){echo 'bad';return 'Q';}}class A{public private(set) static string $s='abc';public readonly string $r;function __construct(){$this->r='abc';}}$a=new A;try{A::$s[0]=new T;}catch(Error){echo 's';}try{$a->r[0]=new T;}catch(Error){echo 'r';}"
        ),
        b"sr"
    );
}

#[test]
fn a_retained_original_buffer_allows_writing_a_replaced_destination() {
    assert_eq!(
        output(
            "function prepare(){global $s;}prepare();class A{function __toString(){global $s,$during;$during=$s;$s='changed';return 'Q';}}$s='abc';var_dump($s[0]=new A,$s,$during);"
        ),
        b"string(1) \"Q\"\nstring(7) \"Qhanged\"\nstring(3) \"abc\"\n"
    );
}

#[test]
fn extension_restores_the_original_buffer_and_updates_copies_made_during_conversion() {
    assert_eq!(
        output(
            "function p(){global $s;}p();class A{function __toString(){global $s,$copy;$copy=$s;$s='changed';return 'Q';}}$s='abc';var_dump($s[5]=new A,$s,$copy);"
        ),
        b"string(1) \"Q\"\nstring(6) \"abc  Q\"\nstring(6) \"abc  Q\"\n"
    );
    assert_eq!(
        output(
            "class A{function __toString(){global $a,$old,$copy;$old=$a[0];$a[0]='changed';$copy=$a;return 'Q';}}$a=['abc'];var_dump($a[0][5]=new A,$a[0],$old,$copy[0]);"
        ),
        b"string(1) \"Q\"\nstring(6) \"abc  Q\"\nstring(6) \"abc  Q\"\nstring(6) \"abc  Q\"\n"
    );
}
