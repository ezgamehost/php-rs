use php_rs::Runtime;
fn output(code: &str) -> Vec<u8> {
    let mut rt = Runtime::default();
    rt.execute_code(code.as_bytes()).unwrap();
    rt.output
}

#[test]
fn scalar_casts_preserve_values_and_null_is_empty() {
    assert_eq!(output("foreach([null,false,true,0,-7,1.5,\"\",\"text\"] as $v){$o=(object)$v;echo get_class($o),\":\";var_dump((array)$o);}"),b"stdClass:array(0) {\n}\nstdClass:array(1) {\n  [\"scalar\"]=>\n  bool(false)\n}\nstdClass:array(1) {\n  [\"scalar\"]=>\n  bool(true)\n}\nstdClass:array(1) {\n  [\"scalar\"]=>\n  int(0)\n}\nstdClass:array(1) {\n  [\"scalar\"]=>\n  int(-7)\n}\nstdClass:array(1) {\n  [\"scalar\"]=>\n  float(1.5)\n}\nstdClass:array(1) {\n  [\"scalar\"]=>\n  string(0) \"\"\n}\nstdClass:array(1) {\n  [\"scalar\"]=>\n  string(4) \"text\"\n}\n");
}

#[test]
fn array_keys_become_accessible_string_properties() {
    assert_eq!(output("$a=[0=>\"zero\",-1=>\"negative\",\"01\"=>\"padded\",\"\"=>\"empty\",\"x\"=>\"named\"];$o=(object)$a;var_dump($o->{0},$o->{-1},$o->{\"01\"},$o->{\"\"},$o->x,(array)$o===$a);foreach($o as $k=>$v){var_dump($k);}"),b"string(4) \"zero\"\nstring(8) \"negative\"\nstring(6) \"padded\"\nstring(5) \"empty\"\nstring(5) \"named\"\nbool(true)\nstring(1) \"0\"\nstring(2) \"-1\"\nstring(2) \"01\"\nstring(0) \"\"\nstring(1) \"x\"\n");
}

#[test]
fn casts_preserve_object_and_closure_identity_without_magic_calls() {
    assert_eq!(
        output(
            "class A{function __toString(){echo \"bad\";return \"x\";}}$a=new A;$f=fn()=>3;$b=(object)$a;$g=(object)$f;var_dump($a===$b,$f===$g,$g());"
        ),
        b"bool(true)\nbool(true)\nint(3)\n"
    );
}

#[test]
fn array_casts_are_shallow_and_separate_value_storage() {
    assert_eq!(output("$child=new stdClass;$child->p=1;$a=[\"nested\"=>[1],\"text\"=>\"abc\",\"child\"=>$child];$o=(object)$a;$o->nested[0]=2;$o->text[0]=\"Z\";$o->child->p=3;var_dump($a[\"nested\"],$a[\"text\"],$a[\"child\"]->p,$o->nested,$o->text);"),b"array(1) {\n  [0]=>\n  int(1)\n}\nstring(3) \"abc\"\nint(3)\narray(1) {\n  [0]=>\n  int(2)\n}\nstring(3) \"Zbc\"\n");
}

#[test]
fn object_array_roundtrips_preserve_numeric_keys_and_order() {
    assert_eq!(output("$o=(object)[2=>\"a\",-4=>\"b\",\"01\"=>\"c\",0=>\"d\"];$o->{3}=\"e\";unset($o->{2});$a=(array)$o;$a[]=\"f\";var_dump($a);"),b"array(5) {\n  [-4]=>\n  string(1) \"b\"\n  [\"01\"]=>\n  string(1) \"c\"\n  [0]=>\n  string(1) \"d\"\n  [3]=>\n  string(1) \"e\"\n  [4]=>\n  string(1) \"f\"\n}\n");
}

#[test]
fn parameter_defaults_allocate_fresh_objects() {
    assert_eq!(
        output(
            "function f($o=(object)[\"p\"=>1]){$o->p++;return $o;}$a=f();$b=f();var_dump($a===$b,$a->p,$b->p);"
        ),
        b"bool(false)\nint(2)\nint(2)\n"
    );
}

