//! Original paid order/stack ownership, replacement credit and recovery cases.
use super::*;
fn page(count:usize,ids:bool)->(Document,Forms,Page) {
    let mut markup=String::from("<html><body><div>");
    for index in 0..count {if ids{markup.push_str(&format!("<p id='key{index:05}'></p>"));}else{markup.push_str("<p></p>");}}
    markup.push_str("</div></body></html>");
    let mut doc=crate::html::parse_loaded_document(&markup,&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}
fn evict_scratch(page:&mut Page) {
    let old=std::mem::take(&mut page.state.tree_stack);
    page.state.retained-=old.capacity()*size_of::<tree_order::Visit>();drop(old);
    page.state.dirty=true;page.state.copy_work=page.state.limits.copy_bytes;page.state.host_work=page.state.limits.host_work;
}

#[test]
fn anonymous_leaf_appends_preserve_current_order_id_ordinals_and_sibling_slots() {
    let (mut doc, mut forms, mut page) = page(11, true);
    let result = page.console_eval(&mut doc, &mut forms,
        "var early=document.getElementById('key00000'),later=document.getElementById('key00001');var added=document.createElement('span');early.appendChild(added);");
    assert_eq!(result.errors, 0, "{}", page.report());
    assert!(!page.state.dirty);
    for &(node, ordinal) in &page.state.tree.ids {
        assert_eq!(page.state.tree.order[ordinal], node);
    }
    let result = page.console_eval(&mut doc, &mut forms, r#"
if(early.firstChild!==added||added.parentNode!==early||added.nextSibling!==null||!added.isConnected)throw 'appended topology';
if(added.compareDocumentPosition(later)!==4||document.getElementById('key00001')!==later)throw 'live order';
added.id='key00001';if(document.getElementById('key00001')!==added)throw 'duplicate order after ID mutation';
early.removeChild(added);if(document.getElementById('key00001')!==later||added.isConnected)throw 'removal rebuild';
"#);
    assert_eq!(result.errors, 0, "{}", page.report());
}
#[test]
fn tree_order_storage_six_thousand_real_nodes_pay_actual_order_and_reusable_stack_capacity() {
    let(doc,_,mut page)=page(6000,false);
    assert_eq!(page.state.retained,document_bytes(&doc)+page.state.tree_storage_bytes());
    assert_eq!(page.state.tree.order.len(),doc.nodes.len());
    assert!(page.state.tree_stack.is_empty());assert!(page.state.tree_stack.capacity()>=6000);
    let before=page.state.retained;let cap=page.state.tree_stack.capacity();let order=page.state.tree.order.clone();
    page.state.retained_limit=before;page.state.dirty=true;page.state.rebuild(&doc).unwrap();
    assert_eq!(page.state.retained,before);assert_eq!(page.state.tree.order,order);assert_eq!(page.state.tree_stack.capacity(),cap);
}
#[test]
fn tree_order_storage_late_copy_refusal_keeps_new_scratch_paid_and_old_tree_then_recovers() {
    let(doc,_,mut calibration)=page(32,true);evict_scratch(&mut calibration);
    let copies=calibration.state.copy_work;calibration.state.rebuild(&doc).unwrap();let spent=copies-calibration.state.copy_work;
    let(doc,_,mut page)=page(32,true);evict_scratch(&mut page);
    let before=page.state.retained;let order=page.state.tree.order.clone();let ids=page.state.tree.ids.clone();
    page.state.copy_work=spent-1;
    assert!(page.state.rebuild(&doc).is_err());assert!(page.state.dirty);assert!(page.state.tree_stack.is_empty());
    assert_eq!(page.state.tree.order,order);assert_eq!(page.state.tree.ids,ids);
    assert!(page.state.tree_stack.capacity()>=32);
    assert_eq!(page.state.retained,before+page.state.tree_stack.capacity()*size_of::<tree_order::Visit>());
    page.state.retained_limit=page.state.retained;page.state.copy_work=page.state.limits.copy_bytes;page.state.host_work=page.state.limits.host_work;
    page.state.rebuild(&doc).unwrap();assert!(!page.state.dirty);
    assert_eq!(page.state.retained,document_bytes(&doc)+page.state.tree_storage_bytes());
    assert!(page.state.document_id(&doc,"key00031").unwrap().is_some());
}
#[test]
fn tree_order_storage_exact_replacement_credit_funds_scratch_and_failed_credit_is_evicted() {
    let(doc,_,mut page)=page(32,true);
    assert_eq!(page.state.tree_stack.capacity(),32);
    let scratch=page.state.tree_stack.capacity()*size_of::<tree_order::Visit>();evict_scratch(&mut page);
    let before=page.state.retained;let order=page.state.tree.order.clone();let ids=page.state.tree.ids.clone();
    let old_order=page.state.order_bytes();let needed_order=order.len()*size_of::<usize>();assert!(old_order>needed_order);
    page.state.retained_limit=before+scratch-old_order+needed_order-1;
    assert!(page.state.rebuild(&doc).is_err());assert!(page.state.dirty);
    assert_eq!(page.state.tree_stack.capacity(),0);assert_eq!(page.state.retained,before);
    assert_eq!(page.state.tree.order,order);assert_eq!(page.state.tree.ids,ids);
    page.state.retained_limit+=1;page.state.copy_work=page.state.limits.copy_bytes;page.state.host_work=page.state.limits.host_work;
    page.state.rebuild(&doc).unwrap();assert!(!page.state.dirty);
    assert_eq!(page.state.retained,page.state.retained_limit);
    assert_eq!(page.state.retained,document_bytes(&doc)+page.state.tree_storage_bytes());
    assert_eq!(page.state.tree_stack.capacity(),32);assert_eq!(page.state.order_bytes(),needed_order);
}
