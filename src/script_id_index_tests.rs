//! Original retained ID-index backing, duplicate order and failure acceptance.
use super::*;
fn page(markup:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse_loaded_document(markup,&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}

#[test]
fn id_index_storage_exact_record_and_copy_refusal_leave_previous_tree_and_recover() {
    let(mut doc,mut forms,mut page)=page("<html><head></head><body><p></p><i></i><i></i><i></i></body></html>");
    assert_eq!(page.state.tree.order.len(),page.state.tree.order.capacity(),"ID admission must not borrow spare order slots");
    let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="p")).unwrap();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.set(node_handle(node),"id",Value::String("new".into())).unwrap();
    assert!(bridge.state.dirty);assert!(bridge.state.tree.ids.is_empty());
    let before=bridge.state.retained;let slot=size_of::<(usize,usize)>();
    bridge.state.retained_limit=before+slot-1;
    assert!(bridge.state.rebuild(bridge.doc).is_err());
    assert_eq!(bridge.state.retained,before);assert_eq!(bridge.state.tree.ids.capacity(),0);assert!(bridge.state.dirty);
    bridge.state.retained_limit+=1;bridge.state.copy_work=0;
    assert!(bridge.state.rebuild(bridge.doc).is_err());
    assert_eq!(bridge.state.retained,before);assert_eq!(bridge.state.tree.ids.capacity(),0);
    bridge.state.copy_work=bridge.state.limits.copy_bytes;
    bridge.state.rebuild(bridge.doc).unwrap();
    assert_eq!(bridge.state.retained,before+slot);assert_eq!(bridge.state.tree.ids.capacity(),1);
    assert_eq!(bridge.state.document_id(bridge.doc,"new").unwrap(),Some(node));
    bridge.state.host_work=0;assert!(bridge.state.document_id(bridge.doc,"new").is_err());
}

#[test]
fn id_index_duplicate_first_document_order_tracks_moves_renames_and_detach() {
    let(mut doc,mut forms,mut page)=page("<html><body><div id='same'></div><p id='same'></p></body></html>");
    let div=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="div")).unwrap();
    let p=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="p")).unwrap();
    assert_eq!(page.state.document_id(&doc,"same").unwrap(),Some(div));
    let outcome=page.console_eval(&mut doc,&mut forms,"var d=document.getElementById('same');document.body.appendChild(d);document.title=document.getElementById('same').tagName;");
    assert_eq!(outcome.errors,0);assert_eq!(doc.title,"P");
    assert_eq!(page.state.document_id(&doc,"same").unwrap(),Some(p));
    let outcome=page.console_eval(&mut doc,&mut forms,"document.getElementById('same').id='other';document.title=document.getElementById('same').tagName;document.body.removeChild(d);document.title=String(document.getElementById('same'));");
    assert_eq!(outcome.errors,0);assert_eq!(doc.title,"null");
    assert_eq!(page.state.document_id(&doc,"other").unwrap(),Some(p));
}

#[test]
fn id_index_many_real_owners_use_only_paid_record_capacity_and_borrow_wide_ids() {
    let mut markup=String::from("<html><body>");
    for index in (0..6_000).rev(){markup.push_str(&format!("<p id='key{index:05}'></p>"));}
    let wide_id="x".repeat(32_000);
    markup.push_str(&format!("<p id='{wide_id}'></p></body></html>"));
    let(doc,_,mut page)=page(&markup);
    assert_eq!(page.state.retained,document_bytes(&doc)+page.state.tree_storage_bytes());
    assert_eq!(page.state.tree.ids.len(),6_001);
    for index in [0,1,2999,5999] {
        let id=format!("key{index:05}");let node=page.state.document_id(&doc,&id).unwrap().unwrap();
        assert_eq!(doc.attr(node,"id"),Some(id.as_str()));
    }
    let first=page.state.document_id(&doc,"key00000").unwrap().unwrap();
    assert!(page.state.document_id(&doc,&wide_id).unwrap().is_some());
    // A replacement can reuse the old index allowance without copying ID text.
    let before=page.state.retained;
    page.state.dirty=true;
    page.state.retained_limit=page.state.retained;
    page.state.rebuild(&doc).unwrap();
    assert_eq!(page.state.retained,before);
    assert_eq!(page.state.document_id(&doc,"key00000").unwrap(),Some(first));
}

#[test]
fn id_index_replacement_credit_obeys_reduced_headroom_and_refunds_dropped_spares() {
    let(doc,_,mut page)=page("<html><head></head><body><p id='kept'></p><i></i><i></i><i></i></body></html>");
    assert_eq!(page.state.tree.order.len(),page.state.tree.order.capacity(),"only the ID index can refund spare slots");
    let owner=page.state.document_id(&doc,"kept").unwrap().unwrap();
    let before=page.state.retained;let slot=size_of::<(usize,usize)>();
    let old=page.state.tree.ids.capacity()*slot;assert!(old>slot);
    page.state.retained_limit=before-old+slot-1;page.state.dirty=true;
    assert!(page.state.rebuild(&doc).is_err());
    assert_eq!(page.state.retained,before);assert_eq!(page.state.tree.ids.capacity()*slot,old);
    assert!(page.state.dirty);assert_eq!(page.state.document_id(&doc,"kept").unwrap(),Some(owner));
    page.state.retained_limit+=1;page.state.rebuild(&doc).unwrap();
    assert_eq!(page.state.retained,before-old+slot);assert_eq!(page.state.tree.ids.capacity(),1);
    assert_eq!(page.state.document_id(&doc,"kept").unwrap(),Some(owner));
}
