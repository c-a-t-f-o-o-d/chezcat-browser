//! Original native bitmap-domain joint backing and genuine owner acceptance.
use super::*;
fn page(spare:usize)->(Document,Forms,Page){
    let markup=format!("<html><body>{}</body></html>","<p></p>".repeat(252));
    let mut doc=crate::html::parse_loaded_document(&markup,&||Ok(())).unwrap();assert_eq!(doc.nodes.len(),255);
    let mut nodes=Vec::new();nodes.try_reserve_exact(doc.nodes.len()+spare).unwrap();nodes.append(&mut doc.nodes);doc.nodes=nodes;
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn stores(state:&State)->[usize;5]{[state.dom.created.storage_bytes(),state.detached_nodes.storage_bytes(),state.detached_images.storage_bytes(),state.dom.fragment_scripts.storage_bytes(),state.dom.force_async.storage_bytes()]}
#[test]
fn node_identity_joint_exact_five_bitmap_growth_preserves_old_flags_and_publishes_once(){
    let(mut doc,mut forms,mut page)=page(8);
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let Value::Host(held)=bridge.create_dom_node(NodeKind::DocumentFragment).unwrap()else{panic!("real Fragment")};
    assert_eq!(bridge.doc.nodes.len(),256);assert_eq!(stores(bridge.state),[4*size_of::<u64>();5]);
    let before=bridge.state.retained;let core=bridge.doc.nodes.capacity();let needed=5*size_of::<u64>();
    bridge.state.retained_limit=before+needed-1;
    assert!(bridge.create_dom_node(NodeKind::DocumentFragment).is_err());
    assert_eq!(stores(bridge.state),[4*size_of::<u64>();5]);assert_eq!(bridge.doc.nodes.len(),256);assert_eq!(bridge.doc.nodes.capacity(),core);
    assert_eq!(bridge.state.retained,before);assert!(bridge.state.dom.created.contains(&255));assert!(bridge.state.detached_nodes.contains(&255));
    bridge.state.retained_limit+=1;bridge.state.copy_work=0;
    assert!(bridge.create_dom_node(NodeKind::DocumentFragment).is_err());assert_eq!(stores(bridge.state),[4*size_of::<u64>();5]);
    bridge.state.copy_work=size_of::<Node>()+2+size_of::<Option<usize>>()+size_of::<usize>()+5*5*size_of::<u64>();
    let Value::Host(created)=bridge.create_dom_node(NodeKind::DocumentFragment).unwrap()else{panic!("real Fragment")};
    assert_eq!(created,node_handle(256));assert_eq!(held,node_handle(255));assert_eq!(bridge.state.copy_work,0);
    assert_eq!(stores(bridge.state),[5*size_of::<u64>();5]);assert_eq!(bridge.state.retained,before+needed);
    for node in [255,256]{assert!(bridge.state.dom.created.contains(&node));assert!(bridge.state.detached_nodes.contains(&node));assert!(!bridge.state.detached_images.contains(&node));assert!(!bridge.state.dom.fragment_scripts.contains(&node));assert!(!bridge.state.dom.force_async.contains(&node));}
}
#[test]
fn node_identity_loaded_detach_reuses_paid_domain_at_full_retained_allowance(){
    let(mut doc,mut forms,mut page)=page(0);let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="p")).unwrap();
    let before=page.state.retained;let stores_before=stores(&page.state);assert_eq!(before,document_bytes(&doc)+page.state.tree_storage_bytes());
    page.state.retained_limit=before;
    let result=page.console_eval(&mut doc,&mut forms,"document.body.removeChild(document.body.firstChild);");
    assert_eq!(result.errors,0,"{}",page.report());assert_eq!(page.state.retained,before);assert_eq!(stores(&page.state),stores_before);
    assert!(page.state.detached_nodes.contains(&node));assert!(!page.state.dom.created.contains(&node));assert_eq!(page.state.detached_nodes.len(),1);
    page.state.rebuild(&doc).unwrap();assert_eq!(page.state.retained,before);assert_eq!(page.state.tree.parents[node],None);assert!(!page.state.dom.connected[node]);
}
