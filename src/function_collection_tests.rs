//! Original active reclamation workloads; no site program replay.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { panic!("unexpected get") }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { panic!("unexpected set") }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { panic!("unexpected call") }
}

const PRESSURE: &str = "function checkpoint(){return undefined;}function pressure(){var trash={};for(var p=0;p<750;p++)trash.fn=function(){return p;};}function make(n){var box={};box.fn=function(){return n;};return box.fn;}";

#[test]
fn original_two_thousand_replaced_cells_collect_without_changing_state_order_or_live_identity() {
    let mut runtime = Runtime::with_limits(limits::BROWSER);
    runtime.execute(r#"
var order='',slots=[],first,previous,count=0;
var model=Object.create(Object.prototype,{payload:{get:function(){order+='g';return this.read();}}});
function replace(value){var state=value,writes=0;var cell=Object.create(model);
cell.write=function(next){order+='w';state=next;writes++;};
cell.read=function(){order+='r';return state+writes;};slots[0]=cell;count++;return cell;}
Promise.resolve().then(function(){for(var i=0;i<2000;i++){var current=replace(i);
if(i===0)first=current;
if(previous&&(previous===current||previous.write===current.write||previous.read===current.read))throw 'identity';
order='';current.write(i*2);if(current.payload!==i*2+1||order!=='wgr')throw 'receiver/state/order';previous=current;}
if(first===slots[0]||first.payload!==1||slots[0].payload!==3999||count!==2000)throw 'retained state';});0;
"#, &mut NoHost).unwrap();
    let deadline = runtime.job_deadline;
    runtime.microtask_checkpoint(&mut NoHost).unwrap();
    assert!(runtime.function_arena.collections > 3);
    assert!(runtime.function_arena.retired > 3000);
    assert!(runtime.function_arena.environments_retired > 1500);
    assert!(runtime.function_arena.script_created >= 4003);
    assert!(runtime.function_arena.occupied < MAX_FUNCTIONS);
    assert!(runtime.functions.len() < MAX_FUNCTIONS);
    assert_eq!(runtime.job_deadline, deadline);
    assert!(runtime.transient_registration_is_empty());
    println!("{}", runtime.persistent_allocation_report());
}

#[test]
fn collection_keeps_unexported_pending_returns_throws_and_saved_eval_completions() {
    for source in [
        "function returning(){try{return make(71);}finally{pressure();checkpoint();}}if(returning()()!==71)throw 'return';0;",
        "try{try{throw make(72);}finally{pressure();checkpoint();}}catch(e){if(e()!==72)throw 'throw';}0;",
        "var result=eval('make(73);var ignored=(pressure(),checkpoint());');if(result()!==73)throw 'list completion';0;",
        "var result=eval('var i=0;while(i++<1||(pressure(),checkpoint(),false)){make(74);}');if(result()!==74)throw 'loop completion';0;",
        "var result=eval('for(var i=0;i<1;(pressure(),checkpoint())){i++;make(75);}');if(result()!==75)throw 'for completion';0;",
    ] {
        for disabled in [true, false] {
            let mut runtime = Runtime::new();
            runtime.function_arena.disable_collection = disabled;
            runtime.execute(PRESSURE, &mut NoHost).unwrap();
            runtime.execute(source, &mut NoHost).unwrap();
            assert_eq!(runtime.function_arena.collections, usize::from(!disabled), "{source}");
            if !disabled { assert!(runtime.function_arena.retired >= 749, "{source}"); }
            assert!(runtime.transient_registration_is_empty());
        }
    }
}

#[test]
fn collection_honors_exported_function_and_object_pins_after_script_names_disappear() {
    let mut runtime = Runtime::new();
    runtime.execute(PRESSURE, &mut NoHost).unwrap();
    let function = runtime.execute("make(81);", &mut NoHost).unwrap();
    let object = runtime.execute("var box={};box.fn=function(){return 83;};box;", &mut NoHost).unwrap();
    runtime.execute("box=null;pressure();checkpoint();0;", &mut NoHost).unwrap();
    assert!(runtime.function_arena.retired >= 749);
    assert_eq!(runtime.invoke(function, &[], Value::Undefined, &mut NoHost).unwrap().to_number(), 81.0);
    runtime.define_global("recovered", object).unwrap();
    assert_eq!(runtime.execute("recovered.fn();", &mut NoHost).unwrap().to_number(), 83.0);
    assert!(runtime.transient_registration_is_empty());
}

fn ready() -> Runtime {
    let mut runtime = Runtime::new();
    runtime.execute(PRESSURE, &mut NoHost).unwrap();
    runtime.execute("pressure('retained payload');0;", &mut NoHost).unwrap();
    assert_eq!(runtime.function_arena.collections, 0);
    assert!(runtime.function_arena.occupied >= MAX_FUNCTIONS * 3 / 4);
    assert!(runtime.transient_registration_is_empty());
    runtime
}

#[test]
fn failed_collection_mark_or_work_budget_retires_nothing_and_never_resets_deadline() {
    let mut probe = ready();
    let steps = probe.steps;
    probe.collect_function_candidates_at_entry().unwrap();
    let full_cost = steps - probe.steps;
    for failure in ["zero-steps", "steps", "late-steps", "time", "invalid", "refund"] {
        let mut runtime = ready();
        if failure == "refund" { runtime.value_bytes = 0; }
        let before = (runtime.function_arena.occupied, runtime.functions.len(), runtime.bindings, runtime.value_bytes,
            runtime.environments.iter().filter(|env| env.is_some()).count(), runtime.function_arena.script_created);
        if failure == "zero-steps" { runtime.steps = 0; }
        if failure == "steps" { runtime.steps = 5; }
        if failure == "late-steps" { runtime.steps = full_cost - 1; }
        if failure == "time" { runtime.job_deadline = Instant::now() - Duration::from_secs(1); }
        if failure == "invalid" { runtime.exported_function_roots.insert(usize::MAX); }
        let deadline = runtime.job_deadline;
        assert!(runtime.collect_function_candidates_at_entry().is_err());
        assert_eq!((runtime.function_arena.occupied, runtime.functions.len(), runtime.bindings, runtime.value_bytes,
            runtime.environments.iter().filter(|env| env.is_some()).count(), runtime.function_arena.script_created), before, "{failure}");
        assert_eq!(runtime.job_deadline, deadline);
        assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (0, 0));
        if matches!(failure, "zero-steps" | "steps" | "late-steps" | "time") { assert_eq!(runtime.steps, 0); }
    }
}

