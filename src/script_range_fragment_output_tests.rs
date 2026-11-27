//! Decoded contextual Text admission is independent of source admission.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document_with_scripting("<body><textarea id=area>Context</textarea><p id=out>Real</p>",true,&||Ok(())).unwrap();let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://range-output.test/",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn refusal(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->String{let source=format!("var failure=null;try{{{s}}}catch(e){{failure=e.name+': '+e.message;}}");match p.runtime.execute(&source,&mut Bridge{doc:d,forms:f,state:&mut p.state}){Err(e)=>e,Ok(_)=>{let Value::String(e)=p.runtime.execute("failure",&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()else{panic!("Expected refusal: {s}")};e}}}

#[test]
fn range_fragment_output_rcdata_decoded_text_limit_refuses_before_native_publication(){
 for context in ["title","textarea"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,&format!("var r=document.createRange(),context=document.createElement({context:?}),source='\\0\\0',watch=new MutationObserver(function(){{}});r.selectNodeContents(context);watch.observe(document.body,{{childList:true,subtree:true}});"));
  let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let controls=f.controls.len();let strings=p.state.limits.host.string_bytes;p.state.limits.host.string_bytes=3;
  let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment(source);");assert!(e.contains("Decoded contextual Text"),"{context}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);assert_eq!(f.controls.len(),controls);
  p.state.limits.host.string_bytes=strings;p.begin_job();run(&mut d,&mut f,&mut p,"if(watch.takeRecords().length||r.startContainer!==context||r.endOffset||context.firstChild!==null)throw 'decoded refusal no native prefix';var fresh=r.createContextualFragment('Fresh&amp;');if(fresh.firstChild.data!=='Fresh&'||fresh.firstChild.parentNode!==fresh||fresh.firstChild.isConnected)throw 'decoded refusal fresh recovery';");
 }
}
#[test]
fn range_fragment_output_literal_contexts_decoded_limit_keep_resources_and_native_state(){
 for context in ["script","style","xmp","noembed","noframes","plaintext","noscript"]{
  let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,&format!("var r=document.createRange(),context=document.createElement({context:?}),source='\\0\\0';r.selectNodeContents(context);"));
  let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let strings=p.state.limits.host.string_bytes;p.state.limits.host.string_bytes=3;let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment(source);");assert!(e.contains("Decoded contextual Text"),"{context}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);p.state.limits.host.string_bytes=strings;p.begin_job();
  run(&mut d,&mut f,&mut p,"var fresh=r.createContextualFragment('A&amp;');if(fresh.firstChild.data!=='A&amp;'||r.startContainer!==context||context.firstChild!==null)throw 'literal recovery and entity data';");assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
 }
}
#[test]
fn range_fragment_output_decoded_scalar_boundary_and_exact_units_use_logical_bytes(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var r=document.createRange(),context=document.getElementById('area'),source='\\0\\0',raw='\\udfff\\ud800';r.selectNodeContents(context);");let strings=p.state.limits.host.string_bytes;
 p.state.limits.host.string_bytes=6;run(&mut d,&mut f,&mut p,"var exact=r.createContextualFragment(source);");p.state.limits.host.string_bytes=strings;p.begin_job();run(&mut d,&mut f,&mut p,"if(exact.firstChild.data!=='\\ufffd\\ufffd'||exact.childNodes.length!==1)throw 'decoded scalar exact allowance';");
 p.state.limits.host.string_bytes=4;run(&mut d,&mut f,&mut p,"var units=r.createContextualFragment(raw);");p.state.limits.host.string_bytes=strings;p.begin_job();run(&mut d,&mut f,&mut p,"if(units.firstChild.data!==raw||units.firstChild.data.charCodeAt(0)!==57343||units.firstChild.data.charCodeAt(1)!==55296)throw 'exact logical units without projection';");
 let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;p.state.limits.host.string_bytes=5;let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment(source);");assert!(e.contains("Decoded contextual Text"),"{e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);p.state.limits.host.string_bytes=strings;
}
#[test]
fn range_fragment_output_author_conversion_effects_survive_decoded_refusal_and_recover(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var r=document.createRange(),area=document.getElementById('area'),out=document.getElementById('out'),calls=0,captured=null,watch=new MutationObserver(function(){});r.selectNodeContents(out);watch.observe(out,{childList:true});var source={toString:function(){calls++;out.textContent='X';r.selectNodeContents(area);captured=watch.takeRecords();return '\\0\\0';}};");
 let count=d.nodes.len();let strings=p.state.limits.host.string_bytes;p.state.limits.host.string_bytes=3;let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment(source);");assert!(e.contains("Decoded contextual Text"),"{e}");p.state.limits.host.string_bytes=strings;p.begin_job();
 run(&mut d,&mut f,&mut p,"if(watch.takeRecords().length)throw 'unexpected duplicate author record';var records=captured;if(calls!==1||out.textContent!=='X'||r.startContainer!==area||area.value!=='Context'||records.length!==1||records[0].target!==out||records[0].addedNodes[0]!==out.firstChild)throw 'author effects '+calls+'|'+out.textContent+'|'+(r.startContainer===area)+'|'+area.value+'|'+records.length+'|'+(records.length===1&&records[0].target===out)+'|'+(records.length===1&&records[0].addedNodes[0]===out.firstChild);var fresh=r.createContextualFragment('Recovered&amp;');if(fresh.firstChild.data!=='Recovered&'||fresh.ownerDocument!==document)throw 'author effect recovery';");
 assert_eq!(d.nodes.len(),count+3);assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
}
