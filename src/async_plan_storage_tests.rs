//! Original shared builder storage, refusal and real suspension acceptance.
use super::*;
fn storage(limit:usize)->Storage{Storage{limit,used:std::cell::Cell::new(0),peak:std::cell::Cell::new(0),deadline:None}}
#[test]
fn plan_builder_joint_backing_exact_fit_refusal_and_release_preserve_existing_records() {
    let ledger=storage(38);
    let mut first=StorageVec::new(&ledger);first.reserve(4,false).unwrap();
    for value in [11u64,22,33,44]{push(&mut first,value).unwrap();}
    let mut second=StorageVec::new(&ledger);second.reserve(3,false).unwrap();
    for value in [5u16,6,7]{push(&mut second,value).unwrap();}
    assert_eq!((ledger.used.get(),ledger.peak.get()),(38,38));
    assert!(push(&mut first,55).unwrap_err().message.contains("compilation storage"));
    assert_eq!(first.as_slice(),[11,22,33,44]);assert_eq!(second.as_slice(),[5,6,7]);
    drop(second);assert_eq!(ledger.used.get(),32);drop(first);assert_eq!(ledger.used.get(),0);
    let short=storage(37);let mut first=StorageVec::<u64>::new(&short);first.reserve(4,false).unwrap();
    let mut second=StorageVec::<u16>::new(&short);assert!(second.reserve(3,false).is_err());
    assert_eq!(short.used.get(),32);assert_eq!(second.capacity(),0);drop((first,second));assert_eq!(short.used.get(),0);
}
#[test]
fn plan_builder_refusal_and_deadline_release_every_sizing_and_emission_reservation() {
    for source in [
        "async function f(p,v){if(v){let x=await p;use(x);}else{await p;}try{return await p;}catch(e){return handle(e);}}",
        "function* g(){try{yield 1;yield 2;}finally{cleanup();}}",
        "async function f(p){return use(use(use(await p)),use(1));}",
    ] {
        let mut program=Parser::parse(source,&mut MAX_REGEXP_WORK.clone()).unwrap();
        let expected=if program.templates[0].kind==FunctionKind::Generator {2}else{source.matches("await ").count()};
        let old=std::mem::take(&mut program.templates[0].resumption_plan);program.resumption_plan_nodes-=old.len();
        let ledger=storage(1024*1024);
        let code=compile_with_storage(&ledger,&program,0,expected,MAX_AST).unwrap();
        assert_eq!(code.len(),old.len());assert_eq!(ledger.used.get(),0);
        assert!(ledger.peak.get()>code.capacity()*std::mem::size_of::<Instruction>());
        let mut refused=0;
        for allowance in [0,1,program.stmts.len(),code.capacity()*std::mem::size_of::<Instruction>(),code.capacity()*std::mem::size_of::<Instruction>()+program.stmts.len()] {
            let ledger=storage(allowance);
            let error=compile_with_storage(&ledger,&program,0,expected,MAX_AST).err().unwrap();
            assert!(error.message.contains("compilation storage"),"{}",error.message);
            assert_eq!(ledger.used.get(),0);refused+=1;
        }
        assert_eq!(refused,5);
        let mut ledger=storage(1024*1024);ledger.deadline=Some(Instant::now()-Duration::from_millis(1));
        assert_eq!(compile_with_storage(&ledger,&program,0,expected,MAX_AST).err().unwrap().message,"Script compilation deadline reached.");
        assert_eq!(ledger.used.get(),0);
        // The original Program is unchanged; retry builds the same control
        // sequence after every failed sizing/emission attempt.
        let ledger=storage(1024*1024);let retry=compile_with_storage(&ledger,&program,0,expected,MAX_AST).unwrap();
        assert_eq!(retry.len(),old.len());assert_eq!(ledger.used.get(),0);
    }
}
struct NoHost;
impl Host for NoHost {
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("Unexpected host get")}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("Unexpected host set")}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("Unexpected host call")}
}
#[test]
fn browser_plan_builder_keeps_five_thousand_real_awaits_and_generator_yields() {
    let mut runtime=Runtime::new();
    let source=format!("var hits=0,answer=0;async function run(){{{}return hits;}}run().then(function(value){{answer=value;}});","await 1;hits++;".repeat(5000));
    runtime.execute(&source,&mut NoHost).unwrap();
    assert!(runtime.programs[0].templates[0].resumption_plan.len()>5000);
    assert!(runtime.microtask_checkpoint(&mut NoHost).unwrap().is_empty());
    assert_eq!(runtime.binding_value(0,"answer").unwrap().to_number(),5000.0);
    runtime.begin_job();let source=format!("function* items(){{{}}}var iter=items(),total=0,part;while(!(part=iter.next()).done)total+=part.value;total;","yield 1;".repeat(5000));
    assert_eq!(runtime.execute(&source,&mut NoHost).unwrap().to_number(),5000.0);
    assert!(runtime.value_bytes<=runtime.limits.heap.retained_values);
}

#[test]
fn plan_builder_selected_compiler_quota_refuses_before_effects_and_program_publication() {
    let source=format!("var shouldNotRun=1;async function f(p){{{}}}f(1);","await p;".repeat(1000));
    let mut parsed=Parser::parse(&source,&mut MAX_REGEXP_WORK.clone()).unwrap();
    let plan=std::mem::take(&mut parsed.templates[0].resumption_plan);parsed.resumption_plan_nodes-=plan.len();
    let (_,tokens)=tokenize_for_parse(&source,&mut MAX_REGEXP_WORK.clone(),limits::REGRESSION_CAPACITIES,None).unwrap();
    let allowance=tokens+parsed.compilation_ast_bytes()+8*std::mem::size_of::<DeferredBody>()+1024;
    let mut runtime=Runtime::new();let baseline=runtime.value_bytes;
    runtime.limits.capacity.compilation_bytes=allowance;
    let error=runtime.execute(&source,&mut NoHost).unwrap_err();
    assert!(error.contains("building resumption plan"),"{allowance}: {error}");
    assert_eq!((runtime.programs.len(),runtime.retained_source,runtime.retained_ast,runtime.retained_regexp,runtime.value_bytes),(0,0,0,0,baseline));
    assert!(runtime.lookup(0,"shouldNotRun").unwrap().is_none());assert!(!runtime.has_microtasks());
    runtime.limits.capacity.compilation_bytes=limits::REGRESSION_CAPACITIES.compilation_bytes;runtime.begin_job();
    runtime.execute(&source,&mut NoHost).unwrap();assert!(runtime.microtask_checkpoint(&mut NoHost).unwrap().is_empty());
    assert_eq!(runtime.binding_value(0,"shouldNotRun").unwrap().to_number(),1.0);
}
