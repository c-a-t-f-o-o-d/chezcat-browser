//! Original owned Program payload and byte-pressure acceptance.
use super::*;
struct NoHost;
impl Host for NoHost {
    fn allow_eval(&mut self)->Result<bool,String>{Ok(true)}
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("Unexpected host get")}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("Unexpected host set")}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("Unexpected host call")}
}
fn payload(source:&str)->usize {
    Parser::parse(source,&mut MAX_REGEXP_WORK.clone()).unwrap().retained_payload_bytes(||Ok(())).unwrap()
}
#[test]
fn program_payload_accounts_large_literals_and_nested_flat_arenas() {
    let source=format!("var literal='{}'; var {{a:b=1,...rest}}=obj; var [x,...tail]=arr; class Example {{ named(a){{return a;}} ['computed'](){{return 2;}} }} function* suspended(a){{yield a;return 3;}} tag`cooked${{x}}tail`; `head${{x}}end`; obj?.method?.(x,tail); switch(x){{case 1: x++;break;default:x=2;}}", "x".repeat(65536));
    let mut runtime=Runtime::new();let before=runtime.value_bytes;
    let program=runtime.register_program(&source,false).unwrap();
    assert!(program.classes.len()>0 && program.patterns.len()>2 && program.templates.len()>=3);
    assert!(program.templates.iter().any(|t|!t.resumption_plan.is_empty()));
    assert!(program.exprs.iter().any(|e|matches!(e.kind,ExprKind::TemplateObject(_))));
    assert!(program.exprs.iter().any(|e|matches!(e.kind,ExprKind::OptionalChain(..))));
    let spine=std::mem::size_of::<Program>()+program.source.capacity()+arena(&program.exprs)+arena(&program.stmts)+arena(&program.patterns)+arena(&program.templates)+arena(&program.classes)+arena(&program.body);
    assert!(program.retained_bytes.get()>=spine+65536);
    assert_eq!(runtime.value_bytes-before,program.retained_bytes.get()+arena(&runtime.programs));
    assert!(runtime.value_bytes<=runtime.limits.heap.retained_values);
}
#[test]
fn program_payload_pressure_retires_registry_only_code_with_spare_slots() {
    let mut runtime=Runtime::new();let baseline=runtime.value_bytes;
    let old=runtime.register_program("1;",false).unwrap();let weak=Rc::downgrade(&old);drop(old);
    assert!(runtime.programs.len()<runtime.programs.capacity());
    let backing=arena(&runtime.programs);
    runtime.limits.heap.retained_values=baseline+backing+payload("2;");
    runtime.begin_job();let new=runtime.register_program("2;",false).unwrap();
    assert!(weak.upgrade().is_none());assert_eq!(runtime.program_allocation.retired,1);
    assert_eq!((runtime.programs.len(),arena(&runtime.programs)),(1,backing));
    assert_eq!(runtime.value_bytes,baseline+backing+new.retained_bytes.get());
    assert_eq!((runtime.retained_source,runtime.retained_ast),(2,new.node_count()));
}
#[test]
fn program_payload_refusal_preserves_live_closure_and_recovers() {
    let mut runtime=Runtime::new();
    let callback=runtime.execute(&format!("(function(){{return '{}';}});","k".repeat(65536)),&mut NoHost).unwrap();
    let weak=Rc::downgrade(runtime.programs.last().unwrap());let limit=runtime.limits.heap.retained_values;
    let source="var shouldNotRun=1; /a/;";
    runtime.limits.heap.retained_values=runtime.value_bytes+payload(source)-1;
    let before=(runtime.programs.len(),runtime.value_bytes,runtime.retained_source,runtime.retained_ast,runtime.retained_regexp);
    runtime.begin_job();let error=runtime.execute(source,&mut NoHost).unwrap_err();
    assert!(error.contains("program storage"),"{error}");assert!(weak.upgrade().is_some());
    assert_eq!((runtime.programs.len(),runtime.value_bytes,runtime.retained_source,runtime.retained_ast,runtime.retained_regexp),before);
    runtime.limits.heap.retained_values=limit;runtime.begin_job();
    assert_eq!(runtime.invoke(callback,&[],Value::Undefined,&mut NoHost).unwrap().to_text(),"k".repeat(65536));
    runtime.begin_job();assert_eq!(runtime.execute("typeof shouldNotRun;",&mut NoHost).unwrap().to_text(),"undefined");
    runtime.limits.capacity.retained_ast=0;runtime.begin_job();
    assert_eq!(runtime.execute("21+21;",&mut NoHost).unwrap().to_number(),42.0);
}
#[test]
fn program_payload_scan_failure_keeps_shared_parsed_program_unpublished() {
    let parsed=Rc::new(Parser::parse("var a='kept'; /a/;",&mut MAX_REGEXP_WORK.clone()).unwrap());
    let mut runtime=Runtime::new();let baseline=runtime.value_bytes;
    runtime.steps=1;
    assert!(runtime.retain_parsed_program(parsed.clone()).err().unwrap().message.contains("step limit"));
    assert_eq!((runtime.programs.len(),runtime.retained_source,runtime.retained_ast,runtime.retained_regexp,runtime.value_bytes),(0,0,0,0,baseline));
    assert_eq!(parsed.retained_bytes.get(),0);
    runtime.begin_job();let admitted=runtime.retain_parsed_program(parsed.clone()).unwrap();
    assert!(Rc::ptr_eq(&admitted,&parsed));assert!(parsed.retained_bytes.get()>0);
}

