//! Original capacity/refund witnesses for owned DOM character buffers.
use super::*;

fn wide(text: &str, capacity: usize) -> String {
    let mut value = String::new();
    value.try_reserve_exact(capacity).unwrap();
    value.push_str(text);
    value
}
fn start(mut doc: Document) -> (Document, Forms, Page) {
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}

#[test]
fn character_storage_constructors_admit_capacity_before_identity_and_copy_only_text() {
    for family in 0..3 {
        let (mut doc, mut forms, mut page) = start(crate::html::parse("<html><body></body></html>"));
        let kind = || match family {
            0 => NodeKind::Text(wide("x", 64)),
            1 => NodeKind::Comment(wide("x", 64)),
            _ => NodeKind::ProcessingInstruction { target: wide("fixture", 32), data: wide("x", 64) },
        };
        let cost = if family == 2 { 96 } else { 64 };
        let cost=cost+Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.main_node_minimum(1).unwrap();
        let copied = if family == 2 { 8 } else { 1 };
        let before = page.state.retained;
        let nodes = doc.nodes.len();
        page.state.retained_limit = before + cost - 1;
        assert!(Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.create_dom_node(kind()).is_err());
        assert_eq!(doc.nodes.len(), nodes);
        assert_eq!(page.state.retained, before);
        assert!(page.state.dom.created.is_empty());
        page.begin_job();
        page.state.retained_limit += 1;
        page.state.copy_work = copied + size_of::<Node>()+(nodes+1)*(2+size_of::<Option<usize>>()); // paid spare slot still needs record initialization
        Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.create_dom_node(kind()).unwrap();
        assert_eq!(page.state.retained, before + cost);
        assert_eq!(page.state.copy_work, 0);
    }
}

#[test]
fn character_storage_baseline_and_shrinking_setters_refund_owned_buffers() {
    // The inert tokenizer preserves Comment nodes; ordinary loaded parsing
    // still omits them. Exercise real parsed character records here.
    let mut doc = crate::html::parse_inert_html("<html><body><p>x<!--y--></p></body></html>", 64, &|| Ok(())).unwrap().0;
    let text = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Text(data) if data == "x")).unwrap();
    let comment = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Comment(_))).unwrap();
    doc.nodes[text].kind = NodeKind::Text(wide("x", 64));
    doc.nodes[comment].kind = NodeKind::Comment(wide("y", 128));
    doc.title = wide("old", 32);
    let baseline = document_bytes(&doc);
    let (mut doc, mut forms, mut page) = start(doc);
    let expected=baseline+page.state.node_state_bytes();
    assert_eq!(page.state.retained, expected);
    assert_eq!(node_bytes(&doc.nodes[text]), 64);
    assert_eq!(node_bytes(&doc.nodes[comment]), 128);
    page.state.retained_limit = expected - 64 + 1;
    Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.set(node_handle(text), "data", Value::String("z".into())).unwrap();
    assert_eq!(page.state.retained, expected - 64 + 1);
    Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.set(node_handle(comment), "nodeValue", Value::Null).unwrap();
    assert_eq!(page.state.retained, expected - 64 + 1 - 128);
    Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.set(DOCUMENT, "title", Value::String("n".into())).unwrap();
    assert_eq!(page.state.retained, expected - 64 + 1 - 128 - 32 + 1);
    assert_eq!(doc.title, "n");
}

#[test]
fn character_storage_normalize_exact_refund_keeps_detached_text_and_recovers() {
    let mut doc = crate::html::parse("<html><body><p>x</p></body></html>");
    let parent = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "p")).unwrap();
    let first = doc.nodes[parent].children[0];
    doc.nodes[first].kind = NodeKind::Text(wide("x", 64));
    let second = doc.nodes.len();
    doc.nodes.push(Node { kind: NodeKind::Text("yz".into()), children: Vec::new(), namespace: crate::html::Namespace::None });
    doc.nodes[parent].children.push(second);
    let (mut doc, mut forms, mut page) = start(doc);
    let before = page.state.retained;
    let final_bytes = before - 64 + 3 - child_link_bytes(&doc.nodes[parent].children) + size_of::<usize>();
    page.state.retained_limit = final_bytes - 1;
    assert!(Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.normalize_node(node_handle(parent)).is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(doc.nodes[parent].children, [first, second]);
    page.begin_job();
    page.state.retained_limit = final_bytes;
    Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.normalize_node(node_handle(parent)).unwrap();
    assert_eq!(page.state.retained, final_bytes);
    assert_eq!(doc.nodes[first].kind.character_data(), Some("xyz"));
    assert_eq!(doc.nodes[second].kind.character_data(), Some("yz"));
    assert!(page.state.detached_nodes.contains(&second));
    assert_eq!(doc.nodes[parent].children, [first]);
}

#[test]
fn character_storage_clone_compacts_source_and_content_moves_real_capacity() {
    let mut doc = crate::html::parse("<html><body><p>x</p></body></html>");
    let parent = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "p")).unwrap();
    let source = doc.nodes[parent].children[0];
    doc.nodes[source].kind = NodeKind::Text(wide("x", 128));
    let (mut doc, mut forms, mut page) = start(doc);
    let before = page.state.retained;
    let clone_cost=1+Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.main_node_minimum(1).unwrap();
    page.state.retained_limit = before + clone_cost;
    let Value::Host(clone) = (Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }).clone_node(node_handle(source), false).unwrap() else { panic!("real clone") };
    assert_eq!(node_bytes(&doc.nodes[node_index(clone).unwrap()]), 1);
    assert_eq!(page.state.retained, before + clone_cost);
    let text_cost=64+Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.main_node_minimum(1).unwrap();
    page.state.retained_limit = before + clone_cost + text_cost;
    Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.set_text_content(parent, wide("new", 64)).unwrap();
    assert_eq!(page.state.retained, before + clone_cost + text_cost);
    assert_eq!(doc.nodes[source].kind.character_data(), Some("x"));
    assert!(page.state.detached_nodes.contains(&source));
    let count = doc.nodes.len();
    Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.set_text_content(parent, wide("", 64)).unwrap();
    assert_eq!(doc.nodes.len(), count);
    assert_eq!(page.state.retained, before + clone_cost + text_cost);
}
