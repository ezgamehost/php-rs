use php_rs::Runtime;

fn output(code: &str) -> Vec<u8> {
    let mut rt = Runtime::default();
    rt.execute_code(code.as_bytes()).unwrap();
    rt.output
}

#[test]
fn basic_properties_methods_and_nested_chains() {
    assert_eq!(
        output(
            "class A{public $x=7;function next(){return $this;}function none(){return null;}}$a=new A;var_dump(null?->x,null?->f(),$a?->x,$a?->next()->x,$a->none()?->x);"
        ),
        b"NULL\nNULL\nint(7)\nint(7)\nNULL\n"
    );
}

#[test]
fn short_circuit_skips_names_arguments_and_offsets() {
    assert_eq!(
        output(
            "function side(){echo \"bad\";return \"x\";}$n=null;var_dump($n?->{side()}(side())->x[side()],$n?->x[side()]->{side()},$n?->f(...side()),$n?->f(x:side()));"
        ),
        b"NULL\nNULL\nNULL\nNULL\n"
    );
}

#[test]
fn parentheses_preserve_member_chains() {
    assert_eq!(
        output(
            "$n=null;var_dump(($n?->x)->y,($n?->x)[print \"bad\"],($n?->x)->f(print \"bad\"),($n?->x)::$p,($n?->x)::f());"
        ),
        b"NULL\nNULL\nNULL\nNULL\nNULL\n"
    );
}

#[test]
fn function_calls_and_class_constants_end_chains() {
    assert_eq!(
        output(
            "foreach([1,2,3,4] as $i){try{if($i===1)(null?->x)();if($i===2)null?->f()();if($i===3)null?->x::C;if($i===4)(null?->x)::class;}catch(Error){echo \"caught\";}}"
        ),
        b"caughtcaughtcaughtcaught"
    );
}

#[test]
fn subchains_do_not_cancel_the_enclosing_operation() {
    assert_eq!(
        output(
            "class A{function f($v){var_dump($v);return $this;}public $x=9;}$a=new A;$n=null;var_dump($a?->f($n?->x)->x);$arr=[\"\"=>7];var_dump($arr[$n?->x]);"
        ),
        b"NULL\nint(9)\nint(7)\n"
    );
}

#[test]
fn static_members_after_nullsafe_calls() {
    assert_eq!(
        output(
            "class A{public static $s=8;static function f(){return 9;}function next(){return $this;}}$a=new A;var_dump($a?->next()::$s,$a?->next()::f(),null?->next()::$s,null?->next()::f());"
        ),
        b"int(8)\nint(9)\nNULL\nNULL\n"
    );
}

#[test]
fn isset_empty_and_coalescing_propagate_quiet_reads() {
    assert_eq!(
        output(
            "$n=null;var_dump(isset($n?->p),isset($n?->f()->p),empty($n?->p),$n?->p??7,$missing?->f()??9,isset($missing?->f()[0]));"
        ),
        b"bool(false)\nbool(false)\nbool(true)\nint(7)\nint(9)\nbool(false)\n"
    );
}

#[test]
fn null_return_is_distinct_from_a_short_circuit() {
    assert_eq!(
        output(
            "class A{function none(){return null;}}$a=new A;try{$a?->none()->f();}catch(Error){echo \"caught\";}var_dump($a?->none()?->f());"
        ),
        b"caughtNULL\n"
    );
}

#[test]
fn nullsafe_access_captures_arrow_variables() {
    assert_eq!(
        output(
            "class A{public $p=8;}$a=new A;$n=null;$f=fn()=>$a?->p;$g=fn()=>$n?->p;var_dump($f(),$g());"
        ),
        b"int(8)\nNULL\n"
    );
}

#[test]
fn dynamic_names_are_evaluated_once() {
    assert_eq!(
        output(
            "function name(){echo \"name;\";return \"f\";}class A{function f(){echo \"call;\";return $this;}public $p=3;}$a=new A;var_dump($a?->{name()}()->p);"
        ),
        b"name;call;int(3)\n"
    );
}

#[test]
fn nullsafe_interpolation() {
    assert_eq!(
        output("$a=new stdClass;$a->p=\"yes\";$n=null;echo \"[{$a?->p}][{$n?->p}]\";"),
        b"[yes][]"
    );
}

#[test]
fn keys_and_names_can_contain_independent_nullsafe_chains() {
    assert_eq!(
        output(
            "$n=null;$a=new stdClass;$a->{\"\"}=8;var_dump($a->{$n?->p});$arr=[\"\"=>1];$arr[$n?->p]=2;var_dump($arr);"
        ),
        b"int(8)\narray(1) {\n  [\"\"]=>\n  int(2)\n}\n"
    );
}

