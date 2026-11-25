//! Required Area identity-preserving connection retains actual attributes.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<html><body><map id=map>Fallback map<area id=list type=MiXeD nohref=legacy></map><input id=neighbor value=seed></body></html>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let location=crate::navigation::address("https://area-connection.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&location);p.set_resource_base(&location);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}

#[test]
fn area_connection_author_created_main_area_retains_identity_attributes_and_map_collection(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var target=document.getElementById('map'),first=target.areas[0],source=document.createElement('area');source.alt='A\udfffZ';source.href='leaf';source.noHref=true;source.rel='one two';var attr=source.getAttributeNode('alt'),tokens=source.relList,areas=target.areas;if(target.appendChild(source)!==source||source.parentNode!==target||source.ownerDocument!==document||source.alt!=='A\udfffZ'||source.href!=='https://area-connection.test/leaf'||source.noHref!==true||source.getAttributeNode('alt')!==attr||attr.ownerElement!==source||source.relList!==tokens||areas.length!==2||areas[0]!==first||areas[1]!==source)throw 'actual created area connection';target.removeChild(source);if(source.parentNode!==null||areas.length!==1||areas[0]!==first||attr.ownerElement!==source||tokens.value!=='one two')throw 'actual area disconnection';"#);}
#[test]
fn area_connection_inert_owner_moves_to_current_main_base_with_saved_handles(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var target=document.getElementById('map'),owner=document.implementation.createHTMLDocument(''),base=owner.createElement('base');base.setAttribute('href','https://other-area.test/root/');owner.head.appendChild(base);var source=owner.createElement('area');source.href='leaf';source.alt='source';var attr=source.getAttributeNode('href'),tokens=source.relList;if(source.href!=='https://other-area.test/root/leaf')throw 'real source base';var first=target.areas[0];if(target.insertBefore(source,first)!==source||source.ownerDocument!==document||source.parentNode!==target||source.href!=='https://area-connection.test/leaf'||source.getAttributeNode('href')!==attr||attr.ownerDocument!==document||source.relList!==tokens||target.areas[0]!==source||target.areas[1]!==first)throw 'identity owner and base transfer';"#);}
#[test]
fn area_connection_native_leaf_template_parking_retains_owner_attr_and_real_interface(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var template=document.createElement('template'),source=document.getElementById('list').cloneNode(false),attr=source.getAttributeNode('nohref');template.content.appendChild(source);if(!(source instanceof HTMLAreaElement)||source.parentNode!==template.content||source.ownerDocument!==template.content.ownerDocument||attr.ownerDocument!==source.ownerDocument||source.getAttributeNode('nohref')!==attr||source.noHref!==true||source.type!=='MiXeD')throw 'actual area template parking';document.body.appendChild(template);if(source.parentNode!==template.content||attr.ownerElement!==source||source.noHref!==true)throw 'template remains inert';"#);}

#[test]
fn area_connection_inert_clones_and_parsed_template_leaves_keep_real_round_trip_identity(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var target=document.getElementById('map'),areas=target.areas,owner=new Document(),source=owner.createElementNS('http://www.w3.org/1999/xhtml','area');source.alt='A\udfffZ';source.href='leaf';
var copy=source.cloneNode(false),attr=copy.getAttributeNode('alt'),tokens=copy.relList;
target.appendChild(copy);if(copy===source||copy.parentNode!==target||copy.ownerDocument!==document||attr.ownerElement!==copy||attr.ownerDocument!==document||copy.alt!=='A\udfffZ'||copy.relList!==tokens||areas[1]!==copy||source.ownerDocument!==owner)throw 'real inert clone connection';
target.removeChild(copy);var template=document.createElement('template');template.content.appendChild(copy);
if(copy.ownerDocument!==template.content.ownerDocument||attr.ownerDocument!==copy.ownerDocument||copy.parentNode!==template.content)throw 'parked copy owner';
target.appendChild(template.content);if(copy.parentNode!==target||attr.ownerDocument!==document||copy.getAttributeNode('alt')!==attr||areas[1]!==copy)throw 'same parked copy reconnect';
template.innerHTML='<area alt="parsed" href="parsed-leaf">';var fragment=template.content,parsed=fragment.firstChild,parsedAttr=parsed.getAttributeNode('alt'),donor=fragment.cloneNode(true),cloned=donor.firstChild;
target.appendChild(fragment);target.appendChild(donor);
if(fragment.firstChild!==null||donor.firstChild!==null||parsed.parentNode!==target||cloned.parentNode!==target||parsed===cloned||parsed.ownerDocument!==document||cloned.ownerDocument!==document||parsed.getAttributeNode('alt')!==parsedAttr||parsedAttr.ownerDocument!==document||areas[2]!==parsed||areas[3]!==cloned||parsed.href!=='https://area-connection.test/parsed-leaf')throw 'real parsed and cloned template areas';
"#);
 assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.state.images.has_changes());
}

#[test]
fn area_connection_active_handler_and_foreign_namespace_refuse_without_owner_or_collection_changes(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var target=document.getElementById('map'),areas=target.areas,first=areas[0],owner=new Document(),source=owner.createElementNS('http://www.w3.org/1999/xhtml','area');source.alt='held';source.setAttribute('onclick','bad()');var attr=source.getAttributeNode('alt'),watch=new MutationObserver(function(){});watch.observe(target,{childList:true});
var caught=false;try{target.appendChild(source);}catch(e){caught=e instanceof DOMException&&e.name==='NotSupportedError';}
if(!caught||source.ownerDocument!==owner||source.parentNode!==null||source.getAttributeNode('alt')!==attr||attr.ownerDocument!==owner||areas.length!==1||areas[0]!==first||watch.takeRecords().length)throw 'active area source cannot connect';
var foreign=owner.createElementNS('urn:area','area');caught=false;try{target.appendChild(foreign);}catch(e){caught=e instanceof DOMException&&e.name==='NotSupportedError';}
if(!caught||foreign.ownerDocument!==owner||foreign.parentNode!==null||areas.length!==1||watch.takeRecords().length)throw 'foreign area cannot activate';
source.removeAttribute('onclick');target.appendChild(source);if(source.ownerDocument!==document||attr.ownerDocument!==document||areas[1]!==source||watch.takeRecords().length!==1)throw 'actual safe recovery';
"#);assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.state.images.has_changes());
}

#[test]
fn area_connection_storage_work_copy_and_deadline_refusals_preserve_identity_and_recover(){
 for parking in [false,true]{for mode in 0..4{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,if parking{
   "var target=document.createElement('template').content,source=document.getElementById('list').cloneNode(false);"
  }else{
   "var target=document.getElementById('map'),owner=new Document(),source=owner.createElementNS('http://www.w3.org/1999/xhtml','area');"
  });
  run(&mut d,&mut f,&mut p,"source.alt='retained';var attr=source.getAttributeNode('alt'),tokens=source.relList,watch=new MutationObserver(function(){});watch.observe(target,{childList:true});var originalOwner=source.ownerDocument;");
  p.state.rebuild(&d).unwrap();let tree=format!("{:?}",d.nodes);
  let before=(p.state.retained,p.state.dom_commits,p.state.inert_documents.backing_bytes(),Bridge{doc: &mut d,forms: &mut f,state: &mut p.state}.inert_node_count(),p.state.attributes.len(),p.state.detached_nodes.clone());
  let limit=p.state.retained_limit;p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,_=>p.state.execution_deadline=Some(Instant::now())};
  let error=p.runtime.execute("target.appendChild(source);",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();
  assert!(error.contains("work")||error.contains("allowance")||error.contains("limit")||error.contains("bound")||error.contains("time"),"{parking}/{mode}: {error}");
  assert_eq!(format!("{:?}",d.nodes),tree);assert_eq!((p.state.retained,p.state.dom_commits,p.state.inert_documents.backing_bytes(),Bridge{doc: &mut d,forms: &mut f,state: &mut p.state}.inert_node_count(),p.state.attributes.len(),p.state.detached_nodes.clone()),before);
  p.state.retained_limit=limit;p.state.execution_deadline=None;p.begin_job();
  run(&mut d,&mut f,&mut p,"if(source.parentNode!==null||source.ownerDocument!==originalOwner||source.getAttributeNode('alt')!==attr||attr.ownerElement!==source||source.relList!==tokens||watch.takeRecords().length)throw 'atomic area transfer refusal';target.appendChild(source);var records=watch.takeRecords();if(source.parentNode!==target||source.ownerDocument!==target.ownerDocument||attr.ownerDocument!==target.ownerDocument||source.getAttributeNode('alt')!==attr||source.relList!==tokens||records.length!==1)throw 'fresh area transfer recovery '+[source.parentNode===target,source.ownerDocument===target.ownerDocument,attr.ownerDocument===target.ownerDocument,source.getAttributeNode('alt')===attr,source.relList===tokens,records.length].join(',');");
  assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.state.images.has_changes());
 }}
}

