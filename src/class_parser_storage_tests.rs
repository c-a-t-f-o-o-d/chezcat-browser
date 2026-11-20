//! Original class-construction storage and semantic acceptance.
use super::*;
use super::super::*;
fn parser(source:&str)->Parser{super::super::expression_parser_tests::scratch_parser_for_storage(source)}
#[test]
fn class_token_copy_exact_fit_and_one_byte_short_preserve_token_and_caller_storage() {
    for (source,identifier,text) in [("Example",true,"Example"),("'method'",false,"method"),("constructor",false,"constructor")] {
        let mut parser=parser(source);let before=parser.compilation_bytes();let retained=parser.expression_storage_bytes;
        parser.capacity.compilation_bytes=before+text.len()-1;
        let error=parser.class_token_text(identifier).unwrap_err();
        assert!(error.message.contains("compilation storage"));
        assert_eq!((parser.current,parser.expression_storage_bytes),(0,retained));
        parser.capacity.compilation_bytes+=1;
        let copied=parser.class_token_text(identifier).unwrap();
        assert_eq!(copied,text);assert_eq!(parser.expression_storage_bytes,retained+copied.capacity());
        assert_eq!(parser.current,1);
    }
}
#[test]
fn class_pending_member_vector_and_names_stay_charged_and_constructor_key_is_released() {
    for source in ["class Named {constructor(){}first(){}second(){}}","class Named {first(){}second(){}}"] {
        let mut parser=parser(source);
        parser.statement().unwrap();
        let class=&parser.program.classes[0];
        let expected=class.name.as_ref().unwrap().capacity()+class.members.capacity()*std::mem::size_of::<Member>()+class.members.iter().map(|member|match &member.key{MemberKey::Literal(name)=>name.capacity(),MemberKey::Computed(_)=>0}).sum::<usize>();
        assert_eq!(parser.expression_storage_bytes,expected);
        assert_eq!(class.members.len(),2);
        assert!(parser.program.classes.capacity()>0);
        assert!(parser.compilation_bytes()<=parser.capacity.compilation_bytes);
    }
}
struct NoHost;
impl Host for NoHost {
    fn get(&mut self,_:u32,_:&str)->Result<Value,String>{panic!("Unexpected host get")}
    fn set(&mut self,_:u32,_:&str,_:Value)->Result<(),String>{panic!("Unexpected host set")}
    fn call(&mut self,_:u32,_:&str,_:&[Value])->Result<Value,String>{panic!("Unexpected host call")}
}
#[test]
fn browser_class_builder_keeps_six_thousand_methods_with_real_constructor_and_calls() {
    let mut source=String::from("class Example {constructor(){this.kept=7;}");
    for index in 0..6000 {source.push_str(&format!("m{index}(){{return {index};}}"));}
    source.push_str("}var instance=new Example();instance.kept+instance.m0()+instance.m5999();");
    let mut runtime=Runtime::new();
    assert_eq!(runtime.execute(&source,&mut NoHost).unwrap().to_number(),6006.0);
    assert_eq!(runtime.programs[0].classes[0].members.len(),6000);
    assert!(runtime.value_bytes<=runtime.limits.heap.retained_values);
    runtime.begin_job();
    assert_eq!(runtime.execute("var key='computed';class Keys{[key](){return 11;} 1(){return 13;}}var keys=new Keys();keys.computed()+keys[1]();",&mut NoHost).unwrap().to_number(),24.0);
}

#[test]
fn class_builder_quota_and_deadline_refuse_before_effects_and_recover() {
    let mut source=String::from("var shouldNotRun=1;class Example{");
    for index in 0..100{source.push_str(&format!("m{}{index}(){{}}","x".repeat(192)));}
    source.push_str("}shouldNotRun;");
    let mut staged=parser(&source);
    while !matches!(staged.token().kind,TokenKind::End){let statement=staged.statement().unwrap();staged.program.body.push(statement);}
    assert!(staged.expression_storage_bytes>20000);
    let allowance=staged.compilation_bytes()-staged.expression_storage_bytes+1024;
    let mut runtime=Runtime::for_test();let baseline=runtime.value_bytes;
    runtime.limits.capacity.compilation_bytes=allowance;
    assert!(tokenize_for_parse(&source,&mut MAX_REGEXP_WORK.clone(),runtime.limits.capacity,None).is_ok());
    let error=runtime.execute(&source,&mut NoHost).unwrap_err();
    assert!(error.contains("compilation storage")&&!error.contains("Completed Program"),"{error}");
    assert_eq!((runtime.programs.len(),runtime.retained_source,runtime.retained_ast,runtime.retained_regexp,runtime.value_bytes),(0,0,0,0,baseline));
    assert!(runtime.lookup(0,"shouldNotRun").unwrap().is_none());
    runtime.limits.capacity.compilation_bytes=limits::REGRESSION_CAPACITIES.compilation_bytes;runtime.begin_job();
    assert_eq!(runtime.execute(&source,&mut NoHost).unwrap().to_number(),1.0);
    let mut parser=parser("Example");let before=(parser.current,parser.expression_storage_bytes);
    parser.compilation_deadline=Some(Instant::now()-Duration::from_millis(1));
    assert_eq!(parser.class_token_text(true).unwrap_err().message,"Script compilation deadline reached.");
    assert_eq!((parser.current,parser.expression_storage_bytes),before);
}
