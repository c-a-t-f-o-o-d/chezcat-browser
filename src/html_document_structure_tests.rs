use super::*;

fn names(doc: &Document, parent: usize) -> Vec<String> {
    doc.nodes[parent]
        .children
        .iter()
        .map(|&n| match &doc.nodes[n].kind {
            NodeKind::Element { tag, .. } => tag.clone(),
            NodeKind::Text(t) => format!("#{t}"),
            NodeKind::Comment(t) => format!("!{t}"),
            NodeKind::DocumentType(d) => format!("doctype {}", d.name),
            _ => "?".into(),
        })
        .collect()
}
fn find(doc: &Document, name: &str) -> usize {
    doc.nodes
        .iter()
        .position(|n| matches!(&n.kind,NodeKind::Element{tag,..} if tag==name))
        .unwrap()
}

#[test]
fn html_document_structure_metadata_eof_and_empty_document() {
    for source in [""," \n\t","<!doctype html><!--before--><meta charset=utf-8><title>Title</title><script>var ok=1;</script>"]{
        let doc=parse_loaded_document(source,&||Ok(())).unwrap();
        let html=find(&doc,"html");let head=find(&doc,"head");let body=find(&doc,"body");
        assert_eq!(names(&doc,html),["head","body"]);assert!(doc.nodes[body].children.is_empty());
        if source.starts_with("<!doctype"){
            assert_eq!(names(&doc,0),["doctype html","!before","html"]);assert_eq!(names(&doc,head),["meta","title","script"]);
            assert_eq!(doc.title,"Title");
        }else{assert_eq!(names(&doc,0),["html"]);assert!(doc.nodes[head].children.is_empty());}
    }
}

#[test]
fn html_document_structure_reprocesses_tokens_and_keeps_comments_and_text_in_order() {
    let source="<!--before--><html id=original><head data-h=1><title>T</title> \t</head>\n<!--between--><meta name=tail><body id=b><p>live</p><body id=wrong data-extra=2><html id=other lang=en></body><!--after-body--></html><!--after-html-->";
    let doc = parse_loaded_document(source, &|| Ok(())).unwrap();
    let html = find(&doc, "html");
    let head = find(&doc, "head");
    let body = find(&doc, "body");
    assert_eq!(names(&doc, 0), ["!before", "html", "!after-html"]);
    assert_eq!(
        names(&doc, html),
        ["head", "#\n", "!between", "body", "!after-body"]
    );
    assert_eq!(names(&doc, head), ["title", "# \t", "meta"]);
    assert_eq!(names(&doc, body), ["p"]);
    assert_eq!(doc.attr(html, "id"), Some("original"));
    assert_eq!(doc.attr(html, "lang"), Some("en"));
    assert_eq!(doc.attr(body, "id"), Some("b"));
    assert_eq!(doc.attr(body, "data-extra"), Some("2"));
    let doc = parse_loaded_document("<head> \t&#65;<em>next</em>", &|| Ok(())).unwrap();
    assert_eq!(names(&doc, find(&doc, "head")), ["# \t"]);
    assert_eq!(names(&doc, find(&doc, "body")), ["#A", "em"]);
}

#[test]
fn html_document_structure_keeps_template_edges_ordinary_frameset_and_fragment_context() {
    let doc = parse_loaded_document(
        "<template><html id=forged><span>held</span></template><p>visible",
        &|| Ok(()),
    )
    .unwrap();
    let html = find(&doc, "html");
    assert_eq!(doc.attr(html, "id"), None);
    assert_eq!(names(&doc, find(&doc, "head")), ["template"]);
    assert_eq!(names(&doc, find(&doc, "body")), ["p"]);
    assert_eq!(names(&doc, find(&doc, "template")), ["span"]);
    let doc = parse_loaded_document("<frameset><frame></frameset>", &|| Ok(())).unwrap();
    assert_eq!(names(&doc, find(&doc, "html")), ["head", "frameset"]);
    let fragment = parse_fragment("<em>part</em>", "div", false).unwrap();
    assert_eq!(names(&fragment, 0), ["em"]);
}

#[test]
fn html_document_structure_counts_real_nodes_and_respects_cancellation() {
    let parse = |n| {
        parse_internal(
            "",
            None,
            false,
            Some(LoadedTree {
                nodes: n,
                check_cancel: &|| Ok(()),
            }),
            None,
        )
    };
    assert!(parse(3).unwrap_err().contains("node limit"));
    assert_eq!(parse(4).unwrap().nodes.len(), 4);
    assert!(
        parse_loaded_document("<meta><title>T</title>", &|| Err("cancelled".into()))
            .unwrap_err()
            .contains("cancelled")
    );
    let doc = parse_loaded_document(
        "<script nonce=a nonce=b>let x=1;</script><body><script nonce=c></script>",
        &|| Ok(()),
    )
    .unwrap();
    let scripts: Vec<_> = doc
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(n, e)| {
            matches!(&e.kind,NodeKind::Element{tag,..} if tag=="script").then_some(n)
        })
        .collect();
    assert_eq!(scripts.len(), 2);
    assert!(doc.nonce_unsafe_nodes.contains(&scripts[0]));
    assert!(!doc.nonce_unsafe_nodes.contains(&scripts[1]));
}
