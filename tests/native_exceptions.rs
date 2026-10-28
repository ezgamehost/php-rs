use php_rs::{ErrorKind, Runtime};

fn output(code: &str) -> Vec<u8> {
    let mut rt = Runtime::default();
    rt.execute_code(code.as_bytes()).unwrap();
    rt.output
}

#[test]
fn error_exception_defaults_capture_creation_location() {
    assert_eq!(
        output(
            "$e=new ErrorException;var_dump($e->getMessage(),$e->getCode(),$e->getSeverity(),$e->getFile()===__FILE__,$e->getLine()===__LINE__,$e->getPrevious());"
        ),
        b"string(0) \"\"\nint(0)\nint(1)\nbool(true)\nbool(true)\nNULL\n"
    );
}

#[test]
fn error_exception_named_arguments_and_previous_identity() {
    assert_eq!(output("$p=new Error(\"prior\");$e=new ErrorException(previous:$p,line:-9,filename:\"virtual.php\",severity:512,code:42,message:\"broken\");var_dump($e->getMessage(),$e->getCode(),$e->getSeverity(),$e->getFile(),$e->getLine(),$e->getPrevious()===$p);"), b"string(6) \"broken\"\nint(42)\nint(512)\nstring(11) \"virtual.php\"\nint(-9)\nbool(true)\n");
}

#[test]
fn error_exception_filename_and_signed_line_rules() {
    assert_eq!(output("foreach([new ErrorException(filename:\"\"),new ErrorException(filename:\"x\"),new ErrorException(line:0),new ErrorException(line:-5),new ErrorException(filename:null,line:null)] as $e){var_dump($e->getFile(),$e->getLine());}"), b"string(0) \"\"\nint(0)\nstring(1) \"x\"\nint(0)\nstring(17) \"Command line code\"\nint(0)\nstring(17) \"Command line code\"\nint(-5)\nstring(17) \"Command line code\"\nint(1)\n");
}

#[test]
fn repeated_construction_preserves_optional_state() {
    assert_eq!(output("$e=new ErrorException(\"first\",7,9,\"custom\",-5,new Exception(\"old\"));$e->__construct();var_dump($e->getMessage(),$e->getCode(),$e->getSeverity(),$e->getFile(),$e->getLine(),$e->getPrevious()->getMessage());$e->__construct(previous:null);var_dump($e->getMessage(),$e->getCode(),$e->getPrevious()->getMessage());$e->__construct(filename:\"next\");var_dump($e->getLine());"), b"string(5) \"first\"\nint(7)\nint(1)\nstring(6) \"custom\"\nint(-5)\nstring(3) \"old\"\nstring(0) \"\"\nint(7)\nstring(3) \"old\"\nint(0)\n");
}

#[test]
fn subclass_constructors_override_native_constructor() {
    assert_eq!(
        output(
            "class A extends ErrorException{function __construct(){}}$a=new A;var_dump($a->getSeverity());class B extends ErrorException{function __construct(){parent::__construct(\"hello\",2,4,\"B.php\",8);}}$b=new B;var_dump($b->getMessage(),$b->getSeverity(),$b->getFile(),$b->getLine());"
        ),
        b"int(1)\nstring(5) \"hello\"\nint(4)\nstring(5) \"B.php\"\nint(8)\n"
    );
}

#[test]
fn nullable_location_parameters_do_not_deprecate() {
    assert_eq!(
        output(
            "$e=new ErrorException(null,null,null,null,null);var_dump($e->getMessage(),$e->getCode(),$e->getSeverity(),$e->getFile()===__FILE__,$e->getLine()===__LINE__);"
        ),
        b"string(0) \"\"\nint(0)\nint(0)\nbool(true)\nbool(true)\n"
    );
}

#[test]
fn argument_validation_precedes_native_mutation() {
    assert_eq!(
        output(
            "$e=new ErrorException;try{$e->__construct(\"changed\",2,3,[],4);}catch(TypeError){}var_dump($e->getMessage(),$e->getCode(),$e->getSeverity());"
        ),
        b"string(0) \"\"\nint(0)\nint(1)\n"
    );
}

#[test]
fn spl_hierarchy_and_catch_dispatch() {
    assert_eq!(output("foreach([\"LogicException\",\"BadFunctionCallException\",\"BadMethodCallException\",\"DomainException\",\"InvalidArgumentException\",\"LengthException\",\"OutOfRangeException\",\"RuntimeException\",\"OutOfBoundsException\",\"OverflowException\",\"RangeException\",\"UnderflowException\",\"UnexpectedValueException\"] as $name){$e=new $name(\"m\",9);echo get_class($e),\":\",get_parent_class($e),\":\",$e->getMessage(),\":\",$e->getCode(),\":\";try{throw $e;}catch(LogicException){echo \"logic\";}catch(RuntimeException){echo \"runtime\";}echo \"\n\";}"), b"LogicException:Exception:m:9:logic\nBadFunctionCallException:LogicException:m:9:logic\nBadMethodCallException:BadFunctionCallException:m:9:logic\nDomainException:LogicException:m:9:logic\nInvalidArgumentException:LogicException:m:9:logic\nLengthException:LogicException:m:9:logic\nOutOfRangeException:LogicException:m:9:logic\nRuntimeException:Exception:m:9:runtime\nOutOfBoundsException:RuntimeException:m:9:runtime\nOverflowException:RuntimeException:m:9:runtime\nRangeException:RuntimeException:m:9:runtime\nUnderflowException:RuntimeException:m:9:runtime\nUnexpectedValueException:RuntimeException:m:9:runtime\n");
}

