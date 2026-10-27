use php_rs::{ErrorKind, Runtime, run};

#[test]
fn constructor_and_methods_share_instance_state() {
    assert_eq!(run(b"<?php $c=new Counter(4);class Counter{public $n=0;function __construct($n){$this->n=$n;}function add($x=1){$this->n+=$x;return $this->n;}}echo $c->add(),$c->ADD(x:3),$c->n;").unwrap(),b"588");
}
#[test]
fn objects_share_handles_but_property_arrays_use_copy_on_write() {
    assert_eq!(run(b"<?php class Box{public $items=[];} $a=new Box;$b=$a;$c=new Box;$b->items[]=3;$copy=$a->items;$copy[]=4;echo count($a->items),count($copy),count($c->items);var_dump($a===$b,$a===$c,$a==$c);").unwrap(),b"120bool(true)\nbool(false)\nbool(false)\n");
}
#[test]
fn dynamic_names_nested_paths_and_unset() {
    assert_eq!(run(b"<?php class Box{public $child;public $n=1;public $a=[2];}$a=new Box;$a->child=new Box;$name='n';$a->child->$name++;$a->a[0]+=3;unset($a->child->n);echo $a->a[0],$a->child->n??7;var_dump(isset($a->child->n),empty($a->child->n));").unwrap(),b"57bool(false)\nbool(true)\n");
}
#[test]
fn bound_method_and_nested_closures_retain_this() {
    assert_eq!(run(b"<?php class Box{public $n=5;function get(){return $this->n;}function make(){return fn()=>$this->get();}}$b=new Box;$f=$b->get(...);$g=$b->make();unset($b);echo $f(),$g();").unwrap(),b"55");
}
#[test]
fn methods_use_distinct_static_state_and_restore_this() {
    assert_eq!(run(b"<?php class A{public $n=1;function get(){static $n=0;return ++$n;}function other($o){$o->get();return $this->n;}}class B{function get(){static $n=10;return ++$n;}}$a=new A;$b=new B;echo $a->get(),$b->get(),$a->other($b),$a->get();").unwrap(),b"11112");
}
#[test]
fn class_types_and_object_introspection() {
    assert_eq!(run(b"<?php class Box{} function f(Box $b):object{return $b;}$b=f(new Box);echo get_class($b),get_debug_type($b),gettype($b);var_dump(is_object($b),class_exists('bOx'),method_exists($b,'missing'));").unwrap(),b"BoxBoxobjectbool(true)\nbool(true)\nbool(false)\n");
}
#[test]
fn conditional_classes_and_dynamic_construction() {
    assert_eq!(run(b"<?php var_dump(class_exists('Later'));if(true){class Later{public $n=2;}}$name='later';echo (new $name)->n;").unwrap(),b"bool(false)\n2");
}
#[test]
fn stdclass_dynamic_properties_and_iteration() {
    assert_eq!(run(b"<?php $o=new stdClass;$o->x=1;$o->{'2'}=3;foreach($o as $k=>$v){echo $k,':',$v,';';}echo count((array)$o);").unwrap(),b"x:1;2:3;2");
}
#[test]
fn missing_property_warns_and_dynamic_creation_is_deprecated() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class Box{}$b=new Box;echo $b->x;$b->x=3;$b->x=4;")
        .unwrap();
    assert_eq!(rt.warnings.len(), 2);
    assert!(rt.warnings[0].contains("Undefined property: Box::$x"));
    assert!(rt.warnings[1].contains("Creation of dynamic property Box::$x"));
}
#[test]
fn invalid_object_operations_return_php_errors() {
    for code in [
        "new Missing;",
        "class A{}(new A)->missing();",
        "$a=null;$a->x=1;",
        "class A{}echo new A;",
    ] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::Error,
            "{code}"
        );
    }
}
#[test]
fn this_cannot_be_reassigned_and_static_closures_do_not_bind_it() {
    assert!(php_rs::lint(b"<?php class A{function f(){$this=3;}}").is_err());
    assert_eq!(run(b"<?php class A{function f(){return static fn()=>isset($this);}}$f=(new A)->f();var_dump($f());").unwrap(),b"bool(false)\n");
}

