//! Template fragment tokenization follows the actual receiver document.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document_with_scripting("<body><div id=mount><template id=loaded><template id=nested><i>Old</i></template></template></div><input id=neighbor value=seed><p id=out>Real</p>",true,&||Ok(())).unwrap();let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn refusal(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->String{let source=format!("var failure=null;try{{{s}}}catch(e){{failure=e.name+': '+e.message;}}");match p.runtime.execute(&source,&mut Bridge{doc:d,forms:f,state:&mut p.state}){Err(e)=>e,Ok(_)=>{let Value::String(e)=p.runtime.execute("failure",&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()else{panic!("Expected refusal: {s}")};e}}}
#[test]
fn template_parser_owner_main_and_nested_receivers_have_distinct_real_noscript_trees(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var loaded=document.getElementById('loaded'),nested=loaded.content.firstChild,content=nested.content,owner=content.ownerDocument,payload='<noscript id="fallback"><b id="label">A&amp;B</b><input id="hidden" value="secret"></noscript>';
var main=document.createElement('template');main.innerHTML=payload;var literal=main.content.firstChild;if(literal.localName!=='noscript'||literal.firstChild.nodeType!==3||literal.querySelector('#label')!==null||main.ownerDocument!==document)throw 'main context keeps scripting enabled';
nested.innerHTML=payload;var fallback=content.firstChild,label=fallback.firstChild;if(fallback.localName!=='noscript'||label.nodeType!==1||label.localName!=='b'||label.textContent!=='A&B'||fallback.childNodes.length!==2||fallback.lastChild.localName!=='input'||fallback.ownerDocument!==owner||label.ownerDocument!==owner||fallback.isConnected||nested.content!==content||document.getElementById('hidden')!==null||document.getElementById('label')!==null||nested.innerHTML!==payload)throw 'nested receiver uses inert context and actual fallback elements';
"##);assert_eq!(f.controls.len(),1);assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_image_changes().2.is_empty());
}
#[test]
fn template_parser_owner_observer_projection_matches_actual_fallback_identities(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var loaded=document.getElementById('loaded'),nested=loaded.content.firstChild,content=nested.content,old=content.firstChild,watch=new MutationObserver(function(){});watch.observe(content,{childList:true});nested.innerHTML='<noscript><b id="label">Decoded&amp;</b></noscript>';var fallback=content.firstChild,label=fallback.firstChild,records=watch.takeRecords();if(label.nodeType!==1||label.localName!=='b'||label.textContent!=='Decoded&'||records.length!==1||records[0].target!==content||records[0].addedNodes.length!==1||records[0].addedNodes[0]!==fallback||records[0].addedNodes[0].firstChild!==label||records[0].removedNodes.length!==1||records[0].removedNodes[0]!==old||old.parentNode!==null||fallback.parentNode!==content||label.parentNode!==fallback||old.textContent!=='Old')throw 'projected observer identities use actual owner-context parse';
nested.innerHTML='<noscript><i>Next</i><br></noscript>';var second=watch.takeRecords();if(second.length!==1||second[0].removedNodes[0]!==fallback||fallback.parentNode!==null||fallback.firstChild!==label||content.firstChild.firstChild.localName!=='i'||content.firstChild.lastChild.localName!=='br'||nested.content!==content)throw 'fresh replacement and detached old identities';
"##);
}
#[test]
fn template_parser_owner_factory_parser_clone_and_import_context_is_receiver_owned(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var other=document.implementation.createHTMLDocument('inert'),factory=other.createElement('template'),parsed=new DOMParser().parseFromString('<template id="parsed"><i>Old</i></template>','text/html').getElementById('parsed'),payload='<noscript><b>Fallback&amp;</b></noscript>';
factory.innerHTML=payload;parsed.innerHTML=payload;if(factory.content.firstChild.firstChild.nodeType!==1||factory.content.firstChild.firstChild.textContent!=='Fallback&'||parsed.content.firstChild.firstChild.nodeType!==1||parsed.content.firstChild.firstChild.textContent!=='Fallback&'||factory.ownerDocument!==other||factory.content.ownerDocument===other||parsed.ownerDocument===document)throw 'factory and parsed inert receiver contexts';
var clone=factory.cloneNode(true),copy=document.importNode(factory,true);if(clone.content===factory.content||clone.content.firstChild===factory.content.firstChild||copy.ownerDocument!==document||copy.content===factory.content)throw 'distinct actual imported owner trees';clone.innerHTML=payload;copy.innerHTML=payload;if(clone.content.firstChild.firstChild.nodeType!==1||copy.content.firstChild.firstChild.nodeType!==3||factory.content.firstChild.firstChild.nodeType!==1)throw 'context follows current destination receiver owner';
"##);
}
#[test]
fn template_parser_owner_resource_refusal_and_fresh_recovery_preserve_old_content(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var nested=document.getElementById('loaded').content.firstChild,content=nested.content,old=content.firstChild,watch=new MutationObserver(function(){});watch.observe(content,{childList:true});");let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let strings=p.state.limits.host.string_bytes;
 for mode in 0..4{p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.limits.host.string_bytes=8,_=>p.state.execution_deadline=Some(Instant::now()-Duration::from_secs(1))};let e=refusal(&mut d,&mut f,&mut p,"nested.innerHTML='<noscript><b>Fallback&amp;</b></noscript>';").to_ascii_lowercase();assert!(e.contains("limit")||e.contains("allowance"),"{mode}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);assert_eq!(f.controls.len(),1);p.state.limits.host.string_bytes=strings;}
 p.begin_job();run(&mut d,&mut f,&mut p,"if(nested.content!==content||content.firstChild!==old||old.parentNode!==content||watch.takeRecords().length)throw 'no parsed prefix after refusal';nested.innerHTML='<noscript><b>Recovered&amp;</b></noscript>';var fallback=content.firstChild,records=watch.takeRecords();if(fallback.firstChild.nodeType!==1||fallback.firstChild.textContent!=='Recovered&'||records.length!==1||records[0].addedNodes[0]!==fallback||records[0].removedNodes[0]!==old||old.parentNode!==null||document.getElementById('neighbor').value!=='seed'||document.getElementById('out').textContent!=='Real')throw 'fresh owner-context setter and actual record after refusal';");
 assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());assert!(p.take_image_changes().2.is_empty());
}
