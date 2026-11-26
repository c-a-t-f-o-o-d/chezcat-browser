//! Loaded noscript tokenization captures scripting before constructing resource nodes.
use super::*;

fn element(document:&Document,name:&str)->usize{
    document.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag==name)).unwrap()
}
fn has_id(document:&Document,id:&str)->bool{
    document.nodes.iter().enumerate().any(|(node,_)|document.attr(node,"id")==Some(id))
}

#[test]
fn noscript_loader_context_enabled_head_keeps_literal_text_and_following_metadata(){
    let raw="<img id=hidden src=missing><script>bad()</script>&amp;";
    let source=format!("<head><noscript>{raw}</noscript><title>After</title></head><body><p id=real>Real</p>");
    let document=parse_loaded_document_with_scripting(&source,true,&||Ok(())).unwrap();
    assert!(document.scripting_enabled);assert_eq!(document.title,"After");assert!(has_id(&document,"real"));assert!(!has_id(&document,"hidden"));
    let node=element(&document,"noscript");assert!(document.nodes[element(&document,"head")].children.contains(&node));
    let [child]=document.nodes[node].children.as_slice() else{panic!("one actual raw Text child")};
    assert!(matches!(&document.nodes[*child].kind,NodeKind::Text(text) if text==raw));
    assert!(!document.nodes.iter().any(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="script"||tag=="img")));
}

#[test]
fn noscript_loader_context_enabled_body_preserves_raw_entities_and_exact_end_boundary(){
    let raw="é&amp;<b id=fake>ignored</b></noscriptmore>\r\n\0tail";
    let source=format!("<body><noscript>{raw}</NOSCRIPT ><p id=after>After</p>");
    let document=parse_loaded_document_with_scripting(&source,true,&||Ok(())).unwrap();
    let node=element(&document,"noscript");assert!(document.nodes[element(&document,"body")].children.contains(&node));
    let [child]=document.nodes[node].children.as_slice() else{panic!("literal body fallback")};
    let expected=raw.replace("\r\n","\n").replace('\0',"\u{fffd}");
    assert!(matches!(&document.nodes[*child].kind,NodeKind::Text(text) if text==&expected));
    assert!(!has_id(&document,"fake"));assert!(has_id(&document,"after"));
}

#[test]
fn noscript_loader_context_disabled_default_and_inert_parses_keep_fallback_elements(){
    let source="<body><noscript><b id=fallback>Fallback</b></noscript><p id=real>Real</p>";
    let documents=[parse_loaded_document(source,&||Ok(())).unwrap(),parse_loaded_document_with_scripting(source,false,&||Ok(())).unwrap(),parse_inert_html(source,100_000,&||Ok(())).unwrap().0];
    for document in documents{assert!(!document.scripting_enabled);assert!(has_id(&document,"fallback"));assert!(has_id(&document,"real"));let node=element(&document,"noscript");assert!(document.nodes[node].children.contains(&element(&document,"b")));}
    let enabled=parse_loaded_document_with_scripting(source,true,&||Ok(())).unwrap();assert!(enabled.scripting_enabled);assert!(!has_id(&enabled,"fallback"));
}

#[test]
fn noscript_loader_context_both_modes_cancel_bound_nodes_and_recover(){
    let source=format!("<body><noscript>{}</noscript>","z".repeat(128*1024));
    for scripting in [false,true]{
        let calls=std::cell::Cell::new(0);
        let error=parse_loaded_document_with_scripting(&source,scripting,&||{calls.set(calls.get()+1);if calls.get()>=5{Err("cancel noscript load".into())}else{Ok(())}}).unwrap_err();
        assert_eq!(error,"cancel noscript load");assert_eq!(calls.get(),5);
        let check=||Ok(());let bounded=LoadedTree{nodes:5,check_cancel:&check};
        assert!(parse_internal("<body><noscript>x</noscript>",None,scripting,Some(bounded),None).unwrap_err().contains("node limit"));
        let recovered=parse_loaded_document_with_scripting("<body><noscript>fallback</noscript><p id=after>After</p>",scripting,&check).unwrap();
        assert_eq!(recovered.scripting_enabled,scripting);assert!(has_id(&recovered,"after"));
    }
}
