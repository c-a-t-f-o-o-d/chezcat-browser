//! Original live collection identity, filter backing and enumeration acceptance.
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
fn browser_live_tag_and_class_collections_exceed_old_identity_range_and_keep_actual_live_behavior() {
    let(mut doc,mut forms,mut page)=page("<html><body><div id=one class='red blue'></div><div id=two class=red></div></body></html>");
    let baseline=page.state.retained;
    execute(&mut page,&mut doc,&mut forms,"var tags,classes;for(var i=0;i<70000;i++)tags=document.getElementsByTagName('DIV');for(var i=0;i<70000;i++)classes=document.getElementsByClassName('red red\\tblue');if(tags.length!==2||tags.item(1)!==document.getElementById('two')||tags.namedItem('one')!==classes[0]||classes.length!==1)throw 'large live registries';");
    assert_eq!(page.state.dom.collections.len(),140000);
    let payload=page.state.dom.collections.iter().map(|collection|collection.filter.owned_bytes()).sum::<usize>();
    assert_eq!(page.state.retained,baseline+payload+page.state.dom.collections.capacity()*size_of::<TagCollection>());
    execute(&mut page,&mut doc,&mut forms,"var node=document.getElementById('two');node.className='blue red';if(classes.length!==2||classes[1]!==node)throw 'live class update';document.body.removeChild(node);if(classes.length!==1||tags.length!==1)throw 'live topology update';");
    for handle in [256,512,NODE_BASE,NODE_LIST_BASE,COLLECTION_BASE-1,COLLECTION_END] {assert!(collection_handle_index(handle).is_none());}
    assert_eq!(collection_handle_index(COLLECTION_BASE+139999),Some(139999));
    assert!(page.state.retained<=page.state.retained_limit);
}
#[test]
fn browser_live_collection_registry_and_filter_joint_admission_are_atomic_and_exact() {
    let(mut doc,mut forms,mut page)=page("<html><body><p></p></body></html>");
    let baseline=page.state.retained;let limit=page.state.retained_limit;let slot=size_of::<TagCollection>();
    let value=Value::String("p".into());page.state.retained_limit=baseline+slot;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).is_err());
    assert_eq!(page.state.dom.collections.capacity(),0);assert_eq!(page.state.retained,baseline);
    page.state.retained_limit+=1;page.begin_job();
    Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).unwrap();
    assert_eq!(page.state.dom.collections.capacity(),1);assert_eq!(page.state.retained,baseline+slot+1);
    page.state.retained_limit=limit;page.begin_job();page.state.copy_work=slot;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).unwrap_err().contains("string work limit"));
    assert_eq!(page.state.dom.collections.len(),1);assert_eq!(page.state.dom.collections.capacity(),1);assert_eq!(page.state.retained,baseline+slot+1);
    page.begin_job();Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_tag_collection(DOCUMENT,&value).unwrap();
    assert_eq!(page.state.dom.collections.len(),2);
}
#[test]
fn browser_class_filter_staging_refusal_preserves_registry_and_recovers_with_deduplicated_owned_names() {
    let(mut doc,mut forms,mut page)=page("<html><body><p class='a b'></p></body></html>");
    let baseline=page.state.retained;let limit=page.state.retained_limit;let value=Value::String("b a b".into());
    page.state.retained_limit=baseline+4;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_class_collection(DOCUMENT,&value).unwrap_err().contains("class-filter scratch"));
    assert_eq!(page.state.retained,baseline);assert_eq!(page.state.dom.collections.capacity(),0);
    page.state.retained_limit=limit;page.begin_job();
    let Value::Host(handle)=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.create_class_collection(DOCUMENT,&value).unwrap() else{panic!("Expected collection")};
    let Filter::Classes(names)=&page.state.dom.collections[0].filter else{panic!("Expected class filter")};assert_eq!(names,&["a","b"]);
    let payload=names.capacity()*size_of::<String>()+names.iter().map(String::capacity).sum::<usize>();
    assert_eq!(page.state.retained,baseline+payload+page.state.dom.collections.capacity()*size_of::<TagCollection>());
    assert_eq!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.tag_collection_get(handle,"length").unwrap().to_number(),1.0);
}
#[test]
fn browser_live_collection_enumerates_six_thousand_indices_and_refuses_key_storage_atomically() {
    let(mut doc,mut forms,mut page)=page(&format!("<html><body>{}</body></html>","<p></p>".repeat(6000)));
    execute(&mut page,&mut doc,&mut forms,"var collection=document.getElementsByTagName('p'),keys=Object.keys(collection);if(keys.length!==6000||keys[0]!=='0'||keys[5999]!=='5999')throw 'real index enumeration';");
    let baseline=page.state.retained;let limit=page.state.retained_limit;page.state.retained_limit=baseline;
    page.begin_job();assert!(page.state.index_keys(6000).unwrap_err().contains("collection key allowance"));assert_eq!(page.state.retained,baseline);
    page.state.retained_limit=limit;page.begin_job();let keys=page.state.index_keys(6000).unwrap();assert_eq!(keys.len(),6000);assert_eq!(keys[5999],"5999");
    assert_eq!(page.state.retained,baseline);
}
