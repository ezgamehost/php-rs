use php_rs::Runtime;

fn output(code: &str) -> Vec<u8> {
    let mut runtime = Runtime::default();
    runtime.execute_code(code.as_bytes()).unwrap();
    runtime.output
}

#[test]
fn objects_convert_in_output_interpolation_casts_and_concatenation() {
    assert_eq!(
        output(
            "class A{function __toString(){return 'ok';}}$a=new A;echo $a;print $a;echo \"[$a]\",(string)$a,'x'.$a;$s='y';$s.=$a;echo $s;"
        ),
        b"okok[ok]okxokyok"
    );
}

#[test]
fn implicit_stringable_membership_and_contracts_are_available() {
    assert_eq!(
        output(
            "class A{function __toString(){return 'a';}}class B extends A{}function f(Stringable $x){echo $x;}f(new B);var_dump(new B instanceof Stringable,new Exception instanceof Stringable,interface_exists('Stringable'));"
        ),
        b"abool(true)\nbool(true)\nbool(true)\n"
    );
    assert!(
        Runtime::default()
            .execute_code(b"class A implements Stringable{}")
            .is_err()
    );
}

#[test]
fn magic_return_values_use_the_method_strictness() {
    assert_eq!(
        output(
            "class A{function __toString(){return 12;}}var_dump((string)new A,(new A)->__toString());"
        ),
        b"string(2) \"12\"\nstring(2) \"12\"\n"
    );
    assert_eq!(
        output(
            "declare(strict_types=1);class A{function __toString(){return 12;}}try{echo new A;}catch(TypeError $e){echo $e->getMessage();}"
        ),
        b"A::__toString(): Return value must be of type string, int returned"
    );
}

#[test]
fn nested_stringable_return_values_and_inheritance_work() {
    assert_eq!(
        output(
            "class A{function __toString(){return new B;}}class B{function __toString(){return 'b';}}class C extends A{}echo new C;"
        ),
        b"b"
    );
}

#[test]
fn conversion_exceptions_preserve_frames_and_unwind_state() {
    assert_eq!(output("class A{function __toString(){throw new Exception('x');}}function f(string $s){}try{f(new A);}catch(Throwable $e){echo $e->getTraceAsString();}echo '|',(new Exception)->getTraceAsString();"), b"#0 Command line code(1): A->__toString()\n#1 Command line code(1): f(Object(A))\n#2 {main}|#0 {main}");
}

#[test]
fn weak_string_arguments_returns_properties_and_builtins_convert() {
    assert_eq!(
        output(
            "class A{function __toString(){return 'abc';}}class B{public string $x;public static string $y;}function f(string $x):string{return new A;}$a=new A;$b=new B;$b->x=$a;B::$y=$a;echo f($a),$b->x,B::$y,strlen($a);"
        ),
        b"abcabcabc3"
    );
}

#[test]
fn strict_string_parameters_reject_stringable_objects_without_conversion() {
    assert_eq!(
        output(
            "declare(strict_types=1);class A{function __toString(){echo 'bad';return 'x';}}function f(string $x){}try{f(new A);}catch(TypeError){echo 'f';}try{strlen(new A);}catch(TypeError){echo 'b';}echo (string)new A;"
        ),
        b"fbbadx"
    );
}

#[test]
fn throwable_strings_include_message_origin_and_saved_trace() {
    assert_eq!(output("function f(){return new Exception('oops');}$e=f();echo $e;"), b"Exception: oops in Command line code:1\nStack trace:\n#0 Command line code(1): f()\n#1 {main}");
    assert_eq!(
        output("echo new Error;"),
        b"Error in Command line code:1\nStack trace:\n#0 {main}"
    );
}

#[test]
fn throwable_chains_format_oldest_first_and_ignore_previous_overrides() {
    assert_eq!(output("class E extends Exception{function __toString(){return 'override';}}$a=new E('a');$b=new Exception('b',0,$a);echo $a,'|',$b;"), b"override|E: a in Command line code:1\nStack trace:\n#0 {main}\n\nNext Exception: b in Command line code:1\nStack trace:\n#0 {main}");
}

