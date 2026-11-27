//! Passive SVG shares native HTML markup output without mutation authority.

use super::*;
fn fixture()->(Document,Forms,Page){
 let source=r##"<body><div id=mount><svg id=vector xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width=20 height=20 viewBox="0 0 20 20"><defs><linearGradient id=gradient gradientUnits=userSpaceOnUse><stop offset=0 stop-color=red /></linearGradient><path id=owned d="M0 0 H20 V20 H0 Z"/></defs><g id=group><use id=use xlink:href="#owned"/><rect id=shape width=20 height=20 fill=red /></g></svg></div><svg id=guard><rect onclick=bad width=1 height=1 /></svg><input id=neighbor value=seed><p id=out>Real</p>"##;
 let mut d=crate::html::parse_loaded_document_with_scripting(source,true,&||Ok(())).unwrap();let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://svg-markup.test/",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn refusal(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->String{let source=format!("var failure=null;try{{{s}}}catch(e){{failure=e.name+': '+e.message;}}");match p.runtime.execute(&source,&mut Bridge{doc:d,forms:f,state:&mut p.state}){Err(e)=>e,Ok(_)=>{let Value::String(e)=p.runtime.execute("failure",&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()else{panic!("Expected refusal: {s}")};e}}}

#[test]
fn svg_html_serialization_local_names_namespace_attributes_escaping_and_paired_tags(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var svg=document.getElementById('vector'),mount=document.getElementById('mount'),gradient=document.getElementById('gradient'),path=document.getElementById('owned'),group=document.getElementById('group'),use=document.getElementById('use'),shape=document.getElementById('shape');shape.setAttribute('data-label','A&"\u00a0<>');group.appendChild(document.createComment('A&B<>'));
if(gradient.outerHTML!=='<linearGradient id="gradient" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="red"></stop></linearGradient>'||path.outerHTML!=='<path id="owned" d="M0 0 H20 V20 H0 Z"></path>'||use.outerHTML!=='<use id="use" xlink:href="#owned"></use>'||shape.outerHTML!=='<rect id="shape" width="20" height="20" fill="red" data-label="A&amp;&quot;&nbsp;&lt;&gt;"></rect>'||group.innerHTML!==use.outerHTML+shape.outerHTML+'<!--A&B<>-->'||mount.innerHTML!==svg.outerHTML||svg.outerHTML.indexOf('<svg id="vector" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="20" height="20" viewBox="0 0 20 20">')!==0||svg.outerHTML.slice(-6)!=='</svg>')throw 'real SVG HTML names escaping and paired tags';
"##);
}
#[test]
fn svg_html_serialization_original_round_trip_detached_clone_and_namespace_identity(){
 let(mut d,mut f,mut p)=fixture();let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let controls=f.controls.len();run(&mut d,&mut f,&mut p,r##"
var svg=document.getElementById('vector'),mount=document.getElementById('mount'),watch=new MutationObserver(function(){});watch.observe(mount,{childList:true,subtree:true});var source=svg.outerHTML;if(watch.takeRecords().length||svg.parentNode!==mount||!svg.isConnected)throw 'read does not mutate native SVG';
"##);assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);assert_eq!(f.controls.len(),controls);
 run(&mut d,&mut f,&mut p,r##"var r=document.createRange();r.selectNodeContents(mount);var fragment=r.createContextualFragment(source),copy=fragment.firstChild,clone=copy.cloneNode(true);if(copy===svg||copy.ownerDocument!==document||copy.isConnected||copy.namespaceURI!=='http://www.w3.org/2000/svg'||copy.firstChild.firstChild.localName!=='linearGradient'||copy.lastChild.firstChild.getAttributeNS('http://www.w3.org/1999/xlink','href')!=='#owned'||copy.outerHTML!==source||clone===copy||clone.firstChild===copy.firstChild||clone.outerHTML!==source||watch.takeRecords().length)throw 'original parsed SVG round trip and independent native clone';"##);
 assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
}
#[test]
fn svg_html_serialization_mixed_exact_attribute_units_and_live_native_reads(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var shape=document.getElementById('shape'),svg=document.getElementById('vector'),mount=document.getElementById('mount'),raw='\udfff\ud800&"<>',watch=new MutationObserver(function(){});mount.setAttribute('data-exact',raw);watch.observe(shape,{attributes:true});var first=mount.outerHTML,needle='data-exact="\udfff\ud800&amp;&quot;&lt;&gt;"';if(first.indexOf(needle)<0||mount.outerHTML.indexOf(needle)<0||mount.getAttribute('data-exact')!==raw||watch.takeRecords().length)throw 'actual exact mixed HTML and SVG attribute output';shape.setAttribute('fill','blue');var second=mount.outerHTML,records=watch.takeRecords();if(first===second||first.indexOf('fill="red"')<0||second.indexOf('fill="blue"')<0||second.indexOf(needle)<0||records.length!==1||records[0].target!==shape||records[0].attributeName!=='fill')throw 'fresh serialized native data';
"##);
}
#[test]
fn svg_html_serialization_passive_owner_and_quota_refusal_preserves_tree_then_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var svg=document.getElementById('vector'),guarded=document.getElementById('guard'),foreign=document.createElementNS('urn:foreign','node');");let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let controls=f.controls.len();
 for code in ["guarded.innerHTML;","guarded.outerHTML;"]{p.begin_job();assert!(!refusal(&mut d,&mut f,&mut p,code).is_empty());assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);}
 run(&mut d,&mut f,&mut p,"if(foreign.outerHTML!==undefined)throw \'foreign factory must not fabricate a markup getter\';");
 let strings=p.state.limits.host.string_bytes;
 for mode in 0..4{p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.limits.host.string_bytes=8,_=>p.state.execution_deadline=Some(Instant::now()-Duration::from_secs(1))};let e=refusal(&mut d,&mut f,&mut p,"svg.outerHTML;").to_ascii_lowercase();assert!(e.contains("limit")||e.contains("allowance"),"{mode}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);assert_eq!(f.controls.len(),controls);p.state.limits.host.string_bytes=strings;}
 p.begin_job();run(&mut d,&mut f,&mut p,"if(svg.outerHTML.indexOf('<svg id=\"vector\"')!==0||document.getElementById('neighbor').value!=='seed'||document.getElementById('out').textContent!=='Real')throw 'fresh serialization after refusal';");assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
}
