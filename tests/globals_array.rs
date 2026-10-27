use php_rs::{Runtime, Value, lint, run};

fn check(code: &str, expected: &str) {
    let mut rt = Runtime::default();
    rt.execute_code(code.as_bytes()).unwrap();
    assert_eq!(rt.output, expected.as_bytes());
    assert!(rt.warnings.is_empty(), "{:?}", rt.warnings);
}

#[test]
fn global_elements_are_live_from_nested_local_scopes() {
    check(
        "$x=1; function inner(){ $GLOBALS['x']+=2; } function outer(){ $x=99; inner(); echo $GLOBALS['x'],':',$x; } outer(); echo ':',$x;",
        "3:99:3",
    );
}

#[test]
fn snapshots_and_nested_arrays_have_value_semantics() {
    check(
        "$x=1; $a=['n'=>2]; $copy=$GLOBALS; $x=7; $GLOBALS['a']['n']=9; echo $copy['x'],$copy['a']['n']; $copy['a']['n']=8; echo $a['n']; var_dump(isset($copy['copy']));",
        "129bool(false)\n",
    );
}

#[test]
fn global_targets_support_all_assignment_operations() {
    check(
        "$GLOBALS['a'][]=2; $GLOBALS['a'][]=3; ++$GLOBALS['a'][0]; $GLOBALS['x']??=7; [$GLOBALS['a'][1],$GLOBALS['x']]=[5,9]; foreach([6] as $GLOBALS['k']=>$GLOBALS['v']){} $GLOBALS['s']='abc'; $GLOBALS['s'][1]='Z'; unset($GLOBALS['a'][0]); echo $a[1],$x,$k,$v,$s;",
        "5906aZc",
    );
}

#[test]
fn unset_global_detaches_existing_aliases() {
    check(
        "$x=1; function f(){global $x; unset($GLOBALS['x']); $GLOBALS['x']=3; echo $x; $x=7; echo $GLOBALS['x'];} f(); echo $x;",
        "133",
    );
}

#[test]
fn key_expressions_run_once_and_read_the_current_global() {
    check(
        "$n=0;$x=1;function keyname(){++$GLOBALS['n'];$GLOBALS['x']=3;return 'x';} echo $GLOBALS[keyname()]; echo ($GLOBALS)[keyname()]; $GLOBALS[keyname()]+=2; echo $x,$n;",
        "3353",
    );
}

#[test]
fn global_keys_are_variable_names_not_array_offsets() {
    check(
        "$GLOBALS[1.5]=2; $GLOBALS[null]=3; $GLOBALS[true]=4; echo ${'1.5'},${''},${'1'}; var_dump(isset($GLOBALS[1.5]),isset($GLOBALS[null]));",
        "234bool(true)\nbool(true)\n",
    );
}

#[test]
fn array_keys_use_string_conversion_with_its_warning() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$GLOBALS[[]]=7; echo $Array;").unwrap();
    assert_eq!(rt.output, b"7");
    assert_eq!(rt.warnings, ["Array to string conversion on line 1"]);
}

#[test]
fn probes_are_silent_and_missing_reads_name_the_global() {
    let mut rt = Runtime::default();
    rt.execute_code(b"var_dump(isset($GLOBALS),isset($GLOBALS['x']),empty($GLOBALS['x'])); echo $GLOBALS['x']??'fallback'; echo $GLOBALS['x'];").unwrap();
    assert_eq!(rt.output, b"bool(true)\nbool(false)\nbool(true)\nfallback");
    assert_eq!(rt.warnings, ["Undefined global variable $x on line 1"]);
}

#[test]
fn dynamic_global_name_does_not_expose_the_special_array() {
    check(
        "$name='GLOBALS'; var_dump(isset($$name)); $$name=4; echo $GLOBALS['GLOBALS'],$$name; function f(){ $n='GLOBALS'; $$n=8; echo $$n,$GLOBALS['GLOBALS']; } f();",
        "bool(false)\n4484",
    );
}

#[test]
fn literal_and_folded_global_names_use_the_special_array() {
    check(
        "$x=2; echo ${'GLOBALS'}['x'],${'GLO'.'BALS'}['x']; (($GLOBALS))['x']=7; echo $x;",
        "227",
    );
}

#[test]
fn global_and_static_declarations_can_bind_a_normal_globals_symbol() {
    check(
        "global $GLOBALS; var_dump(array_key_exists('GLOBALS',$GLOBALS)); static $GLOBALS=4; $n='GLOBALS'; echo $$n;",
        "bool(true)\n4",
    );
}

