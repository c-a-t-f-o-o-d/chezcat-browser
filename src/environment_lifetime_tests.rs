//! Original single-job scope lifetime tests. These do not run captured scripts.
use super::*;

struct NoHost;
impl Host for NoHost {
    fn get(&mut self, _: u32, _: &str) -> Result<Value, String> { Err("Native read denied.".into()) }
    fn set(&mut self, _: u32, _: &str, _: Value) -> Result<(), String> { Err("Native write denied.".into()) }
    fn call(&mut self, _: u32, _: &str, _: &[Value]) -> Result<Value, String> { Err("Native call denied.".into()) }
    fn allow_eval(&mut self) -> Result<bool, String> { Ok(true) }
}

fn assert_accounting(runtime: &Runtime) {
    assert_eq!(runtime.bindings, runtime.environments.iter().flatten().map(|e| e.bindings.len()).sum::<usize>());
    let free: HashSet<_> = runtime.free_environments.iter().copied().collect();
    assert_eq!(free.len(), runtime.free_environments.len(), "duplicate free slot");
    for (id, env) in runtime.environments.iter().enumerate() {
        assert_eq!(free.contains(&id), env.is_none(), "slot {id}");
        if let Some(env) = env {
            if let Some(parent) = env.parent {
                assert!(runtime.environments[parent].is_some(), "dangling parent for {id}");
            }
        }
    }
}

#[test]
fn environment_lifetime_long_single_job_reuses_uncaptured_blocks_and_calls() {
    for source in [
        "var n=0;while(n<6000){{let local=n;{const next=local+1;n=next;}}}n;",
        "function bump(a){let b=a+1;{let scratch=a;}return b;}var n=0;while(n<6000)n=bump(n);n;",
        "var n=0;for(let i=0;i<6000;i++){let local=i;n++;}n;",
        "var n=0;for(let item of Array(6000)){let copy=item;n++;}n;",
        "var n=0;while(n<6000){switch(n%2){case 0:let one=1;n+=one;break;default:n++;}}n;",
        "var n=0;for(var i=0;i<6000;i++){try{let v=i;throw 1;}catch(e){n+=e;}}n;",
    ] {
        // The actual browser profile, not a special enlarged fixture profile.
        let mut runtime = Runtime::for_capacity_test();
        assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "6000", "{source}");
        assert!(runtime.environments.len() < 16, "unbounded arena growth: {}", runtime.environments.len());
        assert_accounting(&runtime);
        assert_eq!((runtime.call_depth, runtime.eval_depth), (0, 0));
    }
}

#[test]
fn environment_lifetime_original_closures_arguments_and_eval_fixture() {
    let mut runtime = Runtime::for_capacity_test();
    assert_eq!(runtime.execute(include_str!("../tools/fixtures/environment-lifetimes.js"), &mut NoHost).unwrap().to_text(), "ok");
    assert!(runtime.environments.len() < 40);
    assert_accounting(&runtime);
    runtime.begin_job();
    assert_eq!(runtime.execute("saved()+':'+fromEval()+':'+capturedDefault()+':'+closures[2]();", &mut NoHost).unwrap().to_text(), "13:22:6:2");
}

#[test]
fn environment_lifetime_parameter_failures_release_partial_setup() {
    let mut runtime = Runtime::for_capacity_test();
    let source = "function bad(){throw 1;}function f(a=2,b=bad()){return a+b;}var n=0;for(var i=0;i<5000;i++){try{f();}catch(e){n+=e;}}n;";
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "5000");
    assert!(runtime.environments.len() < 16);
    assert_accounting(&runtime);
}

#[test]
fn environment_lifetime_mapped_arguments_pin_ancestors_until_detached() {
    let mut runtime = Runtime::for_capacity_test();
    runtime.execute("var setter;function save(a){setter=function(v){a=v;};return arguments;}var args=save(5);", &mut NoHost).unwrap();
    runtime.begin_job();
    runtime.execute("for(var i=0;i<6000;i++){let unused=i;}setter(17);", &mut NoHost).unwrap();
    runtime.begin_job();
    assert_eq!(runtime.execute("args[0];", &mut NoHost).unwrap().to_text(), "17");
    runtime.begin_job();
    runtime.execute("Object.defineProperty(args,'0',{value:23,writable:false});setter(29);", &mut NoHost).unwrap();
    runtime.begin_job();
    assert_eq!(runtime.execute("args[0];", &mut NoHost).unwrap().to_text(), "23");
    assert_accounting(&runtime);
}

#[test]
fn environment_lifetime_capture_chain_and_capacity_remain_bounded() {
    let mut runtime = Runtime::for_capacity_test();
    let mut parent = 0;
    for _ in 1..MAX_ENVIRONMENTS {
        parent = runtime.new_environment(parent, false).unwrap();
    }
    runtime.capture_environment(parent);
    for id in (1..MAX_ENVIRONMENTS).rev() { runtime.release_environment(id); }
    assert_eq!(runtime.environments.iter().flatten().count(), MAX_ENVIRONMENTS);
    assert!(runtime.new_environment(0, false).unwrap_err().message.contains("Environment limit"));
    // No actual function/arguments handle was published by this direct fixture.
    runtime.collect_environments();
    assert_eq!(runtime.environments.iter().flatten().count(), 1);
    let reused = runtime.new_environment(0, false).unwrap();
    runtime.release_environment(reused);
    runtime.release_environment(reused); // No duplicate free-list entry.
    assert_accounting(&runtime);
}

#[test]
fn environment_lifetime_fatal_exit_releases_bindings_without_refunding_work() {
    let mut runtime = Runtime::for_test();
    runtime.execute("var caught=false,finished=false;", &mut NoHost).unwrap();
    let bindings = runtime.bindings;
    let bytes = runtime.value_bytes;
    runtime.begin_job();
    let error = runtime.execute("try{while(true){let local='temporary';}}catch(e){caught=true;}finally{finished=true;}", &mut NoHost).unwrap_err();
    assert!(error.contains("limit"), "{error}");
    assert!(!error.contains("Environment limit"), "transient environments still accumulated: {error}");
    assert_eq!((runtime.bindings, runtime.value_bytes), (bindings, bytes));
    assert!(runtime.job_usage().0 != 0 || runtime.job_usage().1 != 0);
    assert_accounting(&runtime);
    runtime.begin_job();
    assert_eq!(runtime.execute("!caught&&!finished;", &mut NoHost).unwrap().to_text(), "true");
}

#[test]
fn environment_lifetime_loop_abrupt_completions_preserve_captured_bindings() {
    let mut runtime = Runtime::for_capacity_test();
    let source = "var saved=[],n=0;outer:for(let i=0;i<5;i++){try{switch(i){case 0:let v=10;saved.push(function(){return v;});continue outer;case 1:break;default:let w=i;saved.push(function(){return w;});if(i===3)break outer;}}finally{n++;}}if(n!==4||saved[0]()!==10||saved[1]()!==2||saved[2]()!==3)throw 'completion';'ok';";
    assert_eq!(runtime.execute(source, &mut NoHost).unwrap().to_text(), "ok");
    assert_accounting(&runtime);
}
