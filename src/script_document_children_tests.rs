//! Genuine document edge operations use the pinned pre-insertion exclusions.
use super::*;
fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse("<!doctype html><html><body><div id=mount></div></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}
#[test]
fn document_children_xml_append_insert_remove_replace_update_real_roots_and_live_lists(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createDocument(null,''),list=d.childNodes,root=d.createElement('Root'),dtype=d.implementation.createDocumentType('Root','',''),comment=d.createComment('marker'),pi=d.createProcessingInstruction('original','data');
if(d.appendChild(root)!==root||d.documentElement!==root||root.parentNode!==d||root.ownerDocument!==d||list.length!==1)throw 'real append root';
d.insertBefore(dtype,root);d.insertBefore(comment,dtype);d.appendChild(pi);if(d.doctype!==dtype||list.length!==4||list[0]!==comment||list[1]!==dtype||list[2]!==root||list[3]!==pi||dtype.nextSibling!==root)throw 'real document order';
var other=d.createElement('Second');if(d.replaceChild(other,root)!==root||root.parentNode!==null||root.ownerDocument!==d||other.parentNode!==d||d.documentElement!==other||list[2]!==other)throw 'real root replacement';
if(d.removeChild(dtype)!==dtype||d.doctype!==null||dtype.parentNode!==null||dtype.ownerDocument!==d||list.length!==3)throw 'real doctype removal';d.insertBefore(dtype,other);
var next=d.implementation.createDocumentType('Second','public','system');if(d.replaceChild(next,dtype)!==dtype||d.doctype!==next||dtype.parentNode!==null||next.ownerDocument!==d||next.nextSibling!==other)throw 'real doctype replacement';
var fragment=d.createDocumentFragment();fragment.appendChild(root);d.replaceChild(fragment,other);if(fragment.childNodes.length||d.documentElement!==root||other.parentNode!==null||root.parentNode!==d||list[2]!==root)throw 'document fragment root replacement';
var parking=d.createDocumentFragment();parking.appendChild(root);if(d.documentElement!==null||root.parentNode!==parking||list.length!==3)throw 'moving root clears actual document element';d.appendChild(parking);if(d.documentElement!==root||parking.childNodes.length||root.parentNode!==d)throw 'root returns with saved identity';
"#);
}
#[test]
fn document_children_current_dom_rejects_duplicate_text_bad_order_and_existing_node_reinsertions(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createDocument(null,'Root'),root=d.documentElement,dtype=d.implementation.createDocumentType('Root','',''),watch=new MutationObserver(function(){});d.insertBefore(dtype,root);watch.observe(d,{childList:true});
function hierarchy(action){var caught=false;try{action();}catch(e){caught=e.name==='HierarchyRequestError';}if(!caught)throw 'required document hierarchy error';}
hierarchy(function(){d.appendChild(root);});hierarchy(function(){d.insertBefore(root,root);});hierarchy(function(){d.insertBefore(dtype,dtype);});hierarchy(function(){d.appendChild(dtype);});
hierarchy(function(){d.appendChild(d.createTextNode(''));});hierarchy(function(){d.appendChild(d.createCDATASection(''));});hierarchy(function(){d.appendChild(d.createElement('Second'));});
var two=d.createDocumentFragment();two.appendChild(d.createElement('One'));two.appendChild(d.createElement('Two'));hierarchy(function(){d.replaceChild(two,root);});if(two.childNodes.length!==2)throw 'refused fragment remains';
var bad=d.createDocumentFragment();bad.appendChild(d.createComment('prefix'));bad.appendChild(d.createTextNode('text'));hierarchy(function(){d.replaceChild(bad,root);});if(bad.childNodes.length!==2)throw 'no published fragment prefix';
hierarchy(function(){d.replaceChild(dtype,root);});hierarchy(function(){root.appendChild(dtype);});
hierarchy(function(){root.appendChild(root);});hierarchy(function(){d.insertBefore(d,root);});
if(watch.takeRecords().length||d.documentElement!==root||d.doctype!==dtype||root.parentNode!==d||dtype.parentNode!==d||d.childNodes.length!==2)throw 'all validity errors leave source records unchanged';
var only=document.implementation.createDocument(null,'Root'),saved=only.documentElement,own=only.implementation.createDocumentType('Root','','');only.replaceChild(own,saved);if(only.documentElement!==null||only.doctype!==own||saved.parentNode!==null)throw 'doctype may replace sole element';only.replaceChild(saved,own);if(only.documentElement!==saved||only.doctype!==null||own.parentNode!==null)throw 'element may replace excluded doctype';
"#);
}
#[test]
fn document_children_html_metadata_and_mutation_records_follow_actual_edges(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var d=document.implementation.createHTMLDocument('initial'),old=d.documentElement,head=d.head,body=d.body,watch=new MutationObserver(function(){});watch.observe(d,{childList:true});d.removeChild(old);var records=watch.takeRecords();if(d.documentElement!==null||d.head!==null||d.body!==null||d.title!==''||old.parentNode!==null||head.parentNode!==old||records.length!==1||records[0].removedNodes[0]!==old||records[0].target!==d)throw 'actual removed HTML root metadata';
d.appendChild(old);if(d.documentElement!==old||d.head!==head||d.body!==body||d.title!=='initial')throw 'current restored HTML root';old.removeChild(head);if(d.head!==null||d.title!=='')throw 'head move has current metadata';old.appendChild(head);if(d.head!==head||d.title!=='initial')throw 'same saved head restored';
var next=d.createElement('html'),newHead=d.createElement('head'),newBody=d.createElement('body');next.appendChild(newHead);next.appendChild(newBody);watch.takeRecords();d.replaceChild(next,old);records=watch.takeRecords();if(d.documentElement!==next||d.head!==newHead||d.body!==newBody||old.parentNode!==null||records.length!==1||records[0].addedNodes[0]!==next||records[0].removedNodes[0]!==old)throw 'current HTML root replacement and observation';
"#);
}
