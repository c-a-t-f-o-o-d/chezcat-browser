//! Template markup reads use current appropriate-owner contents without activation.
use super::*;
fn fixture()->(Document,Forms,Page){
 let source=r##"<body><div id=mount><template id=t data-label="A&amp;&quot;"><span>One&amp;&lt;</span><template id=nested><br><noscript>Literal&amp;<b>raw</b></noscript></template></template></div><input id=neighbor value=seed><p id=out>Real</p><noscript id=active>Literal&amp;<b>raw</b></noscript>"##;
 let mut d=crate::html::parse_loaded_document_with_scripting(source,true,&||Ok(())).unwrap();let mut f=Forms::new(&d);let mut p=Page::new();assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn refusal(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str)->String{let source=format!("var failure=null;try{{{s}}}catch(e){{failure=e.name+': '+e.message;}}");match p.runtime.execute(&source,&mut Bridge{doc:d,forms:f,state:&mut p.state}){Err(e)=>e,Ok(_)=>{let Value::String(e)=p.runtime.execute("failure",&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap()else{panic!("Expected refusal: {s}")};e}}}
#[test]
fn template_html_serialization_loaded_nested_current_content_and_outer_wrappers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var t=document.getElementById('t'),mount=document.getElementById('mount'),content=t.content,nested=content.querySelector('#nested'),nestedContent=nested.content,owner=content.ownerDocument,first=content.firstChild;
var active=document.getElementById('active');if(active.innerHTML!=='Literal&amp;<b>raw</b>'||active.outerHTML!=='<noscript id="active">Literal&amp;<b>raw</b></noscript>')throw 'main owner keeps enabled raw text';
var expected='<span>One&amp;&lt;</span><template id="nested"><br><noscript>Literal&amp;amp;&lt;b&gt;raw&lt;/b&gt;</noscript></template>';
if(t.innerHTML!==expected||t.outerHTML!=='<template id="t" data-label="A&amp;&quot;">'+expected+'</template>'||mount.innerHTML!==t.outerHTML||mount.outerHTML!=='<div id="mount">'+t.outerHTML+'</div>'||t.childNodes.length||content!==t.content||content.ownerDocument!==owner||nested.content!==nestedContent||content.isConnected||first.isConnected||document.getElementById('nested')!==null)throw 'real template contents and containing markup';
nested.innerHTML='<i>Changed&amp;</i>';if(t.outerHTML.indexOf('<template id="nested"><i>Changed&amp;</i></template>')<0||mount.innerHTML!==t.outerHTML||content.firstChild!==first||nested.content!==nestedContent)throw 'serialization reads current content instead of loaded source';
"##);
}
#[test]
fn template_html_serialization_import_clone_and_exact_native_content(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var t=document.getElementById('t'),mount=document.getElementById('mount');t.innerHTML='<span id="exact">Seed</span><template><b>Nested</b></template>';var content=t.content,span=content.firstChild,text=span.firstChild,raw='\udfff\ud800&<>\u00a0';text.data=raw;
var expected='<span id="exact">\udfff\ud800&amp;&lt;&gt;&nbsp;</span><template><b>Nested</b></template>';if(t.innerHTML!==expected)throw 'existing exact content getter setup';var copy=t.cloneNode(true),shallow=t.cloneNode(false);if(t.outerHTML!=='<template id="t" data-label="A&amp;&quot;">'+expected+'</template>'||copy.outerHTML!==t.outerHTML||copy.content===content||copy.content.firstChild===span||copy.content.ownerDocument!==content.ownerDocument||copy.content.firstChild.firstChild.data!==raw||shallow.outerHTML!=='<template id="t" data-label="A&amp;&quot;"></template>'||mount.innerHTML!==t.outerHTML)throw 'exact template markup and disjoint native clone';text.appendData('X');if(t.outerHTML.indexOf('&nbsp;X</span>')<0||copy.outerHTML.indexOf('&nbsp;X</span>')>=0||span.firstChild!==text)throw 'current exact native data and independent clone';
"##);
}
#[test]
fn template_html_serialization_combined_output_quotas_refusal_and_fresh_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var t=document.getElementById('t'),mount=document.getElementById('mount'),content=t.content,initial=t.innerHTML;");let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let controls=f.controls.len();let strings=p.state.limits.host.string_bytes;let Value::String(inner)=p.runtime.execute("initial",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap()else{panic!("actual scalar template content")};let inner_bytes=inner.len();
 // The old serializer refuses the main template path, so this is also a baseline witness.
 run(&mut d,&mut f,&mut p,"if(mount.innerHTML!==t.outerHTML)throw 'genuine combined template output';");
 for mode in 0..4{p.begin_job();match mode{0=>p.state.host_work=0,1=>p.state.copy_work=0,2=>p.state.limits.host.string_bytes=inner_bytes,_=>p.state.execution_deadline=Some(Instant::now()-Duration::from_secs(1))};if mode==2{run(&mut d,&mut f,&mut p,"if(t.innerHTML!==initial)throw 'content alone fits exact output boundary';");}let e=refusal(&mut d,&mut f,&mut p,"mount.outerHTML;").to_ascii_lowercase();assert!(e.contains("limit")||e.contains("allowance"),"{mode}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);assert_eq!(f.controls.len(),controls);p.state.limits.host.string_bytes=strings;}
 p.begin_job();run(&mut d,&mut f,&mut p,"if(mount.innerHTML!==t.outerHTML||t.content!==content||t.innerHTML!==initial||document.getElementById('neighbor').value!=='seed'||document.getElementById('out').textContent!=='Real')throw 'fresh getter after refusal';");
}
#[test]
fn template_html_serialization_inert_resources_and_observers_do_not_activate(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r##"
var t=document.getElementById('t'),mount=document.getElementById('mount');t.innerHTML='<style>#out{color:red}</style><input id="hidden" value="secret"><img src="https://unrequested.test/image"><script>window.bad=1;</script><template><b>Deep</b></template>';var content=t.content,first=content.firstChild,watch=new MutationObserver(function(){});watch.observe(mount,{childList:true,subtree:true});var expected=t.innerHTML;if(t.outerHTML!=='<template id="t" data-label="A&amp;&quot;">'+expected+'</template>'||mount.innerHTML!==t.outerHTML||watch.takeRecords().length||content.firstChild!==first||content.isConnected||first.isConnected||document.getElementById('hidden')!==null||document.querySelectorAll('script').length||window.bad!==undefined)throw 'inert serialized contents and no DOM records';
"##);assert_eq!(f.controls.len(),1);assert_eq!(p.state.stylesheets.source_bytes,0);assert!(p.take_dynamic_scripts().is_empty());assert!(p.take_dynamic_stylesheets().is_empty());assert!(p.take_image_changes().2.is_empty());
}
