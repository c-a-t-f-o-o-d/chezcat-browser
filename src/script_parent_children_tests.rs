//! Original connected ParentNode reads, brands, work refusal and frame scope.
use super::*;
fn page()->(Document,Forms,Page){
    let mut doc=crate::html::parse_loaded_document("<html><body><div id='host'></div><template id='seed'><b id='seed-child'></b></template></body></html>",&||Ok(())).unwrap();doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();
    let location=crate::navigation::address("https://parent-node.test/source",&crate::navigation::Location::home()).unwrap();
    page.set_location(&location);page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);(doc,forms,page)
}
fn execute(doc:&mut Document,forms:&mut Forms,page:&mut Page,source:&str){let result=page.console_eval(doc,forms,source);assert_eq!(result.errors,0,"{}",page.report());}
#[test]
fn parent_node_scalar_mixed_fragments_template_import_and_reconnection_keep_real_owners(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var f=document.createDocumentFragment(),a=document.createElement('span'),b=document.createElement('p');
f.appendChild(document.createTextNode('before'));f.appendChild(a);f.appendChild(document.createComment('middle'));f.appendChild(b);f.appendChild(document.createTextNode('after'));
if(f.childElementCount!==2||f.firstElementChild!==a||f.lastElementChild!==b)throw 'mixed fragment';
var seed=document.getElementById('seed'),content=seed.content,original=content.firstElementChild;
if(seed.childElementCount!==0||seed.firstElementChild!==null||content.childElementCount!==1||content.lastElementChild!==original||original.ownerDocument!==content.ownerDocument)throw 'inert ownership';
var imported=document.importNode(content,true),copy=imported.firstElementChild;
if(imported.childElementCount!==1||copy===original||copy.ownerDocument!==document)throw 'real import';
var host=document.getElementById('host');host.appendChild(f);host.appendChild(imported);
if(f.childElementCount!==0||f.firstElementChild!==null||f.lastElementChild!==null||host.childElementCount!==3||host.firstElementChild!==a||host.lastElementChild!==copy||content.firstElementChild!==original)throw 'reconnected view';
host.removeChild(a);if(host.firstElementChild!==b||host.childElementCount!==2)throw 'live removal';
if(document.firstElementChild!==document.documentElement||document.lastElementChild!==document.documentElement||document.childElementCount!==1)throw 'Document ParentNode';
"#);
}
#[test]
fn parent_node_scalar_getters_have_real_brands_descriptors_and_ordinary_prototype_lookup(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var f=document.createDocumentFragment(),a=document.createElement('span');f.appendChild(a);
var P=DocumentFragment.prototype,d=Object.getOwnPropertyDescriptor(P,'firstElementChild'),g=d.get;
if(typeof g!=='function'||g.length!==0||g.name!=='get firstElementChild'||d.set!==undefined||!d.enumerable||!d.configurable||g.call(f)!==a||Object.prototype.hasOwnProperty.call(f,'firstElementChild'))throw 'getter descriptor';
for(var value of [{},window,document.createTextNode('x'),document.createComment('x'),document.createAttribute('x')]){var caught=false;try{g.call(value);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'invalid ParentNode brand';}
var calls=0;if(g.call(f,{toString:function(){calls++;throw 'extra argument';}})!==a||calls!==0)throw 'getter conversion';
var caught=false;try{(function(){'use strict';f.firstElementChild=null;})();}catch(e){caught=e instanceof TypeError;}if(!caught||f.firstElementChild!==a)throw 'readonly getter';
Object.defineProperty(P,'firstElementChild',{value:17,configurable:true});if(f.firstElementChild!==17)throw 'prototype replacement';
delete P.firstElementChild;if(f.firstElementChild!==undefined||'firstElementChild' in f)throw 'deleted getter resurrected';Object.defineProperty(P,'firstElementChild',d);if(f.firstElementChild!==a)throw 'getter restoration';
"#);
}
#[test]
fn parent_node_scalar_initial_blank_context_reads_use_guarded_real_child_handles(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var frame=document.createElement('iframe');frame.style.display='none';document.body.appendChild(frame);
var child=frame.contentDocument,html=child.documentElement;
if(child.firstElementChild!==html||child.lastElementChild!==html||child.childElementCount!==1||html.firstElementChild!==child.head||html.lastElementChild!==child.body||html.childElementCount!==2||child.body.firstElementChild!==null||child.body.childElementCount!==0)throw 'real child topology';
var g=Object.getOwnPropertyDescriptor(Document.prototype,'firstElementChild').get;if(g.call(child)!==html)throw 'borrowed same-origin getter';
var view=html.children;if(view!==html.children||view.length!==2||Array.from(view)[0]!==child.head||view[1]!==child.body||child.children[0]!==html||view.namedItem('missing')!==null||!(view instanceof frame.contentWindow.HTMLCollection))throw 'real child collection realm';
"#);
}
#[test]
fn parent_node_scalar_work_refusal_preserves_tree_and_recovers_without_handle_allocation(){
    let(mut doc,mut forms,mut page)=page();let node=doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..}if tag=="body")).unwrap();
    let children=doc.nodes[node].children.clone();let before=page.state.retained;let lists=page.state.node_lists.len();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};bridge.state.host_work=0;
    assert!(bridge.parent_node_get(node_handle(node),"firstElementChild").is_err());assert_eq!(bridge.doc.nodes[node].children,children);assert_eq!(bridge.state.retained,before);assert_eq!(bridge.state.node_lists.len(),lists);
    bridge.state.host_work=bridge.state.limits.host_work;
    assert!(matches!(bridge.parent_node_get(node_handle(node),"firstElementChild").unwrap(),Value::Host(handle) if handle==node_handle(children[0])));
    assert!(!bridge.parent_node_receiver(node_handle(0)));assert!(!bridge.parent_node_receiver(node_handle(bridge.doc.nodes.len()+1)));
}

