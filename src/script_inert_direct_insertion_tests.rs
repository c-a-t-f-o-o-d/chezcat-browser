//! Real direct insertion shares promotion and preserves source identities.
use super::*;
fn fixture()->(Document,Forms,Page){let mut doc=crate::html::parse("<!doctype html><html><body><div id=mount><span id=one>one</span><span id=two>two</span></div><input id=field value=seed></body></html>");doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)}
fn run(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){assert_eq!(page.console_eval(doc,forms,source).errors,0,"{source}\n{}",page.report());}
#[test]
fn inert_direct_insertion_implicit_adoption_preserves_source_node_attr_views_and_both_observers(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var owner=document.implementation.createHTMLDocument('source'),root=owner.createElement('div'),text=owner.createTextNode('\ud800'),mount=document.getElementById('mount');root.setAttribute('data-note','held');root.appendChild(text);owner.body.appendChild(root);var attr=root.getAttributeNode('data-note'),sourceList=owner.body.childNodes,list=root.childNodes,state={held:root},old=new MutationObserver(function(){}),next=new MutationObserver(function(){});root.bindingState=state;old.observe(owner,{childList:true,subtree:true});next.observe(mount,{childList:true});
if(mount.appendChild(root)!==root||root.ownerDocument!==document||root.parentNode!==mount||text.ownerDocument!==document||root.firstChild!==text||text.data.charCodeAt(0)!==0xd800||root.getAttributeNode('data-note')!==attr||attr.ownerDocument!==document||attr.ownerElement!==root||root.bindingState!==state||state.held!==root||root.childNodes!==list||sourceList.length!==0||owner.body.firstChild!==null||owner.title!=='source')throw 'same direct native records';
var removed=old.takeRecords(),added=next.takeRecords();if(removed.length!==1||removed[0].target!==owner.body||removed[0].removedNodes[0]!==root||removed[0].previousSibling!==null||removed[0].nextSibling!==null||added.length!==1||added[0].target!==mount||added[0].addedNodes[0]!==root||added[0].previousSibling!==document.getElementById('two')||added[0].nextSibling!==null)throw 'actual old and new parent observations';
"#);
}
#[test]
fn inert_direct_insertion_insert_before_and_character_leaf_factories_keep_native_kind_units_and_order(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var owner=document.implementation.createDocument(null,''),mount=document.getElementById('mount'),one=document.getElementById('one'),two=document.getElementById('two'),text=owner.createTextNode('A'),cdata=owner.createCDATASection('B'),comment=owner.createComment('marker'),pi=owner.createProcessingInstruction('original','\ud800');
if(mount.insertBefore(text,two)!==text||mount.insertBefore(cdata,null)!==cdata||mount.appendChild(comment)!==comment||mount.appendChild(pi)!==pi)throw 'direct leaf returns';var list=mount.childNodes;if(list.length!==6||list[0]!==one||list[1]!==text||list[2]!==two||list[3]!==cdata||list[4]!==comment||list[5]!==pi||cdata.nodeType!==4||!(cdata instanceof CDATASection)||comment.nodeType!==8||pi.nodeType!==7||!(pi instanceof ProcessingInstruction)||pi.target!=='original'||pi.data.charCodeAt(0)!==0xd800||mount.textContent!=='oneAtwoB')throw 'actual inserted leaf kinds and text';for(var node of [text,cdata,comment,pi])if(node.ownerDocument!==document||node.parentNode!==mount)throw 'actual direct leaf owners';
"#);assert!(!page.has_navigation_request());assert!(page.take_dynamic_scripts().is_empty()&&page.take_dynamic_stylesheets().is_empty());
}
#[test]
fn inert_direct_insertion_validation_refuses_hierarchy_bad_reference_and_unmaterialized_producers_without_effects(){
 let(mut doc,mut forms,mut page)=fixture();run(&mut doc,&mut forms,&mut page,r#"
var owner=document.implementation.createHTMLDocument('source'),source=owner.createElement('span'),mount=document.getElementById('mount'),outside=document.getElementById('field'),watch=new MutationObserver(function(){});owner.body.appendChild(source);watch.observe(owner,{childList:true,subtree:true});
var caught=false;try{mount.insertBefore(source,outside);}catch(e){caught=e instanceof DOMException&&e.name==='NotFoundError';}if(!caught||source.parentNode!==owner.body||source.ownerDocument!==owner||watch.takeRecords().length||mount.childNodes.length!==2)throw 'typed reference refusal with no prefix';
caught=false;try{document.createTextNode('leaf').appendChild(source);}catch(e){caught=e instanceof DOMException&&e.name==='HierarchyRequestError';}if(!caught||source.parentNode!==owner.body)throw 'typed parent hierarchy';
var xml=document.implementation.createDocument(null,''),foreign=xml.createElement('ordinary');caught=false;try{mount.appendChild(foreign);}catch(e){caught=e instanceof DOMException&&e.name==='NotSupportedError';}if(!caught||foreign.parentNode!==null||foreign.ownerDocument!==xml||mount.childNodes.length!==2)throw 'unmaterialized producer does not clone or invent identity';
var dtype=owner.doctype;caught=false;try{mount.appendChild(dtype);}catch(e){caught=e instanceof DOMException&&e.name==='HierarchyRequestError';}if(!caught||owner.doctype!==dtype||dtype.parentNode!==owner||watch.takeRecords().length)throw 'doctype stays under real document';
"#);
}
#[test]
fn inert_direct_insertion_saved_child_method_revalidates_origin_and_preserves_child_write_guard(){
 let(mut doc,mut forms,mut page)=fixture();page.set_location(&crate::navigation::address("https://direct-insertion-origin.test/",&crate::navigation::Location::home()).unwrap());run(&mut doc,&mut forms,&mut page,r#"
var owner=document.implementation.createHTMLDocument('owner'),source=owner.createElement('span'),pending=owner.createElement('em'),mount=document.getElementById('mount'),frame=document.createElement('iframe');owner.body.appendChild(source);owner.body.appendChild(pending);frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,saved=w.Node.prototype.appendChild;if(saved===Node.prototype.appendChild||Object.getPrototypeOf(saved)!==w.Function.prototype||saved.call(mount,source)!==source||source.ownerDocument!==document||source.parentNode!==mount)throw 'actual captured realm direct insertion';var caught=false;try{Node.prototype.appendChild.call(w.document.body,pending);}catch(e){caught=e instanceof TypeError;}if(!caught||pending.ownerDocument!==owner||pending.parentNode!==owner.body)throw 'child mutation guard';
"#);
 page.set_location(&crate::navigation::address("https://other-direct-insertion.test/",&crate::navigation::Location::home()).unwrap());let before=(doc.nodes.len(),page.state.dom_commits);
 assert_eq!(page.console_eval(&mut doc,&mut forms,"saved.call(mount,pending);").errors,1,"{}",page.report());assert!(page.report().contains("same-origin")||page.report().contains("Security"),"{}",page.report());assert_eq!((doc.nodes.len(),page.state.dom_commits),before);run(&mut doc,&mut forms,&mut page,"if(pending.ownerDocument!==owner||pending.parentNode!==owner.body||source.ownerDocument!==document||source.parentNode!==mount)throw 'denied direct insertion has no prefix';");
}