#[test]
fn global_constants_keep_object_identity() {
    assert_eq!(
        output("const C=(object)[\"p\"=>1];$a=C;$a->p=2;var_dump(C->p,$a===C);"),
        b"int(2)\nbool(true)\n"
    );
}

#[test]
fn static_initializers_keep_one_object() {
    assert_eq!(
        output(
            "function f(){static $o=(object)[\"p\"=>0];$o->p++;return $o;}$a=f();$b=f();var_dump($a===$b,$a->p);"
        ),
        b"bool(true)\nint(2)\n"
    );
}

#[test]
fn nullsafe_access_accepts_cast_objects() {
    assert_eq!(
        output(
            "$o=(object)[\"p\"=>0];$array=[[null]];var_dump($array[0][$o->p]?->missing,((object)[\"x\"=>9])?->x);"
        ),
        b"NULL\nint(9)\n"
    );
}

#[test]
fn cloned_cast_objects_separate_values_but_preserve_handles() {
    assert_eq!(
        output(
            "$o=(object)[\"a\"=>[1],\"b\"=>new stdClass];$copy=clone $o;$copy->a[0]=2;$copy->b->x=3;var_dump($copy===$o,$o->a,$o->b->x);"
        ),
        b"bool(false)\narray(1) {\n  [0]=>\n  int(1)\n}\nint(3)\n"
    );
}

#[test]
fn casting_twice_creates_distinct_equal_objects() {
    assert_eq!(
        output(
            "$a=[\"x\"=>1];$o=(object)$a;$p=(object)$a;var_dump($o===$p,$o==$p,(bool)(object)null);"
        ),
        b"bool(false)\nbool(true)\nbool(true)\n"
    );
}

#[test]
fn object_dump_uses_string_names_for_numeric_keys() {
    assert_eq!(
        output("$o=(object)[0=>1,\"x\"=>2];var_dump($o);"),
        b"object(stdClass)#1 (2) {\n  [\"0\"]=>\n  int(1)\n  [\"x\"]=>\n  int(2)\n}\n"
    );
}

#[test]
fn mangled_keys_survive_roundtrip() {
    assert_eq!(
        output("$a=[\"\\0A\\0x\"=>7,\"\\0*\\0y\"=>8];$o=(object)$a;var_dump((array)$o===$a);"),
        b"bool(true)\n"
    );
}

#[test]
fn object_casts_are_rejected_in_class_initializers() {
    for code in [
        "class A{public $p=(object)[];}",
        "class A{public static $p=(object)[];}",
        "class A{const C=(object)[];}",
        "class A{public $p=[(object)[]];}",
        "class A{const C=false?(object)[]:null;}",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';{code}").as_bytes())
                .is_err(),
            "{code}"
        );
        assert!(rt.output.is_empty());
    }
}
#[test]
fn nan_warns_once_but_is_preserved_in_the_scalar_property() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$o=(object)NAN;var_dump($o->scalar);")
        .unwrap();
    assert_eq!(rt.output, b"float(NAN)\n");
    assert_eq!(rt.warnings.len(), 1);
    assert!(rt.warnings[0].contains("coerced to object"));
}

#[test]
fn encoded_property_names_are_decoded_for_iteration_and_dump() {
    assert_eq!(output("$o=(object)[\"\\0A\\0x\"=>7,\"\\0*\\0y\"=>8];var_dump($o);foreach($o as $k=>$v){var_dump($k,$v);}"),b"object(stdClass)#1 (2) {\n  [\"x\":\"A\":private]=>\n  int(7)\n  [\"y\":protected]=>\n  int(8)\n}\nstring(1) \"x\"\nint(7)\nstring(1) \"y\"\nint(8)\n");
}

#[test]
fn malformed_encoded_names_are_preserved_with_notices() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$o=(object)[\"\\0\"=>1];var_dump($o);foreach($o as $k=>$v){var_dump($k);}foreach($o as $v){}").unwrap();
    assert_eq!(rt.warnings.len(), 2);
    assert!(
        rt.warnings
            .iter()
            .all(|w| w.contains("Illegal member variable name"))
    );
    assert_eq!(
        rt.output,
        b"object(stdClass)#1 (1) {\n  [\"\0\"]=>\n  int(1)\n}\nstring(1) \"\0\"\n"
    );
}
