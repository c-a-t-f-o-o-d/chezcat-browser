//! Passive optgroup connection preserves actual identity and ownership.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse("<html><body><div id=target>Fallback group<span id=tail>Tail</span></div><select id=select><optgroup id=group label=Group><option value=one>One</option></optgroup></select><input id=neighbor value=seed></body></html>");d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();let location=crate::navigation::address("https://optgroup-fragments.test/source",&crate::navigation::Location::home()).unwrap();p.set_location(&location);p.set_resource_base(&location);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}

#[test]
fn optgroup_fragments_template_real_raw_attributes_clone_and_connection(){let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"var template=document.createElement('template');template.innerHTML='<optgroup label="Parsed" disabled></optgroup>';var fragment=template.content,x=fragment.firstChild;x.label='A\ud800Z';var attr=x.getAttributeNode('label'),copy=fragment.cloneNode(true),y=copy.firstChild,target=document.getElementById('target');if(!(x instanceof HTMLOptGroupElement)||x.label!=='A\ud800Z'||!x.disabled||x.ownerDocument!==fragment.ownerDocument||y===x||y.getAttributeNode('label')===attr)throw 'actual parsed group';target.appendChild(fragment);target.appendChild(copy);if(x.parentNode!==target||y.parentNode!==target||x.ownerDocument!==document||y.ownerDocument!==document||x.getAttributeNode('label')!==attr||attr.ownerDocument!==document||x.label!==y.label||!y.disabled)throw 'parsed group clone connection';"#);}
#[test]
fn optgroup_fragments_native_container_replacement_preserves_neighbor_select(){let(mut d,mut f,mut p)=fixture();let select=(0..d.nodes.len()).find(|&n|d.attr(n,"id")==Some("select")).unwrap();let native=format!("{:?}",f.controls[&select].select);run(&mut d,&mut f,&mut p,r#"var target=document.getElementById('target'),watch=new MutationObserver(function(){});watch.observe(target,{childList:true});target.innerHTML='<optgroup label="First" disabled></optgroup><optgroup label="Second"></optgroup>';var first=target.firstChild,last=target.lastChild,records=watch.takeRecords();if(target.childNodes.length!==2||!(first instanceof HTMLOptGroupElement)||first.label!=='First'||!first.disabled||last.label!=='Second'||last.disabled||first.ownerDocument!==document||records.length!==1||records[0].addedNodes.length!==2||document.getElementById('neighbor').value!=='seed')throw 'actual native group replacement';"#);assert_eq!(format!("{:?}",f.controls[&select].select),native);}

#[test]
fn optgroup_fragments_resource_refusals_preserve_native_and_template_state(){
 for template in [false,true]{for mode in 0..5{
  // Native parking capacity does not cap the separately charged inert arena.
  if template && mode==3 {continue;}
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,if template{"var target=document.createElement('template');target.innerHTML='<span>Old</span>';var container=target.content;"}else{"var target=document.getElementById('target'),container=target;"});
  run(&mut d,&mut f,&mut p,"var old=container.firstChild,watch=new MutationObserver(function(){});watch.observe(container,{childList:true});");
  p.state.rebuild(&d).unwrap();let tree=format!("{:?}",d.nodes);let native=format!("{:?}",f.controls.iter().map(|(n,c)|(*n,&c.select)).collect::<Vec<_>>());
  let before=(p.state.retained,p.state.dom_commits,p.state.inert_documents.backing_bytes(),p.state.attributes.len(),p.state.detached_nodes.clone());let limit=p.state.retained_limit;let nodes=p.state.node_limit;p.begin_job();
  match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.retained_limit=p.state.retained,3=>p.state.node_limit=0,_=>p.state.execution_deadline=Some(Instant::now())};
  let error=p.runtime.execute("target.innerHTML='<optgroup label=New disabled></optgroup>';",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).expect_err(&format!("{template}/{mode} must refuse"));
  assert!(error.contains("work")||error.contains("allowance")||error.contains("limit")||error.contains("bound")||error.contains("time")||error.contains("node"),"{template}/{mode}: {error}");
  assert_eq!(format!("{:?}",d.nodes),tree);assert_eq!((p.state.retained,p.state.dom_commits,p.state.inert_documents.backing_bytes(),p.state.attributes.len(),p.state.detached_nodes.clone()),before);
  assert_eq!(format!("{:?}",f.controls.iter().map(|(n,c)|(*n,&c.select)).collect::<Vec<_>>()),native);
  p.state.retained_limit=limit;p.state.node_limit=nodes;p.state.execution_deadline=None;p.begin_job();
  run(&mut d,&mut f,&mut p,"if(container.firstChild!==old||watch.takeRecords().length)throw 'atomic parsed group refusal';target.innerHTML='<optgroup label=Recovered disabled></optgroup>';var x=container.firstChild;if(x.label!=='Recovered'||!x.disabled||!(x instanceof HTMLOptGroupElement)||x.ownerDocument!==container.ownerDocument||watch.takeRecords().length!==1)throw 'fresh parsed group recovery';");
  assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty()&&p.take_dynamic_stylesheets().is_empty());assert!(!p.state.images.has_changes());
 }}
}
#[test]
fn optgroup_fragments_scope_refusals_preserve_existing_tree_and_recover(){
 for template in [false,true]{for source in ["<optgroup><div><optgroup>","<optgroup><div></optgroup>","<optgroup><select></optgroup>","<optgroup><option>One</option></optgroup>"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,if template{"var target=document.createElement('template');target.innerHTML='<span>Old</span>';var container=target.content;"}else{"var target=document.getElementById('target'),container=target;"});
  run(&mut d,&mut f,&mut p,"var old=container.firstChild,watch=new MutationObserver(function(){});watch.observe(container,{childList:true});");
  p.state.rebuild(&d).unwrap();let tree=format!("{:?}",d.nodes);let before=(p.state.retained,p.state.dom_commits,p.state.inert_documents.backing_bytes());p.begin_job();
  let code=format!("target.innerHTML='{source}';");let error=p.runtime.execute(&code,&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();assert!(error.contains("nonconventional select recovery"),"{error}");
  assert_eq!(format!("{:?}",d.nodes),tree);assert_eq!((p.state.retained,p.state.dom_commits,p.state.inert_documents.backing_bytes()),before);p.begin_job();
  run(&mut d,&mut f,&mut p,"if(container.firstChild!==old||watch.takeRecords().length)throw 'scope refusal atomic';target.innerHTML='<optgroup label=First><optgroup label=Second>';if(container.childNodes.length!==2||container.firstChild.label!=='First'||container.lastChild.label!=='Second')throw 'omitted group recovery';");
 }}
}

