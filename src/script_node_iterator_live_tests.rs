//! Original native iterator traversal, filtering and removal witnesses.
use super::*;
fn fixture()->(Document,Forms,Page){
 let mut d=crate::html::parse_loaded_document("<!doctype html><html><body><div id=mount></div><input id=neighbor value=seed></body></html>",&||Ok(())).unwrap();d.scripting_enabled=true;let mut f=Forms::new(&d);let mut p=Page::new();p.set_location(&crate::navigation::address("https://iterator.test/source",&crate::navigation::Location::home()).unwrap());assert_eq!(p.start(&mut d,&mut f,&[]).errors,0);(d,f,p)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();}
#[test]
fn node_iterator_live_forward_reverse_masks_reject_skip_and_root_boundaries(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('mount');root.innerHTML='<section id=reject><b id=hidden>Hidden</b></section><section id=skip><b id=a>A</b><i id=b>B</i></section><p id=c>C</p>';
var it=document.createNodeIterator(root,1,function(n){return n.id==='reject'?2:n.id==='skip'?3:1;}),seen='',node;
while((node=it.nextNode())!==null)seen+=node.id+',';
if(seen!=='mount,hidden,a,b,c,'||it.referenceNode!==root.lastChild||it.pointerBeforeReferenceNode)throw 'iterator reject does not prune';
seen='';while((node=it.previousNode())!==null)seen+=node.id+',';
if(seen!=='c,b,a,hidden,mount,'||it.referenceNode!==root||!it.pointerBeforeReferenceNode||it.nextNode()!==root)throw 'reverse pointer and root boundary';
var calls=0,text=document.createNodeIterator(root,4,function(n){calls++;if(n.nodeType!==3)throw 'masked callback';return 1;}),value='';
while((node=text.nextNode())!==null)value+=node.data;if(value!=='HiddenABC'||calls!==4)throw 'native text mask';
var ignored={toString:function(){throw 'extra args ignored';}};if(it.previousNode(ignored)!==root)throw 'ignored args and pointer direction';
"#);
}
#[test]
fn node_iterator_live_callback_lookup_conversion_active_guard_and_abrupt_identity(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('mount');root.innerHTML='<span>A</span><b>B</b>';var gets=0,calls=0,iterator,filter={get acceptNode(){gets++;return function(n){if(this!==filter)throw 'callback this';calls++;return {valueOf:function(){var caught=false;try{iterator.nextNode();}catch(e){caught=e instanceof DOMException&&e.name==='InvalidStateError';}if(!caught)throw 'active through conversion';return 65537.9;}};};}};
iterator=document.createNodeIterator(root,1,filter);if(iterator.nextNode()!==root||gets!==1||calls!==1||iterator.pointerBeforeReferenceNode)throw 'callback identity and ushort conversion';
var abrupt={},armed=true;Object.defineProperty(filter,'acceptNode',{configurable:true,value:function(n){if(armed)throw abrupt;return 1;}});
var caught=false;try{iterator.nextNode();}catch(e){caught=e===abrupt;}if(!caught||iterator.referenceNode!==root||iterator.pointerBeforeReferenceNode)throw 'original abrupt and reference';armed=false;
if(iterator.nextNode()!==root.firstChild||iterator.referenceNode!==root.firstChild)throw 'active candidate recovery';
var strict=document.createNodeIterator(root,1,function(n){'use strict';if(this!==undefined||arguments.length!==1)throw 'function callback this';return 1;});if(strict.nextNode()!==root)throw 'function callback';
var missing=document.createNodeIterator(root,1,{}),errors=0;try{missing.nextNode();}catch(e){if(e instanceof TypeError)errors++;}if(errors!==1||missing.referenceNode!==root||!missing.pointerBeforeReferenceNode)throw 'missing operation leaves initial tuple';
"#);
}
#[test]
fn node_iterator_live_before_after_removal_adjusts_reference_without_observers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('mount');root.innerHTML='<span>A</span><b>B</b><i>C</i>';var a=root.firstChild,b=a.nextSibling,c=root.lastChild,before=document.createNodeIterator(root,1),after=document.createNodeIterator(root,1);
for(var it of [before,after]){it.nextNode();it.nextNode();if(it.nextNode()!==b)throw 'position b';}
if(before.previousNode()!==b||!before.pointerBeforeReferenceNode||after.detach()!==undefined)throw 'before and detached state';
b.remove();
if(before.referenceNode!==c||!before.pointerBeforeReferenceNode||after.referenceNode!==a.firstChild||after.pointerBeforeReferenceNode)throw 'pre-removal reference tuples';
if(before.nextNode()!==c||after.nextNode()!==c)throw 'continue from adjusted tuples';
root.remove();if(before.referenceNode!==c||after.referenceNode!==c||before.root!==root||after.root!==root)throw 'removing iterator root preserves subtree';
"#);
}
#[test]
fn node_iterator_live_moves_fragments_replacement_and_content_update_saved_positions(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('mount');root.innerHTML='<span>A</span><b>B</b><i>C</i>';var a=root.firstChild,b=a.nextSibling,c=root.lastChild,it=document.createNodeIterator(root,1),outside=document.createElement('div');it.nextNode();it.nextNode();it.nextNode();outside.appendChild(b);
if(it.referenceNode!==a.firstChild||it.pointerBeforeReferenceNode||it.nextNode()!==c)throw 'native move pre-removal';
var fragment=document.createDocumentFragment(),fresh=document.createElement('em');fresh.textContent='Fresh';fragment.appendChild(fresh);root.replaceChild(fragment,c);
if(it.referenceNode!==a.firstChild||it.nextNode()!==fresh||fragment.childNodes.length)throw 'fragment replacement cursor';
var watch=new MutationObserver(function(){});watch.observe(root,{childList:true,subtree:true});root.textContent='Replacement';
if(it.referenceNode!==root||it.pointerBeforeReferenceNode||it.nextNode()!==null||watch.takeRecords().length!==1)throw 'replace all removes selected element';
var text=document.createNodeIterator(root,4);if(text.nextNode().data!=='Replacement')throw 'new native content';root.innerHTML='<b>Later</b>';if(text.referenceNode!==root||text.pointerBeforeReferenceNode||text.nextNode().data!=='Later')throw 'innerHTML replacement continuation';
"#);
}
#[test]
fn node_iterator_live_inert_template_roots_raw_units_and_independent_native_copies(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var template=document.createElement('template');template.innerHTML='<span>A</span><b>B</b><i>C</i>';var root=template.content,owner=root.ownerDocument,it=document.createNodeIterator(root,1),a=root.firstChild,b=a.nextSibling,c=root.lastChild;it.nextNode();if(it.nextNode()!==b)throw 'fragment masked root';b.remove();
if(it.referenceNode!==a.firstChild||it.nextNode()!==c||c.ownerDocument!==owner||owner===document)throw 'inert native removal';
var copy=template.cloneNode(true),copied=document.createNodeIterator(copy.content,4);if(copied.nextNode()===a.firstChild||copied.referenceNode.ownerDocument!==owner)throw 'independent native copied root';
var exact=owner.createDocumentFragment(),first=owner.createTextNode('\uD800'),second=owner.createTextNode('\uDC00');exact.appendChild(first);exact.appendChild(second);var text=document.createNodeIterator(exact,4);if(text.nextNode()!==first||text.nextNode()!==second||first.data.charCodeAt(0)!==55296||second.data.charCodeAt(0)!==56320)throw 'exact raw units';second.remove();if(text.referenceNode!==first||text.pointerBeforeReferenceNode||text.previousNode()!==first)throw 'inert CharacterData removal tuple';
"#);
}
#[test]
fn node_iterator_live_normalization_multiple_removals_and_root_exclusion(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
for(var owner of [document,new Document()]){
 var root=owner.createElementNS('http://www.w3.org/1999/xhtml','div'),a=owner.createTextNode('A'),empty=owner.createTextNode(''),b=owner.createTextNode('B'),tail=owner.createTextNode('');for(var n of [a,empty,b,tail])root.appendChild(n);
 var it=document.createNodeIterator(root,4),inside=document.createNodeIterator(b,4);it.nextNode();it.nextNode();it.nextNode();if(it.nextNode()!==tail||inside.nextNode()!==b)throw 'normalization setup';root.normalize();
 if(root.firstChild!==a||root.childNodes.length!==1||a.data!=='AB'||it.referenceNode!==a||it.pointerBeforeReferenceNode||it.previousNode()!==a||inside.referenceNode!==b||inside.root!==b||inside.pointerBeforeReferenceNode)throw 'aggregate normalization removal tuples';
}
"#);
}
#[test]
fn node_iterator_live_callback_removal_adjusts_candidate_and_keeps_author_effects(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('mount');root.innerHTML='<span>A</span><b>B</b><i>C</i>';var a=root.firstChild,b=a.nextSibling,c=root.lastChild,armed=true,it=document.createNodeIterator(root,1,function(n){if(n===b&&armed){armed=false;b.remove();}return 1;});
it.nextNode();it.nextNode();if(it.nextNode()!==b||it.referenceNode!==a.firstChild||it.pointerBeforeReferenceNode||b.parentNode!==null||it.nextNode()!==c)throw 'accepted removed node and adjusted candidate';
root.innerHTML='<span>A</span><b>B</b><i>C</i>';a=root.firstChild;b=a.nextSibling;c=root.lastChild;var abrupt={},bad=document.createNodeIterator(root,1,function(n){if(n===b){b.remove();throw abrupt;}return 1;});bad.nextNode();bad.nextNode();var caught=false;try{bad.nextNode();}catch(e){caught=e===abrupt;}if(!caught||bad.referenceNode!==a||bad.pointerBeforeReferenceNode||b.parentNode!==null||bad.nextNode()!==c)throw 'author removal survives callback error without candidate prefix';
"#);
}
#[test]
fn node_iterator_live_child_callback_realms_and_native_failure_recovery(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('mount');root.innerHTML='<span>A</span><b>B</b>';var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var child=frame.contentWindow,childArray=child.Array;
var callback=child.eval("(function(n){'use strict';if(this!==undefined)throw 'foreign callback this';return Array===parent.Array?2:1;})"),it=document.createNodeIterator(root,1,callback);
if(it.nextNode()!==root||Array===childArray)throw 'foreign callback realm';
var missing=child.eval('({})'),bad=document.createNodeIterator(root,1,missing),caught=false;try{bad.nextNode();}catch(e){caught=e instanceof child.TypeError;}if(!caught||bad.referenceNode!==root||!bad.pointerBeforeReferenceNode)throw 'callback object creation realm and initial state';
"#);
}
#[test]
fn node_iterator_live_native_mutation_budget_refusal_preserves_positions_and_recovers(){
 let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,"var root=document.getElementById('mount');root.innerHTML='<span>A</span><b>B</b>';var a=root.firstChild,b=root.lastChild,it=document.createNodeIterator(root,1);it.nextNode();it.nextNode();it.nextNode();");
 p.begin_job();p.state.copy_work=0;let failed=p.runtime.execute("b.remove();",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state});assert!(failed.is_err());
 run(&mut d,&mut f,&mut p,"if(b.parentNode!==root||it.referenceNode!==b||it.pointerBeforeReferenceNode)throw 'failed native removal must not publish cursor';b.remove();if(b.parentNode!==null||it.referenceNode!==a.firstChild||it.pointerBeforeReferenceNode||it.previousNode()!==a)throw 'fresh removal recovery';");
}
