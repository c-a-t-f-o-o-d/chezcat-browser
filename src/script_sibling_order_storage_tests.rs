//! Original paid topology and detached clone traversal acceptance.
use super::*;
fn page()->(Document,Forms,Page) {
    let mut doc=crate::html::parse_loaded_document("<html><body><div><b></b><b></b></div></body></html>",&||Ok(())).unwrap();
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
#[test]
fn sibling_order_detached_deep_clone_queries_use_complete_real_topology_without_forced_rebuild() {
    let(mut doc,mut forms,mut page)=page();
    let source=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="div")).unwrap();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let Value::Host(clone)=bridge.clone_node(node_handle(source),true).unwrap()else{panic!("real clone")};
    assert!(!bridge.state.dirty);
    let children=bridge.doc.nodes[node_index(clone).unwrap()].children.clone();
    let Value::Host(list)=bridge.selector_query(clone,"querySelectorAll","b").unwrap()else{panic!("real NodeList")};
    assert!(matches!(bridge.get(list,"length").unwrap(),Value::Number(count) if count==2.0));
    for (slot,child) in children.iter().enumerate(){
        assert_eq!(bridge.state.tree.sibling_slots[*child],slot);
        assert_eq!(bridge.state.tree.parents[*child],node_index(clone));
    }
}

#[test]
fn sibling_order_empty_text_replacement_preserves_existing_paid_slot_rows() {
    let(mut doc,mut forms,mut page)=page();
    let source=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="div")).unwrap();
    let count=doc.nodes.len();let capacity=page.state.tree.sibling_slots.capacity();let bytes=page.state.node_state_bytes();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.set_text_content(source,String::new()).unwrap();
    assert_eq!(bridge.doc.nodes.len(),count);assert_eq!(bridge.state.tree.sibling_slots.len(),count);
    assert_eq!(bridge.state.tree.sibling_slots.capacity(),capacity);assert_eq!(bridge.state.node_state_bytes(),bytes);
    assert!(bridge.state.dirty);bridge.state.rebuild(bridge.doc).unwrap();
    assert_eq!(bridge.state.tree.sibling_slots.len(),count);
}
