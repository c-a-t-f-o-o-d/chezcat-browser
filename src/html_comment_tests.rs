//! Original observable comment-data, owner, namespace and admission cases.
use super::*;
fn comments(doc:&Document)->Vec<&str>{doc.nodes.iter().filter_map(|node|if let NodeKind::Comment(data)=&node.kind{Some(data.as_str())}else{None}).collect()}
#[test]
fn fragment_comments_literal_data_termination_and_eof_follow_token_boundaries(){
    for(source,data,tail)in[("<!---->tail","","tail"),("<!-->tail","","tail"),("<!--->tail","","tail"),("<!--雪 &amp; <b>--!>tail","雪 &amp; <b>","tail"),("<!--a<!--b-->tail","a<!--b","tail"),("<!--a---!>tail","a-","tail"),("<!--a-","a",""),("<!--a--","a",""),("<!--a--!","a",""),("<!--a---","a-",""),("<!--","","")]{
        let doc=parse_fragment(source,"div",true).unwrap();assert_eq!(comments(&doc),vec![data],"{source}");
        let children=&doc.nodes[0].children;assert_eq!(children[0],1);assert_eq!(doc.nodes[1].namespace,Namespace::None);
        assert_eq!(doc.nodes[0].children.len(),if tail.is_empty(){1}else{2});
        if !tail.is_empty(){assert!(matches!(&doc.nodes[children[1]].kind,NodeKind::Text(text)if text==tail));}
    }
}
#[test]
fn fragment_comments_document_inert_and_fragment_paths_keep_real_order_without_decoding_data(){
    let markup="<!--lead--><div>one<!--雪 &amp;\r\ntwo--><b></b><!--last--></div>";
    for doc in [parse(markup),parse_loaded_document(markup,&||Ok(())).unwrap(),parse_inert_html(markup,100,&||Ok(())).unwrap().0,parse_fragment(markup,"template",true).unwrap()]{
        assert_eq!(comments(&doc),vec!["lead","雪 &amp;\ntwo","last"]);
        assert!(matches!(doc.nodes[doc.nodes[0].children[0]].kind,NodeKind::Comment(_)));
        let div=doc.nodes.iter().position(|n|matches!(&n.kind,NodeKind::Element{tag,..} if tag=="div")).unwrap();
        let second=doc.nodes[0].children[1];
        if second!=div {
            assert!(matches!(&doc.nodes[second].kind,NodeKind::Element{tag,..} if tag=="html"));
            let body=doc.nodes[second].children.iter().copied().find(|&n|matches!(&doc.nodes[n].kind,NodeKind::Element{tag,..} if tag=="body")).unwrap();
            assert_eq!(doc.nodes[body].children,[div]);
        }
        assert_eq!(doc.nodes[div].children.len(),4);assert_eq!(doc.nodes[doc.nodes[div].children[1]].namespace,Namespace::None);
    }
    let loaded=parse_loaded_document("<!--x\0y-->",&||Ok(())).unwrap();assert_eq!(comments(&loaded),vec!["x\u{fffd}y"]);
}
#[test]
fn fragment_comments_preserve_checked_svg_island_and_select_topology(){
    let doc=parse_fragment("<svg><!--marker--><g><rect width='2' height='3'/><!--end--></g></svg>","template",true).unwrap();
    assert_eq!(comments(&doc),vec!["marker","end"]);
    for node in &doc.nodes[1..]{if matches!(node.kind,NodeKind::Element{..}){assert_eq!(node.namespace,Namespace::Svg);}else{assert_eq!(node.namespace,Namespace::None);}}
    let doc=parse_fragment("<select><!--a--><option>x<!--b-->y</option><!--c--></select>","div",true).unwrap();
    assert_eq!(comments(&doc),vec!["a","b","c"]);let select=doc.nodes[0].children[0];assert_eq!(doc.nodes[select].children.len(),3);
}
#[test]
fn fragment_comments_raw_contexts_remain_text_and_real_node_limits_still_refuse(){
    for context in ["textarea","title","style","script","plaintext"]{let doc=parse_fragment("<!--literal-->",context,true).unwrap();assert!(comments(&doc).is_empty());assert!(matches!(&doc.nodes[1].kind,NodeKind::Text(text)if text=="<!--literal-->"));}
    let loaded=Some(LoadedTree{nodes:2,check_cancel:&||Ok(())});assert!(parse_internal("<!--one--><!--two-->",None,false,loaded,None).unwrap_err().contains("node limit"));
    assert!(parse_fragment(&"<!---->".repeat(MAX_NODES),"div",true).unwrap_err().contains("node limit"));
    let pi=parse_fragment("<?pi?>","div",true).unwrap();
    assert!(matches!(&pi.nodes[pi.nodes[0].children[0]].kind,NodeKind::ProcessingInstruction{target,data} if target=="pi" && data.is_empty()));
    let declaration=parse_fragment("<!DOCTYPE html>","div",true).unwrap();
    assert_eq!(declaration.nodes.len(),1);assert!(declaration.nodes[0].children.is_empty());
    assert_eq!(declaration.mode,DocumentMode::NoQuirks);
    for (markup,data) in [("<![CDATA[unsupported]]>","[CDATA[unsupported]]"),("<!unimplemented>","unimplemented")] {
        let doc=parse_fragment(markup,"div",true).unwrap();
        assert_eq!(comments(&doc),[data]);assert_eq!(doc.nodes.len(),2);
        assert!(matches!(&doc.nodes[doc.nodes[0].children[0]].kind,NodeKind::Comment(value) if value==data));
    }
}
