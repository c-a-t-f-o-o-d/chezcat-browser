//! Original live TreeWalker callback and cursor witnesses.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<!doctype html><body><div id=root><section id=reject><b id=hidden>Hidden</b></section><section id=skip><b id=a>A</b><i id=b>B</i></section><p id=c>C</p></div><input id=neighbor value=seed></body>");
    doc.scripting_enabled=true;let mut forms=Forms::new(&doc);let mut page=Page::new();
    page.set_location(&crate::navigation::address("https://tree-filter.test/source",&crate::navigation::Location::home()).unwrap());
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    (doc,forms,page)
}
fn run(d:&mut Document,f:&mut Forms,p:&mut Page,s:&str){
    p.begin_job();p.runtime.execute(s,&mut Bridge{doc:d,forms:f,state:&mut p.state}).unwrap();
}
#[test]
fn tree_walker_filter_function_object_identity_this_and_unsigned_conversion(){
    let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),reject=document.getElementById('reject'),skip=document.getElementById('skip'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),calls=0;
function fn(n){'use strict';if(this!==undefined||arguments.length!==1||n!==reject)throw 'function this and node';calls++;return {valueOf:function(){calls++;return 65537.9;}};}
Object.defineProperty(fn,'acceptNode',{get:function(){throw 'callable must ignore method';}});
var w=document.createTreeWalker(root,1,fn);if(w.filter!==fn||calls||w.firstChild()!==reject||calls!==2)throw 'function identity and ushort';
var obj={get acceptNode(){calls++;return function(n){if(this!==obj||n!==reject)throw 'object this';calls++;return 1;};}};
var o=document.createTreeWalker(root,1,obj);if(o.filter!==obj||o.nextNode()!==reject||calls!==4)throw 'dynamic object operation';
Object.defineProperty(obj,'acceptNode',{value:function(){return 2;},configurable:true});o.currentNode=root;
if(o.nextNode()!==null)throw 'changed callback must reject all subtrees';
var proxy=new Proxy({acceptNode:function(n){if(this!==proxy||n!==reject)throw 'proxy receiver';return 1;}},{});
if(document.createTreeWalker(root,1,proxy).nextNode()!==reject)throw 'ordinary proxy callback';
var textCalls=0,t=document.createTreeWalker(root,4,function(n){textCalls++;if(n.nodeType!==3)throw 'mask';return 1;});
if(t.nextNode().nodeValue!=='Hidden'||textCalls!==1)throw 'masked elements must skip without callback';
var bad=document.createTreeWalker(root,1,{}),caught=0;try{bad.nextNode();}catch(e){if(e instanceof TypeError)caught++;}
try{document.createTreeWalker(root,1,7);}catch(e){if(e instanceof TypeError)caught++;}
if(caught!==2||bad.currentNode!==root)throw 'callback type and cursor';
"#);
}
#[test]
fn tree_walker_filter_all_seven_operations_prune_reject_and_descend_skip(){
    let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),skip=document.getElementById('skip'),seen='';
