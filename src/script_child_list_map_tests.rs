//! Original native SameObject index storage and promotion acceptance.
use super::*;
fn page(markup:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse_loaded_document(markup,&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
#[test]
fn browser_child_list_index_retains_seventy_thousand_real_sameobject_lists_with_actual_backing() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body>{}</body></html>","<div></div>".repeat(70000)));
    execute(&mut page,&mut doc,&mut forms,"var all=document.querySelectorAll('div'),last;if(all.length!==70000)throw 'loaded tree';");
    let baseline=page.state.retained;let records=page.state.node_lists.capacity()*size_of::<queries::NodeList>();
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<all.length;i++){last=all[i].childNodes;if(last.length!==0||last!==all[i].childNodes)throw 'real SameObject';}");
    assert_eq!(page.state.dom.child_lists.len(),70000);
    assert_eq!(page.state.node_lists.len(),70001);
    assert_eq!(page.state.retained,baseline+page.state.dom.child_lists.storage_bytes()+page.state.node_lists.capacity()*size_of::<queries::NodeList>()-records);
    assert!(page.state.retained<=page.state.retained_limit);
}
#[test]
fn browser_child_list_index_and_registry_joint_exact_fit_and_refusal_leave_no_capacity_published() {
    let(mut doc,mut forms,mut page)=page("<html><body><div></div><p></p></body></html>");
    let owner=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="div")).unwrap();
    let next=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="p")).unwrap();
    let baseline=page.state.retained;let limit=page.state.retained_limit;
    let slot=size_of::<queries::NodeList>()+size_of::<(usize,u32)>();
    page.state.retained_limit=baseline+slot-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_child_list(node_handle(owner)).is_err());
    assert_eq!(page.state.retained,baseline);assert_eq!(page.state.node_lists.capacity(),0);assert_eq!(page.state.dom.child_lists.storage_bytes(),0);
    page.state.retained_limit+=1;page.begin_job();
    let handle=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_child_list(node_handle(owner)).unwrap();
    assert_eq!(page.state.retained,baseline+slot);
    assert!(matches!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_child_list(node_handle(owner)).unwrap(),Value::Host(id) if matches!(handle,Value::Host(saved) if id==saved)));
    page.state.retained_limit=limit;let before=page.state.retained;page.begin_job();page.state.copy_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_child_list(node_handle(next)).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.retained,before);assert_eq!(page.state.node_lists.len(),1);assert_eq!(page.state.dom.child_lists.len(),1);
    assert_eq!(page.state.dom.child_lists.storage_bytes(),size_of::<(usize,u32)>());
    page.begin_job();Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_child_list(node_handle(next)).unwrap();
    assert_eq!(page.state.dom.child_lists.len(),2);
}
#[test]
fn browser_inert_child_list_index_reuses_sameobject_through_real_promotion_and_keeps_backing_charged() {
    let(mut doc,mut forms,mut page)=page("<html><body><template id=t><div><b>inert</b></div></template></body></html>");
    execute(&mut page,&mut doc,&mut forms,"var template=document.getElementById('t');template.innerHTML='<div><b>inert</b></div>';var fragment=template.content,node=fragment.firstChild,kept=node.childNodes,fragmentList=fragment.childNodes;if(kept.length!==1||kept!==node.childNodes)throw 'inert SameObject';");
    let inert=page.state.dom.inert_child_lists.storage_bytes();assert!(inert>0);
    let Value::Host(fragment)=execute(&mut page,&mut doc,&mut forms,"fragment;") else{panic!("Expected real fragment")};
    let parent=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="body")).unwrap();
    let before=(page.state.retained,page.state.dom.child_lists.storage_bytes(),page.state.dom.inert_child_lists.len(),format!("{:?}",doc.nodes));
    let limit=page.state.retained_limit;page.state.retained_limit=page.state.retained;page.begin_job();
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.splice_checked_inert_fragment(parent,fragment,None).unwrap_err().contains("retained promotion"));
    assert_eq!((page.state.retained,page.state.dom.child_lists.storage_bytes(),page.state.dom.inert_child_lists.len(),format!("{:?}",doc.nodes)),before);
    page.state.retained_limit=limit;
    execute(&mut page,&mut doc,&mut forms,"document.body.appendChild(fragment);if(kept!==node.childNodes||kept.length!==1||kept[0]!==node.firstChild||fragmentList!==fragment.childNodes||fragmentList.length!==0||!node.isConnected)throw 'promoted SameObject';");
    assert_eq!(page.state.dom.inert_child_lists.storage_bytes(),inert);
    assert!(page.state.dom.child_lists.storage_bytes()>0);
    assert_eq!(page.state.dom.child_lists.len(),1);
    assert_eq!(page.state.dom.inert_child_lists.len(),1);
    assert!(page.state.retained>=page.state.dom.child_lists.storage_bytes()+inert+page.state.node_lists.capacity()*size_of::<queries::NodeList>());
}
#[test]
fn native_sorted_identity_index_preserves_order_and_prepaid_backing_across_removal() {
    let mut map=IdentityMap::<u32>::default();
    for key in (0..4000).rev(){let (next,_)=map.prepare(1,1024*1024).unwrap();map.commit(next);map.insert(key,key+10);}
    let bytes=map.storage_bytes();
    for key in (0..4000).step_by(2){assert_eq!(map.remove(&key),Some(key+10));}
    assert_eq!(map.storage_bytes(),bytes);
    for key in 0..4000{assert_eq!(map.get(&key).copied(),if key%2==0{None}else{Some(key+10)});}
    let before=map.len();assert!(map.prepare(4000,0).is_err());assert_eq!(map.len(),before);assert_eq!(map.storage_bytes(),bytes);
}
