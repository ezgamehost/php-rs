use php_rs::{ErrorKind, Runtime, run};

#[test]
fn clones_copy_property_storage_but_share_nested_objects() {
    assert_eq!(run(b"<?php class A{public $x=1;public $a=[1];public $o;} $a=new A;$a->o=new stdClass;$b=clone $a;$b->x=2;$b->a[]=3;$b->o->x=4;echo $a->x,$b->x,count($a->a),count($b->a),$a->o->x;var_dump($a===$b,$a->o===$b->o);").unwrap(), b"12124bool(false)\nbool(true)\n");
}
#[test]
fn clone_method_runs_on_copy_and_inherits_declaring_scope() {
    assert_eq!(run(b"<?php class A{public $x=1;function __clone(){echo self::class,static::class;$this->x++;}}class B extends A{}$a=new B;$b=clone $a;echo $a->x,$b->x;").unwrap(),b"AB12");
}
#[test]
fn constructor_is_not_repeated_and_uninitialized_properties_stay_absent() {
    assert_eq!(run(b"<?php class A{public int $x;function __construct(){echo 'new';}}$a=new A;$b=clone $a;var_dump(isset($b->x),property_exists($b,'x'));").unwrap(), b"newbool(false)\nbool(true)\n");
}
#[test]
fn readonly_properties_reinitialize_once_during_clone_method() {
    assert_eq!(run(b"<?php class A{function __construct(public readonly int $x=1){}function __clone(){$this->x++;}}$a=new A;$b=clone $a;echo $a->x,$b->x;").unwrap(),b"12");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{function __construct(public readonly int $x=1){}function __clone(){$this->x=2;$this->x=3;}}$a=new A;").unwrap();
    assert_eq!(
        rt.execute_code(b"$b=clone $a;").unwrap_err().kind,
        ErrorKind::Error
    );
    rt.execute_code(b"echo $a->x;").unwrap();
    assert_eq!(rt.output, b"1");
}
#[test]
fn clone_reinitialization_ends_on_return_and_on_error() {
    for body in [
        "global $saved;$saved=$this;",
        "global $saved;$saved=$this;missing();",
    ] {
        let mut rt = Runtime::default();
        let code = format!(
            "class A{{function __construct(public readonly int $x=1){{}}function __clone(){{{body}}}function set(){{$this->x=2;}}}}$a=new A;"
        );
        rt.execute_code(code.as_bytes()).unwrap();
        let _ = rt.execute_code(b"$b=clone $a;");
        assert_eq!(
            rt.execute_code(b"$saved->set();").unwrap_err().kind,
            ErrorKind::Error
        );
    }
}
#[test]
fn readonly_clone_supports_unset_then_reinitialization_and_nested_cloning() {
    assert_eq!(run(b"<?php class A{function __construct(public readonly object $x){}function __clone(){unset($this->x);$this->x=new stdClass;}}$a=new A(new stdClass);$b=clone $a;var_dump($a->x===$b->x);").unwrap(),b"bool(false)\n");
    assert_eq!(run(b"<?php class A{function __construct(public readonly object $x){}function __clone(){$this->x=clone $this->x;}}$a=new A(new stdClass);$b=clone $a;var_dump($a->x===$b->x);").unwrap(),b"bool(false)\n");
}
#[test]
fn clone_with_updates_after_magic_method_in_array_order() {
    assert_eq!(run(b"<?php class A{public int $x=1;public $y;function __clone(){echo $this->x;$this->x=2;}}$a=new A;$b=clone($a,['x'=>'3','y'=>4]);echo $a->x,$b->x,$b->y;").unwrap(),b"1134");
}
#[test]
fn clone_with_readonly_updates_uses_caller_scope() {
    assert_eq!(run(b"<?php readonly class A{function __construct(public int $x=1){}function __clone(){$this->x=2;}function with(){return clone($this,['x'=>3]);}}$a=new A;$b=$a->with();echo $a->x,$b->x;").unwrap(),b"13");
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{function __construct(public readonly int $x=1){}}$a=new A;")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"clone($a,['x'=>2]);").unwrap_err().kind,
        ErrorKind::Error
    );
}
#[test]
fn clone_with_untouched_readonly_slots_match_php_850_reinitialization_flags() {
    assert_eq!(run(b"<?php class A{function __construct(public readonly int $x=1,public readonly int $y=2){}function with(){return clone($this,['x'=>3]);}function set(){$this->y=4;}}$a=(new A)->with();$a->set();echo $a->x,$a->y;").unwrap(), b"34");
}
#[test]
fn cloning_supports_function_call_forms_and_operator_precedence() {
    assert_eq!(run(b"<?php $a=new stdClass;$a->x=new stdClass;$b=clone($a)->x;$c=(clone($a,))->x;var_dump($b===$a->x,$c===$a->x);$f=clone(...);$g='clone';$d=$f(object:$a,withProperties:['y'=>2]);$e=$g(...[$a,['y'=>3]]);echo $d->y,$e->y;var_dump(function_exists('clone'));").unwrap(), b"bool(false)\nbool(true)\n23bool(true)\n");
}
#[test]
fn clone_with_argument_validation_precedes_magic_method() {
    for expr in ["clone($a,null)", "clone($a,1)", "clone(1)", "clone()"] {
        let mut rt = Runtime::default();
        rt.execute_code(b"class A{function __clone(){echo 'clone';}}$a=new A;")
            .unwrap();
        assert!(rt.execute_code(format!("{expr};").as_bytes()).is_err());
        assert!(rt.output.is_empty());
    }
}
#[test]
fn cloned_closures_copy_static_values_and_keep_reference_captures() {
    assert_eq!(run(b"<?php $n=0;$f=function()use(&$n){static $i=0;echo ++$i,++$n;};$f();$g=clone $f;$f();$g();var_dump($f===$g);").unwrap(), b"112223bool(false)\n");
    assert_eq!(
        Runtime::default()
            .execute_code(b"$f=fn()=>1;clone($f,['x'=>2]);")
            .unwrap_err()
            .kind,
        ErrorKind::Error
    );
}
#[test]
fn clone_with_failed_property_write_preserves_earlier_updates_on_escaped_copy() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{public int $x=1;public int $y=2;function __clone(){global $saved;$saved=$this;}}$a=new A;").unwrap();
    assert_eq!(
        rt.execute_code(b"clone($a,['x'=>3,'y'=>'bad']);")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    rt.execute_code(b"echo $a->x,$a->y,$saved->x,$saved->y;")
        .unwrap();
    assert_eq!(rt.output, b"1232");
}
#[test]
fn magic_clone_signatures_are_validated_before_execution() {
    for declaration in [
        "function __clone($x){}",
        "function __clone():int{return 1;}",
        "static function __clone(){}",
    ] {
        let mut rt = Runtime::default();
        assert!(
            rt.execute_code(format!("echo 'bad';class A{{{declaration}}}").as_bytes())
                .is_err()
        );
        assert!(rt.output.is_empty());
    }
}