#[test]
fn throwable_cycles_terminate_and_parent_stringification_works() {
    assert_eq!(
        output(
            "class E extends Exception{function __toString(){return '['.parent::__toString().']';}}$e=new E('a');$e->__construct('b',0,$e);echo $e;"
        ),
        b"[E: b in Command line code:1\nStack trace:\n#0 {main}]"
    );
}

#[test]
fn magic_string_signatures_are_validated() {
    for declaration in [
        "function __toString($x=1){return 'x';}",
        "function __toString():int{return 1;}",
        "function __toString():?string{return 'x';}",
    ] {
        assert!(
            Runtime::default()
                .execute_code(format!("class A{{{declaration}}}").as_bytes())
                .is_err()
        );
    }
    assert_eq!(
        output("class A{function __toString(...$args){return 'x';}}echo new A;"),
        b"x"
    );
}

#[test]
fn comparison_with_strings_invokes_conversion_but_strict_identity_does_not() {
    assert_eq!(
        output(
            "class A{function __toString(){return 'abc';}}$a=new A;var_dump($a=='abc','abc'==$a,$a<'b','b'>$a,$a==='abc');"
        ),
        b"bool(true)\nbool(true)\nbool(true)\nbool(true)\nbool(false)\n"
    );
}

#[test]
fn string_union_acceptance_preserves_objects_before_considering_coercion() {
    assert_eq!(
        output(
            "class B{}class A extends B{function __toString(){echo 'bad';return 'x';}function f(self|string $x,parent|string $y){var_dump($x===$this,$y===$this);}}$a=new A;$a->f($a,$a);"
        ),
        b"bool(true)\nbool(true)\n"
    );
}

#[test]
fn switch_array_search_and_nested_comparison_use_string_conversion() {
    assert_eq!(
        output(
            "class A{function __toString(){return 'x';}}$a=new A;switch($a){case 'x':echo 's';}var_dump(array_keys([$a,'y'],'x'),[$a]==['x']);$b=new stdClass;$b->x=$a;$c=new stdClass;$c->x='x';var_dump($b==$c);"
        ),
        b"sarray(1) {\n  [0]=>\n  int(0)\n}\nbool(true)\nbool(true)\n"
    );
}

#[test]
fn callable_string_union_keeps_an_invokable_object() {
    assert_eq!(
        output(
            "class A{function __invoke(){}function __toString(){echo 'bad';return 'x';}}function f(callable|string $x){var_dump($x instanceof A);}f(new A);"
        ),
        b"bool(true)\n"
    );
}

#[test]
fn return_conversion_failures_chain_the_original_exception_under_type_error() {
    assert_eq!(output("class A{function __toString(){throw new Exception('inner');}}\nfunction f():string{\nreturn new A;\n}\ntry{f();}catch(Throwable $e){echo get_class($e),':',$e->getMessage(),':',$e->getLine(),'|',$e->getTraceAsString(),'|',$e->getPrevious()->getMessage(),'|',$e->getPrevious()->getTraceAsString();}"), b"TypeError:f(): Return value must be of type string, A returned:3|#0 Command line code(5): f()\n#1 {main}|inner|#0 Command line code(3): A->__toString()\n#1 Command line code(5): f()\n#2 {main}");
}

#[test]
fn argument_conversion_uses_the_callee_declaration_site_or_internal_frame() {
    assert_eq!(output("class A{function __toString(){throw new Exception;}}\nfunction f(string $x){}\ntry{f(new A);}catch(Throwable $e){echo $e->getTraceAsString();}\ntry{$f='strlen';$f(new A);}catch(Throwable $e){echo '|',$e->getTraceAsString();}"), b"#0 Command line code(2): A->__toString()\n#1 Command line code(3): f(Object(A))\n#2 {main}|#0 [internal function]: A->__toString()\n#1 Command line code(4): strlen(Object(A))\n#2 {main}");
}
