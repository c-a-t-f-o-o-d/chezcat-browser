//! Original genuine Comment-node tests. No service requests or package execution.
use super::*;

fn fixture(markup:&str,source:&str)->(Document,Forms,Page) {
    let mut doc=crate::html::parse(markup);doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::for_test();
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/comments").unwrap().0),path:None,root:None,fragment:String::new()});
    let out=page.start(&mut doc,&mut forms,&[Script {node:0,source:source.into(),label:"Original genuine Comment factory".into()}]);
    assert_eq!(out.errors,0,"{}",page.report());(doc,forms,page)
}
fn evaluate(page:&mut Page,doc:&mut Document,forms:&mut Forms,source:&str) {
    let out=page.console_eval(doc,forms,source);assert_eq!(out.errors,0,"{}",page.report());
}

#[test]
fn comment_factory_has_real_kind_data_node_slots_and_single_webidl_conversion() {
    let (doc,_,page)=fixture("<html><head></head><body><div id=p></div></body></html>",r#"
var maker=document.createComment,calls=0,extra={toString:function(){throw 'extra argument';}},input={toString:function(){calls++;return 'snow 雪🙂';}};
var c=maker.call(document,input,extra),other=document.createComment('snow 雪🙂');
if(typeof maker!=='function'||maker.length!==1||calls!==1||c===other||c.nodeType!==8||c.nodeName!=='#comment'||c.data!=='snow 雪🙂'||c.length!==8)throw 'genuine factory identity and UTF16 data';
var T=Object.getOwnPropertyDescriptor(Node.prototype,'nodeType').get,N=Object.getOwnPropertyDescriptor(Node.prototype,'nodeName').get;
if(!(c instanceof Node)||c instanceof Element||T.call(c)!==8||N.call(c)!=='#comment'||c.ownerDocument!==document||c.parentNode!==null||c.parentElement!==null||c.isConnected||c.firstChild!==null||c.lastChild!==null||c.childNodes.length)throw 'real Node slots and leaf';
if(c.textContent!==c.data||c.nodeValue!==c.data||c.tagName!==undefined||c.getAttribute!==undefined)throw 'CharacterData not Element';
var errors=0;try{maker.call(document);}catch(e){if(e instanceof TypeError)errors++;else throw e;}try{maker.call(document,Symbol());}catch(e){if(e instanceof TypeError)errors++;else throw e;}try{maker.call({},input);}catch(e){if(e instanceof TypeError)errors++;else throw e;}try{maker(input);}catch(e){if(e instanceof TypeError)errors++;else throw e;}
if(errors!==4||calls!==1)throw 'arity Symbol and separate forged/unbound receiver guards before conversion';
var marker={},caught=false,abruptConversions=0;try{maker.call(document,{toString:function(){abruptConversions++;throw marker;}});}catch(e){caught=e===marker;}if(!caught||abruptConversions!==1)throw 'author abrupt identity and single conversion';
document.title='real-comment-kind-ok';
"#);
    assert_eq!(doc.title,"real-comment-kind-ok");
    assert_eq!(doc.nodes.iter().filter(|node|matches!(node.kind,NodeKind::Comment(_))).count(),2);
    assert!(!page.has_host_tasks());
}

#[test]
fn comment_constructor_and_factory_use_the_same_native_interface() {
    let (doc, _, page) = fixture("<html><head></head><body><div id=p></div></body></html>", r#"
var C = window.Comment, P = C.prototype, calls = 0;
var value = {toString:function(){calls++;return 'snow 雪🙂';}};
if (C.name !== 'Comment' || C.length !== 0 || Object.getPrototypeOf(C) !== CharacterData ||
    Object.getPrototypeOf(P) !== CharacterData.prototype || P.constructor !== C) throw 'Comment interface ancestry';
var c = new C(value, {toString:function(){throw 'unexpected extra conversion';}});
var made = document.createComment('factory'), cloned = c.cloneNode();
for (var item of [c, made, cloned]) {
    if (Object.getPrototypeOf(item) !== P || !(item instanceof C) || !(item instanceof CharacterData) ||
        !(item instanceof Node) || item instanceof Text || item.ownerDocument !== document ||
        item.nodeType !== 8 || item.nodeName !== '#comment' || item.parentNode !== null ||
        Object.prototype.toString.call(item) !== '[object Comment]' || 'assignedSlot' in item) throw 'genuine Comment nodes';
}
if (c === cloned || c.data !== 'snow 雪🙂' || c.length !== 8 || cloned.data !== c.data ||
    new C().data !== '' || new C(undefined).data !== '' || new C(null).data !== 'null') throw 'Comment conversion';
var errors = 0, failure = {}, caught;
try { C(value); } catch (e) { if (e instanceof TypeError) errors++; else throw e; }
try { new C(Symbol()); } catch (e) { if (e instanceof TypeError) errors++; else throw e; }
try { new C({toString:function(){throw failure;}}); } catch (e) { caught = e; }
if (errors !== 2 || calls !== 1 || caught !== failure) throw 'conversion before allocation';
var parent = document.getElementById('p');
parent.appendChild(c);
c.nodeValue = 'edited';
if (parent.firstChild !== c || c.textContent !== 'edited' || parent.textContent !== '') throw 'native Comment mutation';
document.title = 'verified-comment-interface';
"#);
    assert_eq!(doc.title, "verified-comment-interface");
    assert_eq!(doc.nodes.iter().filter(|node| matches!(node.kind, NodeKind::Comment(_))).count(), 6);
    assert!(!page.has_host_tasks());
}

#[test]
fn cdata_section_has_the_text_interface_ancestry_and_html_creation_rejects() {
    let (doc, _, page) = fixture("<html><head></head><body></body></html>", r#"
var C = window.CDATASection, P = C.prototype;
if (C.name !== 'CDATASection' || C.length !== 0 || Object.getPrototypeOf(C) !== Text ||
    Object.getPrototypeOf(P) !== Text.prototype || P.constructor !== C ||
    P[Symbol.toStringTag] !== 'CDATASection' || new Text() instanceof C) throw 'CDATASection interface identity';
var invalid = 0;
try { new C(); } catch (e) { if (e instanceof TypeError) invalid++; else throw e; }
try { C(); } catch (e) { if (e instanceof TypeError) invalid++; else throw e; }
var make = Document.prototype.createCDATASection, other = document.implementation.createHTMLDocument('inert');
if (make.name !== 'createCDATASection' || make.length !== 1 || document.createCDATASection !== make ||
    other.createCDATASection !== make || Object.hasOwn(document,'createCDATASection') ||
    Object.hasOwn(other,'createCDATASection')) throw 'Document factory identity';
var calls = 0, value = {toString:function(){calls++;return 'contains ]]>'; }}, rejected = 0;
for (var owner of [document, other]) {
    try { make.call(owner, value); } catch (e) {
        if (e instanceof DOMException && e.name === 'NotSupportedError' && e.code === 9) rejected++;
        else throw e;
    }
}
try { make.call({}, value); } catch (e) { if (e instanceof TypeError) invalid++; else throw e; }
try { make.call(document); } catch (e) { if (e instanceof TypeError) invalid++; else throw e; }
try { make.call(document,Symbol()); } catch (e) { if (e instanceof TypeError) invalid++; else throw e; }
var marker = {}, caught;
try { make.call(other, {toString:function(){throw marker;}}); } catch (e) { caught = e; }
if (invalid !== 5 || rejected !== 2 || calls !== 2 || caught !== marker) throw 'factory brand, conversion and exceptions';
document.title = 'verified-cdata-html-interface';
"#);
    assert_eq!(doc.title, "verified-cdata-html-interface");
    assert!(!page.has_host_tasks());
}

#[test]
fn processing_instruction_keeps_distinct_character_data_document_and_layout() {
    let (mut doc, mut forms, mut page) = fixture("<html><head></head><body><div id=p>A<span id=s>B</span></div></body></html>", r#"
var C = window.ProcessingInstruction, P = C.prototype, parent = document.getElementById('p');
var make = Document.prototype.createProcessingInstruction, order = '';
if (C.length !== 1 || C.name !== 'ProcessingInstruction' || Object.getPrototypeOf(C) !== CharacterData ||
    Object.getPrototypeOf(P) !== CharacterData.prototype || P.constructor !== C ||
    make.length !== 2 || make !== document.createProcessingInstruction) throw 'ProcessingInstruction interfaces';
var getTarget = Object.getOwnPropertyDescriptor(P,'target').get;
var node = new C({toString:function(){order+='target';return '雪:mode';}}, {
    toString:function(){order+='data';return 'mode="local"';}
});
if (order !== 'targetdata' || getTarget.call(node) !== '雪:mode' || node.target !== '雪:mode' ||
    Object.hasOwn(node,'target') || node.nodeName !== node.target || node.nodeType !== Node.PROCESSING_INSTRUCTION_NODE ||
    node.data !== 'mode="local"' || node.length !== 12 || node.parentNode !== null || node.ownerDocument !== document ||
    Object.getPrototypeOf(node) !== P || !(node instanceof C) || !(node instanceof CharacterData) ||
    node instanceof Text || node instanceof Comment || new C('mode').data !== '' ||
    new C('mode',undefined).data !== '' || new C('mode',null).data !== 'null') throw 'genuine native node';
var marker = document.implementation.createHTMLDocument('independent');
var remote = make.call(marker, 'mode', undefined);
if (marker.createProcessingInstruction !== make || remote.ownerDocument !== marker || remote.data !== 'undefined' ||
    remote.textContent !== remote.data || remote.nodeValue !== remote.data || remote.target !== 'mode' ||
    remote.length !== 9 || remote.parentNode !== null || Object.getPrototypeOf(remote) !== P) throw 'separate inert owner';
var exceptions = 0;
for (var input of ['', '1mode', 'white space']) try { new C(input); } catch (e) {
    if (e instanceof DOMException && e.name === 'InvalidCharacterError') exceptions++; else throw e;
}
try { make.call(document, 'mode', 'bad?>'); } catch (e) {
    if (e instanceof DOMException && e.name === 'InvalidCharacterError') exceptions++; else throw e;
}
try { make.call({}, {toString:function(){throw 'wrong receiver converted';}},'data'); } catch (e) {
    if (e instanceof TypeError) exceptions++; else throw e;
}
try { getTarget.call(document.createComment('fake')); } catch (e) { if (e instanceof TypeError) exceptions++; else throw e; }
try { C('mode'); } catch (e) { if (e instanceof TypeError) exceptions++; else throw e; }
try { make.call(document,'mode'); } catch (e) { if (e instanceof TypeError) exceptions++; else throw e; }
if (exceptions !== 8) throw 'required arguments, XML validation and receiver brand';
var copy = node.cloneNode(), span = document.getElementById('s');
parent.insertBefore(node,span);
if (parent.firstChild.nextSibling !== node || node.nextElementSibling !== span ||
    parent.textContent !== 'AB' || parent.innerHTML !== 'A<?雪:mode mode="local"?><span id="s">B</span>') throw 'native tree and serialization';
var walker = document.createTreeWalker(parent,NodeFilter.SHOW_PROCESSING_INSTRUCTION);
if (walker.nextNode() !== node || walker.nextNode() !== null || copy === node ||
    copy.target !== node.target || copy.data !== node.data || copy.ownerDocument !== document) throw 'filter and cloning';
var records = [], observer = new MutationObserver(function(items){records=items;});
observer.observe(node,{characterData:true,characterDataOldValue:true});
node.nodeValue = 'edited';
var change = observer.takeRecords();
if (copy.data === node.data || node.textContent !== 'edited' || parent.textContent !== 'AB' ||
    change.length !== 1 || change[0].type !== 'characterData' || change[0].target !== node ||
    change[0].oldValue !== 'mode="local"') throw 'character data write and observation';
var shield = document.createElement('script');
document.title = 'verified-processing-instruction';
"#);
    assert_eq!(doc.title, "verified-processing-instruction");
    assert_eq!(doc.nodes.iter().filter(|node| matches!(node.kind, NodeKind::ProcessingInstruction { .. })).count(), 5);
    let commits = page.state.dom_commits;
    let outcome = page.console_eval(&mut doc, &mut forms, "shield.appendChild(copy);");
    assert_eq!(outcome.errors, 1);
    assert!(page.report().contains("Created scripts accept text children only"));
    assert_eq!(page.state.dom_commits, commits);
    evaluate(&mut page, &mut doc, &mut forms,
        "if (shield.firstChild !== null || copy.parentNode !== null) throw 'native script source protection';");
    assert!(!page.has_host_tasks());
}

#[test]
fn comment_data_nullable_setters_and_readonly_length_preserve_comment_kind() {
    let (doc,_,_)=fixture("<html><head></head><body></body></html>",r#"
var c=document.createComment(''),calls=0;
c.data={toString:function(){calls++;return '雪🙂';}};if(calls!==1||c.data!=='雪🙂'||c.length!==3||c.nodeType!==8)throw 'data conversion and units';
c.data=null;if(c.data!=='null')throw 'ordinary DOMString null';c.data=undefined;if(c.data!=='undefined')throw 'ordinary DOMString undefined';
c.nodeValue=null;if(c.data!=='')throw 'nullable nodeValue null';c.nodeValue=undefined;if(c.data!=='')throw 'nullable nodeValue undefined';
c.textContent=null;if(c.data!=='')throw 'nullable textContent null';c.textContent=undefined;if(c.data!=='')throw 'nullable textContent undefined';
c.nodeValue={toString:function(){calls++;return 'payload';}};if(calls!==2||c.textContent!=='payload')throw 'nodeValue object conversion';
c.data='A\u0000\r\n&<>-->';if(c.data!=='A\u0000\r\n&<>-->'||c.nodeType!==8||c.nodeName!=='#comment')throw 'data is not parsed or normalized';
var length=c.length;c.length=99;c.nodeType=3;c.nodeName='#text';if(c.length!==length||c.nodeType!==8||c.nodeName!=='#comment')throw 'readonly facts';
var errors=0;try{(function(){'use strict';c.length=0;})();}catch(e){if(e instanceof TypeError)errors++;else throw e;}
for(var key of ['data','nodeValue','textContent'])try{c[key]=Symbol();}catch(e){if(e instanceof TypeError)errors++;else throw e;}
if(errors!==4||c.data!=='A\u0000\r\n&<>-->')throw 'readonly and Symbol errors';
document.title='comment-data-ok';
"#);
    assert_eq!(doc.title,"comment-data-ok");
    assert!(doc.nodes.iter().any(|node|matches!(&node.kind,NodeKind::Comment(data) if data.contains('\0'))));
}

#[test]
fn comment_topology_queries_normalize_and_serialization_use_distinct_real_data() {
    let (mut doc,mut forms,mut page)=fixture("<html><head></head><body><div id=p></div></body></html>",r#"
var p=document.getElementById('p'),a=document.createTextNode('A'),c=document.createComment('hidden &<>'),b=document.createTextNode('B');
p.appendChild(a);p.appendChild(c);p.appendChild(b);var list=p.childNodes,snapshot=Array.prototype.slice.call(list);
if(list.length!==3||list[1]!==c||c.previousSibling!==a||c.nextSibling!==b||c.parentNode!==p||c.parentElement!==p||!c.isConnected||!(c.compareDocumentPosition(b)&4))throw 'actual topology';
if(p.textContent!=='AB'||p.getElementsByTagName('*').length||p.querySelectorAll('*').length||p.innerHTML!=='A<!--hidden &<>-->B')throw 'queries text aggregation and serialization';
c.data='';p.normalize();if(list.length!==3||list[0]!==a||list[1]!==c||list[2]!==b||p.textContent!=='AB')throw 'empty Comment is a Text normalization barrier';
c.normalize();if(c.data!==''||c.nodeType!==8)throw 'Comment normalize no-op';
p.removeChild(c);if(list.length!==2||c.isConnected||c.parentNode!==null||snapshot[1]!==c||snapshot.length!==3)throw 'detached identity and static Array';
p.appendChild(c);if(list[2]!==c||!c.isConnected||p.childNodes!==list)throw 'reinsert retained identity';
"#);
    let count=doc.nodes.len();let commits=page.state.dom_commits;
    let out=page.console_eval(&mut doc,&mut forms,"document.getElementById('p').innerHTML='x<!--created comments are not parser support-->y';");
    assert_eq!(out.errors,1,"Checked fragment comments remain explicitly unsupported");
    assert_eq!(doc.nodes.len(),count);assert_eq!(page.state.dom_commits,commits);
    evaluate(&mut page,&mut doc,&mut forms,"if(p.childNodes!==list||list[2]!==c||p.textContent!=='AB')throw 'fragment refusal atomicity';");
    let out=page.console_eval(&mut doc,&mut forms,"c.appendChild(document.createTextNode('bad'));");
    assert_eq!(out.errors,1);
    evaluate(&mut page,&mut doc,&mut forms,"if(c.childNodes.length||c.nodeType!==8)throw 'Comment remains leaf';");
}

#[test]
fn comment_resource_strings_do_not_activate_or_modify_loaded_script_boundaries() {
    let (mut doc,mut forms,mut page)=fixture("<html><head><script id=loaded>var authoredLoaded=1;</script></head><body><div id=p></div></body></html>",r#"
var p=document.getElementById('p'),loaded=document.getElementById('loaded'),c=document.createComment('<img src=https://unrequested.test/image><script src=https://unrequested.test/script></script>');
p.appendChild(c);if(p.textContent!==''||c.nodeType!==8||p.querySelectorAll('*').length)throw 'resource-like data remains Comment';
"#);
    assert!(!page.has_host_tasks());let commits=page.state.dom_commits;
    for source in ["loaded.appendChild(c);","var s=document.createElement('script');s.appendChild(c);","loaded.childNodes[0].data='changed';"] {
        let out=page.console_eval(&mut doc,&mut forms,source);assert_eq!(out.errors,1,"{source}: {}",page.report());
        assert_eq!(page.state.dom_commits,commits);assert!(!page.has_host_tasks());
        evaluate(&mut page,&mut doc,&mut forms,"if(c.parentNode!==p||c.nodeType!==8||loaded.textContent!=='var authoredLoaded=1;')throw 'loaded source boundary';");
    }
}

#[test]
fn comment_factory_owner_guard_precedes_conversion_and_admission_failure_is_atomic() {
    let (mut doc,mut forms,mut page)=fixture("<html><head></head><body></body></html>",r#"
var maker=document.createComment,converted=0,value={toString:function(){converted++;return 'created';}},frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var invoke=frame.contentWindow.Function('maker','doc','value','return maker.call(doc,value);');
if(invoke(maker,document,value).nodeType!==8||converted!==1)throw 'same-origin real factory';converted=0;
"#);
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://other.test/").unwrap().0),path:None,root:None,fragment:String::new()});
    let nodes=doc.nodes.len();let retained=page.state.retained;
    let out=page.console_eval(&mut doc,&mut forms,"invoke(maker,document,value);");assert_eq!(out.errors,1);assert_eq!(doc.nodes.len(),nodes);assert_eq!(page.state.retained,retained);
    evaluate(&mut page,&mut doc,&mut forms,"if(converted)throw 'foreign before factory conversion';");
    page.set_location(&crate::navigation::Location {url:Some(crate::url::parse("https://example.test/comments").unwrap().0),path:None,root:None,fragment:String::new()});
    let limit=page.state.node_limit;page.state.node_limit=nodes;
    let out=page.console_eval(&mut doc,&mut forms,"maker.call(document,'rejected');");assert_eq!(out.errors,1);assert!(page.report().contains("DOM appended-node limit reached"),"{}",page.report());assert_eq!(doc.nodes.len(),nodes);assert_eq!(page.state.retained,retained);
    page.state.node_limit=limit;page.state.retained_limit=retained;
    let out=page.console_eval(&mut doc,&mut forms,"maker.call(document,'rejected');");assert_eq!(out.errors,1);assert!(page.report().contains("DOM retained node/payload backing allowance exhausted"),"{}",page.report());assert_eq!(doc.nodes.len(),nodes);assert_eq!(page.state.retained,retained);
    page.state.retained_limit=retained+1024;
    evaluate(&mut page,&mut doc,&mut forms,"if(invoke(maker,document,value).data!=='created'||converted!==1)throw 'fresh-job factory recovery';");
    assert_eq!(doc.nodes.len(),nodes+1);
}

#[test]
fn comment_has_no_layout_flex_item_text_or_link_footprint() {
    let markup="<style>body{margin:0}.row{display:flex;gap:10px;width:120px}.box{width:40px;flex:none}</style><body><div id=row class=row><div id=a class=box>A</div><div id=b class=box>B</div></div></body>";
    let control=crate::html::parse(markup);
    let before=crate::layout::build_viewport(&control,240,200,1.0,&HashMap::new(),&HashMap::new());
    let (doc,_,page)=fixture(markup,"var c=document.createComment('never rendered');document.getElementById('row').insertBefore(c,document.getElementById('b'));");
    let after=crate::layout::build_viewport(&doc,240,200,1.0,&HashMap::new(),&HashMap::new());
    for text in ["A","B"] {
        let a=before.runs.iter().find(|run|run.text==text).unwrap();
        let b=after.runs.iter().find(|run|run.text==text).unwrap();
        assert_eq!((a.x,a.y,a.width(),a.height()),(b.x,b.y,b.width(),b.height()),"Comment must not become a flex item or spacing source: {text}");
    }
    assert!(!after.runs.iter().any(|run|run.text.contains("never rendered")));
    assert_eq!(before.link_targets().count(),after.link_targets().count());assert!(!page.has_host_tasks());
}