#[test]
fn negative_lines_appear_in_string_conversion() {
    assert_eq!(
        output("$e=new ErrorException(\"bad\",0,1,\"input\",-9);echo $e;"),
        b"ErrorException: bad in input:-9\nStack trace:\n#0 {main}"
    );
}

#[test]
fn base_constructor_reentry_preserves_code_and_previous() {
    assert_eq!(output("$e=new Exception(\"first\",7,new Error(\"old\"));$e->__construct();var_dump($e->getMessage(),$e->getCode(),$e->getPrevious()->getMessage());$e->__construct(code:0,previous:null);var_dump($e->getMessage(),$e->getCode(),$e->getPrevious()->getMessage());"), b"string(5) \"first\"\nint(7)\nstring(3) \"old\"\nstring(0) \"\"\nint(7)\nstring(3) \"old\"\n");
}

#[test]
fn native_parameters_convert_stringable_and_weak_scalars() {
    assert_eq!(
        output(
            "class S{function __toString(){return \"text\";}}$e=new ErrorException(new S,\"7\",\"8\",new S,\"-2\");var_dump($e->getMessage(),$e->getCode(),$e->getSeverity(),$e->getFile(),$e->getLine());"
        ),
        b"string(4) \"text\"\nint(7)\nint(8)\nstring(4) \"text\"\nint(-2)\n"
    );
}

#[test]
fn strict_constructor_rejects_weak_types() {
    assert_eq!(
        output(
            "declare(strict_types=1);foreach([null,\"8\",false,[]] as $x){try{new ErrorException(severity:$x);}catch(TypeError){echo \"severity;\";}}try{new ErrorException(filename:7);}catch(TypeError){echo \"file;\";}try{new ErrorException(line:\"8\");}catch(TypeError){echo \"line;\";}"
        ),
        b"severity;severity;severity;severity;file;line;"
    );
}

#[test]
fn native_classes_resolve_case_insensitively_and_allow_subclasses() {
    assert_eq!(output("class Custom extends BadMethodCallException{}$e=new Custom(\"bad\");var_dump($e instanceof BadFunctionCallException,$e instanceof LogicException,$e instanceof Exception,$e instanceof Throwable,$e instanceof Stringable,$e instanceof RuntimeException);echo get_class(new \\errorexception),get_parent_class(\"\\badMETHODcallEXCEPTION\");"), b"bool(true)\nbool(true)\nbool(true)\nbool(true)\nbool(true)\nbool(false)\nErrorExceptionBadFunctionCallException");
}

#[test]
fn native_trace_remains_at_creation_after_constructor_reentry() {
    assert_eq!(
        output(
            "function f(){return new ErrorException(\"a\",0,1,\"custom\",-2);}$e=f();$e->__construct(\"b\",1,4,\"changed\",5);echo $e->getTraceAsString();"
        ),
        b"#0 Command line code(1): f()\n#1 {main}"
    );
}

#[test]
fn severity_getter_is_final() {
    let error = Runtime::default()
        .execute_code(b"class A extends ErrorException{function getSeverity():int{return 2;}}")
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Error);
    assert!(error.message.contains("final"));
}
#[test]
fn only_nonnullable_parameters_deprecate_null() {
    let mut rt = Runtime::default();
    rt.execute_code(b"new ErrorException(null,null,null,null,null,null);")
        .unwrap();
    assert_eq!(rt.warnings.len(), 3);
    for (i, warning) in rt.warnings.iter().enumerate() {
        assert!(warning.contains(&format!("parameter #{}", i + 1)));
    }
}
#[test]
fn uncaught_exception_formats_signed_line() {
    let error = Runtime::default()
        .execute_code(b"throw new ErrorException('bad',0,1,'input',-9);")
        .unwrap_err();
    assert_eq!(error.to_string(), "ErrorException: bad on line -9");
}

#[test]
fn native_arity_errors_identify_the_declaring_method() {
    assert_eq!(output("foreach([new Exception,new Error,new ErrorException] as $e){try{$e->__construct(1,2,3,4,5,6,7);}catch(Throwable $x){echo $x->getMessage(),\"\\n\";}try{$e->getMessage(1);}catch(Throwable $x){echo $x->getMessage(),\"\\n\";}}try{(new ErrorException)->getSeverity(1);}catch(Throwable $x){echo $x->getMessage();}"), b"Exception::__construct() expects at most 3 arguments, 7 given\nException::getMessage() expects exactly 0 arguments, 1 given\nError::__construct() expects at most 3 arguments, 7 given\nError::getMessage() expects exactly 0 arguments, 1 given\nErrorException::__construct() expects at most 6 arguments, 7 given\nException::getMessage() expects exactly 0 arguments, 1 given\nErrorException::getSeverity() expects exactly 0 arguments, 1 given");
}

#[test]
fn native_type_errors_identify_boolean_values() {
    assert_eq!(output("try{new ErrorException(previous:true);}catch(TypeError $e){echo $e->getMessage();}try{new ErrorException(previous:false);}catch(TypeError $e){echo $e->getMessage();}"),b"ErrorException::__construct(): Argument #6 ($previous) must be of type ?Throwable, true givenErrorException::__construct(): Argument #6 ($previous) must be of type ?Throwable, false given");
}
