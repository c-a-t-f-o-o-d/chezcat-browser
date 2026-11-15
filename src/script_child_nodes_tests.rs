//! Original actual Document/Element/Runtime NodeList controls. No service IO.
use super::*;

fn fixture(markup: &str, source: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    page.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://example.test/original").unwrap().0),
        path: None, root: None, fragment: String::new(),
    });
    let outcome = page.start(&mut doc, &mut forms, &[Script {
        node: 0, source: source.into(), label: "Original real childNodes control".into(),
    }]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    (doc, forms, page)
}

fn evaluate(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{}", page.report());
}

#[test]
fn child_nodes_actual_document_and_element_borrowed_slice_are_real_static_arrays() {
    let (doc, _, page) = fixture("<!doctype html><!--unrepresented--><html><head></head><body><div id=parent>lead<span id=inside>middle</span>tail</div></body></html>", r#"
var slice=Array.prototype.slice,children=document.childNodes,root=document.documentElement;
if(children!==document.childNodes||children.length!==1||children[0]!==root||children.item(0)!==root)throw 'real Document direct children';
var copied=slice.call(children);if(!Array.isArray(copied)||copied.length!==1||copied[0]!==root||copied===children)throw 'exact reduced Document host slice';
var parent=document.getElementById('parent'),list=parent.childNodes,span=document.getElementById('inside');
if(list!==parent.childNodes||list.length!==3||list[0].nodeType!==3||list[0].data!=='lead'||list[1]!==span||list[2].data!=='tail')throw 'all direct Element/Text nodes';
var snapshot=slice.call(list),first=list[0],last=list[2];parent.removeChild(span);
if(list.length!==2||list[0]!==first||list[1]!==last||snapshot.length!==3||snapshot[1]!==span||span.isConnected)throw 'live list versus ordinary static array';
parent.appendChild(span);if(list.length!==3||list[2]!==span||snapshot[1]!==span||parent.childNodes!==list)throw 'reinsert identity';
if(slice.call(root.childNodes)[1]!==document.body)throw 'exact reduced documentElement receiver';
document.title='real-child-nodes-ok';
"#);
    assert_eq!(doc.title, "real-child-nodes-ok");
    assert_eq!(page.state.dom.child_lists.len(), 3);
    assert!(!page.has_host_tasks());
}

#[test]
fn child_nodes_live_detached_text_replace_empty_and_index_surface() {
    let (mut doc, mut forms, mut page) = fixture("<html><head></head><body><div id=parent><b id=named>old</b></div></body></html>", r#"
var p=document.getElementById('parent'),list=p.childNodes,b=document.getElementById('named');
if('namedItem' in list||list.namedItem!==undefined||list.named!==undefined||Object.keys(list).join(',')!=='0')throw 'NodeList has no HTMLCollection name aliases';
if(list.item(99)!==null||list[99]!==undefined||list['01']!==undefined||list[-1]!==undefined)throw 'out of range and canonical indices';
var conversions=0;if(list.item({valueOf:function(){conversions++;return 4294967296;}})!==b||conversions!==1||list.item(1.9)!==null||list.item(-1)!==null)throw 'real unsigned long item conversion';
var errors=0;try{list.item();}catch(e){if(e instanceof TypeError)errors++;}try{list.item(Symbol());}catch(e){if(e instanceof TypeError)errors++;}
try{list.item.call({}, {valueOf:function(){conversions++;return 0;}});}catch(e){if(e instanceof TypeError)errors++;}
if(errors!==3||conversions!==1)throw 'arity symbol and brand before conversion';
list[0]=17;list.length=99;if(list[0]!==b||list.length!==1)throw 'readonly indices and length';
var detached=document.createElement('div'),dl=detached.childNodes,text=document.createTextNode('original'),empty=text.childNodes;
if(dl!==detached.childNodes||dl.length||empty!==text.childNodes||empty.length||empty.item(0)!==null)throw 'empty detached and Text SameObject';
detached.appendChild(text);if(dl.length!==1||dl[0]!==text||text.childNodes!==empty)throw 'detached live append';
detached.textContent='replacement';if(dl.length!==1||dl[0].data!=='replacement'||dl[0]===text||text.data!=='original'||text.parentNode!==null||empty.length)throw 'text replacement preserves detached old identity';
var replaced=dl[0];detached.textContent='';if(dl.length!==0||Object.keys(dl).length||replaced.data!=='replacement'||replaced.parentNode!==null)throw 'empty replacement preserves detached replaced identity';
document.body.appendChild(detached);if(detached.childNodes!==dl||!detached.isConnected)throw 'connected same list';
"#);
    let commits=page.state.dom_commits;
    evaluate(&mut page,&mut doc,&mut forms,"if(p.childNodes!==list||list.length!==1)throw 'later job identity';");
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn child_nodes_slice_reads_live_after_coercion_but_keeps_initial_length() {
    let (_, _, page) = fixture("<html><head></head><body><div id=p><b id=a></b><i id=b></i></div></body></html>", r#"
var p=document.getElementById('p'),a=document.getElementById('a'),b=document.getElementById('b'),list=p.childNodes,n=document.createElement('u'),calls=0;
var array=Array.prototype.slice.call(list,{valueOf:function(){calls++;p.removeChild(a);p.appendChild(n);return 0;}});
if(calls!==1||array.length!==2||array[0]!==b||array[1]!==n||list.length!==2)throw 'length before start conversion and Get after mutation';
var shortened=Array.prototype.slice.call(list,{valueOf:function(){p.removeChild(n);return 0;}});
if(shortened.length!==2||shortened[0]!==b||1 in shortened||shortened[1]!==undefined)throw 'HasProperty preserves absent index hole';
var marker={},caught=false;try{Array.prototype.slice.call(list,{valueOf:function(){throw marker;}});}catch(e){caught=e===marker;}
if(!caught||list.length!==1||list[0]!==b)throw 'conversion abrupt identity';
var log='',context={};list.forEach(function(node,index,owner){if(this!==context||node!==b||index!==0||owner!==list)throw 'forEach actual list';log+='ok';},context);if(log!=='ok')throw 'forEach';
"#);
    assert!(page.state.dom.child_lists.len() >= 1);
}

#[test]
fn child_nodes_slice_null_undefined_is_ordinary_abrupt_before_arguments() {
    let (mut doc, mut forms, mut page) = fixture("<html><head></head><body></body></html>", r#"
var calls=0,argument={valueOf:function(){calls++;return 0;}},caught=0;
for(var receiver of [null,undefined])try{Array.prototype.slice.call(receiver,argument);}catch(e){if(e instanceof TypeError)caught++;else throw e;}
if(caught!==2||calls)throw 'ToObject ordinary errors and ordering';
var result=Array.prototype.slice.call(document.childNodes);if(result[0]!==document.documentElement)throw 'same job recovery';
document.title='slice-null-recovered';
"#);
    assert_eq!(doc.title, "slice-null-recovered");
    let out=page.console_eval(&mut doc,&mut forms,"Array.prototype.push.call(null,1);");
    assert_eq!(out.errors,1,"The unrelated unsupported borrowed-method boundary remains explicit");
    evaluate(&mut page,&mut doc,&mut forms,"if(document.childNodes[0]!==result[0])throw 'new job recovery';");
}

#[test]
fn child_nodes_identity_and_retained_admission_failure_are_atomic_and_recover() {
    let (mut doc, mut forms, mut page) = fixture("<html><head></head><body><div id=p></div></body></html>", "var p=document.getElementById('p');");
    let initial=page.state.node_lists.len();let retained=page.state.retained;
    page.state.limits.host.node_lists=initial;
    let out=page.console_eval(&mut doc,&mut forms,"p.childNodes;");
    assert_eq!(out.errors,1);assert_eq!(page.state.node_lists.len(),initial);
    assert!(page.state.dom.child_lists.is_empty());assert_eq!(page.state.retained,retained);
    page.state.limits.host.node_lists=initial+1;
    page.state.retained_limit=retained;
    let out=page.console_eval(&mut doc,&mut forms,"p.childNodes;");
    assert_eq!(out.errors,1);assert_eq!(page.state.node_lists.len(),initial);
    assert!(page.state.dom.child_lists.is_empty());assert_eq!(page.state.retained,retained);
    page.state.retained_limit=retained+1024;
    evaluate(&mut page,&mut doc,&mut forms,"var kept=p.childNodes;if(kept!==p.childNodes||kept.length)throw 'admission recovery';");
    assert_eq!(page.state.node_lists.len(),initial+1);
    evaluate(&mut page,&mut doc,&mut forms,"if(p.childNodes!==kept)throw 'cached SameObject does not need another slot';");
    assert_eq!(page.state.node_lists.len(),initial+1);
}

#[test]
fn child_nodes_owner_guard_precedes_cached_lookup_and_item_conversion() {
    let (mut doc, mut forms, mut page) = fixture("<html><head></head><body><div id=p></div></body></html>", r#"
var list=document.getElementById('p').childNodes,method=list.item,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,ChildFunction=w.Function,RootFunction=Function;
var read=ChildFunction('list','return list.item;'),readLength=ChildFunction('list','return list.length;'),invoke=ChildFunction('f','list','index','return f.call(list,index);');
var converted=0,index={valueOf:function(){converted++;return 0;}};
if(read(list)!==method||Object.getPrototypeOf(method)!==RootFunction.prototype||invoke(method,list,index)!==null||converted!==1)throw 'same-origin owner method';
converted=0;
"#);
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    for source in ["read(list);","readLength(list);","invoke(method,list,index);"] {
        let out=page.console_eval(&mut doc,&mut forms,source);assert_eq!(out.errors,1,"{source}: {}",page.report());
        evaluate(&mut page,&mut doc,&mut forms,"if(converted||list.length!==0)throw 'foreign before conversion';");
    }
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/original").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"if(read(list)!==method||invoke(method,list,index)!==null||converted!==1)throw 'same-origin recovery';");
}

#[test]
fn child_nodes_normal_policy_grows_beyond_old_identity_count_without_max_reservation() {
    let mut doc=crate::html::parse("<html><head></head><body></body></html>");doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();
    let out=page.start(&mut doc,&mut forms,&[Script {node:0,label:"Original generous childNodes profile".into(),source:"var saved=[];for(var i=0;i<300;i++){var node=document.createElement('div');saved.push(node.childNodes);}if(saved.length!==300||saved[0]===saved[1])throw 'distinct list roots';document.title='generous-list-ok';".into()}]);
    assert_eq!(out.errors,0,"{}",page.report());assert_eq!(doc.title,"generous-list-ok");
    assert_eq!(page.state.node_lists.len(),300);assert_eq!(page.state.dom.child_lists.len(),300);
    assert!(page.state.node_lists.capacity()<page.state.limits.host.node_lists);
    assert_eq!(node_list_index(NODE_LIST_BASE),Some(0));
    assert!(node_list_index(NODE_LIST_BASE-1).is_none());
    assert!(node_list_index(NODE_LIST_BASE+NODE_LIST_HANDLES as u32).is_none());
    assert!(node_index(NODE_LIST_BASE).is_none());
}

#[test]
fn child_nodes_parsed_executed_script_text_inspection_does_not_unlock_loaded_mutations() {
    const SOURCE: &str = "var loadedTextRuns=(typeof loadedTextRuns==='undefined'?0:loadedTextRuns)+1;";
    let markup=format!("<html><head></head><body><div id=target>Ordinary destination</div><script id=record>{SOURCE}</script></body></html>");
    let mut doc=crate::html::parse(&markup);doc.scripting_enabled=true;
    let script=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("record")).unwrap();
    let raw=*doc.nodes[script].children.first().unwrap();
    assert!(matches!(&doc.nodes[raw].kind,NodeKind::Text(text) if text==SOURCE));
    // The parser normally coalesces this raw source to one Text. Prepare one
    // additional real empty Text before script execution so normalize has an
    // actual pending removal; do not mistake normalize's legal no-op for a
    // loaded-script mutation refusal. The parsed source Text remains intact.
    let empty=doc.nodes.len();
    doc.nodes.push(Node {kind:NodeKind::Text(String::new()),children:Vec::new(),namespace:crate::html::Namespace::None});
    doc.nodes[script].children.push(empty);
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    let outcome=page.start(&mut doc,&mut forms,&[Script {node:script,source:SOURCE.into(),label:"Original parsed and executed script Text".into()}]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    evaluate(&mut page,&mut doc,&mut forms,r#"
var record=document.getElementById('record'),rawList=record.childNodes,raw=rawList[0],empty=rawList[1];
if(loadedTextRuns!==1||rawList!==record.childNodes||rawList.length!==2||raw.nodeType!==3||raw.nodeName!=='#text'||raw.parentNode!==record||raw.ownerDocument!==document||!raw.isConnected||raw.data!=="var loadedTextRuns=(typeof loadedTextRuns==='undefined'?0:loadedTextRuns)+1;"||empty.data!=='')throw 'real parsed source identity and execution';
"#);
    let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    let registry=page.state.node_lists.len();
    for source in [
        "raw.data='changed';",
        "raw.nodeValue='changed';",
        "raw.textContent='changed';",
        "record.removeChild(raw);",
        "document.getElementById('target').appendChild(raw);",
        "record.normalize();",
        "document.body.normalize();",
    ] {
        let out=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(out.errors,1,"{source}: {}",page.report());
        assert_eq!(page.state.dom_commits,commits,"{source}");
        assert_eq!(format!("{:?}",doc.nodes),nodes,"{source}");
        assert_eq!(page.state.node_lists.len(),registry,"{source}");
        assert!(page.take_dynamic_scripts().is_empty(),"{source}");
        assert!(!page.has_host_tasks(),"{source}: {}",page.report());
        evaluate(&mut page,&mut doc,&mut forms,"if(loadedTextRuns!==1||record.childNodes!==rawList||rawList.length!==2||rawList[0]!==raw||rawList[1]!==empty||raw.parentNode!==record||empty.parentNode!==record)throw 'loaded script recovery kept actual source nodes';");
    }
    assert_eq!(doc.nodes[script].children,[raw,empty]);
    assert!(matches!(&doc.nodes[raw].kind,NodeKind::Text(text) if text==SOURCE));
}

#[test]
fn child_nodes_template_ast_contents_are_not_exposed_as_a_fake_dom_fragment() {
    let (mut doc,mut forms,mut page)=fixture("<html><head></head><body><div id=owner><template id=inert><span id=not-live>Inert AST input</span></template><b id=sibling>Real sibling</b></div></body></html>",r#"
var owner=document.getElementById('owner'),parentList=owner.childNodes,inertTemplate=parentList[0],sibling=document.getElementById('sibling'),detachedTemplate=document.createElement('template');
if(parentList!==owner.childNodes||parentList.length!==2||parentList[1]!==sibling||document.getElementById('not-live')!==null)throw 'parent owns actual template identity without query activation';
"#);
    let owner=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("owner")).unwrap();
    let template=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("inert")).unwrap();
    let handle=page.state.dom.child_lists[&owner];
    let value=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(handle,"0").unwrap();
    assert!(matches!(value,Value::Host(actual) if actual==node_handle(template)));
    assert!(!doc.nodes[template].children.is_empty(),"The fixture genuinely retains an inert parser AST subtree");
    let nodes=format!("{:?}",doc.nodes);let commits=page.state.dom_commits;
    let lists=page.state.node_lists.len();let identities=page.state.dom.child_lists.len();let retained=page.state.retained;
    for source in ["inertTemplate.childNodes;","detachedTemplate.childNodes;"] {
        let out=page.console_eval(&mut doc,&mut forms,source);assert_eq!(out.errors,1,"{source}: {}",page.report());
        assert_eq!(page.state.dom_commits,commits);assert_eq!(format!("{:?}",doc.nodes),nodes);
        assert_eq!(page.state.node_lists.len(),lists);assert_eq!(page.state.dom.child_lists.len(),identities);
        assert_eq!(page.state.retained,retained);assert!(page.take_dynamic_scripts().is_empty());assert!(!page.has_host_tasks());
        evaluate(&mut page,&mut doc,&mut forms,"if(parentList!==owner.childNodes||parentList.length!==2||parentList[0]!==inertTemplate||parentList[1]!==sibling||document.getElementById('not-live')!==null)throw 'template refusal changed parent identities or inert policy';");
    }
    assert!(!page.state.dom.child_lists.contains_key(&template));
}

#[test]
fn child_nodes_cached_list_refreshes_new_text_owner_before_foreign_first_read() {
    let (mut doc,mut forms,mut page)=fixture("<html><head></head><body><p id=p>old</p></body></html>",r#"
var p=document.getElementById('p'),list=p.childNodes,old=list[0],frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,read=w.Function('node','marker','marker.entered++;return node.data;'),entered={entered:0};
if(read(old,entered)!=='old'||entered.entered!==1)throw 'same-origin actual reader control';
"#);
    // No parent getter or Text read follows replacement. This previously let
    // a cached list yield a new connected Text before owner metadata existed.
    evaluate(&mut page,&mut doc,&mut forms,"p.textContent='replacement';var fresh=list[0];");
    let commits=page.state.dom_commits;
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    let out=page.console_eval(&mut doc,&mut forms,"read(fresh,entered);");
    assert_eq!(out.errors,1,"Foreign first read must not obtain new Text before its owner is known: {}",page.report());
    assert_eq!(page.state.dom_commits,commits);
    evaluate(&mut page,&mut doc,&mut forms,"if(entered.entered!==2||fresh.data!=='replacement'||old.data!=='old'||old.parentNode!==null||fresh===old)throw 'reader entered but guarded before data leak';");
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/original").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"if(read(fresh,entered)!=='replacement'||entered.entered!==3||list[0]!==fresh||fresh.parentNode!==p)throw 'same-origin recovery keeps actual replacement identity';");
}