#[test]
fn clone_with_uses_internal_weak_coercion_in_strict_callers() {
    assert_eq!(run(b"<?php declare(strict_types=1);class A{public int $x;}$a=new A;$b=clone($a,['x'=>'2']);var_dump($b->x);").unwrap(),b"int(2)\n");
    let mut rt = Runtime::default();
    rt.execute_code(b"declare(strict_types=1);class A{public int $x;}$a=new A;")
        .unwrap();
    assert!(
        rt.execute_code(b"declare(strict_types=1);clone($a,['x'=>[]]);")
            .is_err()
    );
    assert!(
        rt.execute_code(b"declare(strict_types=1);$a->x='2';")
            .is_err()
    );
}
#[test]
fn clone_call_dereferencing_requires_parentheses() {
    for expression in [
        "clone($a,)->x",
        "clone($a,[])->x",
        "clone(object:$a)->x",
        "clone(...)(new stdClass)",
    ] {
        assert!(
            Runtime::default()
                .execute_code(format!("$a=new stdClass;{expression};").as_bytes())
                .is_err()
        );
    }
    assert_eq!(run(b"<?php class A{function __clone(...$args):void{echo count($args);}}$a=new A;$b=clone $a;").unwrap(),b"0");
    assert_eq!(
        run(b"<?php class A{function __clone():never{exit(0);}}echo 'ok';").unwrap(),
        b"ok"
    );
}

#[test]
fn clone_operands_accept_assignments_and_repeated_clone_operators() {
    assert_eq!(run(b"<?php $a=clone clone $b=new stdClass;var_dump($a==$b,$a===$b);$c=clone $d??=new stdClass;var_dump($c==$d,$c===$d);").unwrap(),b"bool(true)\nbool(false)\nbool(true)\nbool(false)\n");
    assert_eq!(
        run(b"<?php var_dump(1+$a=2, -$b=3, !$c=4, $a,$b,$c);").unwrap(),
        b"int(3)\nint(-3)\nbool(false)\nint(2)\nint(3)\nint(4)\n"
    );
}