#[test]
fn optgroup_fragments_unparked_template_storage_keeps_identity_and_connection_guard(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var t=document.createElement('template'),fragment=t.content;");let nodes=p.state.node_limit;let native=d.nodes.len();let retained=p.state.retained;p.state.node_limit=0;p.begin_job();
 run(&mut d,&mut f,&mut p,"t.innerHTML='<optgroup label=Unparked disabled></optgroup>';var x=fragment.firstChild,attr=x.getAttributeNode('label');if(!(x instanceof HTMLOptGroupElement)||x.label!=='Unparked'||!x.disabled||x.ownerDocument!==fragment.ownerDocument)throw 'real unparked parsed group';");
 assert_eq!(d.nodes.len(),native);assert!(p.state.retained>retained);p.state.node_limit=nodes;p.begin_job();
 run(&mut d,&mut f,&mut p,"var target=document.getElementById('target'),old=target.firstChild,caught=false;try{target.appendChild(fragment);}catch(e){caught=e instanceof TypeError;}if(!caught||fragment.firstChild!==x||x.parentNode!==fragment||x.ownerDocument!==fragment.ownerDocument||x.getAttributeNode('label')!==attr||attr.ownerDocument!==fragment.ownerDocument||target.firstChild!==old||x.label!=='Unparked'||!x.disabled)throw 'unparked connection guard retains actual records';var fresh=document.createElement('template');fresh.innerHTML='<optgroup label=Fresh></optgroup>';var y=fresh.content.firstChild;target.appendChild(fresh.content);if(y.parentNode!==target||y.ownerDocument!==document||y.label!=='Fresh'||fragment.firstChild!==x)throw 'fresh parked connection recovery';");
}