#[test]
fn whole_array_mutations_fail_before_execution_even_in_dead_code() {
    for statement in [
        "$GLOBALS=[];",
        "unset($GLOBALS);",
        "$GLOBALS++;",
        "--$GLOBALS;",
        "$GLOBALS??=[];",
        "[$GLOBALS]=[[]];",
        "foreach([1] as $GLOBALS){}",
        "foreach([1] as $GLOBALS=>$v){}",
        "${'GLOBALS'}=[];",
        "${'GLO'.'BALS'}=[];",
    ] {
        let mut rt = Runtime::default();
        let err = rt
            .execute_code(format!("echo 'before'; if(false){{{statement}}}").as_bytes())
            .unwrap_err();
        assert!(
            err.message.contains("$GLOBALS can only be modified"),
            "{statement}: {err}"
        );
        assert!(rt.output.is_empty());
    }
    for statement in ["$GLOBALS[]=1;", "$GLOBALS[][0]=1;"] {
        assert!(
            lint(format!("<?php {statement}").as_bytes())
                .unwrap_err()
                .message
                .contains("Cannot append to $GLOBALS")
        );
    }
    assert!(
        lint(b"<?php function f($GLOBALS){}")
            .unwrap_err()
            .message
            .contains("Cannot re-assign auto-global variable GLOBALS")
    );
}

#[test]
fn symbol_order_and_external_changes_survive_successive_executions() {
    let mut rt = Runtime::default();
    rt.execute_code(b"$first=1;$second=2;").unwrap();
    rt.variables.insert(b"first".to_vec(), Value::Int(4));
    rt.execute_code(b"$copy=$GLOBALS; echo $copy['first']; foreach($copy as $k=>$v)echo $k; unset($GLOBALS['first']); $first=7; $copy=$GLOBALS; foreach($copy as $k=>$v){if($k==='first'||$k==='second')echo $k;}").unwrap();
    assert_eq!(rt.output, b"4firstsecondfirstsecond");
    rt.variables.remove(b"second".as_slice());
    rt.execute_code(b"var_dump(isset($GLOBALS['second']));")
        .unwrap();
    assert!(rt.output.ends_with(b"bool(false)\n"));
}

#[test]
fn global_writes_survive_errors_and_function_scope_is_restored() {
    let mut rt = Runtime::default();
    assert!(
        rt.execute_code(b"function f(){ $local=8; $GLOBALS['x']=7; return 1/0; } f();")
            .is_err()
    );
    assert_eq!(rt.variables.get(b"x".as_slice()), Some(&Value::Int(7)));
    assert!(!rt.variables.contains_key(b"local".as_slice()));
    assert_eq!(
        run(b"<?php $x=3; function f(){eval('$GLOBALS[\"x\"]=9;');} f(); echo $x;").unwrap(),
        b"9"
    );
}

#[test]
fn globals_control_flow_subjects_are_snapshots() {
    check(
        "$x=1;function f(){$GLOBALS['x']=2;return $GLOBALS;}switch($GLOBALS){case f():echo 'matched';break;default:echo 'miss';} $x=1;echo match($GLOBALS){f()=>'matched',default=>'miss'};",
        "missmiss",
    );
}

#[test]
fn compiled_variables_precede_dynamic_names_and_keep_their_slots() {
    check(
        "$GLOBALS['gd']=0;$gb=$ga=1;unset($GLOBALS['gb']);$gb=3;foreach($GLOBALS as $k=>$v){if($k==='ga'||$k==='gb'||$k==='gd')echo $k;} var_dump(isset($GLOBALS['future'])); $future=1;",
        "gbgagdbool(false)\n",
    );
    check(
        "$n='ga';$$n=1;$n='gb';$$n=2;unset($GLOBALS['ga']);$n='ga';$$n=3;foreach($GLOBALS as $k=>$v){if($k==='ga'||$k==='gb')echo $k;}",
        "gbga",
    );
}

#[test]
fn symbol_slots_follow_loop_and_destructuring_compilation_order() {
    check(
        "while($ga=false){$gb=1;} for($gc=0;$gd=false;$ge=1){$gf=1;} [$gg]=$gh=[1]; $n='ga';$$n=1;$n='gb';$$n=1;$n='gd';$$n=1;$n='ge';$$n=1;$n='gf';$$n=1;foreach($GLOBALS as $k=>$v){if(strlen($k)===2 && $k[0]==='g')echo $k;}",
        "gbgagcgfgegdghgg",
    );
}