var w=document.createTreeWalker(root,1,function(n){seen+=n.id+',';return n.id==='reject'?2:n.id==='skip'?3:1;});
if(w.firstChild()!==a||seen!=='reject,skip,a,')throw 'first child pruning';
seen='';if(w.nextSibling()!==b||w.nextSibling()!==c||w.nextSibling()!==null)throw 'forward sibling';
if(w.previousSibling()!==b||w.previousSibling()!==a||w.previousSibling()!==null)throw 'reverse sibling';
if(w.parentNode()!==root||w.parentNode()!==null||w.lastChild()!==c)throw 'parent and last';
if(w.previousNode()!==b||w.previousNode()!==a||w.previousNode()!==root||w.previousNode()!==null)throw 'previous pruned preorder';
if(w.nextNode()!==a||w.nextNode()!==b||w.nextNode()!==c||w.nextNode()!==null||w.currentNode!==c)throw 'next pruned preorder';
w.currentNode=root;w.lastChild();if(w.previousSibling()!==b)throw 'reverse skip subtree';
// Unknown ushort results are not normalized: child descent requires SKIP,
// whereas preorder only prunes REJECT.
var odd=document.createTreeWalker(root,1,function(n){return n.id==='reject'?0:1;});
if(odd.firstChild()!==skip)throw 'unknown child result';odd.currentNode=root;
if(odd.nextNode().id!=='hidden')throw 'unknown preorder result';
var outside=document.createElement('div'),leaf=document.createElement('i');outside.appendChild(leaf);w.currentNode=outside;
if(w.nextNode()!==leaf||w.root!==root)throw 'legal outside-root cursor';
"#);
}
#[test]
fn tree_walker_filter_reentry_exceptions_conversion_and_live_author_mutations(){
    let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),reject=document.getElementById('reject'),c=document.getElementById('c'),marker={},caught=0,w;
w=document.createTreeWalker(root,1,function(n){try{w.nextNode();}catch(e){if(e.name==='InvalidStateError'&&e instanceof DOMException)caught++;}return {valueOf:function(){try{w.firstChild();}catch(e){if(e.name==='InvalidStateError')caught++;}return 1;}};});
if(w.nextNode()!==reject||caught!==2)throw 'active callback and conversion';
var once=true,x=document.createTreeWalker(root,1,{get acceptNode(){if(once){once=false;x.currentNode=c;root.setAttribute('data-author','kept');throw marker;}return function(){return 1;};}});
try{x.nextNode();throw 'missing exception';}catch(e){if(e!==marker)throw e;}
if(x.currentNode!==c||root.getAttribute('data-author')!=='kept')throw 'author side effects';
x.currentNode=root;if(x.nextNode()!==reject)throw 'getter recovery';
var fail=true,y=document.createTreeWalker(root,1,function(){return {valueOf:function(){if(fail){fail=false;throw marker;}return 1;}};});
try{y.nextNode();throw 'missing conversion exception';}catch(e){if(e!==marker)throw e;}
if(y.currentNode!==root||y.nextNode()!==reject)throw 'conversion recovery';
var live=document.createTreeWalker(root,1,function(n){if(n===reject){root.removeChild(reject);root.appendChild(reject);return 2;}return 1;});
if(live.nextNode()!==null||live.currentNode!==root||root.lastChild!==reject)throw 'live moved rejected node';
var late=document.createElement('em');late.id='late';var changed=false,z=document.createTreeWalker(root,1,function(n){if(!changed){changed=true;n.appendChild(late);return 3;}return n.id==='skip'?3:1;});
if(z.nextNode()!==document.getElementById('a'))throw 'live child insertion';z.currentNode=late;if(z.parentNode()!==root)throw 'live skip parent';
"#);
}
#[test]
fn tree_walker_filter_closure_survives_later_jobs_and_function_collection(){
    let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),reads=0;
function make(){var secret='reject';return function(n){reads++;return n.id===secret?1:2;};}
var held=document.createTreeWalker(root,1,make());0;
"#);
    for _ in 0..3{run(&mut d,&mut f,&mut p,"var trash={};for(var i=0;i<900;i++)trash.fn=function(){return i;};trash=null;0;");}
    run(&mut d,&mut f,&mut p,"if(reads||held.nextNode()!==document.getElementById('reject')||reads!==1)throw 'retained callback closure';held=null;0;");
}
#[test]
fn tree_walker_filter_fatal_host_budget_clears_active_and_preserves_cursor(){
    let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),armed=true,w=document.createTreeWalker(root,1,function(n){if(armed){armed=false;n.setAttribute('data-author','kept');n.textContent='Changed';}return 1;});0;
