//! Original dataset output backing, exact conversion and default-path acceptance.
use super::*;
fn page(markup:&str)->(Document,Forms,Page,usize){
    let mut doc=crate::html::parse_loaded_document(markup,&||Ok(())).unwrap();doc.scripting_enabled=true;let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="p")).unwrap();let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page,node)
}
#[test]
fn browser_dataset_six_thousand_actual_keys_keep_order_and_live_write_delete() {
    let mut markup=String::from("<html><body><p");for index in 0..6000{markup.push_str(&format!(" data-k{index}='{index}'"));}markup.push_str("></p></body></html>");
    let(mut doc,mut forms,mut page,_)=page(&markup);
    let source="var node=document.querySelector('p'),data=node.dataset,keys=Object.keys(data);if(keys.length!==6000||keys[0]!=='k0'||keys[5999]!=='k5999'||data.k5999!=='5999')throw 'real ordered keys';data.k0='updated';delete data.k1;node.setAttributeNS(null,'data-Upper','excluded');node.setAttributeNS('urn:fixture','data-hidden','excluded');keys=Object.keys(data);if(keys.length!==5999||keys[1]!=='k2'||data.k0!=='updated'||node.hasAttribute('data-k1'))throw 'live dataset';";
    page.runtime.execute(source,&mut Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}).unwrap();assert!(page.state.retained<=page.state.retained_limit);
}
#[test]
fn browser_dataset_exact_converted_text_and_vector_refusal_keep_dom_and_recover() {
    let(mut doc,mut forms,mut page,node)=page("<html><body><p data-first-name=x data-x--y=y data-α=z title=plain></p></body></html>");
    let before=page.state.retained;let limit=page.state.retained_limit;let commits=page.state.dom_commits;
    let bundle=3*size_of::<String>()+"firstName".len()+"x-Y".len()+"α".len();page.state.retained_limit=before+bundle-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.dataset_keys(DATASET_BASE+node as u32).is_err());assert_eq!(page.state.retained,before);assert_eq!(page.state.dom_commits,commits);
    page.state.retained_limit+=1;page.begin_job();
    let keys=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.dataset_keys(DATASET_BASE+node as u32).unwrap();assert_eq!(keys,["firstName","x-Y","α"]);assert_eq!(keys.capacity()*size_of::<String>()+keys.iter().map(String::capacity).sum::<usize>(),bundle);drop(keys);
    page.begin_job();page.state.copy_work=bundle-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.dataset_keys(DATASET_BASE+node as u32).unwrap_err().contains("string work limit"));assert_eq!(page.state.retained,before);assert_eq!(page.state.dom_commits,commits);
    page.state.retained_limit=limit;page.begin_job();assert_eq!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.dataset_keys(DATASET_BASE+node as u32).unwrap().len(),3);
}
#[test]
fn browser_dataset_empty_unicode_uppercase_and_namespace_filter_use_only_real_keys() {
    let(mut doc,mut forms,mut page,node)=page("<html><body><p title=ordinary data-=empty data-α=unicode></p></body></html>");
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.set_attribute_ns(node_handle(node),&[Value::String("urn:fixture".into()),Value::String("data-hidden".into()),Value::String("excluded".into())]).unwrap();
    bridge.set_attribute_ns(node_handle(node),&[Value::Null,Value::String("data-Upper".into()),Value::String("excluded".into())]).unwrap();
    let keys=bridge.dataset_keys(DATASET_BASE+node as u32).unwrap();assert_eq!(keys,["","α"]);assert_eq!(keys[0].capacity(),0);
    // Unicode metadata writes retain their existing unsupported boundary.
    // An independently authored empty dataset needs no output allocation.
    let(mut doc,mut forms,mut page,node)=self::page("<html><body><p title=ordinary></p></body></html>");
    page.state.retained_limit=page.state.retained;page.state.copy_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.dataset_keys(DATASET_BASE+node as u32).unwrap().is_empty());assert_eq!(page.state.copy_work,0);
}
#[test]
fn regression_dataset_count_is_explicit_and_does_not_change_attribute_values() {
    let mut doc=crate::html::parse("<html><body><p data-a=one data-b=two></p></body></html>");doc.scripting_enabled=true;let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="p")).unwrap();let mut forms=Forms::new(&doc);let mut limits=crate::javascript::limits::CAPACITY_REGRESSION;limits.host.element_attributes=1;let mut page=Page::with_limits(limits);assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    let before=page.state.retained;assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.dataset_keys(DATASET_BASE+node as u32).unwrap_err().contains("Dataset enumeration limit reached (1 attributes)"));assert_eq!(page.state.retained,before);assert_eq!(doc.attr(node,"data-a"),Some("one"));
    page.state.limits.host.element_attributes=2;page.begin_job();assert_eq!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.dataset_keys(DATASET_BASE+node as u32).unwrap(),["a","b"]);
}
