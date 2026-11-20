//! Original joint per-node backing and rebuild-refund acceptance.
use super::*;
fn page()->(Document,Forms,Page) {
    let mut doc=crate::html::parse_loaded_document("<html><body><p>kept</p></body></html>",&||Ok(())).unwrap();
    let mut nodes=Vec::new();nodes.try_reserve_exact(doc.nodes.len()+8).unwrap();nodes.append(&mut doc.nodes);doc.nodes=nodes;
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn capacities(state:&State)->(usize,usize,usize,usize) {(state.tree.active.capacity(),state.tree.parents.capacity(),state.dom.connected.capacity(),state.tree.sibling_slots.capacity())}
#[test]
fn node_state_joint_exact_metadata_refuses_before_publishing_any_vector_and_reuses_paid_spares() {
    let(mut doc,mut forms,mut page)=page();let nodes=doc.nodes.len();let core=doc.nodes.capacity();
    let before=page.state.retained;let old=capacities(&page.state);assert_eq!(old,(nodes,nodes,nodes,nodes));
    assert_eq!(before,document_bytes(&doc)+page.state.tree_storage_bytes());
    let bytes=2+size_of::<Option<usize>>()+size_of::<usize>();page.state.retained_limit=before+bytes-1;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(bridge.create_dom_node(NodeKind::DocumentFragment).is_err());
    assert_eq!(capacities(bridge.state),old);assert_eq!(bridge.doc.nodes.len(),nodes);assert_eq!(bridge.doc.nodes.capacity(),core);
    bridge.state.retained_limit+=1;bridge.state.copy_work=0;
    assert!(bridge.create_dom_node(NodeKind::DocumentFragment).is_err());assert_eq!(capacities(bridge.state),old);
    bridge.state.copy_work=(nodes+1)*bytes+size_of::<Node>();
    bridge.create_dom_node(NodeKind::DocumentFragment).unwrap();
    assert_eq!(bridge.state.copy_work,0);assert_eq!(bridge.state.retained,before+bytes);
    assert_eq!(capacities(bridge.state),(nodes+1,nodes+1,nodes+1,nodes+1));
    assert_eq!(bridge.doc.nodes.capacity(),core);
}
#[test]
fn node_state_rebuild_refunds_dropped_capacity_and_copy_failure_preserves_the_previous_tree() {
    let(mut doc,mut forms,mut page)=page();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_dom_node(NodeKind::DocumentFragment).unwrap();
    let before=page.state.retained;let old=page.state.node_state_bytes();let caps=capacities(&page.state);
    let compact=doc.nodes.len()*(2+size_of::<Option<usize>>()+size_of::<usize>());assert!(old>compact);
    page.state.retained_limit=before-old+compact;page.state.dirty=true;page.state.copy_work=0;
    assert!(page.state.rebuild(&doc).is_err());assert_eq!(page.state.retained,before);assert_eq!(capacities(&page.state),caps);
    assert!(page.state.dirty);page.state.copy_work=page.state.limits.copy_bytes;
    page.state.rebuild(&doc).unwrap();assert_eq!(page.state.retained,before-old+compact);
    assert_eq!(page.state.node_state_bytes(),compact);assert!(!page.state.dirty);
    assert!(!page.state.dom.connected[doc.nodes.len()-1]);
    assert_eq!(page.state.tree.parents[doc.nodes.len()-1],None);
}
