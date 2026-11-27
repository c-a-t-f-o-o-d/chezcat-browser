//! Passive Text children preserve native identity, records and live ranges.
use super::*;
fn fixture()->(Document,Forms,Page){let mut d=crate::html::parse_loaded_document_with_scripting("<!--root--><body><noscript id=host>held</noscript><button id=button><noscript id=guard>guarded</noscript></button><p id=out>Real</p>",true,&||Ok(())).unwrap();let mut f=Forms::new(&d);let mut p=Page::new();let l=crate::navigation::address("https://noscript-text-ranges.test/",&crate::navigation::Location::home()).unwrap();p.set_location(&l);p.set_resource_base(&l);assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
fn refusal(d:&mut Document,f:&mut Forms,p:&mut Page,code:&str)->String{let source=format!("var caughtFailure=null;try{{{code}}}catch(e){{caughtFailure=e.name+': '+e.message;}}");match p.runtime.execute(&source,&mut Bridge{doc:d,forms:f,state:&mut p.state}){Err(e)=>e,Ok(_)=>{let value=p.runtime.execute("caughtFailure",&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();let Value::String(message)=value else{panic!("Expected an actual native refusal: {value:?}")};message}}}



#[test]
fn range_contextual_fragment_native_identity_context_and_source_records(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),out=document.getElementById('out'),watch=new MutationObserver(function(){});watch.observe(document.body,{subtree:true,childList:true,characterData:true});r.selectNodeContents(out);
var desc=Object.getOwnPropertyDescriptor(Range.prototype,'createContextualFragment');if(!desc||!desc.writable||!desc.configurable||!desc.enumerable||desc.value.length!==1||r.hasOwnProperty('createContextualFragment'))throw 'inherited native descriptor';
var a=r.createContextualFragment('<span id="fresh">A&amp;B</span><!--c-->tail'),b=r.createContextualFragment('');if(a===b||a.nodeType!==11||a.ownerDocument!==document||a.parentNode!==null||a.childNodes.length!==3||a.firstChild.localName!=='span'||a.firstChild.id!=='fresh'||a.firstChild.textContent!=='A&B'||a.firstChild.ownerDocument!==document||a.childNodes[1].nodeType!==8||a.lastChild.data!=='tail'||b.firstChild!==null||out.textContent!=='Real'||r.startContainer!==out||r.startOffset||r.endOffset!==1||watch.takeRecords().length)throw 'genuine detached producer';
var host=document.getElementById('host');r.selectNodeContents(host);var raw=r.createContextualFragment('<b>&amp;</b>');if(raw.childNodes.length!==1||raw.firstChild.data!=='<b>&amp;</b>')throw 'noscript context';r.setStart(host.firstChild,1);r.collapse(true);if(r.createContextualFragment('<i>x</i>').firstChild.data!=='<i>x</i>')throw 'Text parent context';
r.selectNodeContents(document);if(r.createContextualFragment('<em>x</em>').firstChild.localName!=='em')throw 'Document fallback';r.selectNodeContents(document.documentElement);if(r.createContextualFragment('<em>x</em>').firstChild.localName!=='em')throw 'html fallback';r.selectNodeContents(document.createTextNode('x'));if(r.createContextualFragment('<em>x</em>').firstChild.localName!=='em')throw 'parentless fallback';
"#);assert!(p.take_dynamic_scripts().is_empty());
}
#[test]
fn range_contextual_fragment_conversion_reads_live_start_and_brand(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),host=document.getElementById('host'),out=document.getElementById('out'),calls=0;r.selectNodeContents(out);var input={toString:function(){calls++;r.selectNodeContents(host);return '<b>x</b>';}};var a=r.createContextualFragment(input);if(calls!==1||a.firstChild.data!=='<b>x</b>'||r.startContainer!==host)throw 'live context after conversion';
var method=Range.prototype.createContextualFragment;var caught=0;try{method.call({},input);}catch(e){if(e.name!=='TypeError')throw e;caught++;}if(caught!==1||calls!==1)throw 'brand before conversion';caught=0;try{r.createContextualFragment();}catch(e){if(e.name!=='TypeError')throw e;caught++;}try{r.createContextualFragment(Symbol('x'));}catch(e){if(e.name!=='TypeError')throw e;caught++;}if(caught!==2)throw 'required string conversion';
"#);
}
#[test]
fn range_contextual_fragment_refusal_quota_and_fresh_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var r=document.createRange(),out=document.getElementById('out'),watch=new MutationObserver(function(){});r.selectNodeContents(out);watch.observe(document.body,{subtree:true,childList:true});");let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;let limit=p.state.retained_limit;
 for code in ["r.createContextualFragment('<button>x</button>');","r.createContextualFragment('<span onclick=bad>x</span>');","r.createContextualFragment('\\ud800');"]{p.begin_job();let e=refusal(&mut d,&mut f,&mut p,code);assert!(e.contains("not implemented")||e.contains("unsupported"),"{e}");assert_eq!(format!("{:?}",d.nodes),before);}
 for mode in 0..3 {p.begin_job();match mode{0=>p.state.copy_work=0,1=>p.state.host_work=0,_=>p.state.retained_limit=p.state.retained};let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment('<span>payload</span>');").to_ascii_lowercase();assert!(e.contains("limit")||e.contains("allowance")||e.contains("storage"),"{mode}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);p.state.retained_limit=limit;}
 p.begin_job();run(&mut d,&mut f,&mut p,"if(watch.takeRecords().length||out.textContent!=='Real'||r.startContainer!==out||r.endOffset!==1)throw 'initial refusal source unchanged';var fresh=r.createContextualFragment('<b>recovered</b>');if(fresh.firstChild.textContent!=='recovered'||fresh.ownerDocument!==document)throw 'fresh recovery';");assert!(p.take_dynamic_scripts().is_empty());
}