#[test]
fn unset_scalar_properties_is_silent() {
    let mut rt = Runtime::default();
    rt.execute_code(
        b"foreach([null,false,true,1,1.5,'x',[],fn()=>1] as $a){unset($a->x);unset($a->x[0]);}",
    )
    .unwrap();
    assert!(rt.warnings.is_empty());
}
#[test]
fn restored_properties_preserve_declaration_order() {
    assert_eq!(run(b"<?php class A{public $x=1,$y=2;}$a=new A;unset($a->x);$a->x=3;foreach($a as $k=>$v){echo $k,$v;}").unwrap(),b"x3y2");
}
#[test]
fn object_iteration_observes_mutations_and_new_properties() {
    assert_eq!(run(b"<?php $o=new stdClass;$o->a=1;$o->b=2;foreach($o as $k=>$v){echo $k,$v;if($k==='a'){$o->b=3;$o->c=4;}}").unwrap(),b"a1b3c4");
}
#[test]
fn recursive_objects_dump_safely_and_comparison_returns_error() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$a=new stdClass;$a->self=$a;var_dump($a);$b=new stdClass;$b->self=$b;")
        .unwrap();
    assert!(String::from_utf8_lossy(&rt.output).contains("*RECURSION*"));
    assert_eq!(
        rt.execute_code(b"var_dump($a==$b);").unwrap_err().kind,
        ErrorKind::Error
    );
}
#[test]
fn this_requires_an_object_context_and_dynamic_assignment_is_rejected() {
    for code in [
        "echo $this;",
        "class A{function f(){$n='this';$$n=3;}}(new A)->f();",
    ] {
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::Error
        );
    }
}
#[test]
fn method_callbacks_and_invokable_objects_work_with_native_functions() {
    assert_eq!(run(b"<?php class A{public $n=1;function add($v){return $this->n+=$v;}function __invoke($v){return $this->add($v);}}$a=new A;echo array_map([$a,'ADD'],[2,3])[1],array_map($a,[4])[0];").unwrap(),b"610");
}

#[test]
fn method_argument_types_and_errors_preserve_caller_scope() {
    let mut rt = Runtime::default();
    rt.execute_code(b"class A{function f(int $x):int{return $x;}}$a=new A;echo $a->f('3');")
        .unwrap();
    assert_eq!(
        rt.execute_code(b"declare(strict_types=1);$a->f('3');")
            .unwrap_err()
            .kind,
        ErrorKind::TypeError
    );
    rt.execute_code(b"var_dump(isset($this));echo $a->f(4);")
        .unwrap();
    assert_eq!(rt.output, b"3bool(false)\n4");
}
#[test]
fn dynamic_class_designators_and_method_magic_constants() {
    assert_eq!(run(b"<?php class A{function f(){echo __CLASS__,':',__FUNCTION__,':',__METHOD__;function inner(){echo '|',__CLASS__,':',__METHOD__;}inner();}}$names=['A'];(new $names[0])->f();").unwrap(),b"A:f:A::f|:inner");
}
#[test]
fn invalid_class_and_member_declarations_fail_before_execution() {
    for code in [
        "class int{}",
        "class if{}",
        "class A{public $x;public $x;}",
        "class A{function f(){}function F(){}}",
        "class A{function __construct():void{}}",
        "class A{function f($this){}}",
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
fn nul_prefixed_properties_reject_access_but_isset_is_silent() {
    for op in ["echo $o->{$k};", "unset($o->{$k});", "$o->{$k}=2;"] {
        let code = format!("$o=new stdClass;$k=\"\\0bad\";{op}");
        assert_eq!(
            Runtime::default()
                .execute_code(code.as_bytes())
                .unwrap_err()
                .kind,
            ErrorKind::Error
        );
    }
    assert_eq!(
        run(b"<?php $o=new stdClass;$k=\"\\0bad\";var_dump(isset($o->{$k}));").unwrap(),
        b"bool(false)\n"
    );
}