#[test]
fn parent_node_children_connected_live_views_array_from_and_template_promotion(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var f=document.createDocumentFragment(),a=document.createElement('span'),b=document.createElement('input');
a.id='alpha';b.setAttribute('name','beta');
f.appendChild(document.createTextNode('before'));f.appendChild(a);f.appendChild(document.createComment('middle'));f.appendChild(b);
var c=f.children;
if(!(f instanceof DocumentFragment)||typeof Array.from!=='function'||!(c instanceof HTMLCollection)||Object.prototype.toString.call(c)!=='[object HTMLCollection]')throw 'isolated fragment collection brand';
var values=Array.from(c);if(c!==f.children||values.length!==2||values[0]!==a||values[1]!==b||c.item(2)!==null||c[2]!==undefined||c.namedItem('alpha')!==a||c.namedItem('beta')!==b||c.alpha!==a)throw 'live element-only view';
var n=document.createElement('i');f.insertBefore(n,b);if(c.length!==3||c[1]!==n||Array.from(c)[2]!==b)throw 'saved live insertion';
var seed=document.getElementById('seed'),content=seed.content,sourceView=content.children,original=sourceView[0],originalChildren=original.children;
if(seed.children.length!==0||content.children!==sourceView||sourceView.length!==1||Array.from(sourceView)[0]!==original||originalChildren.length!==0)throw 'real inert children';
var imported=document.importNode(content,true),copyView=imported.children,copy=copyView[0],savedCopyChildren=copy.children;
if(copyView.length!==1||copy.ownerDocument!==document||copy===original)throw 'import collection';
var host=document.getElementById('host'),hostView=host.children;host.appendChild(f);host.appendChild(imported);
if(f.children!==c||c.length!==0||imported.children!==copyView||copyView.length!==0||hostView.length!==4||hostView[3]!==copy||sourceView[0]!==original||sourceView.length!==1)throw 'donor empty live views';
if(copy.children!==savedCopyChildren||original.children!==originalChildren)throw 'promotion SameObject';
copy.appendChild(document.createElement('em'));if(savedCopyChildren.length!==1||savedCopyChildren[0]!==copy.firstElementChild)throw 'promoted saved owner';
host.removeChild(a);if(hostView.length!==3||hostView[0]!==n||Array.from(hostView)[1]!==b)throw 'live removal';
if(document.children.length!==1||document.children[0]!==document.documentElement)throw 'document collection';
"#);
}
#[test]
fn parent_node_children_collection_getters_methods_and_iteration_use_ordinary_prototypes(){
    let(mut doc,mut forms,mut page)=page();
    execute(&mut doc,&mut forms,&mut page,r#"
var f=document.createDocumentFragment(),a=document.createElement('b');f.appendChild(a);var c=f.children;
var P=HTMLCollection.prototype,ld=Object.getOwnPropertyDescriptor(P,'length'),cd=Object.getOwnPropertyDescriptor(DocumentFragment.prototype,'children');
if(ld.get.length!==0||ld.get.name!=='get length'||ld.set!==undefined||!ld.enumerable||!ld.configurable||P.item.length!==1||P.namedItem.length!==1||Object.prototype.hasOwnProperty.call(c,'length'))throw 'collection descriptors';
for(var value of [{},f,f.childNodes]){for(var method of [ld.get,P.item,P.namedItem]){var caught=false;try{method.call(value,0);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'collection receiver';}}
var caught=false;try{new HTMLCollection();}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'illegal collection constructor';
var conversions=0,index={valueOf:function(){conversions++;return 0;}};if(c.item(index)!==a||conversions!==1)throw 'index conversion';
Object.defineProperty(DocumentFragment.prototype,'children',{value:17,configurable:true});if(f.children!==17)throw 'children replacement';delete DocumentFragment.prototype.children;if(f.children!==undefined||'children' in f)throw 'children resurrection';Object.defineProperty(DocumentFragment.prototype,'children',cd);
Object.defineProperty(P,'length',{value:7,configurable:true});if(c.length!==7)throw 'length replacement';delete P.length;if(c.length!==undefined||'length' in c)throw 'length resurrection';Object.defineProperty(P,'length',ld);
var item=P.item;P.item=function(){return 19;};if(c.item(0)!==19)throw 'item replacement';delete P.item;if(c.item!==undefined)throw 'item resurrection';P.item=item;
var iterator=P[Symbol.iterator];P[Symbol.iterator]=function(){return {next:function(){return {done:true};}};};if(Array.from(c).length!==0)throw 'iterator replacement';P[Symbol.iterator]=iterator;if(Array.from(c)[0]!==a)throw 'iterator restoration';
var tags=document.getElementsByTagName('div');if(!(tags instanceof HTMLCollection)||Array.from(tags)[0]!==document.getElementById('host'))throw 'shared tag collection';
"#);
}
#[test]
fn parent_node_children_registry_refusal_is_atomic_and_saved_view_recovers(){
    let(mut doc,mut forms,mut page)=page();let baseline=page.state.retained;let limit=page.state.retained_limit;
    page.state.retained_limit=baseline;
    assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.parent_node_get(DOCUMENT,"children").is_err());
    assert!(page.state.dom.collections.is_empty());assert_eq!(page.state.dom.collections.capacity(),0);assert_eq!(page.state.retained,baseline);
    page.state.retained_limit=limit;page.begin_job();
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let Value::Host(handle)=bridge.parent_node_get(DOCUMENT,"children").unwrap()else{panic!("collection")};
    let retained=bridge.state.retained;let count=bridge.state.dom.collections.len();bridge.state.retained_limit=retained;
    assert!(matches!(bridge.parent_node_get(DOCUMENT,"children").unwrap(),Value::Host(saved)if saved==handle));
    assert!(bridge.parent_element_nodes(DOCUMENT).is_err());assert_eq!(bridge.state.retained,retained);assert_eq!(bridge.state.dom.collections.len(),count);
    bridge.state.retained_limit=limit;assert_eq!(bridge.parent_element_nodes(DOCUMENT).unwrap().len(),1);
}