"#);
    p.begin_job();p.state.copy_work=1;
    let failed=p.runtime.execute("w.nextNode();",&mut Bridge{doc:&mut d,forms:&mut f,state:&mut p.state}).unwrap_err();
    assert!(!failed.is_empty());
    run(&mut d,&mut f,&mut p,"if(armed||w.currentNode!==root)throw 'fatal active callback cursor';w.currentNode=root;if(w.nextNode()!==document.getElementById('reject'))throw 'fresh recovery';");
    assert!(!p.has_navigation_request());assert!(p.take_dynamic_scripts().is_empty());
}
#[test]
fn tree_walker_filter_child_callback_realm_and_live_operation_owner(){
    let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var root=document.getElementById('root'),reject=document.getElementById('reject'),frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var child=frame.contentWindow,childArray=child.Array,parentArray=Array;
var filter=child.eval("var filter={calls:0};Object.defineProperty(filter,'acceptNode',{get:function(){this.getterArray=Array;return this.selected;}});filter.selected=function(n){if(this!==filter)throw 'child this';this.calls++;this.methodArray=Array;return 1;};filter;");
var walker=document.createTreeWalker(root,1,filter);
if(walker.nextNode()!==reject||filter.calls!==1||filter.getterArray!==childArray||filter.methodArray!==childArray||childArray===Array)throw 'genuine child callback realm';
filter.selected=function(n){if(this!==filter||n!==reject)throw 'parent operation';this.methodArray=Array;return 1;};walker.currentNode=root;
if(walker.nextNode()!==reject||filter.getterArray!==childArray||filter.methodArray!==parentArray||Array!==parentArray)throw 'live operation and restored caller';
var strict=child.eval("(function(n){'use strict';if(this!==undefined)throw 'strict callback this';return 1;})");
if(document.createTreeWalker(root,1,strict).nextNode()!==reject||Array!==parentArray)throw 'foreign function route';
var missing=child.eval('({})');Object.setPrototypeOf(missing,Object.prototype);var bad=document.createTreeWalker(root,1,missing),caught=0;
try{bad.nextNode();}catch(e){if(e instanceof child.TypeError&&!(e instanceof TypeError))caught++;}
var invalid=child.eval('(function(){return Symbol("not a number");})');
try{document.createTreeWalker(root,1,invalid).nextNode();}catch(e){if(e instanceof child.TypeError&&!(e instanceof TypeError))caught++;}
if(caught!==2||bad.currentNode!==root||Array!==parentArray)throw 'callback error realm';

"#);
}
#[test]
fn tree_walker_filter_inert_template_roots_keep_native_identity_and_exact_units(){
    let(mut d,mut f,mut p)=fixture();run(&mut d,&mut f,&mut p,r#"
var parsed=new DOMParser().parseFromString('<body><template id=t><section id=reject><b>Hidden</b></section><section id=skip><span id=inside>Seed</span><template id=n><b>Nested</b></template></section><p id=last>Last</p></template></body>','text/html');
var content=parsed.getElementById('t').content,owner=content.ownerDocument,inside=content.querySelector('#inside');
inside.firstChild.nodeValue='\uD800X\uDC00';
var filter=function(n){if(n.ownerDocument!==owner)throw 'inert owner';return n.nodeType===1?(n.id==='reject'?2:3):1;};
var walker=document.createTreeWalker(content,4294967295,filter),text=inside.firstChild;
if(walker.root!==content||walker.nextNode()!==text||text.nodeValue.length!==3||text.nodeValue.charCodeAt(0)!==55296||text.nodeValue.charCodeAt(2)!==56320)throw 'inert exact units';
if(walker.nextNode().nodeValue!=='Last'||walker.nextNode()!==null)throw 'separate nested content roots';
walker.currentNode=content;if(walker.firstChild()!==text)throw 'inert skip and reject';
if(parsed.querySelector('#inside')!==null||content.querySelector('#n').content.textContent!=='Nested'||content.isConnected)throw 'inert tree preserved';
"#);
}