#[test]
fn primitive_only_collection_storage_still_spends_finite_traversal_work() {
    let mut costs = Vec::new();
    for count in [0, 1000] {
        let mut runtime = ready();
        runtime.execute(&format!("var map=new Map();map.set(-1,-1);map.delete(-1);for(var i=0;i<{count};i++)map.set(i,i);0;"), &mut NoHost).unwrap();
        let steps = runtime.steps;
        runtime.collect_function_candidates_at_entry().unwrap();
        costs.push(steps - runtime.steps);
        assert!(runtime.function_arena.retired >= 749);
    }
    assert!(costs[1] >= costs[0] + 2000, "primitive Map entries were not charged: {costs:?}");
}

#[test]
fn near_cap_entry_retries_after_candidate_references_clear_without_more_script_creation() {
    let mut runtime = Runtime::new();
    runtime.execute("var kept=[];for(var i=0;i<750;i++)kept[i]=function(){return i;};function checkpoint(){}checkpoint();0;", &mut NoHost).unwrap();
    assert_eq!((runtime.function_arena.collections, runtime.function_arena.retired), (1, 0));
    let created = runtime.function_arena.script_created;
    // Arrays are deliberately conservative roots, so clear their actual slots.
    runtime.execute("kept.length=0;kept=null;0;", &mut NoHost).unwrap();
    while runtime.function_arena.occupied < MAX_FUNCTIONS - 1 { runtime.native_function(Builtin::IsNaN, "fixture").unwrap(); }
    runtime.execute("checkpoint();0;", &mut NoHost).unwrap();
    assert_eq!(runtime.function_arena.script_created, created);
    assert_eq!(runtime.function_arena.collections, 2);
    assert_eq!(runtime.function_arena.retired, 750);
    assert!(runtime.function_arena.occupied < MAX_FUNCTIONS - 750);
}

#[test]
fn collection_refuses_unaudited_entries_and_preserves_live_occupied_cap() {
    let mut runtime = ready();
    let marker = runtime.inhibit_transient_collection();
    let usage = runtime.job_usage();
    runtime.collect_function_candidates_at_entry().unwrap();
    assert_eq!(runtime.job_usage(), usage);
    assert_eq!(runtime.function_arena.collections, 0);
    runtime.restore_transient_marker(marker);
    runtime.call_depth += 1;
    runtime.collect_function_candidates_at_entry().unwrap();
    assert_eq!(runtime.function_arena.collections, 0);
    runtime.call_depth -= 1;
    runtime.collect_function_candidates_at_entry().unwrap();
    assert!(runtime.function_arena.retired >= 749);

    let mut runtime = Runtime::new();
    let error = runtime.execute("var kept=[];for(var i=0;i<1200;i++){kept[i]=function(){return i;};if(i%32===0)kept[i]();}0;", &mut NoHost).unwrap_err();
    assert!(error.contains("Function limit reached (1024)"), "{error}");
    assert_eq!(runtime.function_arena.occupied, MAX_FUNCTIONS);
    assert_eq!(runtime.function_arena.retired, 0);
}

#[test]
fn collection_inside_popped_callback_and_thenable_jobs_keeps_private_values_and_job_roots() {
    for source in [
        "var seen=0,box={};box.fn=function(){var saved=make(91);pressure();checkpoint();if(saved()!==91)throw 'private callback';seen=91;};queueMicrotask(box.fn);box=null;0;",
        "var seen=0,box={};box.then=function(resolve){var saved=make(93);pressure();checkpoint();if(saved()!==93)throw 'private thenable';resolve(saved);};Promise.resolve(box).then(function(value){seen=value();});box=null;0;",
    ] {
        let mut runtime = Runtime::new();
        runtime.execute(PRESSURE, &mut NoHost).unwrap();
        runtime.execute(source, &mut NoHost).unwrap();
        assert!(runtime.microtask_checkpoint(&mut NoHost).unwrap().is_empty());
        assert_eq!(runtime.function_arena.collections, 1);
        assert!(runtime.function_arena.retired >= 749);
        assert!(runtime.execute("seen;", &mut NoHost).unwrap().to_number() >= 91.0);
        assert!(runtime.transient_registration_is_empty());
    }
}