#[test]
fn area_connection_saved_child_append_revalidates_origin_before_transferring_owner(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var owner=new Document(),source=owner.createElementNS('http://www.w3.org/1999/xhtml','area'),target=document.getElementById('map'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var saved=frame.contentWindow.Node.prototype.appendChild,attr=source.getAttributeNode('id');");
 p.set_location(&crate::navigation::address("https://other-area.test/",&crate::navigation::Location::home()).unwrap());
 let tree=format!("{:?}",d.nodes);let before=(p.state.retained,p.state.dom_commits);p.begin_job();
 let error=p.runtime.execute("saved.call(target,source);",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("origin")||error.contains("Security"),"{error}");
 assert_eq!(format!("{:?}",d.nodes),tree);assert_eq!((p.state.retained,p.state.dom_commits),before);
 p.set_location(&crate::navigation::address("https://area-connection.test/source",&crate::navigation::Location::home()).unwrap());p.begin_job();
 run(&mut d,&mut f,&mut p,"if(source.parentNode!==null||source.ownerDocument!==owner)throw 'denied transfer preserves owner';if(saved.call(target,source)!==source||source.ownerDocument!==document||source.parentNode!==target||target.areas[1]!==source)throw 'saved transfer fresh recovery';");
}

#[test]
fn area_connection_template_parking_commits_generation_and_real_observer_records(){
 for name in ["span","area"]{
  let(mut d,mut f,mut p)=fixture();
  run(&mut d,&mut f,&mut p,&format!("var template=document.createElement('template'),target=template.content,source=document.createElement('{name}'),watch=new MutationObserver(function(){{}});watch.observe(target,{{childList:true}});"));
  let before=p.state.dom_commits;
  run(&mut d,&mut f,&mut p,"target.appendChild(source);");
  assert_ne!(p.state.dom_commits,before,"{name}: template parking must publish a DOM commit");
  run(&mut d,&mut f,&mut p,"var records=watch.takeRecords();if(records.length!==1||records[0].target!==target||records[0].addedNodes.length!==1||records[0].addedNodes[0]!==source||records[0].removedNodes.length||records[0].previousSibling!==null||records[0].nextSibling!==null)throw 'real parked leaf child-list record';");
 }
}
