//! Original HTML processing-instruction token, document and fragment evidence.
use super::*;

fn shape(node: &Node) -> String {
    match &node.kind {
        NodeKind::ProcessingInstruction {target,data}=>format!("PI {target}:{data}"),
        NodeKind::Comment(data)=>format!("comment {data}"),
        NodeKind::Element {tag,..}=>tag.clone(),
        NodeKind::Text(text)=>format!("text {text}"),
        NodeKind::DocumentType(dt)=>format!("doctype {}",dt.name),
        _=>String::new(),
    }
}
fn edges(doc:&Document,n:usize)->Vec<String>{doc.nodes[n].children.iter().map(|&child|shape(&doc.nodes[child])).collect()}

#[test]
fn html_processing_instruction_modes_literal_data_targets_question_end_and_bogus_comments(){
    for (input,expected) in [
        ("<?X_9-?>",Some("PI X_9-:")),("<?_t  \t\n   A&amp;<Q??>",Some("PI _t:A&amp;<Q?")),
        ("<?a ? ? >tail",Some("PI a:? ? ")),("<?x data\0>",Some("PI x:data\0")),
        ("<?XmL  x?>",Some("comment ?XmL  x?")),("<?XML-STYLESHEET>",Some("comment ?XML-STYLESHEET")),
        ("<?2a \0x>",Some("comment ?2a \u{fffd}x")),("<?ab:cd x>",Some("comment ?ab:cd x")),
        ("<?alpha.thing>",Some("comment ?alpha.thing")),("<?bad$eof",Some("comment ?bad$eof")),
        ("<?",None),("<?valid",None),("<?valid data?",None),
    ] {
        let (token,consumed)=read(input,None).unwrap();
        assert_eq!(consumed,input.find('>').map_or(input.len(),|i|i+1));
        let actual=token.into_kind().unwrap().map(|kind|shape(&Node{kind,namespace:Namespace::None,children:Vec::new()}));
        assert_eq!(actual.as_deref(),expected,"{input:?}");
    }
}

#[test]
fn html_processing_instruction_document_and_fragment_leaf_placement_and_raw_boundaries(){
    let input="<?before?><!doctype html><html><?root?><head><?head x?></head><body><div><?data <b &amp;?></div></body><?afterbody?></html><?afterhtml?>";
    for doc in [parse_loaded_document(input,&||Ok(())).unwrap(),parse_inert_html(input,100,&||Ok(())).unwrap().0]{
        assert_eq!(edges(&doc,0),["PI before:","doctype html","html","PI afterhtml:"]);
        let html=doc.nodes[0].children[2];assert_eq!(edges(&doc,html),["PI root:","head","body","PI afterbody:"]);
        let head=doc.nodes[html].children[1];assert_eq!(edges(&doc,head),["PI head:x"]);
        let div=doc.nodes[doc.nodes[html].children[2]].children[0];assert_eq!(edges(&doc,div),["PI data:<b &amp;"]);
        assert_eq!(doc.nodes[doc.nodes[div].children[0]].namespace,Namespace::None);
    }
    let doc=parse_fragment("<?target v?><b></b><?xml-stylesheet href=ignored?>","div",true).unwrap();
    assert_eq!(edges(&doc,0),["PI target:v","b","comment ?xml-stylesheet href=ignored?"]);
    for context in ["title","textarea","style","script","iframe","plaintext"] {
        let doc=parse_fragment("<?target v?>",context,true).unwrap();assert_eq!(edges(&doc,0),["text <?target v?>"]);
    }
}

#[test]
fn html_processing_instruction_budget_cancellation_and_eof_leave_no_published_prefix(){
    let parse=|s,n|parse_internal(s,None,false,Some(LoadedTree{nodes:n,check_cancel:&||Ok(())}),None);
    assert!(parse("<?p?>",4).unwrap_err().contains("node limit"));
    assert_eq!(parse("<?p?>",5).unwrap().nodes.len(),5);assert_eq!(parse("<?p unfinished?",4).unwrap().nodes.len(),4);
    for s in [format!("<?{}>","x".repeat(50000)),format!("<?target {}>","x".repeat(50000)),format!("<?-{}>","x".repeat(50000))] {
        let calls=std::cell::Cell::new(0);
        let check=||{calls.set(calls.get()+1);if calls.get()>=3{Err("cancel PI".into())}else{Ok(())}};
        assert!(parse_loaded_document(&s,&check).unwrap_err().contains("cancel PI"));
    }
    assert!(matches!(&parse("<?valid?>",5).unwrap().nodes[1].kind,NodeKind::ProcessingInstruction{target,..} if target=="valid"));
}