#[test]
fn completed_compilation_rejects_literal_backing_omitted_by_incremental_estimate_and_recovers() {
    let source=format!("var effect='{}';effect.length;","x".repeat(65536));
    let capacity=limits::REGRESSION_CAPACITIES;
    let (_,tokens)=tokenize_for_parse(&source,&mut MAX_REGEXP_WORK.clone(),capacity,None).unwrap();
    let parsed=Parser::parse(&source,&mut MAX_REGEXP_WORK.clone()).unwrap();
    let actual=tokens+parsed.retained_payload_bytes(||Ok(())).unwrap();
    let estimate=tokens+parsed.compilation_ast_bytes();
    assert!(actual>estimate+60000,"{actual} versus {estimate}");
    let mut runtime=Runtime::for_test();let limit=runtime.limits.capacity.compilation_bytes;
    runtime.limits.capacity.compilation_bytes=actual-1;
    let before=(runtime.value_bytes,runtime.retained_source,runtime.retained_ast,runtime.retained_regexp);
    let error=runtime.execute(&source,&mut NoHost).unwrap_err();
    assert!(error.contains("Completed Program and parser backing"),"{error}");
    assert!(runtime.programs.is_empty());assert!(runtime.lookup(0,"effect").unwrap().is_none());
    assert_eq!((runtime.value_bytes,runtime.retained_source,runtime.retained_ast,runtime.retained_regexp),before);
    runtime.limits.capacity.compilation_bytes=actual;runtime.begin_job();
    assert_eq!(runtime.execute(&source,&mut NoHost).unwrap().to_number(),65536.0);
    runtime.limits.capacity.compilation_bytes=limit;runtime.begin_job();
    assert_eq!(runtime.execute("effect.length;",&mut NoHost).unwrap().to_number(),65536.0);
}
#[test]
fn completed_compilation_charges_live_parser_backing_and_deadline_without_double_counting_child_vectors() {
    let source="while(false){label:for(;;){break label;}} function f(a){return obj?.method?.(a,1,2);}";
    let mut parser=super::super::expression_parser_tests::scratch_parser_for_storage(source);
    let mut directives=true;
    while !matches!(parser.token().kind,TokenKind::End) {
        let statement=parser.statement().unwrap();parser.directive(statement,&mut directives);parser.program.body.push(statement);
    }
    parser.parse_deferred_bodies(0).unwrap();
    assert!(parser.loops.capacity()>0 && parser.breakables.capacity()>0 && parser.labels.capacity()>0 && parser.deferred_bodies.capacity()>0);
    let expected=parser.token_storage_bytes+parser.program.retained_payload_bytes(||Ok(())).unwrap()+arena(&parser.loops)+arena(&parser.breakables)+arena(&parser.labels)+arena(&parser.deferred_bodies);
    assert_eq!(parser.finished_compilation_bytes().unwrap(),expected);
    parser.capacity.compilation_bytes=expected;parser.check_finished_compilation_storage().unwrap();
    parser.capacity.compilation_bytes-=1;
    assert!(parser.check_finished_compilation_storage().unwrap_err().message.contains("Completed Program"));
    parser.compilation_deadline=Some(Instant::now()-Duration::from_millis(1));
    assert_eq!(parser.check_finished_compilation_storage().unwrap_err().message,"Script compilation deadline reached.");
}
