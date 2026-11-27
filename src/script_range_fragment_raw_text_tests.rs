//! Exact UTF-16 literal fragment source retains native Text and resource guards.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document_with_scripting("<body><noscript id=host>held</noscript><p id=out>Real</p>",true,&||Ok(())).unwrap();let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://range-raw-text.test/",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn refusal(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->String{let source=format!("var failure=null;try{{{s}}}catch(e){{failure=e.name+': '+e.message;}}");match p.runtime.execute(&source,&mut Bridge{doc:d,forms:f,state:&mut p.state}){Err(e)=>e,Ok(_)=>{let Value::String(e)=p.runtime.execute("failure",&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()else{panic!("Expected refusal: {s}")};e}}}
#[test]
fn range_fragment_raw_text_literal_contexts_preserve_units_and_preprocessing(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),raw='\udfff\ud800\r\n\0<b>&amp;</b>\r\ud83d\ude00',expected='\udfff\ud800\n\ufffd<b>&amp;</b>\n\ud83d\ude00',watch=new MutationObserver(function(){});watch.observe(document.body,{childList:true,subtree:true});for(var name of ['noscript','script','style','xmp','iframe','noembed','noframes','plaintext']){var context=document.createElement(name);r.selectNodeContents(context);var fragment=r.createContextualFragment(raw),text=fragment.firstChild;if(fragment.ownerDocument!==document||fragment.nodeType!==11||fragment.childNodes.length!==1||text.nodeType!==3||text.ownerDocument!==document||text.parentNode!==fragment||text.data!==expected||text.data.charCodeAt(0)!==57343||text.data.charCodeAt(1)!==55296||text.data.charCodeAt(3)!==65533||context.firstChild!==null||fragment.isConnected||r.startContainer!==context||r.endOffset)throw 'exact literal tokenizer output';}
if(watch.takeRecords().length||document.getElementById('out').textContent!=='Real'||document.querySelectorAll('script').length)throw 'no source or resource activation';
"#);assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
}
#[test]
fn range_fragment_raw_text_conversion_uses_live_context_without_lossy_projection(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),host=document.getElementById('host'),out=document.getElementById('out'),calls=0;r.selectNodeContents(out);var source={toString:function(){calls++;r.selectNodeContents(host);return '\udfff\ud800<img id=fake src=missing>&amp;';}},fragment=r.createContextualFragment(source);if(calls!==1||fragment.firstChild.data!=='\udfff\ud800<img id=fake src=missing>&amp;'||r.startContainer!==host||host.textContent!=='held'||document.getElementById('fake')!==null)throw 'live raw context after actual conversion';var inverse={toString:function(){calls++;r.selectNodeContents(out);return '\ud800';}};
"#);p.begin_job();let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment(inverse);");assert!(e.contains("not implemented"));p.begin_job();run(&mut d,&mut f,&mut p,"if(calls!==2||r.startContainer!==out||out.textContent!=='Real')throw 'live general markup guard';");
}
#[test]
fn range_fragment_raw_text_clone_connection_records_and_native_text_lifecycle(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var host=document.getElementById('host'),out=document.getElementById('out'),r=document.createRange(),watch=new MutationObserver(function(){});r.selectNodeContents(host);watch.observe(out,{childList:true});var raw='\udfff\ud800<b>&amp;</b>',fragment=r.createContextualFragment(raw),text=fragment.firstChild,copy=fragment.cloneNode(true);if(copy===fragment||copy.firstChild===text||copy.firstChild.data!==raw||copy.ownerDocument!==document)throw 'independent raw native copy';out.appendChild(fragment);var records=watch.takeRecords();if(fragment.firstChild!==null||text!==out.lastChild||text.parentNode!==out||!text.isConnected||text.data!==raw||records.length!==1||records[0].addedNodes[0]!==text||host.textContent!=='held')throw 'native exact text drain and connection';text.appendData('\ud800');if(text.data!==raw+'\ud800'||copy.firstChild.data!==raw)throw 'independent native mutation';out.removeChild(text);if(text.isConnected||text.parentNode!==null||text.data!==raw+'\ud800'||copy.firstChild.ownerDocument!==document)throw 'exact text retirement';
"#);assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());
}
#[test]
fn range_fragment_raw_text_context_owner_and_quota_refusal_no_prefix_recovery(){
 // The raw parser jointly admits the actual temporary unit capacity and
 // native projection/raw backing before allocating the Text payload.
 let input=[0xdfff,0xd800,13,10,0];let mapped=[0xdfff,0xd800,13,10,0xfffd];let bytes=DomString::from_units(&mapped,usize::MAX).unwrap().capacity()+input.len()*2;
 assert!(crate::html::parse_contextual_fragment_units(&input,"noscript",true,bytes-1).is_err());
 let parsed=crate::html::parse_contextual_fragment_units(&input,"noscript",true,bytes).unwrap();let NodeKind::Text(text)=&parsed.nodes[1].kind else{panic!("actual parsed Text")};assert_eq!(text.raw_units().unwrap(),&[0xdfff,0xd800,10,0xfffd]);
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var r=document.createRange(),host=document.getElementById('host'),out=document.getElementById('out'),title=document.createElement('title'),area=document.createElement('textarea'),svg=document.createElementNS('http://www.w3.org/2000/svg','svg'),owner=document.implementation.createHTMLDocument('other'),other=owner.createRange(),watch=new MutationObserver(function(){});other.selectNodeContents(owner.body);watch.observe(document.body,{childList:true,subtree:true});");let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;
 for source in ["r.selectNodeContents(out);r.createContextualFragment('\\ud800');","r.selectNodeContents(document);r.createContextualFragment('\\ud800');","r.selectNodeContents(document.documentElement);r.createContextualFragment('\\ud800');","r.selectNodeContents(svg);r.createContextualFragment('\\ud800');","other.createContextualFragment('\\ud800');"]{p.begin_job();assert!(!refusal(&mut d,&mut f,&mut p,source).is_empty());assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);}
 run(&mut d,&mut f,&mut p,"r.selectNodeContents(host);");let held=p.state.retained_limit;let nodes=p.state.node_limit;let strings=p.state.limits.host.string_bytes;
 for mode in 0..5{p.begin_job();match mode{0=>p.state.copy_work=0,1=>p.state.host_work=0,2=>p.state.retained_limit=p.state.retained,3=>p.state.node_limit=d.nodes.len(),_=>p.state.limits.host.string_bytes=1};let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment('\\udfff\\ud800\\r\\n\\0Literal');").to_ascii_lowercase();assert!(e.contains("limit")||e.contains("allowance")||e.contains("storage"),"{mode}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);p.state.retained_limit=held;p.state.node_limit=nodes;p.state.limits.host.string_bytes=strings;}
 p.begin_job();run(&mut d,&mut f,&mut p,"if(watch.takeRecords().length||host.textContent!=='held'||out.textContent!=='Real')throw 'no raw failure prefix';var fresh=r.createContextualFragment('\\udfff\\ud800');if(fresh.firstChild.data!=='\\udfff\\ud800'||fresh.firstChild.ownerDocument!==document)throw 'fresh exact raw source recovery';");
}

#[test]
fn range_fragment_raw_text_disabled_scripting_keeps_the_markup_guard(){
 let mut d=crate::html::parse_loaded_document_with_scripting("<body><noscript id=host>Held</noscript>",false,&||Ok(())).unwrap();assert!(!d.scripting_enabled);let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);
 // Direct native harness admission; disabled page startup executes no script.
 p.state.node_limit=d.nodes.len().saturating_add(p.state.limits.host.extra_nodes);p.state.retained=document_bytes(&d);p.state.retained_limit=p.state.retained.saturating_add(p.state.limits.host.retained_bytes);
 run(&mut d,&mut f,&mut p,"var r=document.createRange(),host=document.getElementById('host');r.selectNodeContents(host);");let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;p.begin_job();assert!(refusal(&mut d,&mut f,&mut p,"r.createContextualFragment('\\ud800');").contains("not implemented"));assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);assert!(!d.scripting_enabled);
 p.begin_job();run(&mut d,&mut f,&mut p,"var ordinary=r.createContextualFragment('A&amp;B');if(ordinary.firstChild.data!=='A&B'||host.textContent!=='Held')throw 'captured disabled context retains ordinary entity processing';");
}
