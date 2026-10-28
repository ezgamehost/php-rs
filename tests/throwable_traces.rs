use php_rs::Runtime;

fn output(code: &str) -> Vec<u8> {
    let mut runtime = Runtime::default();
    runtime.execute_code(code.as_bytes()).unwrap();
    runtime.output
}

#[test]
fn traces_capture_live_arguments_and_call_sites_at_creation() {
    assert_eq!(
        output(
            "function inner($x){$x=7;return new Exception;}\nfunction outer($a){$a=9;return inner(1);}\n$e=outer(2);echo $e->getTraceAsString();"
        ),
        b"#0 Command line code(2): inner(7)\n#1 Command line code(3): outer(9)\n#2 {main}"
    );
}

#[test]
fn trace_arrays_are_snapshots_and_rethrow_keeps_original_trace() {
    assert_eq!(
        output(
            "function f($x){$e=new Exception;$x[0]=9;return $e;}$e=f([1]);$t=$e->getTrace();$t[0]['args'][0][0]=8;try{throw $e;}catch(Throwable $e){var_dump($e->getTrace()[0]['args'][0][0]);}var_dump((new Exception)->getTrace());"
        ),
        b"int(1)\narray(0) {\n}\n"
    );
}

#[test]
fn methods_and_internal_callbacks_keep_frame_identity() {
    assert_eq!(output("class A{function f($x){return new Exception;}}class B extends A{}$e=array_map([new B,'f'],[3])[0];echo $e->getTraceAsString();"), b"#0 [internal function]: A->f(3)\n#1 Command line code(1): array_map(Array, Array)\n#2 {main}");
}

#[test]
fn type_errors_capture_rejected_arguments_and_restore_stack() {
    assert_eq!(
        output(
            "function f(int $x){}try{f([]);}catch(Throwable $e){echo $e->getTraceAsString();}echo '|',(new Exception)->getTraceAsString();"
        ),
        b"#0 Command line code(1): f(Array)\n#1 {main}|#0 {main}"
    );
}

#[test]
fn named_variadics_defaults_unset_and_extra_arguments_are_preserved() {
    assert_eq!(output("function f($a=3,$b=4,...$rest){unset($a);$rest=[];return new Exception;}echo f(b:9,z:5)->getTraceAsString();function g($a){return new Exception;}echo '|',g(1,2)->getTraceAsString();"), b"#0 Command line code(1): f(NULL, 9, z: 5)\n#1 {main}|#0 Command line code(1): g(1, 2)\n#1 {main}");
}

#[test]
fn eval_frames_are_present_only_after_successful_compilation() {
    assert_eq!(
        output(
            "function f(){return eval('return new Exception;');}echo f()->getTraceAsString();try{eval('%%%');}catch(Throwable $e){echo '|',$e->getTraceAsString();}"
        ),
        b"#0 Command line code(1): eval()\n#1 Command line code(1): f()\n#2 {main}|#0 {main}"
    );
}

#[test]
fn trace_strings_format_values_without_converting_objects() {
    assert_eq!(output("function f(...$args){return new Exception;}echo f('abcdefghijklmnop',true,false,null,[],new stdClass,1.0)->getTraceAsString();"), b"#0 Command line code(1): f('abcdefghijklmno...', true, false, NULL, Array, Object(stdClass), 1.0)\n#1 {main}");
}

#[test]
fn nested_closures_use_lexical_names_and_class_context() {
    assert_eq!(
        output(
            "class A{function f(){return fn()=>fn()=>new Exception;}}$f=(new A)->f();echo $f()()->getTraceAsString();"
        ),
        b"#0 Command line code(1): A->{closure:{closure:A::f():1}:1}()\n#1 {main}"
    );
}

#[test]
fn direct_optimized_calls_and_dynamic_builtins_have_distinct_traces() {
    assert_eq!(output("try{strlen([]);}catch(Throwable $e){echo $e->getTraceAsString();}$f='strlen';try{$f([]);}catch(Throwable $e){echo '|',$e->getTraceAsString();}try{func_get_args();}catch(Throwable $e){echo '|',$e->getTraceAsString();}"), b"#0 {main}|#0 Command line code(1): strlen(Array)\n#1 {main}|#0 Command line code(1): func_get_args()\n#1 {main}");
}

#[test]
fn builtin_aliases_and_named_calls_retain_real_call_frames() {
    assert_eq!(output("try{key_exists(1,1);}catch(Throwable $e){echo $e->getTraceAsString();}try{strlen(string:[]);}catch(Throwable $e){echo '|',$e->getTraceAsString();}"), b"#0 Command line code(1): key_exists(1, 1)\n#1 {main}|#0 Command line code(1): strlen(Array)\n#1 {main}");
}

#[test]
fn included_files_capture_the_include_site_and_canonical_filename() {
    let path = std::env::temp_dir().join(format!("php-rs-trace-{}.php", std::process::id()));
    std::fs::write(&path, b"<?php $x=7;return new Exception;").unwrap();
    let path = path.canonicalize().unwrap();
    let code = format!(
        "function f($x){{return include '{}';}}$e=f(1);$t=$e->getTrace();var_dump($t[0]['function'],$t[0]['file'],$t[0]['line'],$t[0]['args'][0] === $e->getFile(),$t[1]['args'][0]);",
        path.display()
    );
    let actual = output(&code);
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        actual,
        b"string(7) \"include\"\nstring(17) \"Command line code\"\nint(1)\nbool(true)\nint(7)\n"
    );
}

#[test]
fn constructors_and_rejected_native_methods_capture_correct_frames() {
    assert_eq!(output("class E extends Exception{function __construct(){parent::__construct('x');}}function f(){return new E;}echo f()->getTraceAsString();try{(new Exception)->getTrace(1);}catch(Throwable $e){echo '|',$e->getTraceAsString();}"), b"#0 Command line code(1): f()\n#1 {main}|#0 Command line code(1): Exception->getTrace(1)\n#1 {main}");
}
