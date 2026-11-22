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
fn node_prototype_mutations_keep_native_identity_and_checks() {
    let (_, _, page) = fixture("<main id=host><i id=first></i></main>", r#"
var host = document.getElementById('host'), first = document.getElementById('first');
var fragment = document.createDocumentFragment(), proto = Node.prototype, operations = {};
for (var entry of [['appendChild',1],['insertBefore',2],['removeChild',1],['replaceChild',2]]) {
    var key = entry[0], d = Object.getOwnPropertyDescriptor(proto, key);
    if (!d || typeof d.value !== 'function' || !d.enumerable || !d.writable || !d.configurable ||
        d.value.length !== entry[1] || d.value.name !== key || host[key] !== d.value ||
        fragment[key] !== d.value || Object.hasOwn(host, key)) throw 'wrong native method ' + key;
    operations[key] = d.value;
}
var a = document.createElement('b'), b = document.createTextNode('visible'), list = host.childNodes;
var calls = 0, original = operations.appendChild;
proto.appendChild = function(child) { calls++; return original.call(this, child); };
if (fragment.appendChild(a) !== a || calls !== 1 || original.call(host, fragment) !== fragment ||
    fragment.childNodes.length || list[1] !== a || calls !== 1) throw 'prototype wrapper and fragment move';
proto.appendChild = original;
if (operations.insertBefore.call(host, b, first) !== b || list[0] !== b ||
    operations.replaceChild.call(host, first, a) !== a || list[1] !== first ||
    operations.removeChild.call(host, first) !== first || first.parentNode !== null ||
    list.length !== 1 || list[0] !== b) throw 'borrowed operations lost real tree state';
var rejected = 0;
try { original.call({}, a); } catch (error) { if (error instanceof TypeError) rejected++; }
try { original.call(host, {}); } catch (error) { if (error instanceof TypeError) rejected++; }
try { operations.removeChild.call(host, a); } catch (error) { if (error.name === 'NotFoundError') rejected++; }
if (rejected !== 3 || a.parentNode !== null || host.childNodes !== list ||
    list.length !== 1 || list[0] !== b) throw 'guarded mutation or retained list';
"#);
    assert!(!page.has_host_tasks());
}

#[test]
fn node_relationship_descriptors_preserve_checked_tree_links() {
    let (_, _, page) = fixture("<div id=host>lead<b id=child></b>tail</div>", r#"
var host = document.getElementById('host'), child = document.getElementById('child');
var getters = {};
for (var name of ['firstChild','lastChild','previousSibling','nextSibling','parentNode','parentElement']) {
    var d = Object.getOwnPropertyDescriptor(Node.prototype, name);
    if (!d || typeof d.get !== 'function' || !d.enumerable || !d.configurable ||
        d.set !== undefined || Object.hasOwn(host, name) || !(name in host)) throw 'descriptor ' + name;
    getters[name] = d.get;
}
var lead = getters.firstChild.call(host), tail = getters.lastChild.call(host);
if (lead.data !== 'lead' || tail.data !== 'tail' || getters.nextSibling.call(lead) !== child ||
    getters.previousSibling.call(tail) !== child || getters.parentNode.call(child) !== host ||
    getters.parentElement.call(child) !== host || getters.parentNode.call(document) !== null ||
    getters.firstChild.call(document) !== document.documentElement) throw 'genuine tree links';
var template = document.createElement('template');
template.innerHTML = '<i></i><b></b>';
var fragment = template.content, inertFirst = getters.firstChild.call(fragment);
if (inertFirst !== fragment.childNodes[0] || getters.nextSibling.call(inertFirst) !== fragment.childNodes[1] ||
    getters.parentNode.call(inertFirst) !== fragment || getters.parentElement.call(inertFirst) !== null)
    throw 'inert tree links';
Object.defineProperty(host, 'firstChild', {get: function() { return child; }, configurable: true});
if (host.firstChild !== child || getters.firstChild.call(host) !== lead) throw 'own shadow';
host.removeChild(child);
if (getters.nextSibling.call(lead) !== tail || getters.previousSibling.call(tail) !== lead ||
    getters.parentNode.call(child) !== null || !delete host.firstChild || host.firstChild !== lead)
    throw 'live tree links after removal';
var rejected = false;
try { getters.firstChild.call({}); } catch (error) { rejected = error instanceof TypeError; }
if (!rejected) throw 'getter brand';
"#);
    assert!(!page.has_host_tasks());
}

#[test]
fn child_nodes_native_descriptor_and_element_shadow_keep_live_list() {
    let (_, _, page) = fixture("<div id=host><b id=child></b></div>", r#"
var node = document.getElementById('host'), child = document.getElementById('child');
var live = node.childNodes, descriptor = Object.getOwnPropertyDescriptor(Node.prototype, 'childNodes');
if (!descriptor || typeof descriptor.get !== 'function' || descriptor.set !== undefined ||
    !descriptor.enumerable || !descriptor.configurable || descriptor.get.name !== 'get childNodes' ||
    Object.hasOwn(node, 'childNodes') || !('childNodes' in node))
    throw 'wrong descriptor or inheritance';
var read = descriptor.get, fragment = document.createDocumentFragment();
var template = document.createElement('template'), content = template.content;
if (read.call(node) !== live || read.call(document) !== document.childNodes ||
    read.call(fragment) !== fragment.childNodes || read.call(content) !== content.childNodes ||
    read.call(content.ownerDocument) !== content.ownerDocument.childNodes)
    throw 'native getter did not return the existing lists';
var rejected = false;
try { read.call({}); } catch (error) { rejected = error instanceof TypeError; }
if (!rejected) throw 'wrong receiver accepted';
node.childNodes = 'ignored';
if (node.childNodes !== live || Object.hasOwn(node, 'childNodes')) throw 'readonly assignment';
var calls = 0, shadow = function() {
    if (this !== node) throw 'wrong author getter receiver';
    calls++;
    return Array.prototype.slice.call(read.call(this));
};
Object.defineProperty(node, 'childNodes', {get:shadow, configurable:true});
if (Object.getOwnPropertyDescriptor(node, 'childNodes').get !== shadow || calls !== 0 ||
    !Array.isArray(node.childNodes) || calls !== 1) throw 'own descriptor';
node.removeChild(child);
if (read.call(node) !== live || live.length !== 0 || node.childNodes.length !== 0)
    throw 'native list lost identity or liveness';
if (!delete node.childNodes || node.childNodes !== live || Object.hasOwn(node, 'childNodes'))
    throw 'deletion did not reveal the original getter';
"#);
    assert!(!page.has_host_tasks());
}

#[test]
fn child_nodes_actual_document_and_element_borrowed_slice_are_real_static_arrays() {
    let (doc, _, page) = fixture("<!doctype html><!--represented--><html><head></head><body><div id=parent>lead<span id=inside>middle</span>tail</div></body></html>", r#"
var slice=Array.prototype.slice,children=document.childNodes,root=document.documentElement;
if(children!==document.childNodes||children.length!==2||children[0].nodeType!==8||children[0].data!=='represented'||children[1]!==root||children.item(1)!==root)throw 'real Document direct children';
var copied=slice.call(children);if(!Array.isArray(copied)||copied.length!==2||copied[0]!==children[0]||copied[1]!==root||copied===children)throw 'exact reduced Document host slice';
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
    doc.nodes.push(Node {kind:NodeKind::Text(String::new().into()),children:Vec::new(),namespace:crate::html::Namespace::None});
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
fn child_nodes_template_host_and_inert_fragment_keep_separate_identities() {
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
    evaluate(&mut page, &mut doc, &mut forms, r#"
var hostList = inertTemplate.childNodes, detachedList = detachedTemplate.childNodes, fragmentList = inertTemplate.content.childNodes;
if (parentList !== owner.childNodes || parentList.length !== 2 || parentList[0] !== inertTemplate ||
    parentList[1] !== sibling || hostList !== inertTemplate.childNodes || hostList.length !== 0 ||
    detachedList !== detachedTemplate.childNodes || detachedList.length !== 0 ||
    fragmentList !== inertTemplate.content.childNodes || fragmentList.length !== 1 ||
    fragmentList[0] !== inertTemplate.content.firstChild || fragmentList[0].isConnected ||
    document.getElementById('not-live') !== null) throw 'separate template topology and inertness';
"#);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert!(page.state.dom.child_lists.contains_key(&template));
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(!page.has_host_tasks());
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

#[test]
fn child_nodes_real_host_apply_copies_actual_nodes_and_preserves_receiver() {
    let (_,_,page)=fixture("<html><head></head><body><div id=p>lead<b id=b>middle</b>tail</div></body></html>",r#"
var p=document.getElementById('p'),list=p.childNodes,b=document.getElementById('b'),receiver={},calls=0;
function inspect(a,middle,z){calls++;if(this!==receiver||arguments.length!==3||a!==list[0]||a.data!=='lead'||middle!==b||z!==list[2]||z.data!=='tail')throw 'real host apply arguments/this';return 17;}
if(inspect.apply(receiver,list)!==17||calls!==1)throw 'real function target';
var copied=[];if(Array.prototype.push.apply(copied,document.childNodes)!==1||copied[0]!==document.documentElement)throw 'exact native push/apply Document shape';
Array.prototype.push.apply(copied,list);if(copied.length!==4||copied[2]!==b||copied[1]!==list[0])throw 'native push/apply Element shape';
p.removeChild(b);if(copied[2]!==b||list.length!==2||copied.length!==4)throw 'ordinary copied list is static';
var errors=0;for(var wrong of [1,true,'abc'])try{inspect.apply(receiver,wrong);}catch(e){if(e instanceof TypeError)errors++;else throw e;}
if(errors!==3||calls!==1)throw 'primitive argArray is not array-like Object';
"#);
    assert!(!page.has_host_tasks());
}

#[test]
fn child_nodes_real_host_apply_foreign_owner_stops_before_target() {
    let (mut doc,mut forms,mut page)=fixture("<html><head></head><body><div id=p>kept</div></body></html>",r#"
var list=document.getElementById('p').childNodes,frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,invoke=w.Function('target','nodes','marker','marker.entered++;return target.apply(null,nodes);'),marker={entered:0},called=0;
function target(text){called++;return text;}
if(invoke(target,list,marker)!==list[0]||called!==1||marker.entered!==1)throw 'same-origin host apply control';
"#);
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    let out=page.console_eval(&mut doc,&mut forms,"invoke(target,list,marker);");assert_eq!(out.errors,1,"{}",page.report());
    evaluate(&mut page,&mut doc,&mut forms,"if(called!==1||marker.entered!==2||list.length!==1)throw 'foreign apply entered but target untouched';");
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/original").unwrap().0),path:None,root:None,fragment:String::new()});
    evaluate(&mut page,&mut doc,&mut forms,"if(invoke(target,list,marker)!==list[0]||called!==2||marker.entered!==3)throw 'same-origin apply recovery';");
}