#[test]
fn non_null_scalars_still_report_method_errors() {
    assert_eq!(
        output(
            "foreach([false,true,0,1.5,\"\",[]] as $x){try{$x?->f();}catch(Error){echo \"error;\";}}"
        ),
        b"error;error;error;error;error;error;"
    );
}

#[test]
fn nullsafe_write_targets_are_rejected_before_execution() {
    for code in [
        "$a?->x=1;",
        "$a?->x->y=1;",
        "$a?->f()->x=1;",
        "$a?->x[0]=1;",
        "$a?->x+=1;",
        "$a?->x??=1;",
        "++$a?->x;",
        "$a?->x++;",
        "unset($a?->x);",
        "foreach([1] as $a?->x){}",
        "[$a?->x]=[1];",
        "$a?->f(...);",
        "($a?->x)->f(...);",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';{code}").as_bytes())
                .is_err(),
            "{code}"
        );
        assert!(rt.output.is_empty(), "{code}");
    }
}
#[test]
fn nullsafe_only_suppresses_null_receiver_warnings() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$missing?->p; $n=null;$n?->p->q; isset($another?->p); empty($third?->f());")
        .unwrap();
    assert_eq!(rt.warnings.len(), 2);
    assert!(rt.warnings[0].contains("missing"));
    assert!(rt.warnings[1].contains("third"));
}

#[test]
fn static_callable_creation_propagates_short_circuit() {
    assert_eq!(
        output(
            "class A{static function f(){return 8;}function next(){return $this;}}$a=new A;$n=null;var_dump($n?->p::f(...));$f=$a?->next()::f(...);var_dump($f());"
        ),
        b"NULL\nint(8)\n"
    );
}

#[test]
fn static_access_keeps_receiver_warnings_in_quiet_contexts() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$missing?->f()::$s??1; $other?->f()::g()??1; empty(($third?->f()));")
        .unwrap();
    assert_eq!(rt.warnings.len(), 3);
}

#[test]
fn nullsafe_dynamic_method_names_require_strings() {
    assert_eq!(
        output(
            "class A{}$a=new A;$n=null;try{$a?->{$n?->p}();}catch(Error $e){echo $e->getMessage();}try{A::{$n?->p}();}catch(Error $e){echo $e->getMessage();}var_dump($n?->{$n?->p}());"
        ),
        b"Method name must be a stringMethod name must be a stringNULL\n"
    );
}

#[test]
fn boolean_receivers_are_identified_in_method_errors() {
    assert_eq!(
        output(
            "foreach([false,true] as $v){try{$v?->f();}catch(Error $e){echo $e->getMessage(),\";\";}}"
        ),
        b"Call to a member function f() on false;Call to a member function f() on true;"
    );
}

#[test]
fn static_property_probes_suppress_undefined_receivers() {
    let mut rt = Runtime::default();
    rt.execute_code(b"var_dump(empty($missing?->f()::$s),isset($other?->f()::$s));")
        .unwrap();
    assert_eq!(rt.output, b"bool(true)\nbool(false)\n");
    assert!(rt.warnings.is_empty());
}

#[test]
fn simple_interpolation_supports_nullsafe_properties() {
    assert_eq!(
        output("class A{public $p='yes';}$a=new A;$n=null;echo \"[$a?->p][$n?->p][$a->p]\";"),
        b"[yes][][yes]"
    );
}

#[test]
fn nullsafe_properties_work_in_constant_expressions() {
    assert_eq!(
        output(
            "const C=null?->p->q[0];class A{const C=null?->p;public $p=null?->x;}function f($x=null?->p){var_dump($x);}var_dump(C,A::C,(new A)->p);f();"
        ),
        b"NULL\nNULL\nNULL\nNULL\n"
    );
}

#[test]
fn constant_invalid_method_names_fail_unless_the_branch_is_pruned() {
    for code in [
        "null?->{null}();",
        "null?->{1+1}();",
        "null?->f()::{[]}();",
        "if(false){null?->{true}();}",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';{code}").as_bytes())
                .is_err()
        );
        assert!(rt.output.is_empty());
    }
    assert_eq!(output("false && null?->{null}();echo 'ok';"), b"ok");
}

#[test]
fn quiet_property_probes_propagate_through_static_calls() {
    let mut rt = Runtime::default();
    rt.execute_code(b"var_dump(empty($missing?->f()::g()->p));")
        .unwrap();
    assert!(rt.warnings.is_empty());
    assert_eq!(rt.output, b"bool(true)\n");
}