#[test]
fn range_contextual_fragment_comment_rcdata_fallback_and_owner_guards(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var r=document.createRange(),title=document.createElement('title'),comment=document.createComment('c'),parent=document.createElement('p');r.selectNodeContents(title);var a=r.createContextualFragment('<b>&amp;</b>');if(a.childNodes.length!==1||a.firstChild.data!=='<b>&</b>'||title.firstChild!==null)throw 'RCDATA context';parent.appendChild(comment);r.setStart(comment,0);if(r.createContextualFragment('<em>x</em>').firstChild.localName!=='em'||comment.parentNode!==parent)throw 'Comment parent context';
r.selectNodeContents(document.firstChild);if(document.firstChild.nodeType!==8||r.createContextualFragment('<em>x</em>').firstChild.localName!=='em')throw 'Document child Comment fallback';
var attr=document.createAttribute('held');r.selectNodeContents(attr);if(r.createContextualFragment('<em>x</em>').firstChild.localName!=='em')throw 'Attr document fallback';r.selectNodeContents(document.createDocumentFragment());if(r.createContextualFragment('<em>x</em>').firstChild.localName!=='em')throw 'Fragment fallback';
var owner=document.implementation.createHTMLDocument('Other'),foreign=owner.createRange();foreign.selectNodeContents(owner.body);var svg=document.createElementNS('http://www.w3.org/2000/svg','svg');r.selectNodeContents(svg);var frozen=new StaticRange({startContainer:document.body,startOffset:0,endContainer:document.body,endOffset:0});
"#);let before=format!("{:?}",d.nodes);let commits=p.state.dom_commits;
 for code in ["foreign.createContextualFragment('<b>x</b>');","r.createContextualFragment('<b>x</b>');","Range.prototype.createContextualFragment.call(frozen,'x');"]{p.begin_job();let e=refusal(&mut d,&mut f,&mut p,code);assert!(e.contains("not implemented")||e.contains("TypeError"),"{code}: {e}");assert_eq!(format!("{:?}",d.nodes),before);assert_eq!(p.state.dom_commits,commits);}
 p.begin_job();run(&mut d,&mut f,&mut p,"r.selectNodeContents(document.getElementById('out'));");let limit=p.state.node_limit;p.state.node_limit=d.nodes.len();p.begin_job();let e=refusal(&mut d,&mut f,&mut p,"r.createContextualFragment('<span>x</span>');");assert!(e.contains("node limit"),"{e}");assert_eq!(format!("{:?}",d.nodes),before);p.state.node_limit=limit;p.begin_job();run(&mut d,&mut f,&mut p,"if(r.createContextualFragment('fresh').firstChild.data!=='fresh')throw 'node capacity recovery';");
}
