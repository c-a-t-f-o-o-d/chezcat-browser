//! Original NodeList registry and snapshot capacity acceptance.
use super::*;
fn page(markup:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse(markup);doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::with_limits(crate::javascript::limits::BROWSER);
    let result=page.start(&mut doc,&mut forms,&[]);assert_eq!(result.errors,0,"{}",page.report());(doc,forms,page)
}
fn execute(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str)->Value {
    page.begin_job();page.runtime.execute(source,&mut Bridge{doc,forms,state:&mut page.state}).unwrap()
}
#[test]
fn browser_node_lists_exceed_former_identity_result_and_total_reference_counts() {
    let markup=format!("<html><head></head><body>{}</body></html>","<div name=match></div>".repeat(6000));
    let(mut doc,mut forms,mut page)=page(&markup);
    execute(&mut page,&mut doc,&mut forms,"var named;for(var i=0;i<70000;i++)named=document.getElementsByName('match');if(named.length!==6000||named[5999].tagName!=='DIV')throw 'live name list';var kept=document.querySelectorAll('div'),tail=kept[5999];if(kept.length!==6000||tail!==named[5999])throw 'snapshot result';");
    assert_eq!(page.state.node_lists.len(),70001);
    execute(&mut page,&mut doc,&mut forms,"for(var i=0;i<180;i++)document.querySelectorAll('div');document.body.removeChild(tail);if(kept.length!==6000||kept[5999]!==tail||named.length!==5999)throw 'snapshot and live semantics';");
    assert_eq!(page.state.node_list_references,181*6000);
    assert!(page.state.node_list_references>1_048_576);
    let payload=page.state.node_lists.iter().map(|list|match list{NodeList::Name(name)=>name.capacity(),NodeList::Snapshot(nodes)=>nodes.capacity()*size_of::<usize>(),NodeList::MutationSnapshot(nodes)=>nodes.capacity()*size_of::<u32>(),_=>0}).sum::<usize>();
    assert!(page.state.retained>=payload+page.state.node_lists.capacity()*size_of::<NodeList>());
    assert!(page.state.retained<=page.state.retained_limit);
}
#[test]
fn browser_node_list_record_payload_and_growth_copy_refusal_are_atomic() {
    let(mut doc,mut forms,mut page)=page("<html><body>List storage</body></html>");
    let baseline=page.state.retained;let limit=page.state.retained_limit;let slot=size_of::<NodeList>();
    let name=Value::String("query".into());page.state.retained_limit=baseline+slot+5-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_name_list(&name).is_err());
    assert_eq!(page.state.node_lists.capacity(),0);assert_eq!(page.state.retained,baseline);
    page.state.retained_limit=baseline+slot+5;
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_name_list(&name).unwrap();
    assert_eq!(page.state.node_lists.capacity(),1);assert_eq!(page.state.retained,baseline+slot+5);
    page.state.retained_limit=limit;page.state.copy_work=slot+5-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_name_list(&name).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.node_lists.len(),1);assert_eq!(page.state.node_lists.capacity(),1);assert_eq!(page.state.retained,baseline+slot+5);
    page.begin_job();Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_name_list(&name).unwrap();
    assert_eq!(page.state.node_lists.len(),2);
}
#[test]
fn browser_mutation_snapshot_charges_actual_payload_and_publishes_only_complete_records() {
    let(mut doc,mut forms,mut page)=page("<html><body>Mutation nodes</body></html>");
    let baseline=page.state.retained;let slot=size_of::<NodeList>();let nodes=[WINDOW,DOCUMENT];
    page.state.retained_limit=baseline+slot+std::mem::size_of_val(&nodes)-1;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.snapshot_mutation_node_list(&nodes).is_err());
    assert_eq!(page.state.node_lists.len(),0);assert_eq!(page.state.node_lists.capacity(),0);assert_eq!(page.state.retained,baseline);
    page.state.retained_limit+=1;page.begin_job();
    let handle=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.snapshot_mutation_node_list(&nodes).unwrap();
    assert_eq!(page.state.retained,baseline+slot+std::mem::size_of_val(&nodes));
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(matches!(bridge.node_list_get(handle,"0").unwrap(),Value::Host(WINDOW)));
    assert_eq!(bridge.node_list_get(handle,"length").unwrap().to_number(),2.0);
}
#[test]
fn browser_selector_storage_and_copy_refusal_keep_saved_snapshots_and_recover() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body>{}</body></html>","<p></p>".repeat(6000)));
    execute(&mut page,&mut doc,&mut forms,"var kept=document.querySelectorAll('p');");
    let before=(page.state.retained,page.state.node_lists.len(),page.state.node_lists.capacity(),page.state.node_list_references);
    let limit=page.state.retained_limit;page.state.retained_limit=page.state.retained;
    page.begin_job();assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.selector_query(DOCUMENT,"querySelectorAll","p").unwrap_err().contains("DOM retained selector result allowance"));
    assert_eq!((page.state.retained,page.state.node_lists.len(),page.state.node_lists.capacity(),page.state.node_list_references),before);
    page.state.retained_limit=limit;page.begin_job();page.state.copy_work=0;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.selector_query(DOCUMENT,"querySelectorAll","p").unwrap_err().contains("copy work limit"));
    assert_eq!((page.state.retained,page.state.node_lists.len(),page.state.node_lists.capacity(),page.state.node_list_references),before);
    execute(&mut page,&mut doc,&mut forms,"if(kept.length!==6000||kept[5999].tagName!=='P'||document.querySelectorAll('p').length!==6000)throw 'snapshot recovery';");
}

#[test]
fn browser_selector_five_results_fit_exactly_with_one_complete_registry_record() {
    let(mut doc,mut forms,mut page)=page("<html><body><p></p><p></p><p></p><p></p><p></p></body></html>");
    let baseline=page.state.retained;page.state.retained_limit=baseline+size_of::<NodeList>()+5*size_of::<usize>();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let Value::Host(handle)=bridge.selector_query(DOCUMENT,"querySelectorAll","p").unwrap() else{panic!("Expected list")};
    assert_eq!(bridge.node_list_get(handle,"length").unwrap().to_number(),5.0);
    assert_eq!(bridge.state.node_lists.capacity(),1);assert_eq!(bridge.state.retained,bridge.state.retained_limit);
}
