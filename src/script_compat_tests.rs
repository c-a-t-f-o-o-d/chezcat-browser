use super::*;

#[test]
fn reflect_get_real_window_and_native_element_receiver() {
    let (doc, _, page, result) = page("<html><body><div id='target'></div></body></html>", &[r#"
        var target = document.getElementById('target'), saved = Reflect.get;
        var owner = { get name() { return this.localName; } };
        if (saved(owner, 'name', target) !== 'div' || saved(target, 'ownerDocument') !== document)
            throw 'native receiver and direct native read';
        var namespace = window.Reflect;
        if (!('Reflect' in window) || namespace !== Reflect || !delete window.Reflect)
            throw 'window namespace';
        if ('Reflect' in window || typeof Reflect !== 'undefined' ||
            Object.getOwnPropertyDescriptor(window, 'Reflect') !== undefined)
            throw 'deleted window namespace';
        if (saved(owner, 'name', target) !== 'div') throw 'saved operation';
        window.Reflect = namespace;
        document.title = 'Window reflection passed';
    "#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Window reflection passed");
}


#[test]
fn image_script_interfaces_use_real_nodes_and_canonical_owner_prototypes() {
    let (doc, _, page, outcome) = page(
        "<html><body><img id='image' src='pixel.png'><script id='script' src='app.js'></script><div id='other'></div><svg><script id='foreign'></script></svg></body></html>",
        &[r#"
var image=document.getElementById('image'),script=document.getElementById('script');
var I=HTMLImageElement,S=HTMLScriptElement,F=Image;
if(document.getElementById('foreign') instanceof S)throw 'foreign namespace brand';
if(I===F||I.prototype!==F.prototype||I.prototype.constructor!==I)throw 'legacy factory prototype';
for(var pair of [[image,I,'HTMLImageElement'],[script,S,'HTMLScriptElement']]){
    var node=pair[0],C=pair[1],name=pair[2];
    if(Object.getPrototypeOf(C)!==HTMLElement||Object.getPrototypeOf(C.prototype)!==HTMLElement.prototype||
       Object.getPrototypeOf(node)!==C.prototype||node.constructor!==C||!(node instanceof C)||
       !(node instanceof HTMLElement)||Object.prototype.toString.call(node)!=='[object '+name+']')throw 'native interface ancestry';
    var pd=Object.getOwnPropertyDescriptor(C,'prototype'),gd=Object.getOwnPropertyDescriptor(window,name);
    if(pd.writable||pd.configurable||pd.enumerable||!gd.writable||!gd.configurable||gd.enumerable)throw 'interface descriptors';
    var rejected=0;for(var call of [function(){new C();},function(){C();}]){
        try{call();}catch(e){if(!(e instanceof TypeError))throw e;rejected++;}
    }
    if(rejected!==2||document.getElementById('other') instanceof C)throw 'illegal constructor or false brand';
}
var made=new F(9,11),created=document.createElement('script');
if(!(made instanceof I)||!(made instanceof F)||made.width!==9||made.height!==11||!made.complete||
   made.isConnected||!(created instanceof S)||created.isConnected)throw 'real factories';
if(image.src!=='https://www.google.com/pixel.png'||script.src!=='https://www.google.com/app.js')throw 'native resource URL';
I.prototype.interfaceProbe=17;S.prototype.interfaceProbe=23;
if(image.interfaceProbe!==17||made.interfaceProbe!==17||created.interfaceProbe!==23)throw 'prototype inheritance';
image.interfaceProbe=31;if(image.interfaceProbe!==31||!delete image.interfaceProbe||image.interfaceProbe!==17)throw 'own override';
var owner=document.implementation.createHTMLDocument('inert');
for(var pair of [['img',I],['script',S]]){
    var inert=owner.createElement(pair[0]);
    if(!(inert instanceof pair[1])||Object.getPrototypeOf(inert)!==pair[1].prototype||inert.ownerDocument!==owner||inert.isConnected)throw 'inert owner interface';
    var copy=inert.cloneNode(true);if(!(copy instanceof pair[1])||copy.ownerDocument!==owner||copy===inert)throw 'inert clone identity';
}
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow;
if(w.HTMLImageElement===I||w.HTMLScriptElement===S||image instanceof w.HTMLImageElement||
   Object.getPrototypeOf(w.HTMLImageElement.prototype)!==w.HTMLElement.prototype)throw 'owner realm';
delete window.HTMLImageElement;window.HTMLScriptElement=17;
if(!(image instanceof I)||!(made instanceof F)||!(created instanceof S)||new F(3).width!==3)throw 'saved canonical constructors';
document.title='image-script-interfaces-ok';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "image-script-interfaces-ok");
}

#[test]
fn base_node_interfaces_link_real_ancestry_and_author_element_polyfills() {
    let(doc,_,page,outcome)=page("<html><body><div id='base'>Text</div><svg id='vector'></svg></body></html>",&[r#"
var d=document.getElementById('base');d.beforeInterfaceRead=1;
if(Object.getPrototypeOf(HTMLElement)!==Element||Object.getPrototypeOf(Element)!==Node||Object.getPrototypeOf(Node)!==EventTarget||Object.getPrototypeOf(HTMLElement.prototype)!==Element.prototype||Object.getPrototypeOf(Element.prototype)!==Node.prototype||Object.getPrototypeOf(Node.prototype)!==EventTarget.prototype)throw 'interface chain';
var text=d.firstChild,attr=document.createAttribute('title'),img=new Image(),svg=document.getElementById('vector');
if(!(d instanceof HTMLElement)||!(d instanceof Element)||!(d instanceof Node)||!(d instanceof EventTarget)||!(document instanceof Node)||!(text instanceof Node)||!(attr instanceof Node)||text instanceof Element||attr instanceof HTMLElement||document instanceof Element)throw 'node brands';
if(!Element.prototype.isPrototypeOf(d)||!Node.prototype.isPrototypeOf(text)||!Node.prototype.isPrototypeOf(attr)||!Node.prototype.isPrototypeOf(document)||Element.prototype.isPrototypeOf(text)||!EventTarget.prototype.isPrototypeOf(d))throw 'known suffix inspection';
if(!(svg instanceof Element)||svg instanceof HTMLElement||!(img instanceof Image)||!(img instanceof HTMLElement)||!(img instanceof Node)||d instanceof Image||Object.getPrototypeOf(Image.prototype)!==HTMLElement.prototype)throw 'element/image ancestry';
if(Node.ELEMENT_NODE!==1||Node.DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC!==32||Element.TEXT_NODE!==3||HTMLElement.DOCUMENT_NODE!==9||Node.prototype.ATTRIBUTE_NODE!==2)throw 'constants';
var constant=Object.getOwnPropertyDescriptor(Node,'ELEMENT_NODE'),prototype=Object.getOwnPropertyDescriptor(Element,'prototype'),global=Object.getOwnPropertyDescriptor(window,'Element');
if(constant.writable||constant.configurable||!constant.enumerable||prototype.writable||prototype.configurable||global.enumerable||!global.writable||!global.configurable)throw 'descriptors';
var conversions=0,illegal=0;for(var C of [Node,Element,HTMLElement]){try{new C({toString:function(){conversions++;}});}catch(e){if(e instanceof TypeError)illegal++;else throw e;}try{C();}catch(e){if(e instanceof TypeError)illegal++;else throw e;}}if(illegal!==6||conversions!==0)throw 'illegal constructors';
if(d.beforeInterfaceRead!==1||typeof d.animate!=='undefined'||'animate' in d)throw 'initial absence';
var reads=0,calls='',polyfill=function(a,b){if(this!==d)throw 'polyfill receiver';calls+=a+b;this.setAttribute('data-polyfill',calls);return this;};
Object.defineProperty(Element.prototype,'animate',{configurable:true,get:function(){reads++;return polyfill;}});
if(!('animate' in d)||reads!==0||d.animate('A','B')!==d||d['animate']?.('C','D')!==d||reads!==2||d.getAttribute('data-polyfill')!=='ABCD')throw 'polyfill inheritance';
var canonicalType=Object.getOwnPropertyDescriptor(Node.prototype,'nodeType').get,shadowReads=0;
Object.defineProperty(Element.prototype,'nodeType',{configurable:true,get:function(){shadowReads++;return 37;}});
if(d.nodeType!==37||shadowReads!==1||canonicalType.call(d)!==1)throw 'real inherited getter versus native slot';
var shadowMarker={};Object.defineProperty(Element.prototype,'nodeType',{configurable:true,get:function(){throw shadowMarker;}});
try{d.nodeType;throw 'missing inherited getter abrupt';}catch(e){if(e!==shadowMarker)throw e;}
delete Element.prototype.nodeType;if(d.nodeType!==1||canonicalType.call(d)!==1)throw 'canonical getter restoration';
delete Element.prototype.animate;if(typeof d.animate!=='undefined'||'animate' in d)throw 'polyfill deletion';
var oldElement=Element,oldNode=Node;Element=17;delete window.Node;if(!(d instanceof oldElement)||!(text instanceof oldNode)||typeof Node!=='undefined')throw 'canonical anchors';
document.title='base-node-interfaces-ok';
"#]);assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"base-node-interfaces-ok");
}

#[test]
fn node_event_methods_are_canonical_and_text_attr_paths_are_real() {
    let(doc,_,page,outcome)=page("<html><body><div id='base'>Text</div></body></html>",&[r#"
var d=document.getElementById('base'),text=d.firstChild,attr=document.createAttribute('title'),event=document.createEvent('Event'),log='';d.setAttributeNode(attr);
if(d.addEventListener!==EventTarget.prototype.addEventListener||text.dispatchEvent!==d.dispatchEvent||attr.removeEventListener!==document.removeEventListener||document.addEventListener!==d.addEventListener)throw 'canonical node methods';
text.addEventListener('x',function(e){if(this!==text||e.target!==text||e.eventPhase!==2)throw 'text target';log+='T';});d.addEventListener('x',function(e){if(e.target!==text||e.currentTarget!==d||e.eventPhase!==3)throw 'text parent';log+='D';});
event.initEvent('x',true,false);text.dispatchEvent(event);if(log!=='TD')throw 'text propagation';
attr.addEventListener('x',function(e){if(this!==attr||e.target!==attr||e.eventPhase!==2)throw 'attr target';log+='A';});attr.dispatchEvent(event);if(log!=='TDA'||event.target!==attr)throw 'Attr has no parent path';
var original=EventTarget.prototype.dispatchEvent,wrapped=0;EventTarget.prototype.dispatchEvent=function(e){wrapped++;return original.call(this,e);};attr.dispatchEvent(event);if(log!=='TDAA'||wrapped!==1)throw 'prototype override';
delete EventTarget.prototype.dispatchEvent;if('dispatchEvent' in d||'dispatchEvent' in text||'dispatchEvent' in attr||'dispatchEvent' in document||typeof d.dispatchEvent!=='undefined')throw 'removed prototype method';EventTarget.prototype.dispatchEvent=original;
d.dispatchEvent=function(){return 'own';};if(d.dispatchEvent(event)!=='own')throw 'own method';delete d.dispatchEvent;if(d.dispatchEvent!==original)throw 'own deletion';
document.title='node-event-methods-ok';
"#]);assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"node-event-methods-ok");
}

#[test]
fn node_interface_suffix_keeps_concrete_tags_and_named_forms_truthful() {
    let(mut doc,mut forms,mut page,outcome)=page("<html><body><div id='base'></div><section id='opaque'></section><form id='f'><input name='addEventListener'><input name='removeEventListener'><input name='dispatchEvent'></form></body></html>",&[r#"
var d=document.getElementById('base'),opaque=document.getElementById('opaque'),f=document.getElementById('f');
if(f.addEventListener.getAttribute('name')!=='addEventListener'||f.removeEventListener.getAttribute('name')!=='removeEventListener'||f.dispatchEvent.getAttribute('name')!=='dispatchEvent')throw 'named form precedence';
var seen=0,e=document.createEvent('Event');e.initEvent('x',false,false);EventTarget.prototype.addEventListener.call(f,'x',function(){seen++;});EventTarget.prototype.dispatchEvent.call(f,e);if(seen!==1)throw 'borrowed form delivery';
if(Node.prototype[Symbol.toStringTag]!=='Node'||Element.prototype[Symbol.toStringTag]!=='Element'||HTMLElement.prototype[Symbol.toStringTag]!=='HTMLElement')throw 'prototype tags';
var DP=Object.getPrototypeOf(d);
if(DP!==HTMLDivElement.prototype||Object.getPrototypeOf(DP)!==HTMLElement.prototype||d.constructor!==HTMLDivElement||!(d instanceof HTMLDivElement))throw 'genuine DIV concrete suffix';
if(d[Symbol.toStringTag]!=='HTMLDivElement'||d.toString!==Object.prototype.toString||d.toString()!=='[object HTMLDivElement]'||Object.prototype.toString.call(d)!=='[object HTMLDivElement]')throw 'genuine DIV class tag and inherited stringifier';
Object.defineProperty(d,Symbol.toStringTag,{value:'OwnAuthorTag',configurable:true});if(d[Symbol.toStringTag]!=='OwnAuthorTag')throw 'own symbol';delete d[Symbol.toStringTag];
if(d[Symbol.toStringTag]!=='HTMLDivElement'||d.toString()!=='[object HTMLDivElement]')throw 'DIV own tag deletion restores concrete suffix';
"#]);assert_eq!(outcome.errors,0,"{}",page.report());
    let out=page.console_eval(&mut doc,&mut forms,"if(opaque[Symbol.toStringTag]!=='HTMLElement'||Object.getPrototypeOf(opaque)!==HTMLElement.prototype||opaque.constructor!==HTMLElement||!(opaque instanceof HTMLElement))throw 'semantic SECTION suffix';var unsupported=document.createElementNS('urn:opaque-fixture','opaque');");
    assert_eq!(out.errors,0,"{}",page.report());
    let out=page.console_eval(&mut doc,&mut forms,"if(unsupported instanceof HTMLElement)throw 'foreign namespace acquired HTML brand';");
    assert_eq!(out.errors,0,"{}",page.report());
}

fn shared_probe(source: &str) -> (Document,Forms,Page,Outcome) {
    page("<div id=p title=present></div>",&[source])
}

#[test]
fn detached_style_type_uses_real_attribute_identity_domstring_and_metadata_routes(){
    let (mut doc,mut forms,mut page,outcome)=page("<html><body><p id=p>Existing page</p></body></html>",&[r#"
var detachedStyle=document.createElement('style'),conversions='';
if(!('type' in detachedStyle)||detachedStyle.type!==''||detachedStyle.getAttribute('type')!==null||detachedStyle.isConnected)throw 'initial type';
detachedStyle.setAttribute('type','text/css');var styleTypeAttr=detachedStyle.getAttributeNode('type');
detachedStyle.type={toString:function(){conversions+='idl;';return '  TEXT/CSS  ';}};
if(detachedStyle.type!=='  TEXT/CSS  '||styleTypeAttr.value!==detachedStyle.type||detachedStyle.getAttributeNode('type')!==styleTypeAttr)throw 'raw reflection';
detachedStyle.setAttribute('TYPE',{toString:function(){conversions+='attribute;';return 'application/metadata';}});
if(detachedStyle.type!=='application/metadata'||conversions!=='idl;attribute;')throw 'attribute parity';
styleTypeAttr.value='text/css; charset=utf-8';if(detachedStyle.type!==styleTypeAttr.value)throw 'attached Attr';
var replacementType=document.createAttribute('type');replacementType.value='replacement';if(detachedStyle.setAttributeNode(replacementType)!==styleTypeAttr||styleTypeAttr.ownerElement!==null||detachedStyle.type!=='replacement')throw 'adopted Attr';
detachedStyle.setAttributeNS('urn:metadata','type','inert');if(detachedStyle.type!=='replacement')throw 'namespace decoy';
detachedStyle.type=null;if(detachedStyle.type!=='null'||replacementType.value!=='null')throw 'DOMString null';
detachedStyle.removeAttribute('type');if(detachedStyle.type!==''||!('type' in detachedStyle)||replacementType.ownerElement!==null)throw 'removal';
detachedStyle.type='text/css';var symbol=Symbol('type'),caught=0;
for(var operation of [function(){detachedStyle.type=symbol;},function(){detachedStyle.setAttribute('type',symbol);},function(){detachedStyle.getAttributeNode('type').value=symbol;}]){try{operation();}catch(e){if(e instanceof TypeError)caught++;else throw e;}}
if(caught!==3||detachedStyle.type!=='text/css')throw 'symbol rollback';
var thrown=false;try{detachedStyle.type={toString:function(){throw 17;}};}catch(e){thrown=e===17;}if(!thrown||detachedStyle.type!=='text/css')throw 'throwing conversion';
document.title='detached-style-type-ok';
"#]);assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"detached-style-type-ok");assert!(!page.has_host_tasks());assert!(page.take_dynamic_stylesheets().is_empty());
    let style=doc.nodes.iter().position(|n|matches!(&n.kind,NodeKind::Element {tag,..} if tag=="style")).unwrap();let nodes=doc.nodes.len();let commits=page.state.dom_commits;
    let failed = page.console_eval(&mut doc, &mut forms, "detachedStyle.innerHTML='body{display:none}';");
    assert_eq!(failed.errors, 1, "{}", page.report());
    assert_eq!(doc.nodes.len(), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert!(doc.nodes[style].children.is_empty());
    assert!(!page.has_host_tasks());
    let text = page.console_eval(&mut doc, &mut forms, "detachedStyle.textContent='body{display:none}';if(detachedStyle.isConnected||detachedStyle.firstChild.data!==detachedStyle.textContent||detachedStyle.type!=='text/css')throw 'detached style text or type';");
    assert_eq!(text.errors, 0, "{}", page.report());
    assert_eq!(doc.nodes.len(), nodes + 1);
    assert_eq!(doc.nodes[style].children.len(), 1);
    let connected = page.console_eval(&mut doc, &mut forms, "var text=detachedStyle.firstChild; document.body.appendChild(detachedStyle); if(!detachedStyle.isConnected||text!==detachedStyle.firstChild||text.data!==detachedStyle.textContent||detachedStyle.type!=='text/css')throw 'connected style text or type';");
    assert_eq!(connected.errors, 0, "{}", page.report());
    assert!(!page.has_host_tasks());
}

#[test]
fn detached_style_type_limits_and_foreign_owner_fail_before_effects(){
    let (mut doc,mut forms,mut page,outcome)=page("<html><body></body></html>",&["var detachedStyle=document.createElement('style');detachedStyle.type='text/css';var observedType=detachedStyle.getAttributeNode('type');"]);assert_eq!(outcome.errors,0,"{}",page.report());
    let node=doc.nodes.iter().position(|n|matches!(&n.kind,NodeKind::Element {tag,..} if tag=="style")).unwrap();let commits=page.state.dom_commits;let retained_limit=page.state.retained_limit;page.state.retained_limit=page.state.retained;
    let out=page.console_eval(&mut doc,&mut forms,"detachedStyle.type='a-much-longer-type-value';");assert_eq!(out.errors,1,"{}",page.report());assert_eq!(doc.attr(node,"type"),Some("text/css"));assert_eq!(page.state.dom_commits,commits);page.state.retained_limit=retained_limit;
    let limit=page.state.limits.host.string_bytes;page.state.limits.host.string_bytes=2;assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"type").is_err());page.state.limits.host.string_bytes=limit;page.begin_job();page.state.copy_work=0;assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"type").is_err());page.begin_job();
    let out=page.console_eval(&mut doc,&mut forms,"if(detachedStyle.type!=='text/css'||observedType.value!=='text/css')throw 'recovered reflection';var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var readType=f.contentWindow.Function('s','return s.type;'),hasType=f.contentWindow.Function('s','return \"type\" in s;'),writeType=f.contentWindow.Function('s','v','s.type=v;'),readTypeAttr=f.contentWindow.Function('a','return a.value;'),hasTypeAttr=f.contentWindow.Function('a','return \"value\" in a;'),writeTypeAttr=f.contentWindow.Function('a','v','a.value=v;'),styleConversions=0;if(readType(detachedStyle)!=='text/css'||!hasType(detachedStyle)||readTypeAttr(observedType)!=='text/css'||!hasTypeAttr(observedType))throw 'same origin';");assert_eq!(out.errors,0,"{}",page.report());
    page.set_location(&crate::navigation::address("https://different.test/",&crate::navigation::Location::home()).unwrap());
    for source in ["readType(detachedStyle);","hasType(detachedStyle);","writeType(detachedStyle,{toString:function(){styleConversions++;return 'changed';}});","writeTypeAttr(observedType,{toString:function(){styleConversions++;return 'changed';}});","readTypeAttr(observedType);","hasTypeAttr(observedType);"]{let out=page.console_eval(&mut doc,&mut forms,source);assert_eq!(out.errors,1,"{}",page.report());assert!(page.report().contains("same-origin"));assert_eq!(doc.attr(node,"type"),Some("text/css"));assert!(!page.has_host_tasks());}
    let out=page.console_eval(&mut doc,&mut forms,"if(styleConversions!==0)throw 'foreign coercion';");assert_eq!(out.errors,0,"{}",page.report());
}

#[test]
fn parsed_link_href_reflection_and_mutation_use_checked_stylesheet_lifecycle(){
    let (mut doc,mut forms,mut page,outcome)=page("<html><body><div id=holder><link id=relative rel=stylesheet href='../original.css?q=1#section'><link id=absolute href='https://other.test/read-only.css'><link id=missing><link id=empty href=''><link id=invalid href='http://[broken'></div></body></html>",&[]);assert_eq!(outcome.errors,0);
    page.set_resource_base(&crate::navigation::address("https://base.test/assets/index.html",&crate::navigation::Location::home()).unwrap());
    let commits=page.state.dom_commits;
    let out=page.console_eval(&mut doc,&mut forms,r#"
var relative=document.getElementById('relative'),missing=document.getElementById('missing'),empty=document.getElementById('empty');
if(!('href' in relative)||!('href' in missing)||document.querySelectorAll('link[rel~="stylesheet"]')[0]!==relative||relative.href!=='https://base.test/original.css?q=1#section'||relative.getAttribute('href')!=='../original.css?q=1#section')throw 'real stylesheet URL';
if(document.getElementById('absolute').href!=='https://other.test/read-only.css'||missing.href!==''||empty.href!=='https://base.test/assets/index.html'||document.getElementById('invalid').href!=='http://[broken')throw 'URL reflection states';
"#);assert_eq!(out.errors,0,"{}",page.report());assert_eq!(page.state.dom_commits,commits);assert!(page.take_dynamic_stylesheets().is_empty());assert!(!page.has_host_tasks());
    let out=page.console_eval(&mut doc,&mut forms,"relative.href='/first.css';");
    assert_eq!(out.errors,0,"{}",page.report());
    let first=page.state.stylesheets.pending.front().unwrap().clone();
    assert_eq!(first.target.display(),"https://base.test/first.css");
    assert!(first.error.is_none());
    assert!(!first.same_origin_only);
    let out=page.console_eval(&mut doc,&mut forms,"relative.setAttribute('href','/second.css');");
    assert_eq!(out.errors,0,"{}",page.report());
    assert!(first.cancel.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(page.state.stylesheets.pending.len(),1);
    let second=page.state.stylesheets.pending.front().unwrap().clone();
    assert_eq!(second.target.display(),"https://base.test/second.css");
    let out=page.console_eval(&mut doc,&mut forms,"relative.removeAttribute('href');relative.parentNode.removeChild(relative);relative.href='../original.css?q=1#section';if(relative.isConnected)throw 'link detach';");
    assert_eq!(out.errors,0,"{}",page.report());
    assert!(second.cancel.load(std::sync::atomic::Ordering::Acquire));
    assert!(page.take_dynamic_stylesheets().is_empty());
    let removed=page.take_stylesheet_removals();
    assert_eq!(removed.len(),1);
    assert!(removed.contains(&first.node));
    assert_eq!(page.state.stylesheets.in_flight,0);
    assert!(!page.has_host_tasks());
    page.set_resource_base(&crate::navigation::address("https://base.test/new/",&crate::navigation::Location::home()).unwrap());
    let out=page.console_eval(&mut doc,&mut forms,"if(relative.href!=='https://base.test/original.css?q=1#section'||empty.href!=='https://base.test/new/')throw 'live base';missing.setAttributeNS('urn:metadata','href','/inert.css');if(missing.href!=='')throw 'namespace metadata';var created=document.createElement('link');created.href='';if(created.href!=='https://base.test/new/')throw 'created empty';");assert_eq!(out.errors,0,"{}",page.report());assert!(!page.has_host_tasks());
}

#[test]
fn parsed_link_href_limits_local_selection_and_foreign_callers_preserve_boundaries(){
    let (mut doc,mut forms,mut page,outcome)=page("<html><body><link id=sheet href='missing-selected.css'></body></html>",&[]);assert_eq!(outcome.errors,0);
    let node=doc.nodes.iter().position(|n|matches!(&n.kind,NodeKind::Element {tag,..} if tag=="link")).unwrap();let commits=page.state.dom_commits;
    let local=crate::navigation::explicit(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/authored-video.html")).unwrap();page.set_resource_base(&local);
    let value=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"href").unwrap();assert!(matches!(value,Value::String(s) if s.ends_with("/missing-selected.css")&&s.starts_with("file:///")));
    let original=page.state.limits.host.string_bytes;page.state.limits.host.string_bytes=4;
    assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"href").is_err());page.state.limits.host.string_bytes=original;page.begin_job();page.state.copy_work=0;
    assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"href").is_err());assert_eq!(page.state.dom_commits,commits);page.begin_job();
    page.set_resource_base(&crate::navigation::address("https://www.google.com/assets/",&crate::navigation::Location::home()).unwrap());
    let out=page.console_eval(&mut doc,&mut forms,"var sheet=document.getElementById('sheet'),f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var readHref=f.contentWindow.Function('l','return l.href;'),hasHref=f.contentWindow.Function('l','return \"href\" in l;');if(readHref(sheet)!=='https://www.google.com/assets/missing-selected.css'||!hasHref(sheet))throw 'same origin reader';");assert_eq!(out.errors,0,"{}",page.report());
    page.set_location(&crate::navigation::address("https://different.test/",&crate::navigation::Location::home()).unwrap());
    for operation in ["readHref(sheet);","hasHref(sheet);"]{let out=page.console_eval(&mut doc,&mut forms,operation);assert_eq!(out.errors,1,"{}",page.report());assert!(page.report().contains("same-origin"));assert!(!page.has_host_tasks());}
}

#[test]
fn parsed_script_source_reflects_actual_attribute_and_resource_base_without_requests() {
    let (mut doc,mut forms,mut page,outcome)=page("<script id=relative src='../original.js?q=1#section'></script><script id=absolute src='https://other.test/read-only.js'></script><script id=missing></script><script id=empty src=''></script><script id=invalid src='http://[broken'></script>",&[]);
    assert_eq!(outcome.errors,0);
    let base=crate::navigation::address("https://base.test/assets/index.html",&crate::navigation::Location::home()).unwrap();page.set_resource_base(&base);
    let source=r#"
var relative=document.getElementById('relative'),absolute=document.getElementById('absolute'),missing=document.getElementById('missing'),empty=document.getElementById('empty'),invalid=document.getElementById('invalid');
if(!('src' in relative)||!('src' in missing)||relative.src!=='https://base.test/original.js?q=1#section'||relative.getAttribute('src')!=='../original.js?q=1#section')throw 'relative reflection';
if(absolute.src!=='https://other.test/read-only.js'||missing.src!==''||empty.src!=='https://base.test/assets/index.html'||invalid.src!=='http://[broken')throw 'missing empty absolute invalid';
missing.setAttributeNS('urn:original','src','https://metadata.test/never.js');
if(missing.src!==''||missing.getAttributeNS('urn:original','src')!=='https://metadata.test/never.js')throw 'namespace metadata';
relative.setAttributeNS('urn:original','src','/never.js');if(relative.src!=='https://base.test/original.js?q=1#section')throw 'namespaced override';
document.title='parsed-src-ok';
"#;
    let outcome=page.console_eval(&mut doc,&mut forms,source);assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"parsed-src-ok");assert!(page.take_dynamic_scripts().is_empty());assert!(!page.has_host_tasks());
    page.set_resource_base(&crate::navigation::address("https://base.test/new/",&crate::navigation::Location::home()).unwrap());
    let outcome=page.console_eval(&mut doc,&mut forms,"if(relative.src!=='https://base.test/original.js?q=1#section'||empty.src!=='https://base.test/new/')throw 'current base';");assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn parsed_script_source_read_does_not_enable_loaded_script_mutation_or_foreign_aliases() {
    let (mut doc,mut forms,mut page,outcome)=page("<html><body><script id=parsed src='/original.js'></script><svg><script id=foreign src='/foreign.js'></script></svg></body></html>",&["var parsed=document.getElementById('parsed');if(parsed.src!=='https://www.google.com/original.js')throw 'read';"]);assert_eq!(outcome.errors,0,"{}",page.report());
    let node=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"id")==Some("parsed")).then_some(n)).unwrap();let commits=page.state.dom_commits;let retained=page.state.retained;
    for operation in ["parsed.src='/must-not-fetch.js';","parsed.setAttribute('src','/must-not-fetch.js');","parsed.removeAttribute('src');","parsed.text='throw 17;';"] {
        let outcome=page.console_eval(&mut doc,&mut forms,operation);assert_eq!(outcome.errors,1,"{operation}: {}",page.report());assert_eq!(doc.attr(node,"src"),Some("/original.js"));assert_eq!(page.state.dom_commits,commits);assert_eq!(page.state.retained,retained);assert!(page.take_dynamic_scripts().is_empty());assert!(!page.has_host_tasks());
    }
    let foreign=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"id")==Some("foreign")).then_some(n)).unwrap();assert_eq!(doc.nodes[foreign].namespace,crate::html::Namespace::UnknownForeign);
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};assert!(bridge.get(node_handle(foreign),"src").is_err());assert!(!matches!(bridge.has(node_handle(foreign),"src"),Ok(true)));
}

#[test]
fn parsed_script_source_small_copy_and_result_profiles_recover_without_mutation() {
    let (mut doc,mut forms,mut page,outcome)=page("<script id=parsed src='/original.js'></script>",&[]);assert_eq!(outcome.errors,0);let node=doc.nodes.iter().position(|n|matches!(&n.kind,NodeKind::Element {tag,..} if tag=="script")).unwrap();let commits=page.state.dom_commits;
    let original=page.state.limits.host.string_bytes;page.state.limits.host.string_bytes=4;
    assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"src").is_err());assert_eq!(page.state.dom_commits,commits);
    page.state.limits.host.string_bytes=original;page.begin_job();page.state.copy_work=0;
    assert!(Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"src").is_err());assert_eq!(page.state.dom_commits,commits);
    page.begin_job();let value=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}.get(node_handle(node),"src").unwrap();assert!(matches!(value,Value::String(s) if s=="https://www.google.com/original.js"));assert!(!page.has_host_tasks());
}

#[test]
fn attribute_metadata_nonce_updates_real_attr_without_rerunning_a_parsed_script() {
    let (mut doc,mut forms,mut page,outcome)=page("<script id=script nonce=original></script>",&["var count=1,script=document.getElementById('script'),nonce=script.getAttributeNode('nonce');script.setAttribute('nonce','changed');if(nonce.value!=='changed'||script.getAttribute('nonce')!=='changed')throw 'metadata';"]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert!(page.take_dynamic_scripts().is_empty());
    let outcome=page.console_eval(&mut doc,&mut forms,"if(count!==1||script.getAttribute('nonce')!=='changed')throw 'parsed script rerun';");assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn attribute_nodes_shorter_payload_and_prefix_transfers_fit_the_exact_final_retention() {
    for (markup,setup) in [
        ("<div id=p title='a much longer original payload'></div>","var p=document.getElementById('p'),old=p.getAttributeNode('title'),incoming=document.createAttribute('title');incoming.value='x';"),
        ("<div id=p></div>","var p=document.getElementById('p');p.setAttributeNS('urn:original','veryLongPrefix:token','long original value');var old=p.getAttributeNodeNS('urn:original','token'),incoming=document.createAttributeNS('urn:original','p:token');incoming.value='x';"),
    ] {
        let (mut doc,mut forms,mut page,outcome)=page(markup,&[setup]);assert_eq!(outcome.errors,0,"{}",page.report());
        let retained=page.state.retained;page.state.retained_limit=retained;
        let outcome=page.console_eval(&mut doc,&mut forms,"if(p.setAttributeNodeNS(incoming)!==old||incoming.ownerElement!==p||old.ownerElement!==null||incoming.value!=='x')throw 'exact-limit transfer';");
        assert_eq!(outcome.errors,0,"{}",page.report());assert!(page.state.retained<=retained);
    }
}

#[test]
fn attribute_nodes_small_work_retention_and_identity_failures_are_atomic_and_recover() {
    for mode in 0..3 {
        let (mut doc,mut forms,mut page,outcome)=page("<div id=p title=old></div>",&["var incoming=document.createAttribute('title');incoming.value='replacement payload';"]);
        assert_eq!(outcome.errors,0);
        let node=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"id")==Some("p")).then_some(n)).unwrap();
        let incoming=script_attr::ATTR_BASE;
        let retained=page.state.retained;let identities=page.state.attributes.len();let limit=page.state.retained_limit;let identity_limit=page.state.limits.host.attribute_identities;
        match mode {0=>page.state.host_work=0,1=>page.state.retained_limit=retained,_=>page.state.limits.host.attribute_identities=identities}
        let error=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.call(node_handle(node),"setAttributeNode",&[Value::Host(incoming)]).unwrap_err();
        assert!(!error.is_empty());assert_eq!(doc.attr(node,"title"),Some("old"));assert_eq!(page.state.attributes.len(),identities);assert_eq!(page.state.attributes[0].owner,None);assert_eq!(page.state.attributes[0].value,"replacement payload");assert_eq!(page.state.retained,retained);
        page.state.retained_limit=limit;page.state.limits.host.attribute_identities=identity_limit;page.begin_job();
        let outcome=page.console_eval(&mut doc,&mut forms,"var removed=document.getElementById('p').setAttributeNode(incoming);if(removed.value!=='old'||removed.ownerElement!==null||incoming.ownerElement!==document.getElementById('p'))throw 'recovery';");
        assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.attr(node,"title"),Some("replacement payload"));
    }
}

#[test]
fn attribute_nodes_existing_identity_moves_payload_without_double_retention_and_refuses_active_resource_adoption() {
    let (mut doc,mut forms,mut page,outcome)=page("<div id=p title=old></div>",&["var p=document.getElementById('p'),old=p.getAttributeNode('title'),incoming=document.createAttribute('title');incoming.value='replacement payload';"]);
    assert_eq!(outcome.errors,0);let retained=page.state.retained;
    let outcome=page.console_eval(&mut doc,&mut forms,"if(p.setAttributeNode(incoming)!==old)throw 'returned node';");assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(page.state.retained,retained);
    for tag in ["script","link"] {
        let source=format!("var resource=document.createElement('{tag}');var attr=document.createAttribute('{}');attr.value='/must-not-fetch';",if tag=="script"{"src"}else{"href"});
        let (mut doc,mut forms,mut page,outcome)=self::page("<div>Test</div>",&[&source]);assert_eq!(outcome.errors,0,"{}",page.report());
        let resource=page.state.dom.created.iter().find(|&n|super::tag(&doc,n)==tag).unwrap();
        let retained=page.state.retained;let commits=page.state.dom_commits;
        assert!(Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.call(node_handle(resource),"setAttributeNode",&[Value::Host(script_attr::ATTR_BASE)]).unwrap_err().contains("active script/stylesheet"));
        assert_eq!(page.state.retained,retained);assert_eq!(page.state.dom_commits,commits);assert!(doc.attr(resource,if tag=="script"{"src"}else{"href"}).is_none());assert_eq!(page.state.attributes[0].owner,None);
    }
}

#[test]
fn attribute_nodes_create_attach_replace_and_move_real_identities_in_list_order() {
    let (_,_,page,outcome)=page("<div id=p title=old data-tail=tail></div><div id=q></div>",&[r#"
var p=document.getElementById('p'),q=document.getElementById('q'),map=p.attributes,old=p.getAttributeNode('title'),saved=old;
if(document.createAttribute.length!==1||document.createAttributeNS.length!==2||p.setAttributeNode.length!==1||p.setAttributeNodeNS.length!==1)throw 'arity';
var incoming=document.createAttribute('TITLE');incoming.value='new';
if(incoming.name!=='title'||incoming.ownerElement!==null||incoming.ownerDocument!==document||incoming.value!=='new')throw 'factory';
if(p.setAttributeNode(incoming)!==old||old!==saved||incoming.ownerElement!==p||old.ownerElement!==null||old.value!=='old'||p.title!=='new'||map.getNamedItem('title')!==incoming||map.item(1)!==incoming)throw 'replacement identity/order';
old.value='saved';if(p.title!=='new')throw 'detached saved reference';
if(p.setAttributeNode(incoming)!==incoming||map.item(1)!==incoming)throw 'same node';
if(p.removeAttributeNode(incoming)!==incoming||incoming.ownerElement!==null||incoming.value!=='new')throw 'remove';
if(q.setAttributeNode(incoming)!==null||q.getAttributeNode('title')!==incoming||incoming.ownerElement!==q)throw 'move after removal';
if(p.setAttributeNode(old)!==null||p.title!=='saved'||p.attributes.item(p.attributes.length-1)!==old)throw 'append/order';
var a=document.createAttributeNS('urn:original','one:token');a.value='first';p.setAttributeNodeNS(a);
var b=document.createAttributeNS('urn:original','two:token');b.value='second';
if(p.setAttributeNode(b)!==a||a.ownerElement!==null||a.name!=='one:token'||a.value!=='first'||b.ownerElement!==p||b.prefix!=='two'||p.getAttributeNodeNS('urn:original','token')!==b||p.getAttribute('one:token')!==null||p.getAttribute('two:token')!=='second')throw 'expanded replacement';
var upper=document.createAttributeNS(null,'ID');upper.value='inert-case';p.setAttributeNodeNS(upper);if(p.id!=='p'||p.getAttributeNS(null,'ID')!=='inert-case')throw 'case namespace';
var namespaced=document.createAttributeNS('urn:no-permission','n:src');namespaced.value='https://forbidden.test/';p.setAttributeNode(namespaced);if(p.getAttributeNS('urn:no-permission','src')!=='https://forbidden.test/')throw 'inert namespace';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn attribute_nodes_named_exceptions_conversions_and_independent_promise_tail() {
    let (_,_,page,outcome)=page("<div id=p title=old></div><div id=q></div>",&[r#"
var p=document.getElementById('p'),q=document.getElementById('q'),a=p.getAttributeNode('title'),calls=0;
function error(name,fn){var caught=false;try{fn();}catch(e){caught=e.name===name;}if(!caught)throw name;}
var duck={name:'title',value:'fake',toString:function(){calls++;return 'title';}};
error('TypeError',function(){p.setAttributeNode(duck);});error('TypeError',function(){p.setAttributeNode();});
error('TypeError',function(){p.setAttributeNode.call({},a);});error('InUseAttributeError',function(){q.setAttributeNode(a);});
if(calls!==0||a.ownerElement!==p||p.title!=='old'||q.hasAttribute('title'))throw 'invalid argument effects';
error('InvalidCharacterError',function(){document.createAttribute('bad name');});error('NamespaceError',function(){document.createAttributeNS(null,'p:x');});
var order='',ns={toString:function(){order+='N';return 'urn:original';}},name={toString:function(){order+='Q';return 'a:token';}};
var b=document.createAttributeNS(ns,name);if(order!=='NQ'||b.namespaceURI!=='urn:original'||b.prefix!=='a'||b.localName!=='token'||b.value!=='')throw 'factory conversions';
var marker={},caught=false;try{document.createAttribute({toString:function(){throw marker;}});}catch(e){caught=e===marker;}if(!caught)throw 'factory throw identity';
error('TypeError',function(){document.createAttribute(Symbol());});
var log='';Promise.resolve().then(function(){try{q.setAttributeNode(a);}catch(e){if(e.name!=='InUseAttributeError')throw e;log+='caught';}}).finally(function(){log+='-finally';});
Promise.resolve().then(function(){log+='-tail';});window.addEventListener('load',function(){if(log.indexOf('caught')<0||log.indexOf('finally')<0||log.indexOf('tail')<0)throw 'promise continuation';});
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn html_direction_language_and_alignment_reflect_native_attributes_and_keep_conversion_atomic() {
    let (doc,_,page,outcome)=page("<div id=p dir=RtL lang=EN align=LeFt>Text</div><span id=s></span>",&[r#"
var p=document.getElementById('p'),s=document.getElementById('s');
if(p.dir!=='rtl'||p.lang!=='EN'||p.align!=='LeFt'||!('dir' in p)||!('lang' in p)||!('align' in p))throw 'initial reflection';
var attr=p.getAttributeNode('align'),calls=0;p.align={toString:function(){calls++;return 'right';}};
if(calls!==1||p.align!=='right'||attr.value!=='right'||p.getAttribute('align')!=='right')throw 'native attribute identity';
p.dir='AUTO';if(p.dir!=='auto'||p.getAttribute('dir')!=='AUTO')throw 'known direction value';
p.dir='invalid';if(p.dir!==''||p.getAttribute('dir')!=='invalid')throw 'invalid direction';
p.dir=null;if(p.dir!==''||p.getAttribute('dir')!=='null')throw 'DOMString conversion';
p.lang=17;if(p.lang!=='17'||p.getAttribute('lang')!=='17')throw 'language string';
var marker={},caught=false;try{p.align={toString:function(){throw marker;}};}catch(x){caught=x===marker;}if(!caught||p.align!=='right')throw 'conversion rollback';
caught=false;try{p.align=Symbol();}catch(x){caught=x.name==='TypeError';}if(!caught||p.align!=='right')throw 'symbol';
p.removeAttribute('dir');p.removeAttribute('lang');if(p.dir!==''||p.lang!=='')throw 'absence';
if('align' in s||s.align!==undefined)throw 'unrelated interface';
var t=document.createTextNode('t');if('dir' in t||t.dir!==undefined)throw 'text brand';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let node=doc.nodes.iter().enumerate().find_map(|(n,_)|(doc.attr(n,"id")==Some("p")).then_some(n)).unwrap();
    assert_eq!(crate::css::compute(&doc,&HashMap::new())[node].align,crate::css::Align::Right);
}

#[test]
fn html_use_map_reflects_raw_attribute_identity_conversion_and_unrelated_interfaces() {
    let(_,_,page,outcome)=page("<img id=pic usemap='#Raw Map'><input id=entry usemap='#Input'><span id=other></span>",&[r#"
var pic=document.getElementById('pic'),entry=document.getElementById('entry'),other=document.getElementById('other'),attr=pic.getAttributeNode('usemap'),calls=0;
if(pic.useMap!=='#Raw Map'||entry.useMap!=='#Input'||!('useMap' in pic)||!('useMap' in entry)||'useMap' in other||other.useMap!==undefined)throw 'interfaces/raw read';
pic.useMap={toString:function(){calls++;return 'https://example.invalid/maps#Raw';}};
if(calls!==1||pic.useMap!=='https://example.invalid/maps#Raw'||attr!==pic.getAttributeNode('usemap')||attr.value!==pic.useMap)throw 'real string reflection';
pic.setAttribute('usemap','#Changed');if(pic.useMap!=='#Changed'||attr.value!=='#Changed')throw 'attribute path';pic.removeAttribute('usemap');if(pic.useMap!==''||attr.ownerElement!==null||attr.value!=='#Changed')throw 'remove identity';
pic.useMap=null;if(pic.useMap!=='null')throw 'DOMString';var marker={},caught=false;try{pic.useMap={toString:function(){throw marker;}};}catch(e){caught=e===marker;}if(!caught||pic.useMap!=='null')throw 'abrupt conversion';caught=false;try{pic.useMap=Symbol();}catch(e){caught=e.name==='TypeError';}if(!caught||pic.useMap!=='null')throw 'symbol';
var detached=new Image();if(detached.useMap!=='')throw 'default';detached.useMap='#Detached';if(detached.getAttribute('usemap')!=='#Detached'||detached.useMap!=='#Detached')throw 'detached reflection';entry.useMap=17;if(entry.useMap!=='17'||entry.getAttribute('usemap')!=='17')throw 'input string';
"#]);assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn html_use_map_retained_and_copy_failures_preserve_native_attribute() {
    let(mut doc,mut forms,mut page,outcome)=page("<img id=pic usemap='#old'>",&["var pic=document.getElementById('pic');var saved=pic.getAttributeNode('usemap');"]);assert_eq!(outcome.errors,0,"{}",page.report());let node=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("pic")).unwrap();let before=page.state.retained;let limit=page.state.retained_limit;page.state.retained_limit=before;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};assert!(bridge.dom_set(node_handle(node),"useMap",&Value::String("#a-longer-name".into())).is_err());assert_eq!(bridge.doc.attr(node,"usemap"),Some("#old"));assert_eq!(bridge.state.retained,before);bridge.state.retained_limit=limit;let copies=bridge.state.copy_work;bridge.state.copy_work=0;assert!(bridge.dom_get(node_handle(node),"useMap").is_err());assert_eq!(bridge.doc.attr(node,"usemap"),Some("#old"));bridge.state.copy_work=copies;
    let outcome=page.console_eval(&mut doc,&mut forms,"if(saved!==pic.getAttributeNode('usemap')||saved.value!=='#old'||pic.useMap!=='#old')throw 'failed mutation';pic.useMap='#new';if(saved.value!=='#new')throw 'recovery';");assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn replace_child_returns_removed_identity_and_preserves_detached_references_and_collections() {
    let (_,_,page,outcome)=page("<div id=p><span id=old title=kept><b id=inside>old</b></span><em id=tail>tail</em></div><div id=q><i id=incoming>new</i></div>", &[r#"
var p=document.getElementById('p'),q=document.getElementById('q'),old=document.getElementById('old'),inside=document.getElementById('inside'),tail=document.getElementById('tail'),incoming=document.getElementById('incoming');
var attr=old.getAttributeNode('title'),marker={value:13};old.marker=marker;
var live=p.getElementsByTagName('span'),snapshot=p.querySelectorAll('span');
if(typeof p.replaceChild!=='function'||p.replaceChild.length!==2||!('replaceChild' in p))throw 'method binding';
if(p.replaceChild(incoming,old)!==old||p.firstChild!==incoming||incoming.nextSibling!==tail||tail.previousSibling!==incoming||p.lastChild!==tail||q.firstChild!==null)throw 'replacement order';
if(old.parentNode!==null||old.isConnected||inside.parentNode!==old||inside.isConnected||old.ownerDocument!==document||attr.ownerElement!==old||attr.value!=='kept'||old.marker!==marker)throw 'detached identities';
if(live.length!==0||snapshot.length!==1||snapshot.item(0)!==old||document.getElementById('old')!==null)throw 'live/static lookup';
if(p.replaceChild(incoming,incoming)!==incoming||p.firstChild!==incoming||incoming.nextSibling!==tail)throw 'same node';
var text=document.createTextNode('replacement text');if(p.replaceChild(text,tail)!==tail||text.parentNode!==p||text.previousSibling!==incoming||p.lastChild!==text||tail.parentNode!==null)throw 'text replacement';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn replace_child_moves_either_sibling_and_extracts_an_incoming_descendant_from_removed_subtree() {
    let (_,_,page,outcome)=page("<div id=p><i id=a></i><i id=b></i><i id=c></i><i id=d></i></div><div id=r><div id=old><section id=inside><span id=incoming>new</span><b id=stay>stay</b></section></div><em id=tail>tail</em></div>", &[r#"
var p=document.getElementById('p'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c'),d=document.getElementById('d');
if(p.replaceChild(a,c)!==c||p.firstChild!==b||b.nextSibling!==a||a.nextSibling!==d||c.parentNode!==null)throw 'preceding sibling';
if(p.replaceChild(d,b)!==b||p.firstChild!==d||d.nextSibling!==a||p.lastChild!==a||a.previousSibling!==d||b.parentNode!==null)throw 'following sibling';
var r=document.getElementById('r'),old=document.getElementById('old'),inside=document.getElementById('inside'),incoming=document.getElementById('incoming'),stay=document.getElementById('stay'),tail=document.getElementById('tail');
if(r.replaceChild(incoming,old)!==old||r.firstChild!==incoming||incoming.nextSibling!==tail||incoming.parentNode!==r||!incoming.isConnected)throw 'incoming descendant';
if(old.parentNode!==null||old.isConnected||inside.parentNode!==old||inside.firstChild!==stay||stay.parentNode!==inside||stay.isConnected||old.getElementsByTagName('span').length!==0)throw 'outgoing descendants';
var detached=document.createElement('div'),first=document.createElement('b'),second=document.createElement('i');detached.appendChild(first);if(detached.replaceChild(second,first)!==first||second.parentNode!==detached||second.isConnected||first.parentNode!==null)throw 'detached parent';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn replace_child_named_errors_keep_order_and_promise_tail_without_coercing_non_nodes() {
    let (doc,_,page,outcome)=page("<div id=p title=attribute><span id=old></span></div>", &[r#"
var p=document.getElementById('p'),old=document.getElementById('old'),orphan=document.createElement('i'),text=document.createTextNode('text'),attr=p.getAttributeNode('title'),coercions=0;
function check(name,call){var caught='';try{call();}catch(e){caught=e.name;}if(caught!==name)throw 'wrong exception '+caught+' expected '+name;}
check('TypeError',function(){p.replaceChild();});check('TypeError',function(){p.replaceChild(orphan);});
check('TypeError',function(){p.replaceChild({toString:function(){coercions++;return 'node';}},old);});
check('TypeError',function(){p.replaceChild(p,null);});check('TypeError',function(){p.replaceChild.call({},orphan,old);});
check('HierarchyRequestError',function(){text.replaceChild(orphan,old);});
check('HierarchyRequestError',function(){p.replaceChild(p,orphan);});
check('HierarchyRequestError',function(){p.replaceChild(document,old);});check('HierarchyRequestError',function(){p.replaceChild(attr,old);});
check('NotFoundError',function(){p.replaceChild(attr,orphan);});check('HierarchyRequestError',function(){p.replaceChild(document,orphan);});
var detached=document.createElement('div'),detachedOld=document.createElement('b');detached.appendChild(detachedOld);
check('NotFoundError',function(){detached.replaceChild(document,orphan);});check('HierarchyRequestError',function(){detached.replaceChild(document,detachedOld);});
check('NotFoundError',function(){p.replaceChild(orphan,orphan);});check('NotFoundError',function(){p.replaceChild(orphan,attr);});
if(coercions||p.firstChild!==old||old.parentNode!==p||orphan.parentNode!==null)throw 'exception side effects';
var log='';Promise.resolve().then(function(){try{p.replaceChild(orphan,orphan);}catch(e){log+=e.name;}finally{log+=':finally';}});Promise.resolve().then(function(){log+=':tail';document.title=log;});
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"NotFoundError:finally:tail");
}

#[test]
fn replace_child_rejects_unsupported_native_subtrees_before_editing_either_parent() {
    // A submit INPUT has an editable value and remains outside the supported native move cases.
    for (old,new) in [("<span id=old>old</span>","<section id=incoming><input id=field type=submit value=original></section>"),
        ("<span id=old><input id=field type=submit value=original></span>","<section id=incoming>new</section>")] {
        let html=format!("<div id=p>{old}</div><div id=q>{new}</div>");
        let (mut doc,mut forms,mut page,outcome)=page(&html,&["document.getElementById('field').value='edited';"]);
        assert_eq!(outcome.errors,0,"{}",page.report());
        let node=|id:&str|doc.nodes.iter().enumerate().find_map(|(node,_)|(doc.attr(node,"id")==Some(id)).then_some(node)).unwrap();
        let (parent,source,old,incoming)=(node("p"),node("q"),node("old"),node("incoming"));
        let before=(doc.nodes[parent].children.clone(),doc.nodes[source].children.clone());
        {
            let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
            assert!(bridge.call(node_handle(parent),"replaceChild",&[Value::Host(node_handle(incoming)),Value::Host(node_handle(old))]).is_err());
        }
        assert_eq!((doc.nodes[parent].children.clone(),doc.nodes[source].children.clone()),before);
        let result=page.console_eval(&mut doc,&mut forms,"if(document.getElementById('field').value!=='edited'||document.getElementById('old').parentNode!==document.getElementById('p')||document.getElementById('incoming').parentNode!==document.getElementById('q'))throw 'partial replacement';");
        assert_eq!(result.errors,0,"{}",page.report());
    }
}

#[test]
fn replace_child_work_exhaustion_is_atomic_and_next_job_recovers() {
    let mut failures=0;
    let mut successes=0;
    for allowance in [0,1,4,16,64,256] {
        let (mut doc,mut forms,mut page,outcome)=page("<div id=p><span id=old>old</span></div><div id=q><section id=incoming>new</section></div>",&[]);
        assert_eq!(outcome.errors,0);
        let node=|id:&str|doc.nodes.iter().enumerate().find_map(|(node,_)|(doc.attr(node,"id")==Some(id)).then_some(node)).unwrap();
        let (parent,source,old,incoming)=(node("p"),node("q"),node("old"),node("incoming"));
        let before=(doc.nodes[parent].children.clone(),doc.nodes[source].children.clone());
        let commits=page.state.dom_commits;
        page.state.host_work=allowance;
        let result=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.call(node_handle(parent),"replaceChild",&[Value::Host(node_handle(incoming)),Value::Host(node_handle(old))]);
        if result.is_err() {
            failures+=1;
            assert_eq!((doc.nodes[parent].children.clone(),doc.nodes[source].children.clone()),before);
            assert_eq!(page.state.tree.parents[old],Some(parent));
            assert_eq!(page.state.tree.parents[incoming],Some(source));
            assert_eq!(page.state.dom_commits,commits);
            page.begin_job();
            let result=page.console_eval(&mut doc,&mut forms,"document.getElementById('p').replaceChild(document.getElementById('incoming'),document.getElementById('old'));");
            assert_eq!(result.errors,0,"{}",page.report());
        } else { successes+=1; }
        assert_eq!(doc.nodes[parent].children,[incoming]);
        assert!(doc.nodes[source].children.is_empty());
    }
    assert!(failures>0&&successes>0,"Exercise small failure profiles and ordinary completion separately");
}


#[test]
fn shared_lookup_exception_element_absence_and_accessor_controls() {
    let (_,_,page,outcome)=shared_probe(r#"
var p=document.getElementById('p'),calls=0;
if('offlineMissing' in p||typeof p.offlineMissing!=='undefined'||Object.getOwnPropertyDescriptor(p,'offlineMissing')!==undefined)throw 'absence';
p.offlineMissing?.(calls++);if(calls!==0)throw 'optional call';
Object.defineProperty(p,'probe',{get:function(){calls++;if(this!==p)throw 'receiver';return 17;}});
if(p.probe!==17||calls!==1||Object.getOwnPropertyDescriptor(p,'probe').get===undefined)throw 'accessor';
var marker={},same=false;try{Object.defineProperty(p,'failure',{get:function(){throw marker;}});p.failure;}catch(e){same=e===marker;}finally{calls++;}
if(!same||calls!==2)throw 'identity/finally';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn shared_lookup_exception_attr_absence_and_inherited_descriptor() {
    let (_,_,page,outcome)=shared_probe(r#"
var attr=document.getElementById('p').getAttributeNode('title'),calls=0;
if('offlineMissing' in attr||typeof attr.offlineMissing!=='undefined'||Object.getOwnPropertyDescriptor(attr,'offlineMissing')!==undefined)throw 'absence';
attr.offlineMissing?.(calls++);if(calls!==0||attr.name!=='title'||Object.hasOwn(attr,'name')||Object.getOwnPropertyDescriptor(attr,'name')!==undefined)throw 'Attr contract';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn shared_lookup_exception_created_event_absence_and_inherited_descriptor() {
    let (_,_,page,outcome)=shared_probe(r#"
var event=document.createEvent('Event'),calls=0;
if('offlineMissing' in event||typeof event.offlineMissing!=='undefined'||Object.getOwnPropertyDescriptor(event,'offlineMissing')!==undefined)throw 'absence';
event.offlineMissing?.(calls++);if(calls!==0||event.type!==''||event.isTrusted||Object.hasOwn(event,'type')||Object.getOwnPropertyDescriptor(event,'type')!==undefined)throw 'Event contract';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn shared_lookup_exception_native_tail_preserves_inherited_accessor_receiver_and_calls() {
    let (mut doc,mut forms,mut page,outcome)=shared_probe(r#"
var attr=document.getElementById('p').getAttributeNode('title'),event=document.createEvent('Event'),calls=0,marker={};
Object.defineProperty(Object.prototype,'offlineInherited',{configurable:true,get:function(){if(this!==attr&&this!==event)throw 'receiver';calls++;return 25;}});
Object.prototype.offlineCall=function(){return this;};
Object.defineProperty(Object.prototype,'offlineThrows',{get:function(){throw marker;}});
for(var item of [attr,event]){
 if(!('offlineInherited' in item)||Object.hasOwn(item,'offlineInherited')||item.offlineInherited!==25||item.offlineCall()!==item||item.offlineCall?.()!==item)throw 'prototype';
 var same=false;try{item.offlineThrows;}catch(e){same=e===marker;}if(!same)throw 'thrown identity';
}
if(calls!==2||!('normalize' in attr)||typeof attr.normalize!=='function')throw 'presence without accessor execution';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let Value::Host(event)=bridge.call(DOCUMENT,"createEvent",&[Value::String("Event".into())]).unwrap() else {panic!("native Event")};
    assert!(bridge.own_data(event,"isTrusted").is_err(),"Do not invent a data descriptor for the unforgeable accessor");
}

#[test]
fn shared_lookup_exception_dom_failure_rejects_normally_without_cancelling_tail() {
    let (doc,_,page,outcome)=shared_probe(r#"
var p=document.getElementById('p'),orphan=document.createElement('span'),log='';
Promise.resolve().then(function(){try{p.removeChild(orphan);}catch(e){log+=e.name;}finally{log+=':finally';}});
Promise.resolve().then(function(){log+=':tail';document.title=log;});
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"NotFoundError:finally:tail");
}

#[test]
fn shared_lookup_exception_invalid_attribute_name_is_catchable_without_mutation() {
    let (_,_,page,outcome)=shared_probe(r#"
var p=document.getElementById('p'),name='',finallyRan=false;
try{p.setAttribute('data bad','x');}catch(e){name=e.name;}finally{finallyRan=true;}
if(name!=='InvalidCharacterError'||!finallyRan||p.getAttribute('title')!=='present'||p.hasAttribute('data bad'))throw 'attribute error contract';
"#);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn name_namespace_attribute_values_are_exact_live_and_do_not_allocate_attr_identities() {
    let (mut doc,mut forms,mut page,outcome)=page("<div id=p title=old x:y=colon xmlns:x=raw null=n undefined=u></div>",&[r#"
var p=document.getElementById('p'),get=p.getAttributeNS;
if(get.length!==2||get!==p.getAttributeNS||get.call(p,null,'title')!=='old')throw 'metadata/receiver';
if(p.getAttributeNS('', 'title')!=='old'||p.getAttributeNS(undefined,'title')!=='old'||p.getAttributeNS(null,'TITLE')!==null)throw 'nullable/exact name';
if(p.getAttributeNS('http://www.w3.org/1999/xhtml','title')!==null||p.getAttributeNS(null,'missing')!==null||p.getAttributeNS(null,'bad name')!==null)throw 'namespace/miss';
if(p.getAttributeNS(null,'x:y')!=='colon'||p.getAttributeNS(null,'y')!==null||p.getAttributeNS('urn:x','y')!==null)throw 'raw colon';
if(p.getAttributeNS(null,'xmlns:x')!=='raw'||p.getAttributeNS('http://www.w3.org/2000/xmlns/','x')!==null)throw 'raw xmlns';
if(p.getAttributeNS(null,null)!=='n'||p.getAttributeNS(null,undefined)!=='u')throw 'local DOMString';
p.setAttribute('title','new');if(get.call(p,null,'title')!=='new')throw 'live value';document.title='namespace-values-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"namespace-values-ok");
    assert!(page.state.attributes.is_empty());
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("p")).unwrap();
    doc.nodes[node].namespace=crate::html::Namespace::UnknownForeign;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(bridge.call(node_handle(node),"getAttributeNS",&[Value::Null,Value::String("title".into())]).unwrap_err().contains("foreign"));
    bridge.doc.nodes[node].namespace=crate::html::Namespace::Html;
    let retained=bridge.state.retained;
    bridge.state.host_work=0;
    assert!(bridge.call(node_handle(node),"getAttributeNS",&[Value::Null,Value::String("title".into())]).is_err());
    bridge.state.host_work=bridge.state.limits.host_work;
    bridge.state.limits.host.string_bytes=1;
    assert!(bridge.call(node_handle(node),"getAttributeNS",&[Value::Null,Value::String("title".into())]).is_err());
    assert!(bridge.state.attributes.is_empty());
    assert_eq!(bridge.state.retained,retained);
    assert_eq!(bridge.doc.attr(node,"title"),Some("new"));
}

#[test]
fn name_namespace_attribute_value_conversion_is_ordered_nullable_and_reentrant() {
    let (_,_,page,outcome)=page("<div id=p title=old></div>",&[r#"
var p=document.getElementById('p'),log='',marker={};
var ns={toString:function(){log+='N';return '';}};
var name={toString:function(){log+='L';p.setAttribute('title','current');return 'title';}};
if(p.getAttributeNS(ns,name)!=='current'||log!=='NL')throw 'ordered live conversion';
log='';if(p.getAttributeNS({toString:function(){log+='N';return null;}},name)!==null||log!=='NL')throw 'object null string';
log='';var count=0;try{p.getAttributeNS(ns);}catch(e){if(e.name==='TypeError')count++;}
try{p.getAttributeNS(Symbol(),name);}catch(e){if(e.name==='TypeError')count++;}
if(log!=='')throw 'arity/namespace before name';
try{p.getAttributeNS('urn:x',Symbol());}catch(e){if(e.name==='TypeError')count++;}
var same=false;try{p.getAttributeNS(null,{toString:function(){throw marker;}});}catch(e){same=e===marker;}if(!same)throw 'throw identity';
for(var bad of [{},window,document,p.style,p.dataset,document.createTextNode('x')])try{p.getAttributeNS.call(bad,ns,name);}catch(e){if(e.name==='TypeError')count++;}
if(count!==9||log!==''||p.getAttribute('title')!=='current')throw 'brand/coercion';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn name_namespace_live_name_nodelists_keep_tree_order_methods_and_static_snapshots() {
    let (_,_,page,outcome)=page("<div id=root><span id=a name=probe></span><span id=b name=probe></span><span name=Probe></span><span name=''></span><span></span><i id=probe></i></div><div id=outside><span id=c name=probe></span></div><svg name=probe></svg>",&[r#"
var get=document.getElementsByName,root=document.getElementById('root'),outside=document.getElementById('outside');
var a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');
var list=get.call(document,'probe'),snapshot=document.querySelectorAll('span[name=probe]');
if(get.length!==1||get!==document.getElementsByName||list.length!==3||list[0]!==a||list[1]!==b||list[2]!==c)throw 'initial order';
if(list.item(1)!==b||list.item(99)!==null||list[99]!==undefined||list.namedItem!==undefined||list.a!==undefined||'namedItem' in list)throw 'NodeList surface';
if(document.getElementsByName('').length!==1||document.getElementsByName('Probe').length!==1||document.getElementsByName('PROBE').length!==0)throw 'exact name';
if(Object.keys(list).join(',')!=='0,1,2'||Object.getOwnPropertyNames(list).join(',')!=='0,1,2')throw 'indices';
outside.appendChild(b);if(list[1]!==c||list[2]!==b||snapshot[1]!==b||snapshot.length!==3)throw 'move/live versus snapshot';
root.removeChild(a);if(list.length!==2||list[0]!==c||list[1]!==b||snapshot[0]!==a)throw 'remove';
var hits=0,context={};list.forEach(function(node,index,owner){if(this!==context||owner!==list||index!==0||node!==c)throw 'callback';hits++;outside.removeChild(c);outside.removeChild(b);},context);
if(hits!==1||list.length!==0||Object.keys(list).length!==0)throw 'removed live indices must skip';
root.appendChild(a);if(list.length!==1||list[0]!==a||Array.prototype.slice.call(list)[0]!==a)throw 'reinsert/arraylike';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
}

#[test]
fn name_namespace_name_list_conversion_brand_and_retention_failures_are_atomic() {
    let (mut doc,mut forms,mut page,outcome)=page("<span name=null></span><span name=undefined></span>",&[r#"
if(document.getElementsByName(null).length!==1||document.getElementsByName(undefined).length!==1)throw 'DOMString';
var converted=0,arg={toString:function(){converted++;return 'null';}},get=document.getElementsByName,count=0;
for(var bad of [{},window,document.body,document.createTextNode('x')])try{get.call(bad,arg);}catch(e){if(e.name==='TypeError')count++;}
try{get();}catch(e){if(e.name==='TypeError')count++;}try{get(Symbol());}catch(e){if(e.name==='TypeError')count++;}
if(count!==6||converted!==0||document.getElementsByName(arg).length!==1||converted!==1)throw 'brand/arity/conversion';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(page.state.node_list_references,0);
    let before=(page.state.node_lists.len(),page.state.retained);
    page.state.retained_limit=page.state.retained;
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(bridge.call(DOCUMENT,"getElementsByName",&[Value::String("unretained".into())]).is_err());
    assert_eq!(before,(bridge.state.node_lists.len(),bridge.state.retained));
}

#[test]
fn native_element_expandos_are_real_ordinary_properties_with_native_accessor_receivers() {
    let (doc,_,page,outcome)=page("<div id=p></div><form id=f><input id=field name=customProbe></form>",&[r#"
var p=document.getElementById('p');if(p.customProbe!==undefined||'customProbe' in p||Object.hasOwn(p,'customProbe'))throw 'real absent key';
p.one=1;p.only=2;p.ariana=3;p.onCustomProbe=4;p.srcdoc={ordinary:true};
if(p.one!==1||p.only!==2||p.ariana!==3||p.onCustomProbe!==4||p.srcdoc.ordinary!==true||p.getAttribute('srcdoc')!==null)throw 'arbitrary keys/tag-specific native metadata';
delete p.one;delete p.only;delete p.ariana;delete p.onCustomProbe;delete p.srcdoc;
p.customProbe=17;if(p.customProbe!==17||!('customProbe' in p)||!Object.hasOwn(p,'customProbe'))throw 'stored key';
Object.defineProperty(p,'hiddenProbe',{value:9,writable:true,configurable:true});
var seen=null;Object.defineProperty(p,'accessorProbe',{get:function(){seen=this;return this.customProbe+1;},set:function(v){seen=this;this.customProbe=v;},enumerable:true,configurable:true});
if(p.accessorProbe!==18||seen!==p)throw 'getter receiver';p.accessorProbe=30;if(p.customProbe!==30||seen!==p)throw 'setter receiver';
var d=Object.getOwnPropertyDescriptor(p,'accessorProbe');if(typeof d.get!=='function'||!d.enumerable||!d.configurable)throw 'descriptor';
if(Object.keys(p).join(',')!=='customProbe,accessorProbe'||Object.getOwnPropertyNames(p).join(',')!=='customProbe,hiddenProbe,accessorProbe')throw 'own keys/flags';
var key=Symbol('own');p[key]=p;var symbols=Object.getOwnPropertySymbols(p);if(symbols.length!==1||symbols[0]!==key||p[key]!==p)throw 'symbols';
var copy=Object.assign({},p);if(copy.customProbe!==30||copy.accessorProbe!==31||copy[key]!==p||Object.hasOwn(copy,'hiddenProbe'))throw 'copy real own values';
if(!delete p.customProbe||Object.hasOwn(p,'customProbe')||p.customProbe!==undefined)throw 'deletion';
Object.defineProperties(p,{methodProbe:{value:function(){return this===p;},enumerable:true}});if(!p.methodProbe()||p.methodProbe?.()!==true)throw 'ordinary method this';
if(p.id!=='p'||p.localName!=='div'||Object.hasOwn(p,'id')||document.getElementById('f').customProbe!==document.getElementById('field'))throw 'native namespace/named boundary';
document.title='element-expando-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"element-expando-ok");
}

#[test]
fn html_label_ordinary_properties_survive_connection_and_native_input_edits() {
    let (doc, _, page, outcome) = page("<html><body><label id=parsed><input id=editor value=seed></label></body></html>", &[r#"
        var parsed = document.getElementById('parsed'), editor = document.getElementById('editor');
        var created = document.createElement('label'), key = Symbol('label state');
        editor.value = 'edited';
        function check(label) {
            if (label._patchState !== undefined || '_patchState' in label || Object.hasOwn(label, '_patchState'))
                throw 'missing label property';
            if (label.inheritedLabel !== label || Object.hasOwn(label, 'inheritedLabel'))
                throw 'inherited receiver';
            label._patchState = editor;
            Object.defineProperty(label, key, { get: function () { return this._patchState; } });
            if (label[key] !== editor || !Object.hasOwn(label, '_patchState') || label.getAttribute('_patchState') !== null)
                throw 'own state or accessor receiver';
            if (!delete label._patchState || label[key] !== undefined) throw 'deletion';
        }
        Object.defineProperty(HTMLElement.prototype, 'inheritedLabel', {
            get: function () { return this; }, configurable: true
        });
        check(parsed);
        check(created);
        delete HTMLElement.prototype.inheritedLabel;
        created.localState = parsed;
        document.body.appendChild(created);
        if (created.localState !== parsed || created.ownerDocument !== document || editor.parentNode !== parsed ||
            editor.value !== 'edited' || editor.defaultValue !== 'seed') throw 'connection changed state';
        document.title = 'Label properties retained';
    "#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "Label properties retained");
}

#[test]
fn html_label_native_members_stay_reserved() {
    let (mut doc, mut forms, mut scripts, initial) = page(
        "<label id=lbl for=field></label><input id=field>",
        &["var label = document.getElementById('lbl');"],
    );
    assert_eq!(initial.errors, 0, "{}", scripts.report());
    // Canonical label accessors leave inert, nonce and spellcheck reserved until implemented.
    for member in ["inert", "nonce", "spellcheck"] {
        let source = format!("Object.defineProperty(label, '{member}', {{value: 1}});");
        let result = scripts.console_eval(&mut doc, &mut forms, &source);
        assert_eq!(result.errors, 1, "{member}: {}", scripts.report());
    }
    let retained = scripts.console_eval(&mut doc, &mut forms, r#"
        if (Object.hasOwn(label,'control') || Object.hasOwn(label,'form') || Object.hasOwn(label,'htmlFor') || Object.hasOwn(label,'inert') || Object.hasOwn(label,'nonce') || Object.hasOwn(label,'spellcheck') || label.form !== null || label.control !== document.getElementById('field') || label.htmlFor !== 'field' ||
            label.getAttribute('for') !== 'field') throw 'label association changed';
    "#);
    assert_eq!(retained.errors, 0, "{}", scripts.report());
}

#[test]
fn native_element_expandos_first_descriptor_symbol_and_inspection_paths_agree() {
    let (doc,_,page,outcome)=page("<div id=a></div><span id=b></span><p id=c></p><img id=i>",&[r#"
var a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');
Object.defineProperty(a,'firstProbe',{value:1,enumerable:true,configurable:true});
var key=Symbol('first');Object.defineProperty(b,key,{value:2,enumerable:true,configurable:true});
if(Object.keys(c).length||Object.getOwnPropertyNames(c).length||Object.getOwnPropertySymbols(c).length)throw 'empty own inspection';
Object.defineProperties(c,{});Object.assign(c,{assignedProbe:3});
if(a.firstProbe!==1||Object.getOwnPropertyDescriptor(a,'firstProbe').value!==1||b[key]!==2||c.assignedProbe!==3)throw 'first operation paths';
var i=document.getElementById('i');Image.prototype.imageProbe=7;if(i.imageProbe!==7||Object.hasOwn(i,'imageProbe'))throw 'canonical prototype inheritance';delete Image.prototype.imageProbe;
document.title='element-expando-first-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"element-expando-first-ok");
}

#[test]
fn ordinary_properties_follow_verified_html_elements_and_keep_native_control_slots() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<div id=known></div><table id=table></table><form id=f><input name=probe id=field>+         <select id=select name=choice><option value=a>one</option><option value=b selected>two</option></select></form>",
        &[r#"
var div=document.getElementById('known'), table=document.getElementById('table');
var select=document.getElementById('select'), form=document.getElementById('f'), field=document.getElementById('field');
var receivers=[div,table,select], token=Symbol('ordinary property');
Object.defineProperty(HTMLElement.prototype,'receiverProbe',{configurable:true,get:function(){return this.id;}});
for(var i=0;i<receivers.length;i++){
    var e=receivers[i];
    if(e.missingProbe!==undefined||'missingProbe' in e||e.receiverProbe!==e.id||
        Object.hasOwn(e,'receiverProbe'))throw 'ordinary inheritance';
    Object.defineProperty(e,'ownProbe',{value:e.id,writable:true,enumerable:true,configurable:true});
    e[token]=i;
    if(Object.getOwnPropertyDescriptor(e,'ownProbe').value!==e.id||e[token]!==i||
        Object.getOwnPropertySymbols(e)[0]!==token)throw 'ordinary descriptors';
    delete e.ownProbe;delete e[token];
    if(e.ownProbe!==undefined||Object.hasOwn(e,token))throw 'ordinary deletion';
}
if(select.value!=='b'||select.selectedIndex!==1||select.form!==form||form.probe!==field||
    Object.hasOwn(select,'selectedIndex'))throw 'native control readback';
select.selectedIndex=0;
if(select.value!=='a'||select.selectedIndex!==0||form.probe!==field)throw 'native control write';
delete HTMLElement.prototype.receiverProbe;
document.title='verified-html-ordinary-properties';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-html-ordinary-properties");
    for operation in [
        "Object.defineProperty(select,'selectedIndex',{value:99});",
        "Object.defineProperty(select,'0',{value:99});",
    ] {
        let denied = page.console_eval(&mut doc, &mut forms, operation);
        assert_eq!(denied.errors, 1, "{}", page.report());
    }
    let restored = page.console_eval(&mut doc, &mut forms,
        "if(select.selectedIndex!==0||select.value!=='a'||form.probe!==field)throw 'native slots changed';");
    assert_eq!(restored.errors, 0, "{}", page.report());
}

#[test]
fn unsupported_element_lookup_reports_native_receiver_provenance() {
    let (_, _, page, outcome) = page("<table id=secret-marker></table>",
        &["document.getElementById('secret-marker').getBoxQuads;"]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    let report = page.report();
    let line = report.lines().find(|line| line.contains("Element property \"getBoxQuads\" is not supported"))
        .expect("native lookup diagnostic");
    assert!(line.contains("tag=table"), "{line}");
    assert!(line.contains("namespace=http://www.w3.org/1999/xhtml"), "{line}");
    assert!(line.contains("owner=main document"), "{line}");
    assert!(!line.contains("secret-marker"), "{line}");
}

#[test]
fn html_template_content_preserves_inert_identity_tree_and_ownership() {
    let mut doc = crate::html::parse(concat!(
        "<html><head></head><body>",
        "<template id=outer><strong id=hidden>Hidden</strong><template id=nested><em>Deep</em></template></template>",
        "<template id=second>More</template><p id=visible>Visible</p></body></html>"
    ));
    doc.scripting_enabled = true;
    let mut page = Page::for_test();
    let mut forms = Forms::new(&doc);
    let result = page.start(&mut doc, &mut forms, &[Script { node: 0, label: "Original HTML template fixture".into(), source: r#"
var T = HTMLTemplateElement, P = T.prototype, getter = Object.getOwnPropertyDescriptor(P,'content').get;
var outer = document.getElementById('outer'), second = document.getElementById('second');
var f = getter.call(outer), owner = f.ownerDocument, child = f.firstChild, nested = f.lastChild;
if (outer !== document.querySelector('template') || !(outer instanceof T) ||
    Object.getPrototypeOf(outer) !== P || Object.getPrototypeOf(P) !== HTMLElement.prototype ||
    outer.content !== f || Object.hasOwn(outer,'content')) throw 'HTMLTemplateElement interface';
if (!(f instanceof DocumentFragment) || f.nodeType !== 11 || f.nodeName !== '#document-fragment' ||
    f.parentNode !== null || f.isConnected || owner === document || !(owner instanceof Document) ||
    owner.defaultView !== null || owner.documentElement !== null || owner.ownerDocument !== null ||
    second.content.ownerDocument !== owner) throw 'separate shared inert owner';
if (outer.firstChild !== null || outer.lastChild !== null || outer.childNodes.length !== 0 ||
    outer.textContent !== '' || document.getElementById('hidden') !== null ||
    document.querySelector('#nested') !== null || document.getElementById('visible').textContent !== 'Visible')
    throw 'ordinary DOM excludes template-only children';
if (!(f.childNodes instanceof NodeList) || f.childNodes !== f.childNodes || f.childNodes.length !== 2 ||
    f.childNodes.item(0) !== child || f.childNodes[1] !== nested || child.parentNode !== f ||
    child.nextSibling !== nested || nested.previousSibling !== child || child.id !== 'hidden' ||
    child.ownerDocument !== owner || child.firstChild.nodeType !== 3 || child.textContent !== 'Hidden' ||
    f.textContent !== 'Hidden') throw 'genuine ordered fragment content';
if (!(nested instanceof T) || nested.firstChild !== null || nested.childNodes.length !== 0 ||
    nested.content.ownerDocument !== owner || nested.content.firstChild.localName !== 'em' ||
    nested.content.textContent !== 'Deep') throw 'nested template content boundary';
var fresh = document.createElement('template'), pending = document.createElement('template');
var separate = document.implementation.createHTMLDocument('separate');
var foreignCreated = separate.createElement('template');
if (!(fresh instanceof T) || fresh.content === f || fresh.content.childNodes.length !== 0 ||
    fresh.content.ownerDocument !== owner || !(foreignCreated instanceof T) ||
    foreignCreated.content.ownerDocument === owner || foreignCreated.content.ownerDocument === separate ||
    foreignCreated.content.ownerDocument.defaultView !== null) throw 'actual document-specific owner';
var rejected = 0;
try { new T(); } catch (e) { if (e instanceof TypeError) rejected++; else throw e; }
try { getter.call(document.body); } catch (e) { if (e instanceof TypeError) rejected++; else throw e; }
if (rejected !== 2) throw 'receiver and constructor checks';
document.title = 'verified-native-template-content';
"#.into() }]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-native-template-content");
    assert!(!page.has_host_tasks());
    let commits = page.state.dom_commits;
    let refused = page.console_eval(&mut doc, &mut forms, "document.body.appendChild(f);");
    assert_eq!(refused.errors, 1);
    assert_eq!(page.state.dom_commits, commits);
    let verified = page.console_eval(&mut doc, &mut forms,
        "if (f.firstChild !== child || child.parentNode !== f || child.isConnected || document.getElementById('hidden') !== null) throw 'inert ownership changed';");
    assert_eq!(verified.errors, 0, "{}", page.report());
    let limit = page.state.limits.host.extra_nodes;
    let retained = page.state.retained;
    page.state.limits.host.extra_nodes = 0;
    let refused = page.console_eval(&mut doc, &mut forms, "pending.content;");
    assert_eq!(refused.errors, 1);
    assert!(page.report().contains("Inert DOM identity limit reached"));
    assert_eq!(page.state.retained, retained);
    page.state.limits.host.extra_nodes = limit;
    let verified = page.console_eval(&mut doc, &mut forms,
        "if (pending.content.ownerDocument !== owner || pending.content === f || outer.content !== f) throw 'atomic retry';");
    assert_eq!(verified.errors, 0, "{}", page.report());
}

#[test]
fn template_content_clone_preserves_fresh_inert_topology_and_nested_payloads() {
    let (mut doc, mut forms, mut page, outcome) = page(concat!(
        "<html><head></head><body>",
        "<template id=fixture><div id=hidden><b>Token</b></div><template><em>Nested</em></template></template>",
        "<p id=visible>Visible</p></body></html>"
    ), &[r#"
var template=document.getElementById('fixture'), input=template.content, owner=input.ownerDocument;
var original=input.firstChild, nested=input.lastChild, clone=Node.prototype.cloneNode;
var empty=input.cloneNode(), copied=input.cloneNode(true);
if(empty===input||empty===copied||!(empty instanceof DocumentFragment)||empty.ownerDocument!==owner||
    empty.firstChild!==null||empty.childNodes.length!==0||copied===input||copied.ownerDocument!==owner||
    copied.isConnected||copied.parentNode!==null||copied.childNodes.length!==2||
    copied.firstChild===original||copied.firstChild.ownerDocument!==owner||
    copied.firstChild.parentNode!==copied||copied.firstChild.firstChild===original.firstChild||
    copied.firstChild.id!=='hidden'||copied.firstChild.textContent!=='Token'||
    copied.lastChild===nested||copied.firstChild.nextSibling!==copied.lastChild)
    throw 'actual detached clone topology and owners';
var copiedNested=copied.lastChild, shallow=clone.call(nested,false), deep=clone.call(nested,true);
if(!(copiedNested instanceof HTMLTemplateElement)||copiedNested.childNodes.length!==0||
    copiedNested.content===nested.content||copiedNested.content.ownerDocument!==owner||
    copiedNested.content.firstChild===nested.content.firstChild||copiedNested.content.textContent!=='Nested'||
    shallow.content.childNodes.length!==0||deep.content.textContent!=='Nested'||
    deep.content.firstChild===nested.content.firstChild||shallow.content.ownerDocument!==owner||
    shallow.cloneNode(true).content.childNodes.length!==0)throw 'shallow and deep nested template contents';
var detachedOriginal=owner.createTextNode('Independent');
var detachedCopy=detachedOriginal.cloneNode(true);
if(detachedCopy===detachedOriginal||detachedCopy.data!=='Independent'||detachedCopy.ownerDocument!==owner||
    document.getElementById('hidden')!==null||document.getElementById('visible').textContent!=='Visible')
    throw 'factory-created inert leaf and unaffected visible document';
var rejected=false;
try { clone.call({}); } catch(e) { if(e instanceof TypeError) rejected=true; else throw e; }
if(!rejected)throw 'clone receiver branding';
document.title='verified-inert-template-clone';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-inert-template-clone");
    let retained = page.state.retained;
    let limit = page.state.limits.host.extra_nodes;
    page.state.limits.host.extra_nodes = 0;
    let failure = page.console_eval(&mut doc, &mut forms, "input.cloneNode(true);");
    assert_eq!(failure.errors, 1);
    assert!(page.report().contains("Inert DOM identity limit reached during cloning"));
    assert_eq!(page.state.retained, retained);
    page.state.limits.host.extra_nodes = limit;
    let commits = page.state.dom_commits;
    let denied = page.console_eval(&mut doc, &mut forms, "document.body.appendChild(copied);");
    assert_eq!(denied.errors, 1);
    assert_eq!(page.state.dom_commits, commits);
    let recovered = page.console_eval(&mut doc, &mut forms,
        "var retry=input.cloneNode(true);if (retry===copied||retry.firstChild===copied.firstChild||copied.parentNode!==null||input.firstChild!==original) throw 'failed clone or adoption changed identities';");
    assert_eq!(recovered.errors, 0, "{}", page.report());
    assert!(!page.has_host_tasks());
}

#[test]
fn document_import_node_copies_checked_template_content_into_the_real_tree() {
    let (mut doc, mut forms, mut page, outcome) = page(concat!(
        "<html><head></head><body>",
        "<template id=ordinary><section id=imported class=payload data-key=kept>",
        "<a id=import-link href=#destination>Imported link</a></section></template>",
        "<template id=protected><div>prefix</div><img src=https://unrequested.test/forbidden.png srcset='forbidden.png 1x'></template>",
        "<template id=handler><div onclick=\"throw 1\">inert handler</div></template>",
        "<main id=live></main><p id=destination>Destination</p></body></html>"
    ), &[r#"
var T=document.getElementById('ordinary'), input=T.content, inner=input.firstChild, inertOwner=input.ownerDocument;
var mount=document.getElementById('live'), method=Document.prototype.importNode;
if(document.importNode!==method||method.name!=='importNode'||method.length!==1||Object.hasOwn(document,'importNode'))
    throw 'canonical Document import method';
var empty=method.call(document,input), deep=document.importNode(input,true);
var errorCount=0, getterCount=0;
var fromDictionary=document.importNode(input,{get selfOnly(){getterCount++;return false;}});
var shallowElement=document.importNode(inner,{selfOnly:true});
if(getterCount!==1||!(empty instanceof DocumentFragment)||empty.ownerDocument!==document||empty.childNodes.length!==0||
    deep===input||deep.ownerDocument!==document||deep.parentNode!==null||deep.isConnected||
    deep.firstChild===inner||deep.firstChild.ownerDocument!==document||deep.firstChild.parentNode!==deep||
    fromDictionary.firstChild===deep.firstChild||fromDictionary.textContent!=='Imported link'||
    shallowElement.tagName!=='SECTION'||shallowElement.childNodes.length!==0||shallowElement.getAttribute('data-key')!=='kept')
    throw 'shallow and deep import ownership and identity';
if(deep.firstChild.getAttribute('class')!=='payload'||deep.firstChild.getAttribute('data-key')!=='kept'||
    deep.firstChild.firstChild.getAttribute('href')!=='#destination'||document.getElementById('imported')!==null||
    inner.ownerDocument!==inertOwner||inner.parentNode!==input)throw 'copy preserves raw attributes and source document';
try { method.call({},input); } catch(e) { if(e instanceof TypeError)errorCount++;else throw e; }
try { method.call(document,{}); } catch(e) { if(e instanceof TypeError)errorCount++;else throw e; }
try { method.call(document,inertOwner); } catch(e) { if(e instanceof DOMException&&e.name==='NotSupportedError')errorCount++;else throw e; }
try { method.call(document,input,{customElementRegistry:{}}); } catch(e) { if(e instanceof TypeError)errorCount++;else throw e; }
if(errorCount!==4)throw 'real receiver, source and options checks';
mount.appendChild(deep);
var visible=mount.firstChild, link=visible.firstChild;
if(visible!==document.getElementById('imported')||link!==document.getElementById('import-link')||
    link.nodeName!=='A'||link.textContent!=='Imported link'||!link.isConnected||link.ownerDocument!==document||
    deep.childNodes.length!==0||input.firstChild!==inner||inner.isConnected)throw 'real live fragment insertion';
var second=document.importNode(link,true);
if(second===link||second.isConnected||second.firstChild===link.firstChild||second.ownerDocument!==document)
    throw 'same-document native import copy';
document.title='verified-template-import';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-template-import");
    let count = doc.nodes.len();
    let commits = page.state.dom_commits;
    for source in [
        "document.importNode(document.getElementById('protected').content,true);",
        "document.importNode(document.getElementById('handler').content,true);",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}: {}", page.report());
        assert_eq!(doc.nodes.len(), count);
        assert_eq!(page.state.dom_commits, commits);
        assert!(!page.has_host_tasks());
    }
    let out = page.console_eval(&mut doc, &mut forms,
        "if (mount.firstChild!==visible||visible.firstChild!==link||input.firstChild!==inner||inner.isConnected) throw 'failed import changed real topology';");
    assert_eq!(out.errors, 0, "{}", page.report());
}

#[test]
fn template_inner_html_replaces_inert_children_without_activating_resources() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head></head><body><template id=starter><u>Original</u><template><b>Nested</b></template></template><main id=mount></main></body></html>",
        &[r##"
var parsed=document.getElementById('starter');
if(parsed.innerHTML!=='<u>Original</u><template><b>Nested</b></template>')throw 'serialize retained template topology';
var template=document.createElement('template'), inner=template.content, list=inner.childNodes, calls=0;
var source={toString:function(){calls++;return '<div id=item data-key="x&amp;y"><b>First &amp; second</b></div>'; }};
template.innerHTML=source;
var previous=inner.firstChild;
if(calls!==1||template.content!==inner||inner.childNodes!==list||list.length!==1||list[0]!==previous||
    previous.ownerDocument!==inner.ownerDocument||previous.parentNode!==inner||
    inner.textContent!=='First & second'||template.innerHTML!=='<div id="item" data-key="x&amp;y"><b>First &amp; second</b></div>'||
    template.childNodes.length!==0||template.textContent!==''||document.getElementById('item')!==null)
    throw 'native parsed template topology and normalized serialization';
var descriptor=Object.getOwnPropertyDescriptor(Element.prototype,'innerHTML');
descriptor.set.call(template,'<a href="#end" id="fresh">Visible</a>');
var anchor=inner.firstChild;
if(anchor===previous||previous.parentNode!==null||previous.firstChild.textContent!=='First & second'||
    descriptor.get.call(template)!=='<a href="#end" id="fresh">Visible</a>'||list[0]!==anchor)
    throw 'stable content and detached old nodes';
var copy=document.importNode(inner,true);
document.getElementById('mount').appendChild(copy);
if(document.getElementById('fresh').textContent!=='Visible'||anchor.isConnected||anchor.parentNode!==inner)
    throw 'native import continues from generated content';
template.innerHTML='<script id="latent">document.title="forbidden";</script><img src="https://unrequested.test/inert.png">';
if(template.content!==inner||inner.childNodes.length!==2||inner.firstChild.localName!=='script'||
    inner.lastChild.localName!=='img'||document.getElementById('latent')!==null)throw 'inert resource ownership';
var savedFirst=inner.firstChild,savedMarkup=template.innerHTML;
var separate=document.implementation.createHTMLDocument('other'), separateTemplate=separate.createElement('template');
separateTemplate.innerHTML='<i>owned</i>';
if(separateTemplate.innerHTML!=='<i>owned</i>'||separateTemplate.content.ownerDocument===inner.ownerDocument||
    separateTemplate.content.firstChild.textContent!=='owned')throw 'inert factory-created template';
document.title='verified-template-inner-html';
"##]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-template-inner-html");
    assert!(!page.has_host_tasks());
    let before = page.state.retained;
    let live_nodes = doc.nodes.len();
    let refused = page.console_eval(&mut doc, &mut forms, "template.innerHTML='<table><tr><td>unsupported structure';");
    assert_eq!(refused.errors, 1);
    assert_eq!(page.state.retained, before);
    assert_eq!(doc.nodes.len(), live_nodes);
    let verified = page.console_eval(&mut doc, &mut forms, r#"
if(template.content!==inner||inner.firstChild!==savedFirst||template.innerHTML!==savedMarkup||
    document.getElementById('fresh').textContent!=='Visible')throw 'failed parse changed retained content';
template.innerHTML=null;
if(template.content!==inner||list.length!==0||template.innerHTML!==''||savedFirst.parentNode!==null||
    savedFirst.textContent!=='document.title="forbidden";')throw 'null and retained identity';
"#);
    assert_eq!(verified.errors, 0, "{}", page.report());
    assert!(!page.has_host_tasks());
}

#[test]
fn template_inner_html_rolls_back_an_unmaterialized_fragment_on_capacity_failure() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head></head><body></body></html>",
        &["var fresh=document.createElement('template');"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let retained = page.state.retained;
    let backing = page.state.inert_documents.backing_bytes();
    let limit = page.state.limits.host.extra_nodes;
    page.state.limits.host.extra_nodes = 3;
    let failure = page.console_eval(&mut doc, &mut forms, "fresh.innerHTML='<b>no partial tree</b>';");
    assert_eq!(failure.errors, 1);
    assert!(page.report().contains("Inert DOM identity limit reached during template replacement"));
    // Failed unpublished payload is gone, but reusable vector backing stays
    // allocated and paid. The subsequent real content read must remain empty.
    let added = page.state.inert_documents.backing_bytes() - backing;
    assert!(added > 0);
    assert_eq!(page.state.retained, retained + added);
    page.state.limits.host.extra_nodes = limit;
    let verified = page.console_eval(&mut doc, &mut forms,
        "if(fresh.content.firstChild!==null)throw 'partial child';fresh.innerHTML='<b>retry</b>';if(fresh.content.textContent!=='retry')throw 'failed retry';");
    assert_eq!(verified.errors, 0, "{}", page.report());
}

#[test]
fn create_html_document_builds_an_independent_inert_native_tree() {
    let (doc, _, page, outcome) = page("<html><head><title>Main</title></head><body></body></html>", &[r#"
var impl=document.implementation;
if(impl!==document.implementation||!(impl instanceof DOMImplementation)||!impl.hasFeature()||
    Object.hasOwn(document,'implementation')||
    typeof Object.getOwnPropertyDescriptor(Document.prototype,'implementation').get!=='function')throw 'implementation';
var markup='  Starter <b> & word  ';
var other=impl.createHTMLDocument(markup);
if(other===document||!(other instanceof Document)||other.defaultView!==null||other.contentType!=='text/html'||
    other.URL!=='about:blank'||other.title!=='Starter <b> & word'||other.implementation===impl)throw 'independent document';
if(!(other.doctype instanceof DocumentType)||other.doctype.name!=='html'||other.doctype.nodeType!==10||
    other.firstChild!==other.doctype||other.doctype.nextSibling!==other.documentElement||
    other.lastChild!==other.documentElement||other.documentElement.ownerDocument!==other)throw 'structure';
var title=other.head.firstChild, value=title.firstChild;
if(title.localName!=='title'||value.data!==markup||value.nodeName!=='#text'||value.ownerDocument!==other||
    value.parentNode!==title||!value.isConnected||other.body.parentNode!==other.documentElement||
    other.head.nextSibling!==other.body||other.body.previousSibling!==other.head)throw 'literal title';
var empty=other.implementation.createHTMLDocument(undefined);
if(empty===other||empty.title!==''||empty.head.firstChild!==null)throw 'omitted title';
var raw=other.createElement('DIV'), named=other.createElementNS('http://www.w3.org/1999/xhtml','SPAN');
var text=other.createTextNode('<>&');
if(raw.ownerDocument!==other||raw.localName!=='div'||named.localName!=='SPAN'||named.ownerDocument!==other||
    text.ownerDocument!==other||text.textContent!=='<>&'||raw.isConnected||raw.parentNode!==null||
    document.body.firstChild!==null||document.title!=='Main')throw 'detached factory ownership';
var guarded=0;
try {new DOMImplementation();} catch(e){if(e instanceof TypeError)guarded++;}
try {impl.createHTMLDocument.call({});} catch(e){if(e instanceof TypeError)guarded++;}
try {document.body.appendChild(other.createElement('script'));} catch(e){if(e instanceof DOMException&&e.name==='NotSupportedError')guarded++;}
if(guarded!==3||document.body.firstChild!==null||other.body.firstChild!==null)throw 'native isolation';
document.title='inert-html-document-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "inert-html-document-ok");
}

#[test]
fn wide_tree_walker_keeps_sibling_reads_within_one_job_and_tracks_reordering() {
    let mut doc = crate::html::parse(&format!("<div id=root>{}</div>", "<wbr>".repeat(4096)));
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::Limits {
        host_work: 200_000,
        startup_host_work: 200_000,
        ..crate::javascript::limits::CAPACITY_REGRESSION
    });
    let script = Script { node: 0, label: "wide sibling fixture".into(), source: r#"
var root=document.getElementById('root'), first=root.firstChild, second=first.nextSibling;
var walker=document.createTreeWalker(root,NodeFilter.SHOW_ELEMENT), count=0;
while(walker.nextNode())count++;
if(count!==4096||walker.currentNode!==root.lastChild)throw 'wide traversal';
var last=walker.currentNode;
root.insertBefore(last,first);
if(last.previousSibling!==null||last.nextSibling!==first||first.previousSibling!==last||
    first.nextSibling!==second||walker.nextNode()!==first)throw 'sibling positions after reorder';
root.removeChild(last);
if(last.nextSibling!==null||first.previousSibling!==null||first.nextSibling!==second)throw 'detachment';
document.title='wide-dom-sibling-ok';
"#.into() };
    let outcome = page.start(&mut doc, &mut forms, &[script]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "wide-dom-sibling-ok");
}

#[test]
fn tree_walker_masks_preserve_live_cursor_and_native_tree_boundaries() {
    let (doc, _, page, outcome) = page(
        "<div id=root><p id=one>A<b id=nested>B</b></p><p id=two>C</p></div>",
        &[r#"
var root=document.getElementById('root'), one=document.getElementById('one');
var two=document.getElementById('two'), nested=document.getElementById('nested');
var w=document.createTreeWalker(root,NodeFilter.SHOW_ELEMENT,null,false);
if(!(w instanceof TreeWalker)||w.root!==root||w.currentNode!==root||w.whatToShow!==1||w.filter!==null||
    Object.hasOwn(document,'createTreeWalker')||Document.prototype.createTreeWalker.length!==1)throw 'interfaces';
var all=Object.getOwnPropertyDescriptor(NodeFilter,'SHOW_ALL');
if(all.value!==4294967295||all.writable||!all.enumerable||all.configurable||
    typeof NodeFilter!=='function'||Object.hasOwn(NodeFilter,'prototype'))throw 'constants';
if(w.parentNode()!==null||w.firstChild()!==one||w.firstChild()!==nested||
    w.nextSibling()!==null||w.parentNode()!==one||w.nextSibling()!==two||
    w.previousNode()!==nested||w.previousNode()!==one||w.previousNode()!==root)throw 'element traversal';
w.currentNode=two;
var late=document.createElement('i');root.appendChild(late);
if(w.nextNode()!==late||w.nextNode()!==null||w.currentNode!==late)throw 'live appended node';
w.currentNode=root;if(w.lastChild()!==late||w.previousSibling()!==two)throw 'reverse siblings';
var t=document.createTreeWalker(root,NodeFilter.SHOW_TEXT);
if(t.firstChild()!==one.firstChild||t.nextSibling()!==nested.firstChild||
    t.nextSibling()!==two.firstChild||t.previousSibling()!==nested.firstChild||
    t.parentNode()!==null)throw 'masked ancestors';
var detached=document.createElement('span'), child=document.createTextNode('detached');
detached.appendChild(child);t.currentNode=detached;
if(t.nextNode()!==child||t.root!==root)throw 'detached cursor';
var calls=0, caught=0;
try {document.createTreeWalker({}, {valueOf:function(){calls++;return 1;}});} catch(e){if(e instanceof TypeError)caught++;}
try {TreeWalker.prototype.firstChild.call({});} catch(e){if(e instanceof TypeError)caught++;}
try {t.currentNode=null;} catch(e){if(e instanceof TypeError)caught++;}
var filtered=document.createTreeWalker(root,1,function(){calls++;return 1;});
if(calls||caught!==3||t.currentNode!==child||filtered.nextNode()!==one||calls!==1)throw 'validation';
var d=document.createTreeWalker(document,undefined);
if(d.root!==document||d.firstChild()!==root||d.whatToShow!==4294967295)throw 'document root';
var none=document.createTreeWalker(root,-4294967296);
if(none.whatToShow!==0||none.nextNode()!==null||none.currentNode!==root)throw 'empty unsigned mask';
document.title='tree-walker-ok';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "tree-walker-ok");
}

#[test]
fn mutation_observer_batches_direct_detached_character_data_in_microtask_order() {
    let (doc, _, page, outcome) = page("<title>pending</title>", &[r#"
var text=document.createTextNode('initial'), order='';
var first=new MutationObserver(function(records, observer){
    if(this!==first||observer!==first||records.length!==2)throw 'observer callback arguments';
    if(!(records[0] instanceof MutationRecord)||records[0].type!=='characterData'||
        records[0].target!==text||records[0].oldValue!=='initial'||records[1].oldValue!=='one'||
        records[0].addedNodes.length||records[0].removedNodes.length||
        records[0].previousSibling!==null||records[0].nextSibling!==null||
        records[0].attributeName!==null||records[0].attributeNamespace!==null)throw 'record values';
    order+='first|';
});
var second=new MutationObserver(function(records){
    if(records.length!==2||records[0].oldValue!==null||text.data!=='two')throw 'second observer';
    order+='second|';
});
second.observe(text,{characterData:true});
first.observe(text,{characterDataOldValue:true});
Promise.resolve().then(function(){order+='promise|';});
text.textContent='one';
text.data='two';
if(order!=='')throw 'observer ran synchronously';
var drained=new MutationObserver(function(){throw 'drained callback';});
var comment=document.createComment('saved');
drained.observe(comment,{characterData:true,characterDataOldValue:true});
comment.nodeValue='new';
var records=drained.takeRecords();
if(records.length!==1||records[0].oldValue!=='saved'||records[0].target!==comment)throw 'takeRecords';
comment.data='discarded';
drained.disconnect();
if(drained.takeRecords().length)throw 'disconnect clears records';
queueMicrotask(function(){
    if(order!=='promise|first|second|')throw 'observer ordering';
    document.title='observed-character-data';
});
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "observed-character-data");
}

#[test]
fn mutation_observer_keeps_scheduled_peers_and_checks_options_and_brand() {
    let (doc, _, page, outcome) = page("<title>pending</title>", &[r#"
var text=document.createTextNode('zero'), trace='', seen=0;
var first=new MutationObserver(function(records){
    seen++;
    if(seen===1){
        if(second.takeRecords().length!==1||records[0].oldValue!=='zero')throw 'queued values';
        trace+='first|';
        text.data='two';
        throw 'reported observer exception';
    }
    if(records.length!==1||records[0].oldValue!=='one'||trace!=='first|peer|later|')throw 'rescheduled order';
    document.title='observer-rescheduled';
});
var second=new MutationObserver(function(records){
    if(records.length!==1||records[0].oldValue!=='one')throw 'pending peer saw the wrong record';
    trace+='peer|';
});
var types=0;
try {first.observe(text,{characterData:false,characterDataOldValue:true});} catch(e){if(e instanceof TypeError)types++;}
try {first.observe(navigator,{characterData:true});} catch(e){if(e instanceof TypeError)types++;}
first.observe(document.implementation.createHTMLDocument('inert'),{attributes:true});
try {first.observe({},{attributes:true});} catch(e){if(e instanceof TypeError)types++;}
try {MutationObserver.prototype.takeRecords.call({});} catch(e){if(e instanceof TypeError)types++;}
try {new MutationRecord();} catch(e){if(e instanceof TypeError)types++;}
if(types!==5)throw 'validation';
first.observe(text,{characterData:true});
first.observe(text,{characterDataOldValue:true});
second.observe(text,{characterDataOldValue:true});
text.nodeValue='one';
queueMicrotask(function(){trace+='later|';});
"#]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert!(page.report().lines().any(|line| line.starts_with("MutationObserver callback:")
        && line.contains("reported observer exception")));
    assert_eq!(doc.title, "observer-rescheduled");
}

#[test]
fn mutation_observer_child_list_keeps_native_moves_fragments_and_neighbors() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<section id=left><b id=first>before</b><b id=last>after</b></section><section id=right></section>", &[r#"
var l=document.getElementById('left'),r=document.getElementById('right');
var first=document.getElementById('first'),last=document.getElementById('last'), moving=document.createElement('span');
var originalParent=l.parentNode;
var obs=new MutationObserver(function(){throw 'takeRecords should drain this callback';});
obs.observe(document,{childList:true,subtree:true,characterDataOldValue:true});
obs.observe(l,{childList:true});
l.insertBefore(moving,last);
r.appendChild(moving);
var fragment=document.createDocumentFragment(),a=document.createTextNode('origin'),b=document.createElement('i');
fragment.appendChild(a);fragment.appendChild(b);
obs.observe(fragment,{childList:true});
r.replaceChild(fragment,moving);
a.data='changed';
var records=obs.takeRecords();
if(records.length!==6||r.childNodes.length!==2||r.firstChild!==a||r.lastChild!==b||
    fragment.childNodes.length!==0||moving.parentNode!==null)throw 'actual changed tree and deduplicated records';
for(var i=0;i<5;i++){
    var rec=records[i];
    if(!(rec instanceof MutationRecord)||rec.type!=='childList'||rec.oldValue!==null||
        rec.addedNodes!==rec.addedNodes||!(rec.addedNodes instanceof NodeList)||
        rec.removedNodes!==rec.removedNodes||!(rec.removedNodes instanceof NodeList))throw 'real record branded snapshots';
}
if(records[0].target!==l||records[0].addedNodes[0]!==moving||records[0].removedNodes.length!==0||
    records[0].previousSibling!==first||records[0].nextSibling!==last)throw 'ordered insert';
if(records[1].target!==l||records[1].removedNodes[0]!==moving||records[1].addedNodes.length!==0||
    records[1].previousSibling!==first||records[1].nextSibling!==last)throw 'old parent removal';
if(records[2].target!==r||records[2].addedNodes[0]!==moving||records[2].previousSibling!==null||
    records[2].nextSibling!==null)throw 'new parent insertion';
if(records[3].target!==fragment||records[3].removedNodes.length!==2||records[3].removedNodes.item(0)!==a||
    records[3].removedNodes[1]!==b||records[3].addedNodes.length!==0)throw 'fragment removals';
if(records[4].target!==r||records[4].removedNodes[0]!==moving||records[4].addedNodes.length!==2||
    records[4].addedNodes[0]!==a||records[4].addedNodes[1]!==b)throw 'atomic fragment replacement';
if(records[5].type!=='characterData'||records[5].target!==a||records[5].oldValue!=='origin')
    throw 'ancestor CharacterData and oldValue';
l.insertBefore(last,last);
r.appendChild(document.createDocumentFragment());
if(obs.takeRecords().length!==0)throw 'no-op tree operations';
document.title='verified-mutation-child-list';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-mutation-child-list");
    let committed = page.state.dom_commits;
    let refused = page.console_eval(&mut doc, &mut forms, "l.appendChild(l);");
    assert_eq!(refused.errors, 1);
    assert_eq!(page.state.dom_commits, committed);
    let recovered = page.console_eval(&mut doc, &mut forms,
        "if(obs.takeRecords().length!==0||l.parentNode!==originalParent)throw 'refusal queued a record';");
    assert_eq!(recovered.errors, 0, "{}", page.report());
}

#[test]
fn mutation_observer_child_list_uses_the_native_microtask_queue() {
    let (doc, _, page, outcome) = page("<div id=parent><b id=only>child</b></div>", &[r#"
var p=document.getElementById('parent'), old=document.getElementById('only'), trace='';
var observer=new MutationObserver(function(records, self){
    if(trace!=='before|'||records.length!==1||records[0].type!=='childList'||records[0].target!==p||
        records[0].removedNodes[0]!==old||records[0].addedNodes.length!==0||self!==observer)
        throw 'native child list delivery order or values';
    trace+='observer|';
});
observer.observe(p,{childList:true});
queueMicrotask(function(){trace+='before|';});
p.removeChild(old);
queueMicrotask(function(){
    if(trace!=='before|observer|'||old.parentNode!==null)throw 'observer ordering or removal';
    observer.disconnect();
    p.appendChild(old);
    if(observer.takeRecords().length!==0)throw 'disconnect stops native observations';
    document.title='verified-child-list-microtasks';
});
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-child-list-microtasks");
}

#[test]
fn mutation_observer_attribute_filters_keep_real_namespaces_and_old_values() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head></head><body><form id=p><input id=neighbor form=p value=kept></form></body></html>", &[r#"
var p=document.getElementById('p'), seen='', observed=new MutationObserver(function(){throw 'unexpected undrained records';});
var all=new MutationObserver(function(){throw 'unexpected unfiltered callback';});
var filter={
    [Symbol.iterator]:function(){
        seen+='iterator|';var used=false;
        return {next:function(){
            if(used){seen+='done|';return {done:true};}
            used=true;seen+='next|';return {value:{toString:function(){seen+='convert|';return 'data-x';}},done:false};
        }};
    }
};
observed.observe(document,{
    get attributeFilter(){seen+='filter|';return filter;},
    get attributeOldValue(){seen+='old|';return true;},subtree:true,
});
observed.observe(p,{attributes:true,attributeFilter:['data-x']});
all.observe(p,{attributes:true});
if(seen!=='filter|iterator|next|convert|done|old|')throw 'ordered filter iterable and DOMString conversion';
p.setAttribute('DATA-X','one');
p.setAttribute('data-x','two');
p.setAttributeNS('urn:fixture','demo:data-x','ns-one');
p.setAttributeNS('urn:fixture','demo:data-x','ns-two');
p.removeAttributeNS('urn:fixture','data-x');
p.removeAttribute('data-x');
p.removeAttribute('data-missing');
p.setAttribute('data-other','kept');
var current=observed.takeRecords(), every=all.takeRecords();
if(current.length!==3||every.length!==7||current[0].attributeName!=='data-x'||
    current[0].oldValue!==null||current[1].oldValue!=='one'||current[2].oldValue!=='two')
    throw 'filter deduplication and null-namespace old values';
for(var i=0;i<every.length;i++){
    var record=every[i];
    if(!(record instanceof MutationRecord)||record.type!=='attributes'||record.target!==p||
        record.oldValue!==null||record.addedNodes.length!==0||record.removedNodes.length!==0||
        record.previousSibling!==null||record.nextSibling!==null)throw 'genuine attribute record slots';
    var uri=i>=2&&i<=4?'urn:fixture':null;
    if(record.attributeNamespace!==uri||record.attributeName!==(i===6?'data-other':'data-x'))throw 'expanded native names';
}
if(p.getAttribute('data-x')!==null||p.getAttributeNS('urn:fixture','data-x')!==null||
    p.getAttribute('data-other')!=='kept'||document.getElementById('neighbor').value!=='kept')throw 'unchanged native editor';
var errors=0;
try { observed.observe(p,{attributes:false,attributeFilter:['x']}); } catch(e) { if(e instanceof TypeError)errors++;else throw e; }
try { observed.observe(p,{attributeFilter:'data-x'}); } catch(e) { if(e instanceof TypeError)errors++;else throw e; }
if(errors!==2)throw 'option validation';
document.title='verified-mutation-attributes';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "verified-mutation-attributes");
    let commits = page.state.dom_commits;
    let refused = page.console_eval(&mut doc, &mut forms, "p.setAttribute('id','other');");
    assert_eq!(refused.errors, 1);
    assert_eq!(page.state.dom_commits, commits);
    let recovered = page.console_eval(&mut doc, &mut forms,
        "if(observed.takeRecords().length!==0||all.takeRecords().length!==0||p.id!=='p')throw 'failed write queued or mutated';");
    assert_eq!(recovered.errors, 0, "{}", page.report());
}

#[test]
fn inner_html_element_descriptor_wraps_checked_native_accessor() {
    let (mut doc, mut forms, mut page, outcome) = page("<div id=p><b>original</b></div><div id=q></div>", &[r#"
var p=document.getElementById('p'), q=document.getElementById('q');
var d=Object.getOwnPropertyDescriptor(Element.prototype,'innerHTML');
if(!d || !d.enumerable || !d.configurable || d.get.length!==0 || d.set.length!==1 ||
    Object.hasOwn(HTMLElement.prototype,'innerHTML') || Object.hasOwn(p,'innerHTML')) throw 'descriptor';
var writes=0;
Object.defineProperty(p,'innerHTML',{
    get:function(){return d.get.call(this);},
    set:function(value){writes++; d.set.call(this,value);},
    configurable:true
});
p.innerHTML='<i>new</i>';
if(writes!==1 || p.innerHTML!=='<i>new</i>' || p.firstChild.localName!=='i') throw 'wrapped native write';
delete p.innerHTML;
p.innerHTML=null;
if(Object.hasOwn(p,'innerHTML') || p.innerHTML!=='' || p.firstChild!==null || writes!==1) throw 'restore';
Object.defineProperty(q,'innerHTML',{value:'shadow'});
q.innerHTML='ignored';
if(q.innerHTML!=='shadow' || d.get.call(q)!=='') throw 'ordinary shadow';
var converted=false, wrongBrand=false;
try {d.set.call(document.createTextNode(''),{toString:function(){converted=true;return 'bad';}});}
catch(e){wrongBrand=e instanceof TypeError;}
if(!wrongBrand || converted) throw 'brand before conversion';
p.innerHTML='<b>keep</b>';
document.title='inner-html-descriptor-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "inner-html-descriptor-ok");
    let refused = page.console_eval(&mut doc, &mut forms, "d.set.call(p,'<i>prefix</i><iframe></iframe>');");
    assert_eq!(refused.errors, 1, "{}", page.report());
    let retained = page.console_eval(&mut doc, &mut forms, "if(d.get.call(p)!=='<b>keep</b>')throw 'atomic guarded write';");
    assert_eq!(retained.errors, 0, "{}", page.report());
}

#[test]
fn native_element_expandos_preserve_unsupported_native_and_enumeration_boundaries() {
    let (mut doc,mut forms,mut page,outcome)=page("<div id=p></div><iframe id=inert srcdoc=x></iframe>",&["var p=document.getElementById('p');p.customProbe=1;"]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    for operation in ["p.getBoxQuads;","Object.defineProperty(p,'outerHTML',{value:'x'});","Object.defineProperty(document.createElement('input'),'files',{value:[]});","Object.defineProperty(document.createElement('blockquote'),'cite',{value:'https://example.test/'});","Object.defineProperty(document.createElement('meta'),'charset',{value:'x'});","Object.defineProperty(document.createElement('link'),'as',{value:'script'});","Object.setPrototypeOf(p,Array.prototype);","for(var key in p){};"] {
        let outcome=page.console_eval(&mut doc,&mut forms,operation);
        assert_eq!(outcome.errors,1,"{operation}: {}",page.report());
    }
    let shadow = page.console_eval(&mut doc, &mut forms, r#"
var meta=document.createElement('meta'), content=Object.getOwnPropertyDescriptor(HTMLMetaElement.prototype,'content');
meta.content='native';
Object.defineProperty(meta,'content',{value:'author',configurable:true});
if(!Object.hasOwn(meta,'content')||meta.content!=='author'||meta.getAttribute('content')!=='native'||content.get.call(meta)!=='native')throw 'own shadow leaves native metadata intact';
delete meta.content;
if(Object.hasOwn(meta,'content')||meta.content!=='native')throw 'restored inherited metadata';
"#);
    assert_eq!(shadow.errors, 0, "{}", page.report());
    let outcome=page.console_eval(&mut doc,&mut forms,"if(p.id!=='p'||p.customProbe!==1)throw 'recovery';");
    assert_eq!(outcome.errors,0,"{}",page.report());
    let inert=(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("inert")).unwrap();
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let frame=node_handle(inert);
    assert_eq!(bridge.expando_owner(frame),Some(WINDOW));
    assert!(matches!(bridge.get(frame,"contentWindow"),Ok(Value::Null)));
    assert!(matches!(bridge.get(frame,"contentDocument"),Ok(Value::Null)));
    assert!(bridge.state.blank_frames.is_empty());
}

#[test]
fn html_element_names_have_real_namespace_and_null_prefix_independent_of_raw_xmlns() {
    let (mut doc,mut forms,mut page,outcome)=page("<DIV id=p xmlns='urn:raw'></DIV>",&[r#"
var p=document.getElementById('p');if(p.localName!=='div'||p.tagName!=='DIV'||p.prefix!==null||p.namespaceURI!=='http://www.w3.org/1999/xhtml')throw 'parsed names';
var colon=document.createElement('X:Y');if(colon.localName!=='x:y'||colon.prefix!==null||colon.namespaceURI!==p.namespaceURI)throw 'raw HTML colon';
var attr=p.getAttributeNode('id');if(attr.localName!=='id'||attr.namespaceURI!==null||attr.prefix!==null)throw 'attribute distinct namespace';
p.localName='changed';p.prefix='x';p.namespaceURI='urn:other';if(p.localName!=='div'||p.prefix!==null||p.namespaceURI!=='http://www.w3.org/1999/xhtml')throw 'readonly';
var t=document.createTextNode('x');if('localName' in t||t.localName!==undefined||t.prefix!==undefined||document.localName!==undefined)throw 'Element brand';document.title='element-names-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"element-names-ok");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("p")).unwrap();
    doc.nodes[node].namespace=crate::html::Namespace::UnknownForeign;
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(bridge.get(node_handle(node),"localName").unwrap_err().contains("namespace"));
}

#[test]
fn detached_never_published_srcdoc_reflects_domstring_and_real_attr_identity() {
    let (doc,_,page,outcome)=page("<p>ordinary</p>",&[r#"
var f=document.createElement('iframe');if(f.srcdoc!==''||!('srcdoc' in f)||f.hasAttribute('srcdoc')||f.contentWindow!==null)throw 'default';
var calls=0;f.srcdoc={toString:function(){calls++;return '<p>actual raw source</p>';}};
if(calls!==1||f.srcdoc!=='<p>actual raw source</p>'||f.getAttribute('srcdoc')!==f.srcdoc||f.contentWindow!==null)throw 'real reflection/no execution';
var attr=f.getAttributeNode('srcdoc');f.srcdoc=null;if(f.srcdoc!=='null'||attr.value!=='null')throw 'DOMString null';
f.srcdoc=undefined;if(f.srcdoc!=='undefined'||f.getAttributeNode('srcdoc')!==attr)throw 'undefined/identity';
var caught=false;try{f.srcdoc=Symbol();}catch(e){caught=e.name==='TypeError';}if(!caught||f.srcdoc!=='undefined')throw 'Symbol atomic';
var marker={},same=false;try{f.srcdoc={toString:function(){throw marker;}};}catch(e){same=e===marker;}if(!same||f.srcdoc!=='undefined')throw 'throw identity';
f.removeAttribute('srcdoc');if(f.srcdoc!==''||attr.ownerElement!==null||attr.value!=='undefined')throw 'raw removal/detach';
for(var e of [document.createElement('div'),document.createTextNode('x')])if('srcdoc' in e||e.srcdoc!==undefined)throw 'brand';
document.title='srcdoc-reflection-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"srcdoc-reflection-ok");
    assert!(page.state.blank_frames.is_empty());
}

#[test]
fn node_normalize_merges_real_text_runs_preserving_first_nonempty_and_detached_data() {
    let (doc,_,page,outcome)=page("<main id=root></main>",&[r#"
var root=document.getElementById('root'),p=document.createElement('div');root.appendChild(p);
var empty=document.createTextNode(''),a=document.createTextNode('a'),middle=document.createTextNode(''),b=document.createTextNode('b'),end=document.createTextNode('');
for(var n of [empty,a,middle,b,end])p.appendChild(n);
var separator=document.createElement('span'),x=document.createTextNode('x'),y=document.createTextNode('y');separator.appendChild(x);separator.appendChild(y);p.appendChild(separator);
var z=document.createTextNode('z');p.appendChild(z);
empty.normalize();a.normalize();if(p.firstChild!==empty||a.data!=='a'||b.parentNode!==p)throw 'Text receiver excludes itself';
var called=0;if(p.normalize({toString:function(){called++;throw 'ignored';}})!==undefined||called||p.normalize.length!==0||p.normalize!==p.normalize)throw 'metadata/ignored args';
if(p.firstChild!==a||a.data!=='ab'||a.nextSibling!==separator||separator.nextSibling!==z||x.data!=='xy'||separator.firstChild!==x||x.nextSibling!==null)throw 'nested runs/identity';
for(var n of [empty,middle,b,end,y])if(n.parentNode!==null||n.isConnected)throw 'detached';if(b.data!=='b'||y.data!=='y'||!a.isConnected)throw 'saved data/connection';
var attr=root.getAttributeNode('id');attr.normalize();p.normalize();if(a.data!=='ab'||p.firstChild!==a)throw 'idempotent/Attr';
root.appendChild(b);if(b.parentNode!==root||b.data!=='b')throw 'removed identity reusable';
var rejected=0;for(var bad of [{},window,p.style,p.classList])try{p.normalize.call(bad);}catch(e){if(e.name==='TypeError')rejected++;}if(rejected!==4)throw 'genuine Node brand';
document.normalize();document.title='normalize-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"normalize-ok");
}

#[test]
fn node_normalize_quota_and_late_resource_guard_failures_leave_all_text_unchanged() {
    let (mut doc,mut forms,mut page,outcome)=page("<title id=t>x</title><div id=p></div>",&[r#"
var p=document.getElementById('p');p.appendChild(document.createTextNode('left'));p.appendChild(document.createTextNode('right'));
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let p=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("p")).unwrap();
    let t=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("t")).unwrap();
    let original=doc.nodes[p].children.clone();
    let original_retained=page.state.retained;
    let normalized_retained=original_retained-child_link_bytes(&doc.nodes[p].children)
        -doc.nodes[original[0]].kind.character_data_capacity().unwrap()
        +"leftright".len()+size_of::<usize>();
    page.begin_job();
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    for failure in 0..3 {
        bridge.state.retained_limit=if failure==0 {normalized_retained-1} else {bridge.state.limits.host.retained_bytes};
        bridge.state.copy_work=if failure==1 {0} else {bridge.state.limits.copy_bytes};
        bridge.state.host_work=if failure==2 {0} else {bridge.state.limits.host_work};
        assert!(bridge.normalize_node(node_handle(p)).is_err());
        assert_eq!(bridge.doc.nodes[p].children,original);
        assert!(matches!(&bridge.doc.nodes[original[0]].kind,NodeKind::Text(text) if text=="left"));
        assert_eq!(bridge.state.retained,original_retained);
    }
    bridge.state.copy_work=bridge.state.limits.copy_bytes;
    bridge.state.host_work=bridge.state.limits.host_work;
    let Value::Host(empty)=bridge.call(DOCUMENT,"createTextNode",&[Value::String(String::new())]).unwrap() else {unreachable!()};
    let empty=node_index(empty).unwrap();
    bridge.doc.nodes[t].children.push(empty);
    bridge.state.detached_nodes.remove(&empty);
    bridge.state.dirty=true;
    let error=bridge.normalize_node(DOCUMENT).unwrap_err();
    assert!(error.contains("native control contents or inert resource"),"{error}");
    assert_eq!(bridge.doc.nodes[p].children,original);
    assert_eq!(bridge.doc.nodes[t].children.len(),2);
    bridge.doc.nodes[t].children.pop();bridge.state.detached_nodes.insert(empty);bridge.state.dirty=true;
    bridge.normalize_node(node_handle(p)).unwrap();
    assert_eq!(bridge.doc.nodes[p].children,[original[0]]);
    assert!(matches!(&bridge.doc.nodes[original[0]].kind,NodeKind::Text(text) if text=="leftright"));
    assert!(matches!(&bridge.doc.nodes[original[1]].kind,NodeKind::Text(text) if text=="right"));
}

#[test]
fn class_list_add_uses_ordered_boolean_free_domstring_tokens_and_atomic_validation() {
    let (doc,_,page,outcome)=page("<div id=p class=' a a B '></div>",&[r#"
var p=document.getElementById('p'),l=p.classList,a=p.getAttributeNode('class');
if(l.add.length!==0||l.add('b','a','b')!==undefined||l.value!=='a B b'||a.value!=='a B b'||p.getAttributeNode('class')!==a)throw 'ordered set/Attr';
p.className=' x  x ';l.add();if(l.value!=='x')throw 'zero argument normalize';
var log='',caught=false;try{l.add('',{toString:function(){log+='b';p.className='side effect';return 'q';}});}catch(e){caught=e.name==='SyntaxError';}if(!caught||log!=='b'||l.value!=='side effect')throw 'all conversions before validation';
caught=false;try{l.add('',Symbol());}catch(e){caught=e.name==='TypeError';}if(!caught||l.value!=='side effect')throw 'Symbol before validation';
caught=false;try{l.add('bad token','');}catch(e){caught=e.name==='InvalidCharacterError';}if(!caught)throw 'validation order';
l.add({toString:function(){p.className='q';return 'r';}},'q');if(l.value!=='q r')throw 'current post-conversion class';
var empty=document.createElement('div');empty.classList.add();if(empty.hasAttribute('class'))throw 'absent empty set';
document.title='class-add-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"class-add-ok");
}

#[test]
fn iframe_own_inspection_reports_real_absence_without_accessor_or_mutation_authority() {
    let (mut doc,mut forms,mut page,outcome)=page("<iframe id=old srcdoc='authored'></iframe>",&[r#"
var f=document.createElement('iframe');f.id='f';
for(var key of ['id','srcdoc','contentWindow','normalize','hasAttributes'])if(Object.hasOwn(f,key)||Object.getOwnPropertyDescriptor(f,key)!==undefined)throw 'inherited IDL';
if(Object.keys(f).length||Object.getOwnPropertyNames(f).length||f.getAttribute('srcdoc')!==null||f.getAttribute('id')!=='f')throw 'actual own keys/attribute';document.title='iframe-own-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"iframe-own-ok");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("f")).unwrap();
    let inert=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("old")).unwrap();
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.set(node_handle(node),"srcdoc",Value::String("reflected".into())).unwrap();
    bridge.call(node_handle(node),"setAttribute",&[Value::String("srcdoc".into()),Value::String("changed".into())]).unwrap();
    assert!(bridge.inherited_keys(node_handle(node)).is_err());
    assert!(bridge.own_data(node_handle(inert),"srcdoc").unwrap().is_none());
    assert!(bridge.own_keys(node_handle(inert)).unwrap().is_empty());
    assert!(bridge.set(node_handle(inert),"srcdoc",Value::String("forbidden".into())).is_err());
    assert!(bridge.state.blank_frames.is_empty());
    assert_eq!(bridge.doc.attr(node,"srcdoc"),Some("changed"));
    assert_eq!(bridge.doc.attr(inert,"srcdoc"),Some("authored"));
}

#[test]
fn element_has_attributes_reports_actual_presence_and_borrows_only_element_receivers() {
    let (doc,_,page,outcome)=page("<p id=p data-x=''></p>",&[r#"
var p=document.getElementById('p'),f=p.hasAttributes,e=document.createElement('div');
if(f.length!==0||f!==p.hasAttributes||!p.hasAttributes()||e.hasAttributes())throw 'actual/default presence';
e.className='';if(!f.call(e))throw 'empty value still present';e.removeAttribute('class');if(f.call(e))throw 'removal';
var called=0;e.hasAttributes({toString:function(){called++;throw 'not converted';}});if(called)throw 'ignored args';
var rejected=0;for(var bad of [{},window,document,p.style,document.createTextNode('x')])try{f.call(bad);}catch(x){if(x.name==='TypeError')rejected++;}if(rejected!==5)throw 'brand';
document.title='has-attributes-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"has-attributes-ok");
}

#[test]
fn native_instanceof_uses_realm_roots_and_original_mutable_image_prototype() {
    let (doc,_,page,outcome)=page("<p id=p></p><img id=i>",&[r#"
var values=[document,navigator,document.getElementById('p'),document.createTextNode('t'),document.getElementsByClassName('x'),document.getElementById('p').classList,document.getElementById('p').style];
function C(){};for(var x of values){if(x instanceof Array||!(x instanceof Object)||x instanceof C)throw 'real native ancestry';}
C.prototype=Object.prototype;for(var x of values)if(!(x instanceof C))throw 'prototype identity not constructor name';
var OriginalImage=Image,img=new Image(),parsed=document.getElementById('i');
function Alias(){};Alias.prototype=OriginalImage.prototype;
if(!(img instanceof Alias)||!(parsed instanceof Alias)||!(img instanceof OriginalImage)||img instanceof Array)throw 'image canonical anchor';
{let Image=function Replacement(){};if(img instanceof Image||!(img instanceof OriginalImage))throw 'shadowed name';}
Object.setPrototypeOf(OriginalImage.prototype,Array.prototype);
if(!(img instanceof Array)||!(parsed instanceof Array)||Array.isArray(img))throw 'actual changed image ancestry';
Object.setPrototypeOf(OriginalImage.prototype,Object.prototype);document.title='native-instanceof-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"native-instanceof-ok");
}

#[test]
fn host_array_slice_captures_length_but_reads_live_indices_and_keeps_dom_identity() {
    let (doc,_,page,outcome)=page("<main><p id=a class=probe></p><p id=b class=probe></p><p id=c class=probe></p></main>",&[r#"
var live=document.getElementsByClassName('probe'),a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');
var snapshot=Array.prototype.slice.call(live);if(!Array.isArray(snapshot)||Object.getPrototypeOf(snapshot)!==Array.prototype||snapshot.length!==3||snapshot[0]!==a||snapshot[2]!==c)throw 'ordinary snapshot/identity';
var log='',shorter=Array.prototype.slice.call(live,{valueOf:function(){log+='s';a.className='gone';return 0;}},{valueOf:function(){log+='e';return undefined;}});
if(log!=='se'||shorter.length!==0)throw 'explicit end NaN';
a.className='probe';var holes=Array.prototype.slice.call(live,{valueOf:function(){a.className='gone';return 0;}});
if(holes.length!==3||holes[0]!==b||holes[1]!==c||2 in holes||Object.prototype.hasOwnProperty.call(holes,'2'))throw 'captured length/live holes';
if(snapshot.length!==3||snapshot[0]!==a||snapshot[1]!==b)throw 'snapshot stays static';
var tail=Array.prototype.slice.call(live,-1,undefined);if(tail.length!==1||tail[0]!==c)throw 'negative/defaultend';
if(Array.prototype.slice.call(live,Infinity).length||Array.prototype.slice.call(live,-Infinity,1)[0]!==b)throw 'infinite bounds';
var marker={},same=false;try{Array.prototype.slice.call(live,{valueOf:function(){throw marker;}});}catch(e){same=e===marker;}if(!same)throw 'throw identity';
var caught=false;try{Array.prototype.slice.call(live,Symbol());}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'Symbol';document.title='host-slice-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"host-slice-ok");
}

#[test]
fn detached_scroll_dimensions_are_exact_and_connected_measurements_remain_explicit() {
    let (mut doc,mut forms,mut page,outcome)=page("<main><p id=p>rendered</p></main>",&[r#"
var e=document.createElement('div');e.style.width='123px';e.style.height='45px';
if(e.scrollWidth!==0||e.scrollHeight!==0||!('scrollWidth' in e))throw 'detached no box';
e.scrollWidth=9;if(e.scrollWidth!==0)throw 'readonly';document.title='detached-scroll-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"detached-scroll-ok");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("p")).unwrap();
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let error=bridge.get(node_handle(node),"scrollWidth").unwrap_err();
    assert!(error.contains("measured scrolling-area")&&error.contains("connected=true"),"{error}");
}

#[test]
fn class_name_collection_method_preserves_form_named_control_precedence() {
    let (doc,_,page,outcome)=page("<form id=f><input id=field name=getElementsByClassName class=probe></form>",&[r#"
var f=document.getElementById('f'),field=document.getElementById('field');
if(f.getElementsByClassName!==field||!('getElementsByClassName' in f))throw 'named precedence';
var method=document.createElement('div').getElementsByClassName;
if(method.call(f,'probe')[0]!==field)throw 'borrow genuine Element method';
var caught=false;try{f.getElementsByClassName('probe');}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'named field is not callable';document.title='class-collection-form-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"class-collection-form-ok");
}

#[test]
fn class_name_collections_match_all_ascii_tokens_in_live_descendant_order() {
    let (doc,_,page,outcome)=page("<!doctype html><main><section id=s class='a b'><p id=p class='a b'></p><div><b id=b class='b a'></b></div><i id=c class='a B'></i></section><p id=outside class='a b'></p></main>",&[r#"
var s=document.getElementById('s'),p=document.getElementById('p'),b=document.getElementById('b'),c=document.getElementById('c');
var all=document.getElementsByClassName(' a\t b a\n'),inside=s.getElementsByClassName('b a');
if(document.compatMode!=='CSS1Compat'||all.length!==4||all[0]!==s||all[1]!==p||all[2]!==b||all[3]!==document.getElementById('outside'))throw 'standards order';
if(inside.length!==2||inside[0]!==p||inside[1]!==b||inside.namedItem('b')!==b||inside.item(2)!==null)throw 'scope/root exclusion';
if(document.getElementsByClassName('').length||document.getElementsByClassName(' \t\n\r\f ').length)throw 'empty tokens';
if(s.getElementsByClassName('B').length!==1||s.getElementsByClassName('B')[0]!==c)throw 'case';
var unicode=document.createElement('span');unicode.className='a\u00a0b';s.appendChild(unicode);
if(s.getElementsByClassName('a\u00a0b')[0]!==unicode||inside.length!==2)throw 'ASCII whitespace only';
unicode.className='a\u000bb';if(s.getElementsByClassName('a\u000bb')[0]!==unicode)throw 'vertical tab is token content';
p.className='changed';if(inside.length!==1||inside[0]!==b)throw 'live class mutation';
c.className='a b';if(inside.length!==2||inside[1]!==c)throw 'live case change';
var fresh=document.createElement('em');fresh.className='b a';s.insertBefore(fresh,b.parentNode);if(inside[0]!==fresh||inside.length!==3)throw 'live insertion/order';
s.removeChild(fresh);if(inside.length!==2||all.length!==4)throw 'live removal';
var detached=s.getElementsByClassName('a b');s.parentNode.removeChild(s);if(detached.length!==2||all.length!==1)throw 'detached retained scope';
if(Object.keys(detached).join(',')!=='0,1')throw 'indexed enumeration';document.title='class-collection-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"class-collection-ok");
}

#[test]
fn class_name_collection_conversion_metadata_and_interface_brands_are_real() {
    let (doc,_,page,outcome)=page("<p id=p class='null undefined a'></p><div id=d class=a></div>",&[r#"
var p=document.getElementById('p'),d=document.getElementById('d'),docMethod=document.getElementsByClassName,elementMethod=p.getElementsByClassName;
if(docMethod.length!==1||elementMethod.length!==1||docMethod!==document.getElementsByClassName)throw 'metadata';
if(docMethod.call(document,null)[0]!==p||docMethod.call(document,undefined)[0]!==p)throw 'DOMString';
var called=0,classes={toString:function(){called++;p.className='new';return 'new';}};if(docMethod.call(document,classes)[0]!==p||called!==1)throw 'conversion/current DOM';
var bad=0,arg={toString:function(){bad++;return 'a';}},caught=0;
try{docMethod.call(document);}catch(e){if(e.name==='TypeError')caught++;}
try{docMethod.call(document,Symbol());}catch(e){if(e.name==='TypeError')caught++;}
for(var receiver of [{},window,p,p.style,document.createTextNode('x')])try{docMethod.call(receiver,arg);}catch(e){if(e.name==='TypeError')caught++;}
for(var receiver of [{},window,document,p.style,document.createTextNode('x')])try{elementMethod.call(receiver,arg);}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==12||bad)throw 'brand/arity before conversion';
if(elementMethod.call(d,'a').length!==0)throw 'borrow excludes root';
var marker={},same=false;try{docMethod.call(document,{toString:function(){throw marker;}});}catch(e){same=e===marker;}if(!same)throw 'throw identity';
document.title='class-collection-conversion-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"class-collection-conversion-ok");
}

#[test]
fn class_name_collection_accounting_failures_keep_existing_storage_and_dom() {
    let (mut doc,mut forms,mut page,_) = page("<p class='a b'></p>",&[]);
    page.begin_job();
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let before=bridge.state.retained;
    bridge.state.retained_limit=before;
    assert!(bridge.create_class_collection(DOCUMENT,&Value::String("a b".into())).is_err());
    assert_eq!(bridge.state.retained,before);
    assert!(bridge.state.dom.collections.is_empty());
    bridge.state.retained_limit=before+100;
    bridge.state.copy_work=0;
    assert!(bridge.create_class_collection(DOCUMENT,&Value::String("a".into())).is_err());
    assert!(bridge.state.dom.collections.is_empty());
    bridge.state.copy_work=bridge.state.limits.copy_bytes;
    let list=bridge.create_class_collection(DOCUMENT,&Value::String("a".into())).unwrap();
    let Value::Host(list)=list else {unreachable!()};
    bridge.state.host_work=0;
    // Length now lives on the genuine HTMLCollection prototype. Exercise
    // its native getter rather than the removed own-property fallback.
    assert!(bridge.call(list,"\0HTMLCollection.get.length",&[]).is_err());
    assert_eq!(bridge.state.dom.collections.len(),1);
    assert!(bridge.doc.nodes.iter().any(|node|matches!(&node.kind,NodeKind::Element {attrs,..} if attrs.iter().any(|attr|attr.name.html_is("class")&&attr.value=="a b"))));
}

#[test]
fn token_attribute_queries_use_real_attributes_snapshots_namespaces_and_dynamic_matching() {
    let (doc,_,page,outcome)=page("<html><head><link id=sheet rel='preload StyleSheet' href='/original.css'></head><body><div id=word data-words='cold Ready'><span id=child></span></div><p id=metadata></p></body></html>",&[r#"
var sheet=document.getElementById('sheet'),word=document.getElementById('word'),child=document.getElementById('child'),metadata=document.getElementById('metadata');
var styles=document.querySelectorAll('link[rel~="stylesheet"]');if(styles.length!==1||styles[0]!==sheet||document.querySelector('link[rel~=stylesheet s]')!==null||document.querySelector('link[rel~=StyleSheet s]')!==sheet)throw 'actual rel words';
if(word.matches('[data-words~=ready]')||!word.matches('[data-words~=ready i]')||child.closest('[data-words~=ready i]')!==word||document.querySelector('[data-words~=""]')!==null||document.querySelector('[data-words~="cold Ready"]')!==null)throw 'case word empty';
var saved=document.querySelectorAll('[data-words~=ready i]');word.setAttribute('data-words','cold disabled');if(saved.length!==1||saved[0]!==word||document.querySelector('[data-words~=ready i]')!==null||child.closest('[data-words~=ready i]')!==null)throw 'snapshot and current mutation';
metadata.setAttributeNS('urn:original','rel','stylesheet');if(metadata.getAttributeNS('urn:original','rel')!=='stylesheet'||metadata.matches('[rel~=stylesheet]'))throw 'namespace decoy';
word.setAttribute('data-words','left\u000bready');if(word.matches('[data-words~=ready]'))throw 'VT is not CSS whitespace';word.setAttribute('data-words','left\u00a0ready');if(word.matches('[data-words~=ready]')||!word.matches('[data-words~="left\u00a0ready"]'))throw 'NBSP token';
word.setAttribute('data-words','left\tready\nlast\fend\rtrail');if(!word.matches('[data-words~=ready]')||!word.matches('[data-words~=end]')||!word.matches('[data-words~=trail]'))throw 'ASCII separators';document.title='token-query-ok';
"#]);assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"token-query-ok");
}

#[test]
fn unavailable_animation_feature_reads_are_absent_without_advertising_or_running_animation() {
    let (mut doc,mut forms,mut page,outcome)=page("<html><body><div id=d></div><form id=f><input name=animate id=control></form><video id=video></video><x-custom id=custom></x-custom><unlisted id=opaque></unlisted><svg id=foreign></svg></body></html>",&[r#"
var d=document.getElementById('d'),created=document.createElement('span'),video=document.getElementById('video'),calls=0;
for(var element of [document.documentElement,document.body,d,created,video,document.getElementById('custom'),document.getElementById('opaque')]){
 if(element.animate!==undefined||typeof element.animate!=='undefined'||'animate' in element)throw 'absent animation';
 if(element.animate)calls++;
 var caught=false;try{element.animate([]);}catch(e){caught=e instanceof TypeError;}if(!caught)throw 'missing method call';
}
if(calls!==0||document.getElementById('f').animate!==document.getElementById('control'))throw 'feature or form named property';
Object.prototype.animate=17;if(d.animate!==17||!('animate' in d))throw 'ordinary inherited animation key';delete Object.prototype.animate;if(d.animate!==undefined||'animate' in d)throw 'tail deletion';
document.title='animation-absence-ok';
"#]);assert_eq!(outcome.errors,0,"{}",page.report());assert_eq!(doc.title,"animation-absence-ok");
    let commits=page.state.dom_commits;let nodes=doc.nodes.len();
    for operation in ["d.animate=function(){return 17;};","Object.defineProperty(d,'animate',{value:function(){return 17;}});","d.getAnimations;","video.animate=function(){return 17;};","Object.defineProperty(video,'animate',{value:function(){return 17;}});","video.getAnimations;"] {
        let outcome=page.console_eval(&mut doc,&mut forms,operation);assert_eq!(outcome.errors,1,"{operation}: {}",page.report());assert_eq!(page.state.dom_commits,commits);assert_eq!(doc.nodes.len(),nodes);assert!(!page.has_host_tasks());
    }
    let outcome = page.console_eval(&mut doc, &mut forms, r#"
var svg=document.getElementById('foreign');
if(svg.namespaceURI!=='http://www.w3.org/2000/svg'||svg.animate!==undefined||'animate' in svg)throw 'SVG animation exposure';
var denied=false;try{svg.animate([]);}catch(error){denied=error instanceof TypeError;}if(!denied)throw 'SVG animation call';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(doc.nodes.len(), nodes);
    assert!(!page.has_host_tasks());
}

#[test]
fn real_element_instances_have_no_constructor_prototype_or_legacy_capture_methods() {
    let (doc,_,page,outcome)=page("<html><body><div id=d></div><form id=f><input id=fp name=prototype><input id=fc name=setCapture></form></body></html>",&[r#"
var d=document.getElementById('d');
for(var name of ['prototype','setCapture','releaseCapture'])if(d[name]!==undefined||name in d)throw 'instance absence';
if(document.releaseCapture!==undefined)throw 'Document capture absence';
var f=document.getElementById('f');if(f.prototype!==document.getElementById('fp')||f.setCapture!==document.getElementById('fc'))throw 'form named properties';
document.title='element-absence-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"element-absence-ok");
}

#[test]
fn class_list_is_live_ordered_case_sensitive_and_mutates_the_actual_attribute() {
    let (doc,_,page,outcome) = page("<p id=p class=' a  b a A&#9;c '></p>", &[r#"
var p=document.getElementById('p'),l=p.classList,a=p.getAttributeNode('class');
if(l!==p.classList||l.length!==4||l[0]!=='a'||l[3]!=='c'||l[4]!==undefined||l.item(4294967296)!=='a'||l.item(-1)!==null)throw 'live identity/index';
if(l.remove.length!==0||l.contains.length!==1||l.item.length!==1||l.remove!==l.remove)throw 'method metadata';
if(!l.contains('A')||l.contains('missing')||l.contains('')||l.contains('a b'))throw 'contains';
if(Object.keys(l).join(',')!=='0,1,2,3'||Object.prototype.hasOwnProperty.call(l,'length'))throw 'own indices';
if(!l.hasOwnProperty('0')||!l.propertyIsEnumerable('0')||l.hasOwnProperty('value'))throw 'ordinary inherited methods';
Object.prototype.customClassProbe=17;if(l.customClassProbe!==17)throw 'ordinary inherited property';delete Object.prototype.customClassProbe;
if(l.remove('a','c')!==undefined||p.className!=='b A'||a.value!=='b A'||p.getAttributeNode('class')!==a)throw 'real removal/Attr';
p.setAttribute('class','  x x Y  ');if(l.length!==2||l[1]!=='Y'||String(l)!=='  x x Y  ')throw 'raw edit';
l.remove();if(l.value!=='x Y')throw 'zero argument normalize';
l.value=' \u00a0 \u000b z';if(l.length!==3||!l.contains('\u00a0')||!l.contains('\u000b'))throw 'ASCII only';
p.classList={toString:function(){return 'forwarded';}};if(l.value!=='forwarded'||a.value!=='forwarded')throw 'PutForwards';
var fresh=document.createElement('div'),f=fresh.classList;f.remove('missing');if(fresh.hasAttribute('class'))throw 'absent stays absent';
fresh.className='x';f.remove('x');if(f.value!==''||!fresh.hasAttribute('class'))throw 'empty stays present';
if('classList' in document.createTextNode('text')||'classList' in document)throw 'brand';
Promise.resolve(l).then(function(v){if(v!==l)throw 'Promise identity';document.title='class-list-ok';});
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"class-list-ok");
}

#[test]
fn class_list_removal_converts_all_arguments_before_validation_and_observes_callbacks() {
    let (doc,_,page,outcome) = page("<p id=p class='old'></p><b id=b class='b c'></b>", &[r#"
var p=document.getElementById('p'),l=p.classList,log='',caught=false;
try{l.remove('',{toString:function(){log+='b';p.className='after';return 'x';}});}catch(e){caught=e.name==='SyntaxError';}
if(!caught||log!=='b'||p.className!=='after')throw 'all conversions before validation';
caught=false;try{l.remove('',Symbol());}catch(e){caught=e.name==='TypeError';}if(!caught||p.className!=='after')throw 'conversion precedence';
caught=false;try{l.remove(' ', '');}catch(e){caught=e.name==='InvalidCharacterError';}if(!caught)throw 'validation order';
log='';l.remove({toString:function(){log+='a';p.className='a b c';return 'a';}},{toString:function(){log+='b';return 'b';}});
if(log!=='ab'||p.className!=='c')throw 'current post-conversion class';
var marker={};caught=false;try{l.remove({toString:function(){p.className='side effect';throw marker;}});}catch(e){caught=e===marker;}if(!caught||p.className!=='side effect')throw 'exception identity';
var n=0;for(var bad of [{},window,document,p,p.style,document.createTextNode('t')]){caught=false;try{l.remove.call(bad,{toString:function(){n++;return 'x';}});}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'wrong receiver';}
if(n)throw 'brand before conversion';var b=document.getElementById('b');l.remove.call(b.classList,'b');if(b.className!=='c'||p.className!=='side effect')throw 'borrow';
document.title='class-list-conversion-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"class-list-conversion-ok");
}

#[test]
fn class_list_copy_work_and_invalid_namespace_failures_preserve_attribute() {
    let (mut doc,mut forms,mut page,outcome) = page("<p id=p class='a b'></p>",&[]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let node = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("p")).unwrap();
    let handle = script_class_list::handle(node_handle(node));
    page.begin_job();
    let mut bridge = Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.state.copy_work = 0;
    assert!(bridge.call(handle,"remove",&[Value::String("a".into())]).is_err());
    assert_eq!(bridge.doc.attr(node,"class"),Some("a b"));
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.state.host_work = 0;
    assert!(bridge.call(handle,"remove",&[Value::String("a".into())]).is_err());
    assert_eq!(bridge.doc.attr(node,"class"),Some("a b"));
    bridge.state.host_work = bridge.state.limits.host_work;
    bridge.doc.nodes[node].namespace = crate::html::Namespace::UnknownForeign;
    assert!(bridge.get(handle,"value").unwrap_err().contains("namespace"));
    assert!(!bridge.accepts_receiver(handle,"remove",node_handle(node)));
    assert_eq!(bridge.doc.attr(node,"class"),Some("a b"));
    assert!(!bridge.class_list_receiver(handle));
}

#[test]
fn class_list_put_forwards_conversion_is_limited_to_known_html_and_svg_elements() {
    let (mut doc,mut forms,mut page,outcome) = page("<p id=p></p><svg><g id=f></g></svg><math id=unknown></math>",&[]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let html = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("p")).unwrap();
    let foreign = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("f")).unwrap();
    let unknown = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("unknown")).unwrap();
    let mut bridge = Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    assert!(bridge.string_setter(node_handle(html),"classList"));
    assert!(bridge.string_setter(node_handle(foreign),"classList"));
    assert!(!bridge.string_setter(node_handle(unknown),"classList"));
    assert!(!bridge.string_setter(DOCUMENT,"classList"));
    let text = bridge.call(DOCUMENT,"createTextNode",&[Value::String("text".into())]).unwrap();
    let Value::Host(text) = text else {unreachable!()};
    assert!(!bridge.string_setter(text,"classList"));
    assert!(!bridge.class_list_receiver(script_class_list::handle(text)));
}

#[test]
fn class_list_handle_band_never_collides_with_last_event_identity() {
    let (mut doc,mut forms,mut page,_) = page("<p id=p></p>",&[]);
    page.state.next_event = script_class_list::BASE-1;
    let mut bridge = Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let last = bridge.create_event(&[Value::String("Event".into())]).unwrap();
    assert!(matches!(last,Value::Host(handle) if handle==script_class_list::BASE-1));
    assert_eq!(bridge.state.next_event,script_class_list::BASE);
    assert!(bridge.create_event(&[Value::String("Event".into())]).is_err());
    assert_eq!(bridge.state.created_events.len(),1);
    assert_eq!(bridge.state.next_event,script_class_list::BASE);
    assert!(!page.dispatch_to(&mut doc,&mut forms,WINDOW,"load",false,false));
    assert_eq!(page.state.next_event,script_class_list::BASE);
    assert!(page.state.event.is_none());
}

#[test]
fn absent_attribute_removal_observes_ordered_qualified_and_expanded_names() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<body><div id=p stablemarker=keep></div><svg><rect id='shape' width='18'/></svg><input id=edit value=seed></body>",
        &[r#"
var p=document.getElementById('p'), edit=document.getElementById('edit'), shape=document.getElementById('shape');
var detached=document.createElement('div');
var protectedAttr=p.getAttributeNode('stablemarker');edit.value='edited';
p.setAttributeNS('urn:first','m:flag','one');p.setAttributeNS('urn:second','m:flag','two');
p.setAttributeNS(null,'MixedCase','exact');
var first=p.getAttributeNodeNS('urn:first','flag'), second=p.getAttributeNodeNS('urn:second','flag');
var mixed=p.getAttributeNodeNS(null,'MixedCase');
if(p.removeAttribute('flag')!==undefined||p.getAttribute('M:FLAG')!=='one')throw 'qualified lookup';
p.removeAttributeNS('urn:first','M:FLAG');p.removeAttribute('MixedCase');
if(first.ownerElement!==p||mixed.ownerElement!==p)throw 'lookalike removed';
p.removeAttribute('M:FLAG');
if(first.ownerElement!==null||first.value!=='one'||p.getAttributeNode('m:flag')!==second)throw 'first qualified match';
p.removeAttributeNS('urn:first','flag');p.removeAttributeNS('urn:second','flag');
if(second.ownerElement!==null||second.value!=='two'||p.hasAttribute('m:flag'))throw 'expanded match';
if(shape.getAttribute('WIDTH')!==null||shape.hasAttribute('WIDTH')||shape.removeAttribute('WIDTH')!==undefined||
   shape.getAttribute('width')!=='18')throw 'SVG case';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let commits = page.state.dom_commits;
    let retained = page.state.retained;
    for source in ["p.removeAttribute('STABLEMARKER');", "p.removeAttributeNS(null,'stablemarker');"] {
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 1, "{source}: {}", page.report());
    }
    let result = page.console_eval(&mut doc, &mut forms, r#"
if(p.removeAttribute('pending-state')!==undefined||p.removeAttribute('')!==undefined||
   p.removeAttribute('not an attribute')!==undefined||p.removeAttributeNS(null,'pending-state')!==undefined)throw 'missing result';
if(detached.removeAttribute('pending-state')!==undefined||detached.parentNode!==null||detached.ownerDocument!==document)throw 'detached receiver';
if(protectedAttr.ownerElement!==p||protectedAttr.value!=='keep'||p.getAttributeNode('stablemarker')!==protectedAttr||
   mixed.ownerElement!==p||edit.value!=='edited'||edit.defaultValue!=='seed'||edit.ownerDocument!==document)throw 'state or ownership';
"#);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert!(!result.layout_changed);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(page.state.retained, retained);
}

#[test]
fn absent_attribute_removal_checks_receivers_coercion_and_resource_state() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<body><div id=p title=before></div><img id=i><script id=s></script><script id=loaded src=original.js></script><link id=l><input id=e></body>",
        &[r#"
var p=document.getElementById('p'), img=document.getElementById('i'), script=document.getElementById('s'), link=document.getElementById('l');
var loaded=document.getElementById('loaded'), src=loaded.getAttributeNode('src');
var calls=0, key={toString:function(){calls++;p.setAttribute('title','new');return 'TITLE';}}, old=p.getAttributeNode('title');
var remove=p.removeAttribute;
if(remove.call(p,key)!==undefined||calls!==1||old.ownerElement!==null||old.value!=='new')throw 'present after conversion';
var added;
remove.call(p,{toString:function(){p.setAttribute('data-inserted','fresh');added=p.getAttributeNode('data-inserted');return 'DATA-INSERTED';}});
if(p.hasAttribute('data-inserted')||added.ownerElement!==null||added.value!=='fresh')throw 'lookup before key conversion';
var caught=0, marker={};
try{remove.call(document,key);}catch(e){if(e.name==='TypeError')caught++;}
try{remove.call(p);}catch(e){if(e.name==='TypeError')caught++;}
try{remove.call(p,Symbol());}catch(e){if(e.name==='TypeError')caught++;}
try{remove.call(p,{toString:function(){throw marker;}});}catch(e){if(e===marker)caught++;}
if(calls!==1||caught!==4)throw 'conversion or receiver checks';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let commits = page.state.dom_commits;
    let retained = page.state.retained;
    let result = page.console_eval(&mut doc, &mut forms, r#"
img.removeAttribute('src');script.removeAttribute('src');link.removeAttribute('href');
document.getElementById('e').removeAttribute('disabled');
"#);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert!(!result.layout_changed);
    let refusal = page.console_eval(&mut doc, &mut forms, "loaded.removeAttribute('src');");
    assert_eq!(refusal.errors, 1, "{}", page.report());
    let after = page.console_eval(&mut doc, &mut forms,
        "if(src.ownerElement!==loaded||src.value!=='original.js'||loaded.getAttributeNode('src')!==src)throw 'protected resource changed';");
    assert_eq!(after.errors, 0, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(page.state.retained, retained);
    assert!(page.take_dynamic_scripts().is_empty() && page.take_dynamic_stylesheets().is_empty());
    let (changed, removed, requests) = page.take_image_changes();
    assert!(changed.is_empty() && removed.is_empty() && requests.is_empty());
    assert!(!page.has_host_tasks() && !page.has_navigation_request());
}

#[test]
fn absent_attribute_removal_preserves_lookup_limits_and_foreign_boundaries() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=keep></p><math id=m title=old></math>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n,"id") == Some("p")).unwrap();
    let foreign = (0..doc.nodes.len()).find(|&n| doc.attr(n,"id") == Some("m")).unwrap();
    let missing = Value::String("missing".into());
    page.begin_job();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let commits = bridge.state.dom_commits;
    bridge.state.copy_work = 0;
    assert!(bridge.call(node_handle(node), "removeAttribute", std::slice::from_ref(&missing)).is_err());
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.state.host_work = 0;
    assert!(bridge.call(node_handle(node), "removeAttribute", std::slice::from_ref(&missing)).is_err());
    bridge.state.host_work = bridge.state.limits.host_work;
    let limit = bridge.state.limits.host.string_bytes;
    bridge.state.limits.host.string_bytes = 2;
    assert!(bridge.call(node_handle(node), "removeAttribute", std::slice::from_ref(&missing)).is_err());
    bridge.state.limits.host.string_bytes = limit;
    assert!(bridge.call(node_handle(foreign), "removeAttribute", std::slice::from_ref(&missing)).is_err());
    assert!(bridge.call(node_handle(foreign), "removeAttributeNS", &[Value::Null, missing.clone()]).is_err());
    assert!(matches!(bridge.call(node_handle(node), "removeAttribute", &[missing]), Ok(Value::Undefined)));
    assert_eq!(bridge.doc.attr(node,"title"), Some("keep"));
    assert_eq!(bridge.state.dom_commits, commits);
    assert!(bridge.call(node_handle(foreign), "removeAttribute", &[Value::String("data-absent".into())]).is_ok());
    assert!(bridge.call(node_handle(foreign), "removeAttribute", &[Value::String("TITLE".into())]).is_ok());
    assert_eq!(bridge.doc.attr(foreign,"title"), Some("old"),"unknown foreign provenance does not prove HTML name folding");
    assert!(bridge.call(node_handle(foreign), "removeAttribute", &[Value::String("title".into())]).is_ok());
    assert_eq!(bridge.doc.attr(foreign,"title"), None);
}

#[test]
fn remove_attribute_ns_matches_exact_local_names_and_preserves_detached_attr_values() {
    let (doc,_,page,outcome) = page("<p id=p title=old x:y=colon xmlns:x=raw></p>", &[r#"
var p=document.getElementById('p'),a=p.getAttributeNode('title'),c=p.getAttributeNode('x:y');
if(p.removeAttributeNS.length!==2||p.removeAttributeNS!==p.removeAttributeNS)throw 'metadata';
p.removeAttributeNS(null,'TITLE');p.removeAttributeNS('urn:x','title');if(a.ownerElement!==p||a.value!=='old')throw 'exact namespace/name';
if(p.removeAttributeNS('', 'title')!==undefined||a.ownerElement!==null||a.value!=='old'||p.hasAttribute('title'))throw 'remove/Attr';
p.removeAttributeNS(undefined,'y');if(c.ownerElement!==p)throw 'raw colon';
p.removeAttributeNS(null,'x:y');if(c.ownerElement!==null||c.value!=='colon')throw 'raw colon removal';
p.removeAttributeNS('http://www.w3.org/2000/xmlns/','x');if(!p.hasAttribute('xmlns:x'))throw 'raw xmlns';
p.removeAttributeNS(null,'xmlns:x');if(p.hasAttribute('xmlns:x'))throw 'unqualified xmlns';
p.removeAttributeNS(null,'bad name');p.removeAttributeNS(null,'');document.title='remove-ns-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"remove-ns-ok");
}

#[test]
fn remove_attribute_ns_conversion_order_and_brands_keep_existing_mutation_policy() {
    let (doc,_,script_page,outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),log='',name={toString:function(){log+='L';p.setAttribute('title','current');return 'title';}};
var ns={toString:function(){log+='N';return '';}};p.removeAttributeNS(ns,name);if(log!=='NL'||p.hasAttribute('title'))throw 'conversion order/reentrant';
p.setAttribute('title','keep');log='';p.removeAttributeNS({toString:function(){log+='N';return null;}},name);if(log!=='NL'||p.getAttribute('title')!=='current')throw 'object null becomes string';
var calls=0,arg={toString:function(){calls++;return '';}};var caught=0;
try{p.removeAttributeNS(arg);}catch(e){if(e.name==='TypeError')caught++;}
for(var bad of [{},window,document,p.style,document.createTextNode('x')])try{p.removeAttributeNS.call(bad,arg,arg);}catch(e){if(e.name==='TypeError')caught++;}
if(calls||caught!==6)throw 'brand/arity before coercion';
var marker={},same=false;try{p.removeAttributeNS(null,{toString:function(){throw marker;}});}catch(e){same=e===marker;}if(!same||p.getAttribute('title')!=='current')throw 'exception identity';
document.title='remove-ns-conversion-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",script_page.report());
    assert_eq!(doc.title,"remove-ns-conversion-ok");
    let (mut doc,mut forms,mut page,_) = page("<img id=i src='https://example.test/a.png'>",&[]);
    let node = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("i")).unwrap();
    let mut bridge = Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    bridge.remove_attribute_ns(node_handle(node),&[Value::Null,Value::String("src".into())]).unwrap();
    assert_eq!(bridge.doc.attr(node,"src"),None);
    assert!(bridge.state.images.entries.contains_key(&node));
    let (changed, removed, pending) = page.take_image_changes();
    assert!(changed.contains(&node) && removed.contains(&node));
    assert!(pending.is_empty());
}

#[test]
fn html_image_is_map_reflects_boolean_presence_without_coercion_and_keeps_attr_identity() {
    let (doc, _, page, outcome) = page("<img id=i ismap='false'>", &[r#"
var i=document.getElementById('i');if(i.isMap!==true||!('isMap' in i))throw 'presence';
var a=i.getAttributeNode('ismap');i.isMap=true;if(i.getAttribute('ismap')!==''||a.value!==''||i.getAttributeNode('ismap')!==a)throw 'canonical/identity';
var calls=0;i.isMap={toString:function(){calls++;throw 'string';},valueOf:function(){calls++;throw 'number';}};
if(calls||!i.isMap)throw 'no conversion hooks';i.isMap=Symbol();if(!i.isMap)throw 'truthy Symbol';
for(var v of [false,0,-0,NaN,'',null,undefined]){i.isMap=true;i.isMap=v;if(i.isMap!==false||i.hasAttribute('ismap'))throw 'falsy removal';}
if(a.ownerElement!==null||a.value!=='')throw 'detached attr';
for(var e of [document.createElement('img'),new Image()]){
 if(e.isMap!==false||e.hasAttribute('ismap'))throw 'created default';
 e.isMap='false';if(!e.isMap||e.getAttribute('ismap')!=='')throw 'truthy string';
 e.removeAttribute('ismap');if(e.isMap)throw 'raw remove';
 e.setAttribute('ismap','anything');if(!e.isMap)throw 'raw set';
}
for(var e of [document.createElement('input'),document.createElement('embed'),document.createElement('iframe'),document.createElement('div'),document.createTextNode('text')])if('isMap' in e||e.isMap!==undefined)throw 'wrong brand';
if('isMap' in document)throw 'Document brand';document.title='image-map-ok';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    assert_eq!(doc.title,"image-map-ok");
}

#[test]
fn html_image_is_map_failures_leave_real_attribute_unchanged() {
    let (mut doc, mut forms, mut page, outcome) = page("<img id=i ismap=old>", &[]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let node = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("i")).unwrap();
    page.begin_job();
    let mut bridge = Bridge { doc:&mut doc, forms:&mut forms, state:&mut page.state };
    bridge.state.host_work = 0;
    assert!(bridge.set(node_handle(node),"isMap",Value::Bool(false)).is_err());
    assert_eq!(bridge.doc.attr(node,"ismap"),Some("old"));
    bridge.state.host_work = bridge.state.limits.host_work;
    bridge.state.copy_work = 0;
    assert!(bridge.set(node_handle(node),"isMap",Value::Bool(true)).is_err());
    assert_eq!(bridge.doc.attr(node,"ismap"),Some("old"));
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.set(node_handle(node),"isMap",Value::Bool(false)).unwrap();
    bridge.state.retained_limit = bridge.state.retained;
    assert!(bridge.set(node_handle(node),"isMap",Value::Bool(true)).is_err());
    assert_eq!(bridge.doc.attr(node,"ismap"),None);
    bridge.doc.nodes[node].namespace = crate::html::Namespace::UnknownForeign;
    assert!(bridge.get(node_handle(node),"isMap").unwrap_err().contains("namespace"));
}

#[test]
fn html_border_reflects_real_strings_and_legacy_null_only_on_its_brands() {
    let (doc, _, page, outcome) = page("<img id=i border=' 05px '><table id=t border=' 05px '></table><p id=p>ordinary</p>", &[r#"
if(document.getElementById('i').border!==' 05px '||document.getElementById('t').border!==' 05px ')throw 'raw reflection';
for(var tag of ['img','object','table']){
 var e=document.createElement(tag);if(e.border!==''||!('border' in e)||e.hasAttribute('border'))throw 'empty default';
 e.border=null;if(e.getAttribute('border')!==(tag==='table'?'null':''))throw 'original null';
 var attr=e.getAttributeNode('border');e.border=undefined;if(e.border!=='undefined'||attr.value!=='undefined')throw 'undefined';
 e.border={toString:function(){return null;}};if(e.border!=='null')throw 'object-produced null';
 e.setAttribute('border',null);if(e.border!=='null')throw 'raw setter conversion';
 var log='';e.border={toString:function(){log+='s';return 'raw';},valueOf:function(){log+='v';return 1;}};if(log!=='s'||e.border!=='raw'||attr.value!=='raw'||e.getAttributeNode('border')!==attr)throw 'coercion/identity';
 var caught=false;try{e.border=Symbol();}catch(x){caught=x.name==='TypeError';}if(!caught||e.border!=='raw')throw 'Symbol atomic';
 var marker={},caught=false;try{e.border={toString:function(){throw marker;}};}catch(x){caught=x===marker;}if(!caught||e.border!=='raw')throw 'throw atomic';
 e.removeAttribute('border');if(e.border!==''||attr.ownerElement!==null||attr.value!=='raw')throw 'detach';
}
for(var e of [document.createElement('input'),document.createElement('embed'),document.createElement('iframe'),document.createElement('div'),document.createTextNode('text')]){if('border' in e||e.border!==undefined)throw 'wrong brand';}
if('border' in document)throw 'Document brand';document.title='border-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "border-ok");
}

#[test]
fn html_border_budget_and_unknown_namespace_failures_preserve_existing_attributes() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>ordinary</p>", &["var image=document.createElement('img');image.border='old';"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let node = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img")).unwrap();
    page.begin_job();
    page.state.retained_limit = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.set(node_handle(node), "border", Value::String("longer".into())).is_err());
    assert_eq!(bridge.doc.attr(node, "border"), Some("old"));
    bridge.state.copy_work = 0;
    assert!(bridge.get(node_handle(node), "border").is_err());
    assert_eq!(bridge.doc.attr(node, "border"), Some("old"));
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.doc.nodes[node].namespace = crate::html::Namespace::UnknownForeign;
    assert!(bridge.get(node_handle(node), "border").unwrap_err().contains("namespace"));
}

#[test]
fn html_spacing_reflects_unsigned_attributes_on_real_brands_and_keeps_attr_identity() {
    let (doc, _, page, outcome) = page("<p id=p>ordinary</p>", &[r#"
for(var tag of ['img','input','object','embed','marquee']){
 var e=document.createElement(tag);if(e.vspace!==0||e.hspace!==0||!('vspace' in e)||e.hasAttribute('vspace'))throw 'default/brand';
 e.setAttribute('vspace',' +12px');var a=e.getAttributeNode('vspace');if(e.vspace!==12||a.value!==' +12px')throw 'attribute parser';
 for(var pair of [[1.9,'1'],[-1,'0'],[-4294967295,'1'],[4294967303,'7'],[NaN,'0'],[Infinity,'0']]){
  e.vspace=pair[0];if(e.getAttribute('vspace')!==pair[1]||a.value!==pair[1]||e.getAttributeNode('vspace')!==a)throw 'unsigned reflection';
 }
 e.vspace=2147483647;if(e.vspace!==2147483647)throw 'max';
 for(var text of ['-1','2147483648','garbage']){e.setAttribute('vspace',text);if(e.vspace!==0)throw 'invalid attribute';}
 e.vspace='12px';if(e.getAttribute('vspace')!=='0')throw 'number conversion';
 var calls=0;e.hspace={valueOf:function(){calls++;return 17;}};if(calls!==1||e.hspace!==17||e.getAttribute('hspace')!=='17')throw 'ordered numeric conversion';
 var caught=false;try{e.hspace=Symbol();}catch(x){caught=x.name==='TypeError';}if(!caught||e.hspace!==17)throw 'Symbol atomic';
 e.removeAttribute('vspace');if(e.vspace!==0||a.ownerElement!==null||a.value!=='0')throw 'removal';
}
for(var e of [document.createElement('div'),document.createElement('iframe'),document.createTextNode('text')]){
 if('vspace' in e||e.vspace!==undefined)throw 'wrong brand';
}
if('vspace' in document)throw 'Document brand';document.title='spacing-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "spacing-ok");
}

#[test]
fn html_spacing_work_and_retention_failures_do_not_mutate_the_attribute() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>ordinary</p>", &["var image=document.createElement('img');image.vspace=7;"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let node = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "img")).unwrap();
    page.begin_job();
    page.state.retained_limit = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.set(node_handle(node), "vspace", Value::Number(1000.0)).is_err());
    assert_eq!(bridge.doc.attr(node, "vspace"), Some("7"));
    bridge.state.host_work = 0;
    assert!(bridge.get(node_handle(node), "vspace").is_err());
    assert_eq!(bridge.doc.attr(node, "vspace"), Some("7"));
}

#[test]
fn attr_namespace_lookup_keeps_exact_local_names_raw_colons_and_stable_identity() {
    let (doc, _, page, outcome) = page("<p id=p TITLE=old x:y=colon xmlns='urn:raw' xmlns:x='urn:x' null=n undefined=u></p>", &[r#"
var p=document.getElementById('p'),a=p.getAttributeNode('TITLE');
if(p.getAttributeNodeNS(null,'title')!==a||p.getAttributeNodeNS(undefined,'title')!==a||p.getAttributeNodeNS('','title')!==a||p.getAttributeNodeNS(null,'TITLE')!==null)throw 'namespace/case/identity';
if(p.getAttributeNodeNS('urn:x','title')!==null||p.getAttributeNodeNS('http://www.w3.org/1999/xhtml','title')!==null)throw 'unqualified attrs';
var colon=p.getAttributeNodeNS(null,'x:y');if(colon!==p.getAttributeNode('x:y')||colon.localName!=='x:y'||colon.prefix!==null||colon.namespaceURI!==null||p.getAttributeNodeNS(null,'y')!==null||p.getAttributeNodeNS('urn:x','y')!==null)throw 'raw colon';
if(p.getAttributeNodeNS(null,'xmlns').namespaceURI!==null||p.getAttributeNodeNS('http://www.w3.org/2000/xmlns/','x')!==null||p.getAttributeNodeNS(null,'xmlns:x').localName!=='xmlns:x')throw 'raw xmlns';
if(p.getAttributeNodeNS(null,null).value!=='n'||p.getAttributeNodeNS(undefined,undefined).value!=='u'||p.getAttributeNodeNS(null,'bad name')!==null||p.getAttributeNodeNS(null,'')!==null)throw 'local DOMString/query syntax';
p.setAttribute('title','live');if(p.getAttributeNodeNS(null,'title')!==a||a.value!=='live')throw 'live attr';
p.removeAttributeNode(a);p.setAttribute('title','fresh');if(p.getAttributeNodeNS(null,'title')===a||a.value!=='live'||a.ownerElement!==null)throw 'detach/readd';
var get=p.getAttributeNodeNS;if(get!==p.getAttributeNodeNS||get.call(p,null,'title').value!=='fresh')throw 'method borrowing';
document.title='attr-namespace-lookup-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-namespace-lookup-ok");
}

#[test]
fn attr_namespace_conversion_order_nullability_failures_and_reentrancy_are_real() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),log='',marker={},old=p.getAttributeNode('title');
var ns={toString:function(){log+='N';return '';},valueOf:function(){throw 'wrong hint';}},name={toString:function(){log+='L';return 'title';},valueOf:function(){throw 'wrong hint';}};
if(p.getAttributeNodeNS(ns,name)!==old||log!=='NL')throw 'ordered once conversion';
log='';if(p.getAttributeNodeNS({toString:function(){log+='N';return null;}},name)!==null||log!=='NL')throw 'object-produced null';
log='';if(p.getAttributeNodeNS({toString:function(){log+='N';return undefined;}},name)!==null||log!=='NL')throw 'object-produced undefined';
log='';var rejected=0;try{p.getAttributeNodeNS(ns);}catch(e){if(e.name==='TypeError')rejected++;}if(log!=='')throw 'arity before conversion';
try{p.getAttributeNodeNS(Symbol(),name);}catch(e){if(e.name==='TypeError')rejected++;}if(log!=='')throw 'namespace Symbol first';
try{p.getAttributeNodeNS('urn:x',Symbol());}catch(e){if(e.name==='TypeError')rejected++;}
var caught=false;try{p.getAttributeNodeNS({toString:function(){throw marker;}},name);}catch(e){caught=e===marker;}if(!caught||log!=='')throw 'namespace throw identity';
caught=false;try{p.getAttributeNodeNS(null,{toString:function(){throw marker;}});}catch(e){caught=e===marker;}if(!caught||old.value!=='old')throw 'name throw identity';
var fresh=p.getAttributeNodeNS(null,{toString:function(){p.removeAttribute('title');p.setAttribute('title','after');return 'title';}});
if(fresh===old||fresh.value!=='after'||old.ownerElement!==null||old.value!=='old'||rejected!==3)throw 'post-coercion lookup';
var calls=0,arg={toString:function(){calls++;return 'title';}};
for(var bad of [{},window,document,p.style,p.dataset,document.createTextNode('x')]){try{p.getAttributeNodeNS.call(bad,arg,arg);}catch(e){if(e.name==='TypeError')rejected++;}}
if(calls!==0||rejected!==9)throw 'brand before arguments';
document.title='attr-namespace-conversion-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-namespace-conversion-ok");
}

#[test]
fn attr_namespace_misses_do_not_consume_identity_capacity_or_retained_bytes() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let p = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for i in 0..256 {
        let name = format!("data-{i}");
        bridge.call(node_handle(p), "setAttribute", &[Value::String(name.clone()), Value::String("v".into())]).unwrap();
        bridge.call(node_handle(p), "getAttributeNode", &[Value::String(name)]).unwrap();
    }
    let before = bridge.state.retained;
    for (namespace, name) in [(Value::Null, "missing"), (Value::Null, "TITLE"), (Value::String("urn:x".into()), "title")] {
        assert!(matches!(bridge.call(node_handle(p), "getAttributeNodeNS", &[namespace, Value::String(name.into())]).unwrap(), Value::Null));
    }
    assert_eq!(bridge.state.attributes.len(), 256);
    assert_eq!(bridge.state.retained, before);
    let existing = bridge.call(node_handle(p), "getAttributeNodeNS", &[Value::Null, Value::String("data-0".into())]).unwrap();
    assert!(matches!(existing, Value::Host(handle) if script_attr::index(handle) == Some(0)));
    assert!(bridge.call(node_handle(p), "getAttributeNodeNS", &[Value::Null, Value::String("title".into())]).unwrap_err().contains("256"));
    assert_eq!(bridge.state.retained, before);
}

#[test]
fn attr_namespace_foreign_context_and_failed_new_record_keep_existing_boundaries() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p><svg><g id=f title=unknown></g></svg><template><i id=inert></i></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let find = |id: &str| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
    let p = find("p"); let foreign = find("f"); let inert = find("inert");
    let invalid = [node_handle(0), node_handle(doc.nodes.len()+99), node_handle(inert)];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        assert!(!bridge.accepts_receiver(node_handle(p), "getAttributeNodeNS", handle));
        assert!(bridge.method(handle, "getAttributeNodeNS").is_none());
    }
    let value=bridge.call(node_handle(foreign),"getAttributeNodeNS",&[Value::Null,Value::String("title".into())]).unwrap();
    let Value::Host(saved)=value else {panic!("known SVG has real null-namespace Attr data")};
    let attr=script_attr::index(saved).unwrap();
    assert_eq!(bridge.state.attributes[attr].owner,Some(node_handle(foreign)));
    assert!(matches!(bridge.call(node_handle(foreign),"getAttributeNodeNS",&[Value::String("urn:x".into()),Value::String("title".into())]).unwrap(),Value::Null));
    bridge.doc.nodes[foreign].namespace=crate::html::Namespace::UnknownForeign;
    for namespace in [Value::Null, Value::String("urn:x".into())] {
        assert!(bridge.call(node_handle(foreign), "getAttributeNodeNS", &[namespace, Value::String("title".into())]).is_err());
    }
    bridge.doc.nodes[foreign].namespace=crate::html::Namespace::Svg;
    let count=bridge.state.attributes.len();
    let before = bridge.state.retained;
    bridge.state.retained_limit = before;
    assert!(bridge.call(node_handle(p), "getAttributeNodeNS", &[Value::Null, Value::String("title".into())]).is_err());
    assert_eq!(bridge.state.attributes.len(),count);
    assert_eq!(bridge.state.retained, before);
}

#[test]
fn set_attribute_ns_preserves_expanded_identity_qname_order_and_detached_metadata() {
    let (doc, _, page, outcome) = page("<p id=p p:x=raw></p>", &[r#"
var p=document.getElementById('p'),raw=p.getAttributeNode('p:x');
p.setAttributeNS('urn:a','p:x','one');p.setAttributeNS('urn:b','p:x','two');
var a=p.getAttributeNodeNS('urn:a','x'),b=p.getAttributeNodeNS('urn:b','x');
if(a===raw||a===b||a.name!=='p:x'||a.localName!=='x'||a.prefix!=='p'||a.namespaceURI!=='urn:a'||a.ownerElement!==p)throw 'real namespace records';
if(p.getAttribute('p:x')!=='raw'||p.getAttributeNS(null,'p:x')!=='raw'||p.getAttributeNS('urn:a','x')!=='one'||p.getAttributeNS('urn:a','X')!==null)throw 'distinct lookup';
if(a.compareDocumentPosition(b)!==36||b.compareDocumentPosition(a)!==34)throw 'same QName order';
p.setAttributeNS('urn:a','q:x','new');
if(p.getAttributeNodeNS('urn:a','x')!==a||a.name!=='p:x'||a.prefix!=='p'||a.value!=='new'||a.compareDocumentPosition(b)!==36)throw 'replace value only';
p.setAttribute('p:x','plain-update');if(raw.value!=='plain-update'||a.value!=='new')throw 'QName first match';
a.value='exact-a';if(a.value!=='exact-a'||raw.value!=='plain-update'||b.value!=='two')throw 'Attr exact setter';
p.removeAttribute('p:x');if(raw.ownerElement!==null||raw.value!=='plain-update'||p.getAttributeNode('p:x')!==a)throw 'raw detach first';
if(p.removeAttributeNode(b)!==b||b.ownerElement!==null||b.value!=='two'||b.namespaceURI!=='urn:b'||b.prefix!=='p'||a.ownerElement!==p)throw 'remove exact node';
p.removeAttributeNS('urn:a','x');if(a.ownerElement!==null||a.value!=='exact-a'||a.name!=='p:x'||a.localName!=='x'||a.namespaceURI!=='urn:a')throw 'NS detach metadata';
a.value='detached';p.setAttributeNS('urn:a','z:x','fresh');var fresh=p.getAttributeNodeNS('urn:a','x');
if(fresh===a||fresh.name!=='z:x'||fresh.value!=='fresh'||a.value!=='detached'||p.getAttributeNS(null,'p:x')!==null)throw 'readd new identity';
document.title='set-namespace-identity-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "set-namespace-identity-ok");
}

#[test]
fn set_attribute_ns_validates_after_conversions_and_preserves_abrupt_identity() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),log='',marker={},set=p.setAttributeNS;
if(set.length!==3||set!==p.setAttributeNS||typeof set!=='function')throw 'native method';
var ns={toString:function(){log+='N';return 'urn:a';}},name={toString:function(){log+='Q';return 'a:key';}},value={toString:function(){log+='V';return 'live';}};
if(set.call(p,ns,name,value)!==undefined||log!=='NQV'||p.getAttributeNS('urn:a','key')!=='live')throw 'ordered conversions';
log='';var rejected=0;try{p.setAttributeNS(ns,name);}catch(e){if(e.name==='TypeError')rejected++;}
for(var bad of [{},window,document,p.style,p.dataset,document.createTextNode('x')]){try{set.call(bad,ns,name,value);}catch(e){if(e.name==='TypeError')rejected++;}}
if(log!==''||rejected!==7)throw 'arity/receiver before conversion';
for(var args of [[Symbol(),name,value],['urn:a',Symbol(),value],['urn:a','a:key',Symbol()]]){try{set.apply(p,args);}catch(e){if(e.name==='TypeError')rejected++;}}
if(log!==''||rejected!==10)throw 'Symbol ordered failure';
for(var index of [0,1,2]){var args=['urn:a','a:key','next'];args[index]={toString:function(){throw marker;}};var caught=false;try{set.apply(p,args);}catch(e){caught=e===marker;}if(!caught||p.getAttributeNS('urn:a','key')!=='live')throw 'throw identity/atomicity';}
log='';try{p.setAttributeNS(ns,{toString:function(){log+='Q';return 'bad name';}},value);}catch(e){if(e.name==='InvalidCharacterError')rejected++;}
if(log!=='NQV'||rejected!==11)throw 'all conversions before validation';
var old=p.getAttributeNodeNS('urn:a','key');
p.setAttributeNS('urn:a','incoming:key',{toString:function(){p.removeAttributeNS('urn:a','key');p.setAttributeNS('urn:a','during:key','middle');return 'after';}});
var fresh=p.getAttributeNodeNS('urn:a','key');if(fresh===old||old.ownerElement!==null||old.value!=='live'||fresh.name!=='during:key'||fresh.value!=='after')throw 'reentrant current target';
var order='';Promise.resolve().then(function(){try{p.setAttributeNS(null,'p:title','bad');}catch(e){order=e.name;}});Promise.resolve().then(function(){if(order!=='NamespaceError'||p.getAttribute('title')!=='old')throw 'catchable namespace error';document.title='set-namespace-conversions-ok';});
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "set-namespace-conversions-ok");
}

#[test]
fn set_attribute_ns_xml_combinations_case_and_null_namespace_are_distinct() {
    let (doc, _, page, outcome) = page("<p id=p></p>", &[r#"
var p=document.getElementById('p'),xml='http://www.w3.org/XML/1998/namespace',xmlns='http://www.w3.org/2000/xmlns/';
for(var row of [[null,'p:x','NamespaceError'],['','p:x','NamespaceError'],['urn:a','xml:x','NamespaceError'],[null,'xmlns','NamespaceError'],['urn:a','xmlns:x','NamespaceError'],[xmlns,'x','NamespaceError'],[xmlns,'p:x','NamespaceError'],['urn:a','bad name','InvalidCharacterError'],['urn:a',':x','InvalidCharacterError'],['urn:a','x:','InvalidCharacterError'],['urn:a','p:x/y','InvalidCharacterError'],['urn:a','x=bad','InvalidCharacterError']]){
 var observed='';try{p.setAttributeNS(row[0],row[1],'no');}catch(e){observed=e.name;}if(observed!==row[2])throw row[1]+':'+observed;
}
p.setAttributeNS(xml,'xml:lang','en');p.setAttributeNS(xml,'other:free','yes');p.setAttributeNS(xmlns,'xmlns','urn:default');p.setAttributeNS(xmlns,'xmlns:p','urn:p');
if(p.getAttributeNS(xml,'lang')!=='en'||p.getAttributeNS(xml,'free')!=='yes'||p.getAttributeNodeNS(xmlns,'xmlns').prefix!==null||p.getAttributeNS(xmlns,'p')!=='urn:p')throw 'valid XML combinations';
if(p.namespaceURI!=='http://www.w3.org/1999/xhtml'||!p.isDefaultNamespace('http://www.w3.org/1999/xhtml'))throw 'no invented parser namespace';
p.setAttributeNS(null,'title','one');var title=p.getAttributeNode('title');p.setAttributeNS(undefined,'title','two');p.setAttributeNS('','title','three');
if(title.value!=='three'||p.getAttributeNodeNS(null,'title')!==title||title.prefix!==null||title.namespaceURI!==null)throw 'null normalization';
p.setAttributeNS(null,'TITLE','upper');var upper=p.getAttributeNodeNS(null,'TITLE');
if(upper.name!=='TITLE'||upper.localName!=='TITLE'||p.getAttribute('TITLE')!=='three'||p.getAttributeNS(null,'TITLE')!=='upper')throw 'case preserved';
p.removeAttributeNS(null,'TITLE');if(upper.ownerElement!==null||upper.value!=='upper'||title.ownerElement!==p||p.getAttribute('title')!=='three')throw 'exact case removal';
p.setAttributeNS('urn:case','p:Key','case');if(p.getAttributeNS('urn:case','key')!==null||p.getAttributeNS('URN:case','Key')!==null||p.getAttributeNS('urn:case','Key')!=='case')throw 'case-sensitive expanded name';
p.setAttributeNS('urn:a','1prefix:x:y','colons');p.setAttributeNS('urn:a','1local','digit');
if(p.getAttributeNS('urn:a','x:y')!=='colons'||p.getAttributeNodeNS('urn:a','x:y').prefix!=='1prefix'||p.getAttributeNS('urn:a','1local')!=='digit')throw 'current DOM name validation';
document.title='set-namespace-validation-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "set-namespace-validation-ok");
}

#[test]
fn set_attribute_ns_metadata_cannot_acquire_html_resource_form_or_csp_effects() {
    let (doc, forms, mut page, outcome) = page("<html><body><img id=picture><input id=control value=original><p id=p>Text</p></body></html>", &[r#"
var p=document.createElement('div');p.setAttributeNS('urn:a','id','meta');p.setAttribute('id','updated');
if(p.id!==''||p.getAttribute('id')!=='updated')throw 'raw QName not HTML id';p.id='real';if(p.getAttributeNS(null,'id')!=='real'||p.getAttributeNS('urn:a','id')!=='updated')throw 'IDL exact null namespace';
p.setAttributeNS(null,'ID','UPPER');if(p.id!=='real')throw 'uppercase not reflected';
p.setAttributeNS('urn:a','class','meta');p.className='real';if(p.className!=='real'||p.getAttributeNS('urn:a','class')!=='meta')throw 'class reflection';
p.setAttributeNS('urn:a','data-mode','meta');if(p.dataset.mode!==undefined||Object.keys(p.dataset).length!==0)throw 'dataset excludes namespaces';p.dataset.mode='real';if(p.getAttributeNS('urn:a','data-mode')!=='meta'||p.dataset.mode!=='real')throw 'dataset null write';
var img=document.getElementById('picture');img.setAttributeNS('urn:a','src','/never-one');img.setAttribute('src','/never-two');var source=img.getAttributeNodeNS('urn:a','src');source.value='/never-three';
if(img.src!==''||!img.complete||img.currentSrc!==''||img.getAttribute('src')!=='/never-three')throw 'image metadata';img.removeAttributeNode(source);if(source.value!=='/never-three'||source.ownerElement!==null)throw 'resource metadata detach';
var link=document.createElement('link');link.setAttributeNS('urn:a','href','/never-sheet');link.setAttribute('href','/still-never');link.rel='stylesheet';document.body.appendChild(link);if(link.href!=='')throw 'link metadata';
var script=document.createElement('script');script.setAttributeNS('urn:a','src','/never-script');script.setAttribute('src','/still-never-script');script.setAttributeNS('urn:a','nonce','meta');document.body.appendChild(script);if(script.src!=='')throw 'script metadata';
var control=document.getElementById('control');control.setAttributeNS('urn:a','value','metadata');control.setAttributeNS('urn:a','disabled','');if(control.value!=='original'||control.disabled)throw 'form metadata';
document.getElementById('p').setAttributeNS('urn:a','style','display:none; background:url(/never-style)');
document.title='set-namespace-policy-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "set-namespace-policy-ok");
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    assert_eq!(page.state.declined_images, 0);
    let p = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    assert_eq!(doc.attr(p, "style"), None);
    let control = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("control")).unwrap();
    assert!(!forms.controls[&control].disabled);
    let script = doc.nodes.iter().position(|n| matches!(&n.kind, NodeKind::Element { tag, .. } if tag=="script")).unwrap();
    assert_eq!(crate::csp::nonce(&doc, script), None);
}

#[test]
fn set_attribute_ns_budget_failures_preserve_records_identity_and_detached_values() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p></p>", &["var p=document.getElementById('p');p.setAttributeNS('urn:a','p:title','old');var retained=p.getAttributeNodeNS('urn:a','title');"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    page.begin_job();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let args = [Value::String("urn:a".into()), Value::String("q:title".into()), Value::String("longer".into())];
    let before = bridge.state.retained;
    bridge.state.retained_limit = before;
    assert!(bridge.call(node_handle(node), "setAttributeNS", &args).is_err());
    assert_eq!(bridge.state.retained, before);
    let NodeKind::Element { attrs, .. } = &bridge.doc.nodes[node].kind else { unreachable!() };
    assert_eq!(attrs.len(), 2); assert_eq!(attrs[1].name.qualified, "p:title"); assert_eq!(attrs[1].value, "old");
    let Value::Host(handle) = bridge.call(node_handle(node), "getAttributeNodeNS", &[Value::String("urn:a".into()), Value::String("title".into())]).unwrap() else { panic!("missing retained Attr") };
    assert!(matches!(bridge.get(handle, "value").unwrap(), Value::String(value) if value=="old"));
    bridge.state.retained_limit = before + 100;
    bridge.state.copy_work = 0;
    assert!(bridge.call(node_handle(node), "setAttributeNS", &args).is_err());
    assert_eq!(bridge.state.retained, before);
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.state.host_work = 0;
    assert!(bridge.call(node_handle(node), "removeAttributeNS", &args[..2]).is_err());
    assert_eq!(bridge.state.attributes[0].owner.and_then(node_index), Some(node));
    bridge.state.host_work = bridge.state.limits.host_work;
    bridge.doc.nodes[node].namespace = crate::html::Namespace::UnknownForeign;
    assert!(bridge.call(node_handle(node), "setAttributeNS", &args).unwrap_err().contains("foreign"));
    assert_eq!(bridge.state.retained, before);
}

#[test]
fn named_map_is_same_object_live_ordered_and_returns_stable_namespace_attrs() {
    let (doc, _, page, outcome) = page("<html><body><p id=p title=old></p><p id=q></p></body></html>", &[r#"
var p=document.getElementById('p'),q=document.getElementById('q'),map=p.attributes;
if(map!==p.attributes||map===q.attributes||map.length!==2||map[0].name!=='id'||map[1].name!=='title')throw 'same object/order';
var title=map.item(1);if(title!==p.getAttributeNode('TITLE')||map.getNamedItem('TITLE')!==title||map.getNamedItemNS(null,'title')!==title||map.getNamedItemNS(null,'TITLE')!==null)throw 'shared identities/names';
p.setAttributeNS('urn:a','p:key','one');p.setAttributeNS('urn:b','p:key','two');
var a=map[2],b=map.item(3);if(map.length!==4||map.getNamedItem('p:key')!==a||map.getNamedItemNS('urn:b','key')!==b)throw 'namespace/order';
p.setAttributeNS('urn:a','q:key','new');if(map[2]!==a||a.name!=='p:key'||a.value!=='new'||map.length!==4)throw 'replace';
p.removeAttribute('title');if(map.length!==3||map[1]!==a||title.ownerElement!==null||title.value!=='old')throw 'live removal';
p.setAttribute('title','fresh');if(map[3]===title||map[3].name!=='title'||map[3].value!=='fresh')throw 'append after remove';
if(map[99]!==undefined||map.item(99)!==null||map.getNamedItem('absent')!==null||map.getNamedItemNS('urn:missing','key')!==null||'99' in map)throw 'missing distinction';
var get=map.getNamedItem;if(get!==map.getNamedItem||get.call(q.attributes,'id')!==q.getAttributeNode('id'))throw 'borrowed current receiver';
document.body.removeChild(p);if(map!==p.attributes||map.length!==4||map[1]!==a||a.ownerElement!==p)throw 'detached owner map';
document.title='named-map-live-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "named-map-live-ok");
}

#[test]
fn named_map_conversion_order_reentrancy_and_brands_use_the_existing_native_path() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),map=p.attributes,log='',marker={};
if(map.item.length!==1||map.getNamedItem.length!==1||map.getNamedItemNS.length!==2)throw 'lengths';
var ns={toString:function(){log+='N';return '';}};var local={toString:function(){log+='L';return 'title';}};
if(map.getNamedItemNS(ns,local)!==p.getAttributeNode('title')||log!=='NL')throw 'ordered DOMString';
var calls=0,index={valueOf:function(){calls++;return 4294967297;},toString:function(){throw 'wrong hint';}};
if(map.item(index)!==map[1]||calls!==1||map.item(NaN)!==map[0]||map.item(Infinity)!==map[0]||map.item(-1)!==null)throw 'unsigned long';
var rejected=0;log='';try{map.getNamedItemNS(ns);}catch(e){if(e.name==='TypeError')rejected++;}
try{map.item();}catch(e){if(e.name==='TypeError')rejected++;}try{map.getNamedItem();}catch(e){if(e.name==='TypeError')rejected++;}
for(var bad of [{},window,document,p,p.style,p.dataset,document.createTextNode('x')]){try{map.getNamedItemNS.call(bad,ns,local);}catch(e){if(e.name==='TypeError')rejected++;}}
if(log!==''||rejected!==10)throw 'brand/arity before conversion';
try{map.getNamedItem(Symbol());}catch(e){if(e.name==='TypeError')rejected++;}try{map.item(Symbol());}catch(e){if(e.name==='TypeError')rejected++;}
var caught=false;try{map.getNamedItem({toString:function(){throw marker;}});}catch(e){caught=e===marker;}if(!caught||rejected!==12)throw 'abrupt completion';
var old=map[1],fresh=map.getNamedItem({toString:function(){p.removeAttribute('title');p.setAttribute('title','after');return 'TITLE';}});
if(fresh===old||fresh!==map[1]||fresh.value!=='after'||old.ownerElement!==null||old.value!=='old')throw 'lookup after conversion';
document.title='named-map-conversion-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "named-map-conversion-ok");
}

#[test]
fn named_map_index_descriptors_readonly_fields_and_missing_properties_are_honest() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),map=p.attributes,calls=0,rhs={toString:function(){calls++;return 'bad';}};
map.length=rhs;map[0]=rhs;p.attributes=rhs;if(calls!==0||map.length!==2||map[0].value!=='p'||p.attributes!==map)throw 'readonly no conversion';
var rejected=0;try{(function(){'use strict';map.length=3;})();}catch(e){if(e.name==='TypeError')rejected++;}
try{(function(){'use strict';p.attributes={};})();}catch(e){if(e.name==='TypeError')rejected++;}if(rejected!==2)throw 'strict readonly';
var d=Object.getOwnPropertyDescriptor(map,'0');if(d.value!==map[0]||d.writable||!d.enumerable||!d.configurable||!Object.hasOwn(map,'0'))throw 'index descriptor';
if(Object.hasOwn(map,'length')||Object.getOwnPropertyDescriptor(map,'item')!==undefined||Object.getOwnPropertyDescriptor(map,'99')!==undefined)throw 'inherited/missing descriptors';
if(delete map[0]||!delete map[99]||map.length!==2||map[0].value!=='p')throw 'indexed deletion';
if(map.offlineAbsent!==undefined||'offlineAbsent' in map||typeof map.toString!=='function'||!('toString' in map))throw 'absence/prototype';
Object.defineProperty(Object.prototype,'mapProbe',{configurable:true,get:function(){if(this!==map)throw 'wrong inherited receiver';return 42;}});
if(map.mapProbe!==42||!('mapProbe' in map)||Object.hasOwn(map,'mapProbe'))throw 'read-only owner prototype tail';delete Object.prototype.mapProbe;
document.title='named-map-descriptors-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "named-map-descriptors-ok");
}

#[test]
fn named_map_laziness_quota_rollback_and_unsupported_surfaces_preserve_real_entries() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let object = node_handle(node);
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let Value::Host(map) = bridge.get(object, "attributes").unwrap() else { panic!("missing map") };
    let before = bridge.state.retained;
    for _ in 0..4 { assert!(bridge.has(map, "0").unwrap()); assert!(matches!(bridge.call(map, "\0NamedNodeMap.get.length",&[]).unwrap(), Value::Number(2.0))); }
    assert!(bridge.state.attributes.is_empty()); assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit = before;
    assert!(bridge.get(map, "0").is_err()); assert!(bridge.state.attributes.is_empty()); assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit = before + size_of::<script_attr::Attr>() + size_of::<usize>() + "title".len();
    bridge.state.copy_work = 0;
    assert!(bridge.get(map, "1").is_err()); assert!(bridge.state.attributes.is_empty());
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    let value = bridge.get(map, "1").unwrap();
    assert!(matches!(value, Value::Host(handle) if script_attr::index(handle)==Some(0)));
    assert_eq!(bridge.state.attributes.len(), 1);
    assert!(bridge.own_keys(map).unwrap_err().contains("enumeration"));
    assert!(bridge.get(map, "title").unwrap_err().contains("named-property"));
    for name in ["setNamedItem", "setNamedItemNS", "removeNamedItem", "removeNamedItemNS"] {
        assert!(bridge.method(map, name).is_none()); assert!(bridge.get(map, name).unwrap_err().contains("not implemented"));
    }
    bridge.doc.nodes[node].namespace = crate::html::Namespace::UnknownForeign;
    assert!(!bridge.named_map_receiver(map)); assert!(bridge.get(object, "attributes").is_err()); assert!(bridge.get(map, "0").is_err());
    assert!(!bridge.accepts_receiver(map, "item", map));
    let invalid = script_named_map::handle(node_handle(bridge.doc.nodes.len() + 1));
    assert!(!bridge.named_map_receiver(invalid)); assert!(bridge.get(invalid, "length").is_err());
}

#[test]
fn node_text_content_descriptor_wraps_existing_native_access() {
    let (doc, _, page, outcome) = page("<p id=sample>Before</p>", &[r#"
var p = document.getElementById('sample');
var original = Object.getOwnPropertyDescriptor(Node.prototype, 'textContent');
if (!original || !original.enumerable || !original.configurable ||
    typeof original.get !== 'function' || typeof original.set !== 'function')
    throw 'missing native Node descriptor';
Object.defineProperty(Node.prototype, 'textContent', {
    get: original.get,
    set: function(value) { original.set.call(this, value + '!'); }
});
p.textContent = 'After';
if (p.textContent !== 'After!' || original.get.call(p) !== 'After!' ||
    Object.prototype.hasOwnProperty.call(p, 'textContent'))
    throw 'native descriptor did not follow prototype dispatch';
var text = document.createTextNode('old');
text.textContent = 'new';
if (text.data !== 'new!' || original.get.call(text) !== 'new!')
    throw 'Text must use the inherited setter';
original.set.call(document, 'ignored');
if (original.get.call(document) !== null || p.textContent !== 'After!')
    throw 'Document textContent must not replace its children';
var refused = false;
try { original.get.call({}); } catch (error) { refused = error instanceof TypeError; }
if (!refused) throw 'invalid native receiver';
document.title = 'descriptor passed';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "descriptor passed");
}

#[test]
fn named_map_unsupported_descriptor_definition_never_claims_or_publishes_mutation() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p>", &["var p=document.getElementById('p'),map=p.attributes;"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let outcome = page.console_eval(&mut doc, &mut forms, "Object.defineProperty(map,'0',{value:17,writable:true,enumerable:true,configurable:true});document.title='fabricated success';");
    assert_eq!(outcome.errors, 1);
    assert!(page.report().contains("Native host object prototypes and descriptors are not implemented"));
    assert_ne!(doc.title, "fabricated success");
    let outcome = page.console_eval(&mut doc, &mut forms, "if(map[0].value!=='p'||map.length!==2||map!==p.attributes)throw 'failed definition changed map';document.title='map-definition-refused';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "map-definition-refused");
}

#[test]
fn named_map_normal_browser_materializes_more_than_historical_attr_capacity() {
    let mut markup = String::from("<p");
    for i in 0..600 { markup.push_str(&format!(" data-{i}='{i}'")); }
    markup.push_str(">Text</p>");
    let mut doc = crate::html::parse(&markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    let source = "var p=document.querySelector('p'),m=p.attributes;for(var i=0;i<600;i++){var a=m.item(i);if(a!==m[i]||a.value!==String(i)||a.ownerElement!==p)throw 'live index capacity';}if(m.length!==600||m[600]!==undefined||m.item(600)!==null)throw 'length';document.title='named-map-capacity-ok';";
    let outcome = page.start(&mut doc, &mut forms, &[Script { node: 0, label: "original generous map fixture".into(), source: source.into() }]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "named-map-capacity-ok");
    assert_eq!(page.state.attributes.len(), 600);
    assert_eq!(page.state.limits.host.attribute_identities, 65_536);
}

#[test]
fn tab_index_getter_uses_tag_defaults_and_physical_summary_parent() {
    let (doc, _, page, outcome) = page("<html><body><input id=hidden type=hidden><input id=disabled disabled><a id=a>No href</a><div id=d></div><details><i>lead</i><summary id=first>First</summary><summary id=second>Second</summary></details><summary id=outside>Outside</summary></body></html>", &[r#"
for(var id of ['hidden','disabled','a']){if(document.getElementById(id).tabIndex!==0)throw 'default historical zero';}
for(var id of ['d','second','outside']){if(document.getElementById(id).tabIndex!==-1)throw 'default minus one';}
if(document.getElementById('first').tabIndex!==0||'tabIndex' in document||'tabIndex' in document.createTextNode('x'))throw 'interface/default';
for(var name of ['a','area','button','frame','iframe','input','object','select','textarea']){var n=document.createElement(name);if(!('tabIndex' in n)||n.tabIndex!==0)throw 'detached tag default';}
var details=document.createElement('details'),first=document.createElement('summary'),second=document.createElement('summary');
details.appendChild(first);details.appendChild(second);if(first.tabIndex!==0||second.tabIndex!==-1)throw 'detached summary order';details.removeChild(first);if(first.tabIndex!==-1||second.tabIndex!==0)throw 'summary mutation';
document.title='tab-index-defaults-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "tab-index-defaults-ok");
}

#[test]
fn tab_index_getter_and_native_control_order_share_the_authored_integer_parser() {
    for (text, expected) in [(" +12junk", Some(12)), ("1e3", Some(1)), ("0x10", Some(0)),
        ("-0", Some(0)), ("2147483647", Some(i32::MAX)), ("-2147483648", Some(i32::MIN)),
        ("2147483648", None), ("-2147483649", None), ("+", None), ("\u{a0}2", None), ("\x0b2", None)] {
        let markup = format!("<input id=c tabindex='{text}'><div id=d tabindex='{text}'></div>");
        let source = format!("if(document.getElementById('c').tabIndex!=={}||document.getElementById('d').tabIndex!=={})throw 'integer prefix';", expected.unwrap_or(0), expected.unwrap_or(-1));
        let (doc, forms, page, outcome) = page(&markup, &[&source]);
        assert_eq!(outcome.errors, 0, "{text:?}: {}", page.report());
        let control = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("c")).unwrap();
        assert_eq!(forms.controls[&control].tab_index, expected.unwrap_or(0));
    }
}

#[test]
fn tab_index_summary_context_keeps_foreign_provenance_after_moves() {
    let (mut doc, mut forms, mut page, outcome) = page("<html><body><svg><details id=fd></details><summary id=fs></summary></svg></body></html>", &[r#"
var foreign=document.getElementById('fd'),s=document.createElement('summary');s.id='known-parent';foreign.appendChild(s);
var d=document.createElement('details'),bad=document.getElementById('fs'),x=document.createElement('summary');x.id='known-sibling';d.appendChild(bad);d.appendChild(x);
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let nodes: Vec<_> = ["known-parent", "known-sibling"].iter().map(|id| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(*id)).unwrap()).collect();
    {
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for node in nodes {
            assert!(bridge.get(node_handle(node), "tabIndex").unwrap_err().contains("unknown"));
        }
    }
    let outcome = page.console_eval(&mut doc, &mut forms, "d.removeChild(bad);if(x.tabIndex!==0)throw 'known summary after removal';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn tab_index_and_zero_offset_reads_keep_invalid_foreign_write_and_work_boundaries() {
    let (mut doc, mut forms, mut page, outcome) = page("<html><body><span id=connected>Text</span><svg><g id=foreign></g></svg><template><i id=inert></i></template></body></html>", &[r#"
var detached=document.createElement('input');if(detached.offsetLeft!==0||detached.offsetTop!==0||document.body.offsetLeft!==0||document.body.offsetTop!==0)throw 'defined zero offsets';
var effects=0,rhs={valueOf:function(){effects++;return 2;},toString:function(){effects++;return '2';}};
if((detached.offsetLeft=rhs)!==rhs||detached.offsetLeft!==0||effects!==0)throw 'readonly offset';
var caught=false;try{(function(){'use strict';detached.offsetTop=7;})();}catch(e){caught=e.name==='TypeError';}if(!caught||detached.offsetTop!==0)throw 'strict offset';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let find = |id: &str| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(id)).unwrap();
    let connected = find("connected"); let foreign = find("foreign"); let inert = find("inert");
    let invalid = [node_handle(0), node_handle(doc.nodes.len()+99), node_handle(inert), node_handle(foreign)];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        for property in ["tabIndex", "offsetLeft", "offsetTop"] {
            assert!(bridge.get(handle, property).is_err());
            let has=bridge.has(handle, property);
            assert!(has.is_err(),"invalid handle={handle}, property={property}, result={has:?}");
            assert!(!bridge.readonly_property(handle, property));
        }
    }
    let diagnostic = bridge.get(node_handle(connected), "offsetLeft").unwrap_err();
    assert!(diagnostic.contains("class=span; connected=true; created=false; geometry-attached=false"), "{diagnostic}");
    assert!(diagnostic.len() < 300);
    bridge.set(node_handle(connected), "tabIndex", Value::Number(3.0)).unwrap();
    assert_eq!(bridge.doc.attr(connected, "tabindex"), Some("3"));
    bridge.state.copy_work = 0;
    assert!(bridge.get(node_handle(connected), "tabIndex").is_err());
    bridge.state.host_work = 0;
    assert!(bridge.get(node_handle(connected), "offsetTop").is_err());
}

#[test]
fn attr_identity_ownership_detach_and_live_values_follow_existing_mutations() {
    let (doc, _, page, outcome) = page("<div id=p title=old data-x=one style='color:red'></div><div id=q></div>", &[r#"
var p=document.getElementById('p'),q=document.getElementById('q'),a=p.getAttributeNode('TITLE'),x=p.getAttributeNode('data-x'),s=p.getAttributeNode('style');
if(a!==p.getAttributeNode('title')||p.getAttributeNode('missing')!==null)throw 'identity/absence';
if(a.name!=='title'||a.localName!=='title'||a.nodeName!=='title'||a.value!=='old'||a.nodeType!==2||a.ATTRIBUTE_NODE!==2||a.ownerElement!==p||a.ownerDocument!==document||a.namespaceURI!==null||a.prefix!==null||!a.specified||a.isConnected)throw 'metadata';
if(a.parentNode!==null||a.firstChild!==null||a.lastChild!==null||a.nextSibling!==null||a.previousSibling!==null||a.parentElement!==null)throw 'Attr topology';
if(!a.isDefaultNamespace('http://www.w3.org/1999/xhtml')||a.isDefaultNamespace(null))throw 'attached namespace';
p.setAttribute('title','new');p.dataset.x='two';p.style.color='blue';
if(a.value!=='new'||a.nodeValue!=='new'||a.textContent!=='new'||x.value!=='two'||s.value.indexOf('blue')<0||a!==p.getAttributeNode('title'))throw 'live mutations';
var wrong=false;try{q.removeAttributeNode(a);}catch(e){wrong=e.name==='NotFoundError';}if(!wrong||a.ownerElement!==p)throw 'wrong owner';
var remove=q.removeAttributeNode;if(remove.call(p,a)!==a||a.ownerElement!==null||a.value!=='new'||p.hasAttribute('title')||p.getAttributeNode('title')!==null)throw 'detach';
if(!a.isDefaultNamespace(null)||a.isDefaultNamespace('http://www.w3.org/1999/xhtml'))throw 'detached namespace';
p.setAttribute('title','fresh');if(p.getAttributeNode('title')===a||a.value!=='new')throw 'readd identity';
var missing=false;try{p.removeAttributeNode(a);}catch(e){missing=e.name==='NotFoundError';}if(!missing)throw 'detached removal';
p.removeAttribute('data-x');p.setAttribute('data-x','three');if(x.ownerElement!==null||x.value!=='two'||p.getAttributeNode('data-x')===x)throw 'string removal lifecycle';
document.title='attr-lifecycle-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-lifecycle-ok");
}

#[test]
fn attr_value_aliases_coerce_once_preserve_nullability_and_do_not_forge_brands() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),a=p.getAttributeNode('title'),calls=0;
var value={toString:function(){calls++;return 'once';},valueOf:function(){throw 'wrong hint';}};
a.value=value;if(calls!==1||a.value!=='once'||p.getAttribute('title')!=='once')throw 'attached value';
a.nodeValue=null;if(a.value!=='')throw 'nullable nodeValue';a.textContent=undefined;if(a.value!=='')throw 'nullable textContent';
a.value=null;if(a.value!=='null')throw 'ordinary value';a.nodeValue={toString:function(){return null;}};if(a.value!=='null')throw 'object-produced null';
var marker={},caught=false;try{a.value={toString:function(){throw marker;}};}catch(e){caught=e===marker;}if(!caught||a.value!=='null')throw 'failed value';
var rejected=0;try{a.value=Symbol();}catch(e){if(e.name==='TypeError')rejected++;}
for(var v of [null,{},p,p.style,document,document.createTextNode('x')]){try{p.removeAttributeNode(v);}catch(e){if(e.name==='TypeError')rejected++;}}
try{p.getAttributeNode();}catch(e){if(e.name==='TypeError')rejected++;}
try{p.removeAttributeNode();}catch(e){if(e.name==='TypeError')rejected++;}
var effect=0;for(var v of [{},window,p.style]){try{p.getAttributeNode.call(v,{toString:function(){effect++;return 'title';}});}catch(e){if(e.name==='TypeError')rejected++;}}
if(rejected!==12||effect!==0||a.ownerElement!==p)throw 'brand/arity/order';
var rhs={toString:function(){throw 'readonly coercion';}},result=(a.name=rhs);if(result!==rhs||a.name!=='title')throw 'readonly result';
caught=false;try{(function(){'use strict';a.ownerElement=p;})();}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'strict readonly';
p.removeAttributeNode(a);a.value=value;if(a.value!=='once'||calls!==2||p.hasAttribute('title'))throw 'detached write';
document.title='attr-conversion-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-conversion-ok");
}

#[test]
fn attr_removal_and_setters_keep_resource_control_and_inert_policy() {
    let (mut doc, mut forms, mut page, outcome) = page("<body><input id=p type=checkbox value=initial onclick='return false'><img id=image src='https://example.test/x.png' srcset='responsive.png 1x'><link id=link rel=stylesheet href='https://example.test/'><template><i id=inert title=x></i></template></body>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let nodes: Vec<_> = ["p", "image", "link", "inert"].iter().map(|id| (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(*id)).unwrap()).collect();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for (node, name) in [(nodes[0], "type"), (nodes[0], "onclick"), (nodes[1], "src")] {
        let value = bridge.call(node_handle(node), "getAttributeNode", &[Value::String(name.into())]).unwrap();
        let Value::Host(handle) = value else { panic!("expected real Attr") };
        let original = bridge.doc.attr(node, name).unwrap().to_owned();
        let before = bridge.state.retained;
        assert!(bridge.call(node_handle(node), "removeAttributeNode", &[Value::Host(handle)]).is_err());
        assert!(bridge.set(handle, "value", Value::String("changed".into())).is_err());
        assert_eq!(bridge.doc.attr(node, name), Some(original.as_str()));
        assert_eq!(bridge.state.retained, before);
        assert_eq!(bridge.state.attributes[script_attr::index(handle).unwrap()].owner.and_then(node_index), Some(node));
    }
    assert!(bridge.call(node_handle(nodes[3]), "getAttributeNode", &[Value::String("title".into())]).is_err());
    let input = node_handle(nodes[0]);
    let Value::Host(value) = bridge.call(input, "getAttributeNode", &[Value::String("value".into())]).unwrap() else { panic!("missing checkable value Attr") };
    bridge.set(value, "value", Value::String("changed".into())).unwrap();
    assert_eq!(bridge.doc.attr(nodes[0], "value"), Some("changed"));
    assert_eq!(bridge.forms.controls[&nodes[0]].value.text(), "changed");
    bridge.call(input, "removeAttributeNode", &[Value::Host(value)]).unwrap();
    assert_eq!(bridge.doc.attr(nodes[0], "value"), None);
    assert_eq!(bridge.forms.controls[&nodes[0]].value.text(), "on");
    assert_eq!(bridge.state.attributes[script_attr::index(value).unwrap()].owner, None);
    let link=node_handle(nodes[2]);
    let Value::Host(href)=bridge.call(link,"getAttributeNode",&[Value::String("href".into())]).unwrap() else {panic!("missing stylesheet Attr")};
    bridge.set(href,"value",Value::String("https://example.test/changed.css".into())).unwrap();
    assert_eq!(bridge.doc.attr(nodes[2],"href"),Some("https://example.test/changed.css"));
    assert_eq!(bridge.state.attributes[script_attr::index(href).unwrap()].owner.and_then(node_index),Some(nodes[2]));
    assert_eq!(bridge.state.stylesheets.pending.len(),1);
    let request=bridge.state.stylesheets.pending.front().unwrap().clone();
    assert_eq!(request.target.display(),"https://example.test/changed.css");
    assert!(request.error.is_none());
    bridge.call(link,"removeAttributeNode",&[Value::Host(href)]).unwrap();
    assert_eq!(bridge.doc.attr(nodes[2],"href"),None);
    assert_eq!(bridge.state.attributes[script_attr::index(href).unwrap()].owner,None);
    assert!(request.cancel.load(std::sync::atomic::Ordering::Acquire));
    assert!(bridge.state.stylesheets.pending.is_empty());
    assert_eq!(bridge.state.stylesheets.in_flight,0);

}

#[test]
fn attr_identity_limit_and_failed_materialization_are_atomic_and_keep_live_identity() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let p = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let before = bridge.state.retained;
    bridge.state.retained_limit = before;
    assert!(bridge.call(node_handle(p), "getAttributeNode", &[Value::String("title".into())]).is_err());
    assert_eq!(bridge.state.retained, before);
    assert!(bridge.state.attributes.is_empty());
    bridge.state.retained_limit = before + MAX_EXTRA_BYTES;
    for i in 0..256 {
        let name = format!("data-{i}");
        bridge.call(node_handle(p), "setAttribute", &[Value::String(name.clone()), Value::String("x".into())]).unwrap();
        let value = bridge.call(node_handle(p), "getAttributeNode", &[Value::String(name)]).unwrap();
        assert!(matches!(value, Value::Host(handle) if script_attr::index(handle) == Some(i)));
    }
    let retained = bridge.state.retained;
    assert!(bridge.call(node_handle(p), "getAttributeNode", &[Value::String("title".into())]).unwrap_err().contains("256"));
    assert_eq!(bridge.state.attributes.len(), 256);
    assert_eq!(bridge.state.retained, retained);
    let existing = bridge.call(node_handle(p), "getAttributeNode", &[Value::String("data-0".into())]).unwrap();
    assert!(matches!(existing, Value::Host(handle) if script_attr::index(handle) == Some(0)));
}

#[test]
fn attr_detach_retains_exact_value_bytes_and_budget_failures_publish_no_partial_change() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=original></p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let p = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let value = bridge.call(node_handle(p), "getAttributeNode", &[Value::String("title".into())]).unwrap();
    let Value::Host(attr) = value else { panic!("expected Attr") };
    let before = bridge.state.retained;
    bridge.state.host_work = 0;
    assert!(bridge.call(node_handle(p), "removeAttributeNode", &[Value::Host(attr)]).is_err());
    assert_eq!(bridge.doc.attr(p, "title"), Some("original"));
    assert_eq!(bridge.state.attributes[script_attr::index(attr).unwrap()].owner.and_then(node_index), Some(p));
    assert_eq!(bridge.state.retained, before);
    bridge.state.host_work = bridge.state.limits.host_work;
    bridge.state.copy_work = 0;
    assert!(bridge.call(node_handle(p), "removeAttributeNode", &[Value::Host(attr)]).is_err());
    assert_eq!(bridge.doc.attr(p, "title"), Some("original"));
    assert_eq!(bridge.state.retained, before);
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.call(node_handle(p), "removeAttributeNode", &[Value::Host(attr)]).unwrap();
    assert_eq!(bridge.state.retained, before - "title".len());
    assert_eq!(bridge.state.attributes[script_attr::index(attr).unwrap()].value, "original");
    bridge.set(attr, "value", Value::String("longer-value".into())).unwrap();
    assert_eq!(bridge.state.retained, before - "title".len() - "original".len() + "longer-value".len());
    for invalid in [768 + 1, NODE_BASE - 1, script_attr::ATTR_BASE + 1, script_attr::ATTR_BASE + 65_535] {
        assert!(bridge.get(invalid, "value").is_err());
        assert!(bridge.has(invalid, "value").is_err());
        assert!(!bridge.readonly_property(invalid, "name"));
        assert!(bridge.set(invalid, "value", Value::String("x".into())).is_err());
    }
}

#[test]
fn attr_borrowed_methods_reject_invalid_inert_and_non_element_receivers_before_coercion() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p title=old></p><template><i id=inert title=x></i></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let inert = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inert")).unwrap();
    for handle in [node_handle(0), node_handle(inert), node_handle(doc.nodes.len()+99)] {
        page.runtime.define_global("invalidAttrReceiver", Value::Host(handle)).unwrap();
        let outcome = page.console_eval(&mut doc, &mut forms, "var calls=0,p=document.getElementById('p'),arg={toString:function(){calls++;return 'title';}},rejected=0;try{p.getAttributeNode.call(invalidAttrReceiver,arg);}catch(e){if(e.name==='TypeError')rejected++;}try{p.removeAttributeNode.call(invalidAttrReceiver,arg);}catch(e){if(e.name==='TypeError')rejected++;}if(calls!==0||rejected!==2)throw 'invalid native brand';");
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
    assert!(page.state.attributes.is_empty());
}

#[test]
fn attr_coercion_reentrancy_and_promise_resolution_preserve_original_identity() {
    let (doc, _, page, outcome) = page("<p id=p title=old></p>", &[r#"
var p=document.getElementById('p'),a=p.getAttributeNode('title'),effects=0;
a.value={toString:function(){effects++;p.removeAttributeNode(a);p.setAttribute('title','replacement');return 'detached-value';}};
var fresh=p.getAttributeNode('title');
if(effects!==1||a.ownerElement!==null||a.value!=='detached-value'||fresh===a||fresh.value!=='replacement'||fresh.ownerElement!==p)throw 'reentrant owner snapshot';
var name={toString:function(){effects++;p.removeAttribute('title');p.setAttribute('title','after');return 'TITLE';}};
var after=p.getAttributeNode(name);if(effects!==2||after===fresh||fresh.ownerElement!==null||fresh.value!=='replacement'||after.value!=='after')throw 'lookup conversion order';
Promise.resolve(a).then(function(value){if(value!==a||value.value!=='detached-value'||value.ownerElement!==null)throw 'Promise Attr identity';document.title='attr-reentrancy-ok';});
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "attr-reentrancy-ok");
}

#[test]
fn default_namespace_tracks_real_html_nodes_and_physical_text_parents() {
    let (doc, _, page, outcome) = page("<html><body><div id=p xmlns='urn:other'>text</div><b id=empty xmlns=''></b></body></html>", &[r#"
var uri='http://www.w3.org/1999/xhtml',p=document.getElementById('p'),t=document.createTextNode('x');
var e=document.createElement('svg'),image=new Image(),nodes=[document,p,p.firstChild,e,image];
for(var n of nodes){if(!('isDefaultNamespace' in n)||!n.isDefaultNamespace(uri)||n.isDefaultNamespace(null)||n.isDefaultNamespace(undefined)||n.isDefaultNamespace('')||n.isDefaultNamespace('urn:other'))throw 'HTML namespace';}
if(!t.isDefaultNamespace(null)||!t.isDefaultNamespace(undefined)||!t.isDefaultNamespace('')||t.isDefaultNamespace(uri))throw 'unparented text';
e.appendChild(t);if(!t.isDefaultNamespace(uri)||t.isDefaultNamespace(null))throw 'detached parent';
e.removeChild(t);if(!t.isDefaultNamespace(null)||t.isDefaultNamespace(uri))throw 'removed text';
if(!document.getElementById('empty').isDefaultNamespace(uri))throw 'xmlns attribute';
var check=p.isDefaultNamespace;
if(!check.call(document,uri)||!check.call(t,null)||!document.isDefaultNamespace.call(p,uri))throw 'Node borrowing';
document.title='namespace-topology-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "namespace-topology-ok");
}

#[test]
fn default_namespace_nullable_conversion_and_receiver_failures_preserve_effect_order() {
    let (doc, _, page, outcome) = page("<p id=p>coercion</p>", &[r#"
var p=document.getElementById('p'),t=document.createTextNode('x'),calls='',marker={};
var uri={toString:function(){calls+='s';return 'http://www.w3.org/1999/xhtml';},valueOf:function(){throw 'wrong hint';}};
if(!p.isDefaultNamespace(uri)||calls!=='s')throw 'once/string hint';
if(t.isDefaultNamespace({toString:function(){return null;}})||t.isDefaultNamespace({toString:function(){return undefined;}}))throw 'post-coercion nullability';
if(t.isDefaultNamespace('null')||t.isDefaultNamespace('undefined'))throw 'literal nullability';
try{p.isDefaultNamespace({toString:function(){throw marker;}});throw 'missing throw';}catch(e){if(e!==marker)throw e;}
var failures=0;try{p.isDefaultNamespace();}catch(e){if(e.name==='TypeError')failures++;}
for(var v of [Symbol('x'),Object(Symbol('y'))]){try{p.isDefaultNamespace(v);}catch(e){if(e.name==='TypeError')failures++;}}
var arg={toString:function(){calls+='BAD';return '';}};
for(var bad of [{},window,p.style,p.dataset,document.querySelectorAll('p')]){try{p.isDefaultNamespace.call(bad,arg);}catch(e){if(e.name==='TypeError')failures++;}}
if(failures!==8||calls!=='s')throw 'brand/arity/Symbol';
if(document.createTextNode(null).data!=='null')throw 'ordinary string conversion changed';
document.title='namespace-coercion-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "namespace-coercion-ok");
}

#[test]
fn default_namespace_foreign_provenance_survives_moves_and_rejects_unknown_context() {
    let (mut doc, mut forms, mut page, outcome) = page("<html><body><div id=out></div><svg><g id=foreign><i id=inside>f</i></g></svg><p id=html>h</p></body></html>", &[r#"
var g=document.getElementById('foreign'),i=document.getElementById('inside'),out=document.getElementById('out'),uri='http://www.w3.org/1999/xhtml';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let foreign = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("foreign")).unwrap();
    let inside = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inside")).unwrap();
    let text = doc.nodes[inside].children[0];
    for action in ["", "out.appendChild(g);", "out.removeChild(g);"] {
        if !action.is_empty() {
            let outcome = page.console_eval(&mut doc, &mut forms, action);
            assert_eq!(outcome.errors, 0, "{}", page.report());
        }
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        for node in [foreign, inside, text] {
            let error = bridge.call(node_handle(node), "isDefaultNamespace", &[Value::Null]).unwrap_err();
            assert!(error.contains("unsupported foreign parser context"), "{error}");
        }
    }
    let outcome = page.console_eval(&mut doc, &mut forms, "if(!document.getElementById('html').isDefaultNamespace(uri))throw 'ordinary HTML after foreign';var created=document.createElement('svg');g.appendChild(created);if(!created.isDefaultNamespace(uri))throw 'creation provenance';document.title='namespace-provenance-ok';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "namespace-provenance-ok");
}

#[test]
fn default_namespace_uses_actual_document_root_and_retains_invalid_handle_and_work_bounds() {
    for (markup, expected) in [("", true), ("text", true), ("<object>inert</object>", false)] {
        let source = format!("if(document.isDefaultNamespace(null)!=={expected})throw 'document root';");
        let (_, _, page, outcome) = page(markup, &[&source]);
        assert_eq!(outcome.errors, 0, "{}", page.report());
    }
    let (mut doc, mut forms, mut page, outcome) = page("<p>ok</p><template><i id=inert>x</i></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let inert = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inert")).unwrap();
    let invalid = [node_handle(0), node_handle(doc.nodes.len() + 99), node_handle(inert), WINDOW, STYLE_BASE];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        assert!(bridge.method(handle, "isDefaultNamespace").is_none());
        assert!(!bridge.accepts_receiver(DOCUMENT, "isDefaultNamespace", handle));
        assert!(bridge.call(handle, "isDefaultNamespace", &[Value::Null]).is_err());
    }
    bridge.state.host_work = 0;
    assert!(bridge.call(DOCUMENT, "isDefaultNamespace", &[Value::Null]).is_err());
}

fn page(markup: &str, scripts: &[&str]) -> (Document, Forms, Page, Outcome) {
    page_with_limits(markup,scripts,crate::javascript::limits::CAPACITY_REGRESSION)
}

fn page_with_limits(markup: &str, scripts: &[&str], limits:crate::javascript::limits::Limits) -> (Document, Forms, Page, Outcome) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let scripts: Vec<_> = scripts
        .iter()
        .enumerate()
        .map(|(node, source)| Script {
            node,
            source: (*source).into(),
            label: format!("compat script {node}"),
        })
        .collect();
    let mut page = Page::with_limits(limits);
    page.set_location(
        &crate::navigation::address(
            "https://www.google.com/search?q=test",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    let outcome = page.start(&mut doc, &mut forms, &scripts);
    (doc, forms, page, outcome)
}

#[test]
fn page_date_clock_is_integral_utc_and_preserves_monotonic_performance_origin() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let (doc, _, page, outcome) = page(
        "<p>UTC clock</p>",
        &[r#"
var now=Date.now(),current=new Date(),mono=performance.now();
if(!Number.isInteger(now)||!Number.isInteger(current.getTime())||!Number.isInteger(mono))throw 'clock units';
if(Date!==window.Date||Date!==globalThis.Date)throw 'global Date';
if(!Number.isFinite(current.getTime())||mono<0||mono>=86400000)throw 'clock origins';
if(Date.parse(current.toISOString())!==current.getTime())throw 'UTC roundtrip';
if(JSON.parse(JSON.stringify({when:new Date(0)})).when!=='1970-01-01T00:00:00.000Z')throw 'Date JSON';
document.title=String(now);
"#],
    );
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let observed: f64 = doc.title.parse().unwrap();
    assert!(observed >= before.min(after) && observed <= before.max(after));
}

#[test]
fn page_date_clock_requires_active_scripting_and_charges_host_work() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>Clock capability</p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    page.state.host_work = 1;
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    let timestamp = bridge.date_now().unwrap();
    assert!(timestamp.is_finite() && timestamp == timestamp.trunc());
    assert_eq!(bridge.state.host_work, 0);
    assert!(bridge.date_now().unwrap_err().contains("DOM work limit"));
    bridge.state.host_work = 10;
    bridge.state.enabled = false;
    assert!(bridge
        .date_now()
        .unwrap_err()
        .contains("scripting is disabled"));
    bridge.state.enabled = true;
    bridge.doc.scripting_enabled = false;
    assert!(bridge
        .date_now()
        .unwrap_err()
        .contains("scripting is disabled"));
}

#[test]
fn dataset_is_live_reflected_and_enumerable_with_stable_identity() {
    let (doc, _, page, outcome) = page(
        "<div id=p data-first-name='Ada' data-x--y='double' data-α='unicode'></div>",
        &[r#"
var p=document.getElementById('p'),d=p.dataset;
if(d!==p.dataset || d.firstName!=='Ada' || d['x-Y']!=='double' || d['α']!=='unicode')throw 'reflection';
if(Object.keys(d).join(',')!=='firstName,x-Y,α')throw 'attribute order';
d.eqid=17;d.empty='';d.nil=null;d.undef=undefined;
if(p.getAttribute('data-eqid')!=='17'|| d.nil!=='null'||d.undef!=='undefined'||!('empty' in d))throw 'conversion';
d.object={toString(){return 'converted';}};
if(d.object!=='converted')throw 'object ToString';
p.setAttribute('data-first-name','Grace');if(d.firstName!=='Grace')throw 'live read';
if(!delete d.firstName||p.hasAttribute('data-first-name')||'firstName' in d)throw 'delete';
if(d['x-y']!==undefined||!delete d['x-y']||d['x-Y']!=='double')throw 'invalid spelling alias';
var desc=Object.getOwnPropertyDescriptor(d,'eqid');
if(desc.value!=='17'||!desc.enumerable||!desc.configurable||!desc.writable)throw 'descriptor';
if(!Object.prototype.hasOwnProperty.call(d,'eqid')||!d.propertyIsEnumerable('eqid')||!('toString' in d))throw 'own or prototype';
Object.assign(d,{lastName:'Hopper'});var clone=Object.assign({},d);
if(clone.lastName!=='Hopper')throw 'assign';
Object.defineProperty(d,'defined',{value:'yes'});if(p.getAttribute('data-defined')!=='yes')throw 'define';
var keys='';for(var key in d){if(key==='eqid')delete d.empty;keys+=key+',';}
if(keys.indexOf('empty,')!==-1)throw 'for in deletion';
document.title='dataset passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dataset passed");
}

#[test]
fn empty_element_and_style_reads_are_absent_and_keep_dataset_routing() {
    let (doc, _, page, outcome) = page(
        "<p id=p style='color:blue' data-='dataset-empty'>Text</p>",
        &[r#"
var p=document.getElementById('p'),s=p.style,n=0,key={toString(){n++;return '';}};
if(p[key]!==undefined||s[key]!==undefined||n!==2)throw 'empty read/key conversion';
if('' in p||'' in s)throw 'empty presence';
if(p.dataset['']!=='dataset-empty'||p.getAttribute('data-')!=='dataset-empty')throw 'dataset routing';
var marker={},caught=false;try{p[{toString(){throw marker;}}];}catch(e){caught=e===marker;}
if(!caught)throw 'thrown key identity';
if(p.getAttribute('style')!=='color:blue'||p.textContent!=='Text')throw 'native mutation';
document.title='empty-read-ok';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "empty-read-ok");
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    assert_eq!(doc.attr(node, "style"), Some("color:blue"));
    assert_eq!(doc.attr(node, "data-"), Some("dataset-empty"));
    assert!(page.state.timers.is_empty());
}

#[test]
fn empty_element_expandos_round_trip_and_style_writes_remain_rejected() {
    let (doc, _, expando_page, outcome) = page(
        "<p id=p style='color:blue'>Text</p>",
        &[r#"var p=document.getElementById('p');p['']=1;
if(p['']!==1||!('' in p)||Object.keys(p)[0]!==''||p.textContent!=='Text'||p.getAttribute('style')!=='color:blue')throw 'empty expando';
if(!delete p['']||'' in p||p['']!==undefined)throw 'empty deletion';document.title='empty-expando-ok';"#],
    );
    assert_eq!(outcome.errors,0,"{}",expando_page.report());
    assert_eq!(doc.title,"empty-expando-ok");
    for source in ["p.style['']='red';"] {
        let (doc, _, page, outcome) = page(
            "<p id=p style='color:blue'>Text</p>",
            &["var p=document.getElementById('p');", source,
              "if(p['']!==undefined||p.style['']!==undefined||p.textContent!=='Text'||p.getAttribute('style')!=='color:blue')throw 'partial effect';document.title='empty-write-rejected';"],
        );
        assert_eq!(outcome.errors, 1, "{}", page.report());
        assert_eq!(doc.title, "empty-write-rejected");
        assert!(page.report().contains("not writable") || page.report().contains("not supported"));
    }
}

#[test]
fn empty_host_property_reads_keep_handle_validation_and_dom_work_bounds() {
    let (mut doc, mut forms, mut page, outcome) = page("<p id=p>Text</p>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let missing = doc.nodes.len() + 99;
    let node = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("p")).unwrap();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in [node_handle(missing), STYLE_BASE + missing as u32] {
        assert!(bridge.get(handle, "").is_err());
    }
    bridge.state.host_work = 1;
    assert!(matches!(bridge.get(STYLE_BASE + node as u32, "").unwrap(), Value::Undefined));
    assert_eq!(bridge.state.host_work, 0);
    assert!(bridge.get(STYLE_BASE + node as u32, "").unwrap_err().contains("DOM work limit"));
}

#[test]
fn node_constants_are_readonly_on_real_document_element_text_and_detached_nodes() {
    let (doc, _, page, outcome) = page("<p id=p>Text</p>", &[r#"
var p=document.getElementById('p'),text=document.createTextNode('x'),detached=document.createElement('i');
var nodes=[document,p,text,detached],pairs=[
['ELEMENT_NODE',1],['ATTRIBUTE_NODE',2],['TEXT_NODE',3],['CDATA_SECTION_NODE',4],
['ENTITY_REFERENCE_NODE',5],['ENTITY_NODE',6],['PROCESSING_INSTRUCTION_NODE',7],
['COMMENT_NODE',8],['DOCUMENT_NODE',9],['DOCUMENT_TYPE_NODE',10],
['DOCUMENT_FRAGMENT_NODE',11],['NOTATION_NODE',12],['DOCUMENT_POSITION_DISCONNECTED',1],
['DOCUMENT_POSITION_PRECEDING',2],['DOCUMENT_POSITION_FOLLOWING',4],
['DOCUMENT_POSITION_CONTAINS',8],['DOCUMENT_POSITION_CONTAINED_BY',16],
['DOCUMENT_POSITION_IMPLEMENTATION_SPECIFIC',32]];
var effects=0,coercions=0,rhs={valueOf:function(){coercions++;return 0;},toString:function(){coercions++;return '0';}};
function strictSet(node,key){'use strict';node[key]=0;}
for(var node of nodes){for(var pair of pairs){var key=pair[0],value=pair[1];
if(!(key in node)||node[key]!==value)throw 'constant value/presence';
var result=(node[key]=(effects++,rhs));if(result!==rhs||node[key]!==value)throw 'readonly value';
var caught=false;try{strictSet(node,key);}catch(e){caught=e.name==='TypeError';}if(!caught)throw 'strict readonly';}}
if(effects!==72||coercions!==0||p.nodeType!==p.ELEMENT_NODE||text.nodeType!==text.TEXT_NODE)throw 'conversion/type';
if('DOCUMENT_TYPE_NODE' in window||'DOCUMENT_TYPE_NODE' in p.style||'DOCUMENT_TYPE_NODE' in document.querySelectorAll('p'))throw 'wrong receiver';
p.id='changed';p.textContent='changed';text.data='new';if(p.id!=='changed'||p.textContent!=='changed'||text.data!=='new')throw 'ordinary writes';
document.title='node-constants-ok';
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "node-constants-ok");
}

#[test]
fn node_constants_reject_root_out_of_bounds_and_inert_node_handles() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>Visible</p><template><p id=inert>Hidden</p></template>", &[]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let inert = (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some("inert")).unwrap();
    let invalid = [node_handle(0), node_handle(doc.nodes.len() + 99), node_handle(inert)];
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for handle in invalid {
        assert!(bridge.get(handle, "DOCUMENT_TYPE_NODE").is_err());
        assert!(bridge.has(handle, "DOCUMENT_TYPE_NODE").is_err());
        assert!(!bridge.readonly_property(handle, "DOCUMENT_TYPE_NODE"));
        assert!(bridge.set(handle, "DOCUMENT_TYPE_NODE", Value::Number(0.0)).is_err());
    }
}

#[test]
fn event_interface_diagnostic_is_bounded_and_does_not_echo_payload_or_claim_support() {
    for (input, class) in [("eVeNt", "base-event"), ("MouseEvents", "mouse-event"), ("KeyboardEvent", "keyboard-event"), ("PRIVATE_INTERFACE_PAYLOAD", "other")] {
        let report = unsupported_event_interface(Some(&Value::String(input.into())));
        assert!(report.contains(&format!("class={class}; UTF-8 bytes=Some({})", input.len())), "{report}");
        assert!(report.contains("not supported") && report.contains("no event was created"));
        assert!(!report.contains("PRIVATE_INTERFACE_PAYLOAD"));
        assert!(report.len() < 256);
    }
    assert!(unsupported_event_interface(None).contains("class=missing"));
    assert!(unsupported_event_interface(Some(&Value::Number(1.0))).contains("class=non-string"));
}

#[test]
fn event_interface_diagnostic_invokes_no_coercion_or_event_lifecycle() {
    let (mut doc, mut forms, mut page, outcome) = page("<p>Events</p>", &["var conversions=0,request={toString:function(){conversions++;return 'Event';}};"]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    // Preserve the original unsupported-factory diagnostic baseline explicitly.
    page.state.disable_event_factory = true;
    let before = (page.state.next_event, page.state.event.is_some(), page.state.listeners.len());
    let outcome = page.console_eval(&mut doc, &mut forms, "document.createEvent(request);");
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert!(page.report().contains("Requested interface class=non-string"), "{}", page.report());
    assert_eq!((page.state.next_event, page.state.event.is_some(), page.state.listeners.len()), before);
    let outcome = page.console_eval(&mut doc, &mut forms, "if(conversions!==0||'createEvent' in document||typeof document.dispatchEvent!=='function'||typeof window.dispatchEvent!=='function')throw 'factory/dispatch isolation';");
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn dataset_validates_keys_and_keeps_native_attribute_readonly() {
    let (doc, _, page, outcome) = page(
        "<p id=p></p>",
        &[r#"
var p=document.getElementById('p'),d=p.dataset,caught='';
try{d['bad-key']='x';}catch(e){caught+=e.name;}
try{d['bad key']='x';}catch(e){caught+=','+e.name;}
try{Object.defineProperty(d,'bad',{value:1,enumerable:false});}catch(e){caught+=','+e.name;}
if(caught!=='SyntaxError,InvalidCharacterError,TypeError')throw caught;
d['']='blank';if(p.getAttribute('data-')!=='blank')throw 'empty key';
d.toString='shadow';if(d.toString!=='shadow'||Object.keys(d).join(',')!==',toString')throw 'override builtins';
delete d.toString;if(typeof d.toString!=='function')throw 'prototype restored';
p.dataset={fake:1};if(p.dataset!==d)throw 'read only dataset';
var strictFailed=false;try{(function(){'use strict';p.dataset={};})();}catch(e){strictFailed=e.name==='TypeError';}
if(!strictFailed)throw 'strict readonly';document.title='dataset validation passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "dataset validation passed");
}

#[test]
fn navigator_expandos_methods_and_descriptors_work_without_replacing_native_facts() {
    let (doc, _, page, outcome) = page(
        "<p>navigator</p>",
        &[r#"
var agent=navigator.userAgent,original=navigator.sendBeacon;
navigator.feature={count:3};navigator.polyfill=function(a){return this===navigator&&a+this.feature.count;};
if(navigator.polyfill(4)!==7||clientInformation.feature!==navigator.feature)throw 'expando or receiver';
navigator.sendBeacon=function(a){return this===navigator&&a==='queued';};
if(!navigator.sendBeacon('queued'))throw 'method override';
if(!delete navigator.sendBeacon||navigator.sendBeacon!==original)throw 'prototype method restored';
Object.defineProperty(navigator,'custom',{value:5,writable:true,enumerable:true,configurable:true});
if(navigator.custom!==5||Object.keys(navigator).join(',')!=='feature,polyfill,custom')throw 'descriptor or enumeration';
if(!delete navigator.custom||'custom' in navigator)throw 'delete';
navigator.userAgent='fake';navigator.cookieEnabled=true;
if(navigator.userAgent!==agent||navigator.cookieEnabled!==true)throw 'native information changed';
if(Object.getOwnPropertyDescriptor(navigator,'userAgent')!==undefined)throw 'inherited native getter';
var failed=false;try{(function(){'use strict';navigator.userAgent='fake';})();}catch(e){failed=e.name==='TypeError';}
if(!failed||!delete navigator.userAgent||navigator.userAgent!==agent)throw 'protected native getter';
document.title='navigator passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "navigator passed");
}

#[test]
fn image_constructor_creates_detached_nodes_without_claiming_a_load() {
    let (doc, _, page, outcome) = page(
        "<p>images</p>",
        &[r#"
var image=new Image(13,17),other=document.createElement('img'),loaded=false;
if(image===other||!(image instanceof Image)||!(other instanceof Image)||({} instanceof Image))throw 'identity';
if(image.tagName!=='IMG'||image.nodeType!==1||image.parentNode!==null||image.parentElement!==null||image.ownerDocument!==document||image.isConnected)throw 'detached';
if(image.width!==13||image.height!==17||image.getAttribute('width')!=='13'||!image.complete||image.src!=='')throw 'initial dimensions';
image.onload=function(){loaded=true;};image.onerror=function(){loaded=true;};
image.id='detached';image.dataset.source='test';image.src='/pixel.png';
if(image.src!=='https://www.google.com/pixel.png'||image.getAttribute('src')!=='/pixel.png'||image.complete||image.currentSrc!==''||image.naturalWidth!==0||image.naturalHeight!==0)throw 'truthful source';
image.setAttribute('src','next.png');if(image.src!=='https://www.google.com/next.png')throw 'src reflection';
image.width=-1;image.height=5.9;if(image.width!==4294967295||image.height!==5)throw 'unsigned long';
image.width={valueOf(){return 18;}};if(image.width!==18)throw 'numeric conversion';
if(document.images.length!==0||document.getElementById('detached')!==null||loaded)throw 'connected or loaded';
document.title='image passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "image passed");
    assert_eq!(page.state.detached_images.len(), 2);
    assert_eq!(page.state.declined_images, 2);
    assert!(page
        .report()
        .contains("no request or load event was fabricated"));
}

#[test]
fn google_adapted_image_and_beacon_snippets_finish_and_decline_network() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<div id=css></div>",
        &[r#"
var src='/gen_204?image=1',iesg=true,cssId='css',eid='local';
window.addEventListener('load',function(){window.n&&window.n();if(document.images){new Image().src=src;}});
typeof navigator.sendBeacon!=='function'&&(navigator.sendBeacon=function(a){(new Image).src=a});
var queued=navigator.sendBeacon('/gen_204?startup=1','');if(queued!==false)throw 'fabricated queue success';
setTimeout(function(){document.getElementById(cssId).setAttribute('style','');if(navigator.sendBeacon('/gen_204?cad=sg_trbl&ei='+eid,'')!==false)throw 'queued';document.title='beacon passed';},0);
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    page.poll_timer_at(
        &mut doc,
        &mut forms,
        Instant::now() + Duration::from_secs(1),
    );
    assert_eq!(page.state.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "beacon passed");
    assert_eq!(page.state.declined_beacons, 2);
    assert_eq!(page.state.declined_images, 1);
    assert_eq!(page.state.timer_callbacks, 1);
}

#[test]
fn element_method_values_are_stable_callable_and_check_their_receiver() {
    let (doc, _, page, outcome) = page(
        "<button id=b>Button</button>",
        &[r#"
var b=document.getElementById('b'),handler=function(){},method=b.addEventListener;
if(typeof method!=='function'||method!==b.addEventListener)throw 'unstable method';
b.dataset.eqid='g1';b&&b.addEventListener?b.addEventListener('click',handler):0;
method.call(b,'click',handler);b.removeEventListener('click',handler);
if(b.getAttribute.call(b,'data-eqid')!=='g1')throw 'extracted native getter';
var failed=false;try{method.call({},'click',handler);}catch(e){failed=e.name==='TypeError';}
if(!failed)throw 'receiver';document.title='native methods passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "native methods passed");
    assert!(page.state.listeners.is_empty());
}

#[test]
fn beacon_rejects_invalid_urls_and_does_not_claim_cross_origin_transmission() {
    let (doc, _, page, outcome) = page(
        "<p>beacons</p>",
        &[r#"
var caught=0;try{navigator.sendBeacon();}catch(e){if(e.name==='TypeError')caught++;}
try{navigator.sendBeacon('file:///private');}catch(e){if(e.name==='TypeError')caught++;}
try{navigator.sendBeacon('javascript:alert(1)');}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==3)throw 'URL validation';
var source={toString(){return 'https://other.example/report';}};
if(navigator.sendBeacon(source,'body')!==false)throw 'cross origin sent';
document.title='beacon validation passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "beacon validation passed");
    assert_eq!(page.state.declined_beacons, 1);
}

#[test]
fn image_growth_is_bounded_and_following_classic_script_recovers() {
    let (doc, _, page, outcome) = page(
        "<p>node limit</p>",
        &[
            "for(var i=0;i<1000;i++){new Image();}",
            "document.title='node limit recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "node limit recovered");
    assert_eq!(page.state.detached_images.len(), MAX_EXTRA_NODES);
    assert!(page.report().contains("DOM appended-node limit reached"));
}

#[test]
fn classic_script_failure_does_not_poison_later_scripts_or_events() {
    let (doc, _, page, outcome) = page(
        "<p>budgets</p>",
        &[
            "window.addEventListener('load',function(){document.title+=' loaded';});while(true);",
            "document.title='following script';",
            "document.addEventListener('DOMContentLoaded',function(){document.title+=' ready';});",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "following script ready loaded");
    assert!(page.report().contains("step limit"));
    assert_eq!(page.state.executed, 3);
}

#[test]
fn aggregate_startup_allowance_stops_a_chain_of_hostile_classic_scripts() {
    let (doc, _, page, outcome) = page(
        "<p>budgets</p>",
        &[
            "while(true);",
            "while(true);",
            "while(true);",
            "while(true);",
            "while(true);",
            "document.title='should not run';",
        ],
    );
    assert_ne!(doc.title, "should not run");
    assert!(outcome.errors >= 2);
    assert!(page.state.executed <= 4, "{}", page.report());
    assert!(page
        .report()
        .contains("Aggregate startup work/time limit reached"));
}

#[test]
fn many_parameters_work_and_parameter_bound_remains_atomic() {
    let parameters = (0..80)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let arguments = (0..80)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let source=format!("function large({parameters}){{return p0+p32+p79+arguments.length;}}var arrow=({parameters})=>p79;if(large({arguments})!==191||large.apply(null,[{arguments}])!==191||arrow({arguments})!==79)throw 'large args';document.title='parameters passed';");
    let (doc, _, page, outcome) = page("<p>arguments</p>", &[&source]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "parameters passed");
    let parameters = (0..257)
        .map(|index| format!("p{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("document.title='bad';function excessive({parameters}){{}};");
    let (doc, _, page, outcome) = self::page(
        "<p>arguments</p>",
        &[&source, "document.title='recovered';"],
    );
    assert_eq!(outcome.errors, 1);
    assert_eq!(doc.title, "recovered");
    assert!(page
        .report()
        .contains("Function parameter limit reached (256)"));
}

#[test]
fn element_methods_can_be_borrowed_and_inspection_does_not_grant_mutation() {
    let (doc, _, page, outcome) = page(
        "<a id=a href='/original'>A</a><a id=b href='/other' data-α='greek'>B</a><input id=c name=choice type=checkbox>",
        &[r#"
var a=document.getElementById('a'),b=document.getElementById('b'),c=document.getElementById('c');
var get=a.getAttribute,set=a.setAttribute;
if(get.call(b,'href')!=='/other'||get.call(c,'type')!=='checkbox'||!c.hasAttribute('name')||b.getAttribute('data-α')!=='greek')throw 'attribute reads';
var conversions='';
set.call(b,{toString(){conversions+='n';return 'data-result';}},{toString(){conversions+='v';return 'converted';}});
if(conversions!=='nv'||b.dataset.result!=='converted'||a.hasAttribute('data-result'))throw 'conversion or receiver';
if(document.getElementById({toString(){return 'b';}})!==b||get.call(b,'missing')!==null)throw 'argument conversion';
var event=function(){};document.addEventListener.call(b,'click',event);a.removeEventListener.call(b,'click',event);
var failures=0;try{get.call({},'id');}catch(e){if(e.name==='TypeError')failures++;}
try{get.call(document,'id');}catch(e){if(e.name==='TypeError')failures++;}
try{get.call(b);}catch(e){if(e.name==='TypeError')failures++;}
try{set.call(b,'data-empty');}catch(e){if(e.name==='TypeError')failures++;}
if(failures!==4)throw 'receiver or required argument';
var premature=false;try{set.call(b,{toString(){premature=true;return 'data-empty';}});}catch(e){}
if(premature)throw 'required arguments must be checked before conversion';
document.title='read methods passed';
"#,
          "document.getElementById('c').setAttribute('type','radio');",
          "if(document.getElementById('b').getAttribute('href')!=='/other'||document.getElementById('c').getAttribute('type')!=='checkbox')throw 'unsafe mutation';document.title+=' safely';"],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "read methods passed safely");
    assert!(page.state.listeners.is_empty());
    assert!(page.report().contains("Supported attributes"));
}

#[test]
fn selector_queries_return_static_ordered_lists_and_match_ancestors() {
    let (doc, _, page, outcome) = page(
        "<body><section id=root class=group><p id=a class=card><span id=inner>A</span></p><p id=b class=card>B</p><input id=check type=checkbox checked></section><p id=c class=card>C</p></body>",
        &[r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b'),inner=document.getElementById('inner');
var list=document.querySelectorAll('.card,#b,.card'),local=root.querySelectorAll('body .card');
if(list.length!==3||local.length!==2||list[0]!==a||list[1]!==b||list[2].id!=='c'||list[3]!==undefined||list['01']!==undefined)throw 'ordered list';
if(list.item(0)!==a||list.item(-1)!==null||list.item(100)!==null||list.item({valueOf(){return 1;}})!==b)throw 'item';
if(Object.keys(list).join(',')!=='0,1,2'||!('0' in list)||'3' in list||list===document.querySelectorAll('.card'))throw 'identity or keys';
if(Object.getOwnPropertyDescriptor(list,'0').writable||Object.getOwnPropertyDescriptor(list,'length')!==undefined)throw 'descriptors';
if(!a.matches('section > p.card')||a.matches('#b')||inner.closest('.group')!==root||a.closest('p')!==a||a.closest('.absent')!==null)throw 'match or closest';
if(a.matches.call(b,'#b')!==true||a.querySelectorAll.call(root,'.card').length!==2||list.item.call(local,1)!==b)throw 'borrowed query';
var check=document.getElementById('check');check.checked=false;
if(check.matches(':checked')||document.querySelectorAll(':checked').length!==0)throw 'checked state';
var image=new Image();if(!image.matches('img')||image.closest('img')!==image||image.closest('body')!==null)throw 'detached match';
var order='',receiver={mark:7};
if(list.forEach(function(node,index,collection){if(this!==receiver||collection!==list)throw 'callback';order+=node.id+index;},receiver)!==undefined||order!=='a0b1c2')throw 'forEach';
var caught=0;try{list.forEach(null);}catch(e){if(e.name==='TypeError')caught++;}
try{list.forEach(function(){throw Error('stop');});}catch(e){if(e.message==='stop')caught++;}
try{list.item();}catch(e){if(e.name==='TypeError')caught++;}
try{root.querySelectorAll('p:unsupported');}catch(e){if(e.name==='SyntaxError')caught++;}
try{a.closest('[');}catch(e){if(e.name==='SyntaxError')caught++;}
try{a.matches();}catch(e){if(e.name==='TypeError')caught++;}
try{(function(){'use strict';list[0]=b;})();}catch(e){if(e.name==='TypeError')caught++;}
if(caught!==7||delete list[0]||!delete list[99])throw 'errors or deletion';
for(var invalid of ['.123','#123','.-1','-1']){var syntax=false;try{a.matches(invalid);}catch(e){syntax=e.name==='SyntaxError';}if(!syntax)throw 'invalid identifier';}
if(a.matches('.--valid'))throw 'valid identifier';
list.length=0;if(list.length!==3)throw 'readonly length';
if(document.querySelector({toString(){return '#b';}},'ignored')!==b)throw 'selector conversion';
document.title='queries passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "queries passed");
}

#[test]
fn selector_snapshots_preserve_detached_nodes_and_keep_live_lookup_separate() {
    let (doc, _, page, outcome) = page_with_limits(
        "<div id=root><span id=a class=card><b id=inner>old</b></span><span id=b class=card>second</span></div><p id=counter>0</p>",
        &[r#"
var root=document.getElementById('root'),list=root.querySelectorAll('.card'),a=list[0],inner=document.getElementById('inner');
a.className='changed';if(list.length!==2||root.querySelectorAll('.card').length!==1)throw 'static membership';
root.textContent='new';
if(list[0]!==a||a.isConnected||a.id!=='a'||a.textContent!=='old'||a.parentNode!==null||inner.parentNode!==a||inner.closest('span')!==a)throw 'detached identity';
if(document.getElementById('a')!==null||document.getElementById('inner')!==null||document.querySelectorAll('span').length!==0||root.querySelectorAll('*').length!==0)throw 'connected lookup';
if(a.querySelector('b')!==inner||a.querySelectorAll('b')[0]!==inner)throw 'detached subtree';
var visited='';list.forEach(function(node){visited+=node.textContent;});if(visited!=='oldsecond')throw 'snapshot contents';
inner.textContent='retained';if(a.textContent!=='retained'||root.textContent!=='new')throw 'detached mutation';
a.textContent='changed';if(a.textContent!=='changed'||inner.parentNode!==null||inner.textContent!=='retained'||root.textContent!=='new')throw 'detached replacement';
var counter=document.getElementById('counter'),counterChildren=counter.childNodes,originalCounter=counterChildren[0],previous=originalCounter;
for(var i=0;i<1000;i++){var priorData=previous.data;counter.textContent=String(i);var current=counterChildren[0];if(current===previous||previous.data!==priorData||previous.parentNode!==null||previous.isConnected)throw 'fresh replacement retains superseded Text';previous=current;}
if(counter.textContent!=='999'||counterChildren.length!==1||originalCounter.data!=='0'||originalCounter.parentNode!==null||originalCounter.isConnected)throw 'counter or saved original';document.title='snapshot passed';
"#],
        crate::javascript::limits::BROWSER,
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "snapshot passed");
    assert_eq!(page.state.detached_nodes.len(), 1007, "Parsed subtree identities, detached replacements and all superseded counter Texts remain retained.");
    assert!(page.state.retained >= 2908, "Every replacement payload, including superseded Text, remains charged");
    assert!(page.state.retained <= page.state.retained_limit);
    assert!(doc.nodes.len() <= page.state.node_limit);
    let before = crate::html::parse("<div id=root><span id=a class=card><b id=inner>old</b></span><span id=b class=card>second</span></div><p id=counter>0</p>").nodes.len();
    assert_eq!(
        doc.nodes.len(),
        before + 1003,
        "Three earlier replacements and all 1000 counter updates create fresh Text nodes"
    );
}

#[test]
fn selector_snapshot_counts_and_retained_references_are_bounded() {
    let (doc, _, page, outcome) = page(
        "<p id=one>one</p>",
        &[
            "for(var i=0;i<129;i++)document.querySelectorAll('p');",
            "document.title='snapshot cap recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "snapshot cap recovered");
    assert_eq!(page.state.node_lists.len(), MAX_NODE_LISTS);
    assert!(page.report().contains("NodeList snapshot limit"));

    let markup = "<div></div>".repeat(512);
    let (mut doc, mut forms, mut page, outcome) = self::page(&markup, &[
        "var listCount=0;function collect(){document.querySelectorAll('div');listCount++;if(listCount<40)setTimeout(collect,0);}collect();",
    ]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    for _ in 0..40 {
        page.poll_timer_at(
            &mut doc,
            &mut forms,
            Instant::now() + Duration::from_secs(1),
        );
    }
    assert_eq!(page.state.node_list_references, MAX_NODE_LIST_REFERENCES);
    assert_eq!(page.state.node_lists.len(), 32);
    assert!(
        page.report().contains("NodeList retained-reference limit"),
        "{}",
        page.report()
    );
}

#[test]
fn selector_result_and_callback_work_limits_do_not_publish_partial_success() {
    let markup = "<div></div>".repeat(4097);
    let (doc, _, page, outcome) = page(
        &markup,
        &[
            "document.querySelectorAll('div');document.title='incorrect';",
            "document.title='result bound recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "result bound recovered");
    assert!(page.state.node_lists.is_empty());
    assert_eq!(page.state.node_list_references, 0);
    assert!(page.report().contains("selector result limit"));
    let (doc, _, page, outcome) = self::page("<p>one</p>", &[
        "var caught=false;try{document.querySelectorAll('p').forEach(function(){while(true);});}catch(e){caught=true;}finally{caught=true;}",
        "if(caught)throw 'budget was catchable';document.title='callback bound recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "callback bound recovered");
    assert!(page.report().contains("step limit"));
}

#[test]
fn detached_snapshot_ids_do_not_shadow_connected_label_targets() {
    let markup = "<div id=host><span id=field>old</span></div><input id=field><label id=label for=field>Click</label><template><input id=field></template>";
    let before = crate::html::parse(markup);
    assert!(
        crate::forms::label_targets(&before).is_empty(),
        "first connected duplicate is not labelable"
    );
    let (doc, forms, page, outcome) = page(
        markup,
        &[r#"
var host=document.getElementById('host'),saved=host.querySelectorAll('span');host.textContent='';
if(saved[0].id!=='field'||saved[0].isConnected||document.getElementById('field').tagName!=='INPUT')throw 'connected IDs';
document.title='label lookup passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "label lookup passed");
    let input = *forms.controls.keys().next().unwrap();
    let label = doc
        .nodes
        .iter()
        .enumerate()
        .find(|(node, _)| doc.attr(*node, "id") == Some("label"))
        .unwrap()
        .0;
    let labels = crate::forms::label_targets(&doc);
    assert_eq!(labels.get(&label), Some(&input));
    assert_eq!(labels.len(), 1);
}

#[test]
fn element_only_selector_methods_are_not_exposed_on_document() {
    let (doc, _, page, outcome) = page("<p>Element selectors</p>", &[
        "document.matches('p');",
        "document.closest('p');",
        "if('matches' in document||'closest' in document)throw 'wrong interface';document.title='interfaces passed';",
    ]);
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "interfaces passed");
    // These are absent ordinary members, not native Document operations.
    let report = page.report();
    for member in ["matches", "closest"] {
        assert!(report.contains(&format!("Actual value type: undefined. Attempted member \"{member}\".")), "{report}");
    }
    assert!(report.contains("Last escaped engine fault: stage=execute kind=TypeError"), "{report}");
}

#[test]
fn dom_creation_insertion_text_and_saved_nodes_keep_their_identity() {
    let (doc, _, page, outcome) = page(
        "<html><head></head><body><div id=root><span id=a>A</span><span id=b>B</span></div><template><p id=hidden>inert</p></template></body></html>",
        &[r#"
var root=document.getElementById('root'),a=document.getElementById('a'),b=document.getElementById('b');
var box=document.createElement('DIV'),text=document.createTextNode({toString(){return 'A😀';}}),last=document.createTextNode('Z');
box.id='new';if(box.tagName!=='DIV'||box.isConnected||box.parentNode!==null)throw 'created element';
if(box.appendChild(text)!==text||box.appendChild(last)!==last||text.nodeType!==3||text.nodeName!=='#text'||text.length!==3||text.data!=='A😀')throw 'text node';
if(box.firstChild!==text||box.lastChild!==last||text.nextSibling!==last||last.previousSibling!==text||text.parentElement!==box||text.ownerDocument!==document)throw 'links';
text.data='new';if(box.textContent!=='newZ')throw 'live text';
if(root.insertBefore(box,b)!==box||document.getElementById('new')!==box||!text.isConnected||box.previousSibling!==a||box.nextSibling!==b)throw 'insert';
var saved=root.querySelectorAll('div');
root.insertBefore(box,box);root.appendChild(a);if(root.firstChild!==box||root.lastChild!==a)throw 'reorder';
if(root.removeChild(box)!==box||box.isConnected||text.isConnected||box.parentNode!==null||text.parentNode!==box||document.getElementById('new')!==null||saved[0]!==box)throw 'remove identity';
root.insertBefore(box,null);if(!box.isConnected||root.lastChild!==box||root.querySelector('#new')!==box)throw 'reinsert';
box.removeChild(text);if(text.parentNode!==null||text.data!=='new'||text.isConnected)throw 'text removal';
text.nodeValue=null;if(text.data!==''||box.textContent!=='Z')throw 'text null';
var templates=document.querySelectorAll('template,p');
if(document.getElementById('hidden')!==null||templates.length!==1||templates[0].localName!=='template'||
   templates[0].content.firstChild.id!=='hidden')throw 'inert lookup and template host identity';
document.title='DOM tree passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "DOM tree passed");
}

#[test]
fn named_form_controls_request_real_focus_and_keep_builtin_priorities() {
    let (doc, forms, mut page, outcome) = page(
        "<html><head></head><body><form id=search name=f><input id=query name=q><input name=textContent><input id=selector-shadow name=querySelector><input name=getElementsByTagName></form><input id=external name=outside form=search><form name=body></form><form name=getElementById></form><form name=other><input name=q></form><template><form name=inert><input name=q></form></template></body></html>",
        &[r#"
if(document.f!==document.getElementById('search')||!('f' in document)||!('q' in document.f))throw 'named form';
if(document.f.q!==document.getElementById('query')||document.f.query!==document.f.q||document.f.outside!==document.getElementById('external'))throw 'owned controls';
// Selectors now use the canonical Element prototype; this genuine named field overrides it.
var selectorField=document.getElementById('selector-shadow');
if(document.f.querySelector!==selectorField||selectorField.form!==document.f||Element.prototype.querySelector.call(document.f,'#query')!==document.f.q)throw 'named selector versus canonical borrowed operation';
var controls=Element.prototype.getElementsByTagName.call(document.f,'input');
if(controls.length!==4||document.f.getElementsByTagName!==controls[3]||controls[3].form!==document.f)throw 'named control and borrowed tag-name operation';
if(document.body.tagName!=='BODY'||typeof document.getElementById!=='function'||typeof document.f.textContent!=='string')throw 'native priority';
if(document.missing!==undefined||document.inert!==undefined||document.f.missing!==undefined||'missing' in document.f)throw 'missing or inert name';
document.f&&document.f.q.focus();document.title='named focus passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "named focus passed");
    let query = forms
        .controls
        .keys()
        .copied()
        .find(|&node| doc.attr(node, "id") == Some("query"))
        .unwrap();
    assert_eq!(page.take_focus_request(), Some(query));
    assert_eq!(page.take_focus_request(), None);
}

#[test]
fn ambiguous_named_form_collections_are_explicitly_unavailable() {
    let (doc, _, page, outcome) = page(
        "<form name=duplicate></form><form name=duplicate></form><form name=f><input name=q><input name=q></form>",
        &[
            "var wrong=document.duplicate;document.title='incorrect';",
            "document.f.q.focus();document.title='incorrect';",
            "document.title='named ambiguity recovered';",
        ],
    );
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "named ambiguity recovered");
    assert!(page.report().contains("live HTMLCollection"));
    assert!(page.report().contains("RadioNodeList"));
    assert_eq!(page.state.dom.pending_focus, None);
}

#[test]
fn created_script_properties_and_text_are_real_without_fabricated_loads() {
    let (doc, _, page, outcome) = page(
        "<body><div id=root></div></body>",
        &[r#"
var script=document.createElement('script'),fired=0;
if(!script.async||script.defer||script.noModule||script.src!==''||script.type!=='')throw 'script defaults';
script.src={toString(){return '/payload.js';}};script.type='text/javascript';script.nonce='nonce-1';script.charset='utf-8';
script.async=false;script.defer=true;script.noModule=true;script.crossOrigin='anonymous';script.referrerPolicy='no-referrer';
script.integrity='sha256-original';
if(script.src!=='https://www.google.com/payload.js'||script.getAttribute('src')!=='/payload.js'||script.async||!script.defer||!script.noModule||script.getAttribute('nonce')!=='nonce-1'||script.getAttribute('crossorigin')!=='anonymous')throw 'reflection';
script.setAttribute('src','next.js');script.removeAttribute('defer');if(script.src!=='https://www.google.com/next.js'||script.defer)throw 'attribute mutation';
script.onload=function(){fired++;};script.onerror=function(){fired++;};
var old=document.createTextNode('old');script.appendChild(old);if(script.text!=='old'||script.textContent!=='old')throw 'script text';
script.text='new';if(old.data!=='old'||old.parentNode!==null||script.firstChild.data!=='new')throw 'retained text';
script.textContent=null;if(script.firstChild!==null||script.text!=='')throw 'empty text';
if(script.isConnected||script.parentNode!==null||fired||document.querySelector('script')!==null)throw 'no invented load';
document.title='script objects passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "script objects passed");
}

#[test]
fn current_script_is_the_genuine_element_through_classic_evaluation_and_checkpoint() {
    let mut doc = crate::html::parse(r#"<html><head><script id=one></script><script id=two></script></head>
<body><div id=currentScript>named property collision</div></body></html>"#);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let script = |id, source: &str| Script {
        node: (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(id)).unwrap(),
        source: source.into(),
        label: format!("original script {id}"),
    };
    let sources = [
        script("one", r#"
var getter = Object.getOwnPropertyDescriptor(Document.prototype, 'currentScript').get;
var one = document.getElementById('one'), microtask = false;
if (document.currentScript !== one || getter.call(document) !== one ||
    Object.hasOwn(document,'currentScript') || !('currentScript' in document))
    throw 'classic element identity';
var rejected = false;
try { getter.call({}); } catch(error) { rejected = error instanceof TypeError; }
if (!rejected || getter.call(document.implementation.createHTMLDocument()) !== null)
    throw 'document brand and independent owner';
Promise.resolve().then(function(){ microtask = document.currentScript === one; });
throw 'intentional first script fault';
"#),
        script("two", r#"
if (!microtask || document.currentScript !== document.getElementById('two'))
    throw 'checkpoint or independent classic element';
window.addEventListener('load',function(){
    if (document.currentScript !== null || getter.call(document) !== null)
        throw 'load inherited previous currentScript';
    document.title = 'current-script-ok';
});
"#),
    ];
    assert_eq!(page.start(&mut doc, &mut forms, &sources).errors, 1, "{}", page.report());
    assert!(page.report().contains("intentional first script fault"));
    assert_eq!(doc.title, "current-script-ok");
    assert_eq!(page.console_eval(&mut doc, &mut forms,
        "if(document.currentScript!==null)throw 'console has no script element';").errors, 0);
}

#[test]
fn connected_created_scripts_queue_once_and_completion_keeps_native_receivers() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head></head><body><div id=root></div></body></html>",
        &[r#"
var first=document.createElement('script'),second=document.createElement('script'),phases='';
window.addEventListener('load',function(e){if(e.target===first){if(!(e.target instanceof HTMLScriptElement)||e.eventPhase!==1||e.bubbles)throw 'capture phase';phases+='capture,';}},true);
first.src='/one.js';first.async=false;
first.onload=function(e){if(!(this instanceof HTMLScriptElement)||this!==first||e.target!==first||e.currentTarget!==first||e.eventPhase!==2||e.bubbles||window.loadedValue!==17||document.currentScript!==null)throw 'load receiver';phases+='target';document.title=phases;};
document.head.appendChild(first);document.head.appendChild(first);
document.head.appendChild(second);second.src='/two.js';
second.onerror=function(e){if(this!==second||e.target!==second||e.bubbles||this.isConnected||this.parentNode!==null)throw 'detached error receiver';document.title+=' + error';};
document.head.removeChild(second);
var data=document.createElement('script');data.type='application/json';data.id='data';data.text='never executed';document.getElementById('root').appendChild(data);
if(!first.isConnected||first.parentNode!==document.head||second.isConnected||!data.isConnected||document.querySelector('script')!==first||document.getElementById('data')!==data||document.getElementById('root').textContent!=='')throw 'physical or inert tree';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let mut requests = page.take_dynamic_scripts();
    assert_eq!(
        requests.len(),
        2,
        "same script must prepare once and data block never prepares"
    );
    let first = requests.pop_front().unwrap();
    let second = requests.pop_front().unwrap();
    assert!(first.error.is_none(), "{:?}", first.error);
    assert!(second.error.is_none(), "{:?}", second.error);
    assert_eq!(first.target.display(), "https://www.google.com/one.js");
    assert_eq!(second.target.display(), "https://www.google.com/two.js");
    let loaded = page.complete_dynamic_script(
        &mut doc,
        &mut forms,
        &first,
        Ok(Script {
            node: first.node,
            source: "if(document.currentScript!==first)throw 'dynamic currentScript';window.loadedValue=17;".into(),
            label: "original local fixture".into(),
        }),
    );
    assert_eq!(loaded.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "capture,target");
    let failed = page.complete_dynamic_script(
        &mut doc,
        &mut forms,
        &second,
        Err("fixture fetch failure".into()),
    );
    assert_eq!(failed.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "capture,target + error");
    assert!(page.take_dynamic_scripts().is_empty());
}

#[test]
fn dom_tree_rejections_preserve_existing_controls_resources_and_parent_links() {
    let (doc, forms, page, outcome) = page(
        "<html><head></head><body><div id=root><span id=child>old</span></div><span id=outside>outside</span><form id=form><input id=query value=original></form><img id=image src=pixel.png srcset='responsive.png 1x'></body></html>",
        &[
            "var root=document.getElementById('root'),child=document.getElementById('child'),outside=document.getElementById('outside');child.appendChild(root);",
            "root.insertBefore(document.createElement('p'),outside);",
            "document.body.removeChild(document.getElementById('form'));",
            "document.body.appendChild(document.createElement('textarea'));",
            "document.body.appendChild(document.getElementById('image'));",
            "var text=document.createTextNode('safe');text.appendChild(document.createElement('span'));",
            "if(root.parentNode!==document.body||child.parentNode!==root||root.textContent!=='old'||outside.parentNode!==document.body||document.getElementById('query').value!=='original'||document.images.length!==1||text.data!=='safe')throw 'partial mutation';document.title='mutation limits recovered';",
        ],
    );
    assert_eq!(outcome.errors, 6, "{}", page.report());
    assert_eq!(doc.title, "mutation limits recovered");
    assert_eq!(forms.controls.len(), 1);
    assert!(page.report().contains("HierarchyRequestError"));
    assert!(page.report().contains("NotFoundError"));
    assert!(page.report().contains("loaded resources"));
}

#[test]
fn dom_creation_validates_arguments_and_borrowed_native_receivers() {
    let (doc, _, page, outcome) = page(
        "<body><div id=p></div></body>",
        &[r#"
var p=document.getElementById('p'),text=document.createTextNode('x'),bad=0;
try{document.createElement('bad name');}catch(e){if(e.name==='InvalidCharacterError')bad++;}
try{document.createElement();}catch(e){if(e.name==='TypeError')bad++;}
try{document.createTextNode();}catch(e){if(e.name==='TypeError')bad++;}
try{p.appendChild({});}catch(e){if(e.name==='TypeError')bad++;}
try{p.insertBefore(text);}catch(e){if(e.name==='TypeError')bad++;}
try{p.appendChild.call({},text);}catch(e){if(e.name==='TypeError')bad++;}
var other=document.createElement('span');p.appendChild.call(other,text);
if(bad!==6||text.parentNode!==other||other.firstChild!==text||p.firstChild!==null)throw 'receiver or validation';
text.nodeType=1;if(text.nodeType!==3)throw 'readonly';document.title='DOM arguments passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "DOM arguments passed");
}

#[test]
fn general_dom_creation_shares_the_existing_node_growth_bound() {
    let (doc, _, page, outcome) = page("<p>bounds</p>", &[
        "for(var i=0;i<1000;i++){if(i%3===0)new Image();else if(i%3===1)document.createElement('div');else document.createTextNode('');}",
        "document.title='DOM node cap recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "DOM node cap recovered");
    assert_eq!(page.state.dom.created.len(), MAX_EXTRA_NODES);
    assert!(page.report().contains("appended-node limit"));
}

#[test]
fn dynamic_dom_depth_is_bounded_and_next_script_can_run() {
    let (doc, _, page, outcome) = page("<p>depth</p>", &[
        "var tree=document.createElement('div'),tail=tree;for(var i=0;i<150;i++){var child=document.createElement('div');tail.appendChild(child);tail=child;}",
        "if(tree.isConnected||tail.parentNode===null)throw 'invalid detached tree';document.title='DOM depth recovered';",
    ]);
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "DOM depth recovered");
    assert!(page.report().contains("tree depth limit"), "{}", page.report());
}

#[test]
fn tag_collections_are_live_scoped_ordered_and_keep_native_member_priorities() {
    let (doc, _, page, outcome) = page(
        "<body><div id=root><p id=one name=alpha>A</p><p id=two>B</p></div><p id=outside>C</p><img id=image><template><p id=inert>hidden</p></template></body>",
        &[r#"
var root=document.getElementById('root'),one=document.getElementById('one'),two=document.getElementById('two');
var all=document.getElementsByTagName('P'),local=root.getElementsByTagName('p'),saved=root.querySelectorAll('p');
if(all.length!==3||local.length!==2||all[2].id!=='outside'||local.item(0)!==one||local[2]!==undefined||local.item(-1)!==null||local.item(100)!==null)throw 'initial collection';
if(local.namedItem('alpha')!==one||local.one!==one||local.namedItem('')!==null||local.namedItem('inert')!==null||local.namedItem({toString(){return 'two';}})!==two)throw 'named collection';
if(local.item({valueOf(){return 1;}})!==two||Object.keys(local).join(',')!=='0,1'||!('one' in local)||'2' in local)throw 'index reflection';
if(Object.getOwnPropertyDescriptor(local,'0').writable||Object.getOwnPropertyDescriptor(local,'length')!==undefined)throw 'index descriptor';
var child=document.createElement('p');child.id='new';root.insertBefore(child,one);
if(local.length!==3||all.length!==4||local[0]!==child||saved.length!==2||saved[0]!==one)throw 'live insert versus snapshot';
root.appendChild(one);if(local[2]!==one||local.namedItem('new')!==child)throw 'live order';
child.id='renamed';if(local.namedItem('new')!==null||local.namedItem('renamed')!==child)throw 'live rename';
child.id='length';if(local.length!==3||local.namedItem('length')!==child)throw 'length priority';
child.id='item';if(typeof local.item!=='function'||local.namedItem('item')!==child)throw 'method priority';
if(local.item.call(document.images,0)!==document.getElementById('image')||document.images.item.call(local,0)!==child)throw 'borrowed collection method';
local.length=0;local[0]=null;if(local.length!==3||local[0]!==child)throw 'readonly collection';
var bad=0;try{local.item();}catch(e){if(e.name==='TypeError')bad++;}
try{local.namedItem();}catch(e){if(e.name==='TypeError')bad++;}
try{root.getElementsByTagName();}catch(e){if(e.name==='TypeError')bad++;}
try{local.item.call({},0);}catch(e){if(e.name==='TypeError')bad++;}
if(bad!==4)throw 'collection receivers';
root.textContent='replacement';if(local.length!==0||all.length!==1||saved[0]!==one||one.parentNode!==null)throw 'live removal';
root.appendChild(child);if(local.length!==1||local[0]!==child||all.length!==2||root.getElementsByTagName('*').length!==1)throw 'retained live root';
document.title='live tag collections passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "live tag collections passed");
}

#[test]
fn parsed_script_nonce_lookup_is_real_without_activating_inert_contents() {
    let (mut doc, mut forms, mut page, outcome) = page(
        "<html><head><script id=seed nonce=YWJj>original_source_only()</script><template><script id=inert nonce=bad>inert_source()</script></template></head><body><p>visible</p></body></html>",
        &[r#"
var scripts=document.getElementsByTagName('SCRIPT'),seed=document.querySelector('script[nonce]');
if(scripts.length!==1||scripts[0]!==seed||seed!==document.getElementById('seed')||seed.nonce!=='YWJj'||seed.getAttribute('nonce')!=='YWJj'||!seed.isConnected)throw 'actual nonce';
if(document.getElementsByTagName('HEAD')[0]!==document.head||document.head.getElementsByTagName('script')[0]!==seed||document.body.getElementsByTagName('script').length!==0)throw 'root scoping';
if(seed.textContent!=='original_source_only()'||seed.text!=='original_source_only()'||document.head.textContent!=='original_source_only()'||document.querySelectorAll('script').length!==1||document.getElementById('inert')!==null)throw 'inert source';
var added=document.createElement('script');added.nonce=seed.nonce;added.src='/nonce-script.js';document.head.appendChild(added);
if(scripts.length!==2||scripts[1]!==added||added.nonce!==seed.nonce)throw 'live script nonce';
document.title='nonce lookup passed';
"#],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "nonce lookup passed");
    let requests = page.take_dynamic_scripts();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].nonce.as_deref(), Some("YWJj"));
    let seed = (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some("seed"))
        .unwrap();
    page.begin_job();
    Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .queue_dynamic_script(seed)
    .unwrap();
    assert!(
        page.take_dynamic_scripts().is_empty(),
        "parsed script must never enter the dynamic queue"
    );
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert_eq!(styles[seed].display, crate::css::Display::None);
}

#[test]
fn parsed_script_tree_mutation_is_rejected_without_partial_source_changes() {
    let (doc, _, page, outcome) = page(
        "<head><script id=seed nonce=YWJj>original()</script></head><body></body>",
        &[
            "var seed=document.querySelector('script[nonce]');seed.appendChild(document.createTextNode('changed()'));",
            "document.head.removeChild(seed);",
            "if(seed.textContent!=='original()'||seed.parentNode!==document.head||!seed.isConnected)throw 'source changed';document.title='parsed script protected';",
        ],
    );
    assert_eq!(outcome.errors, 2, "{}", page.report());
    assert_eq!(doc.title, "parsed script protected");
}

#[test]
fn live_tag_collections_keep_retention_and_repeated_walks_bounded() {
    let (doc, _, page, outcome) = page(
        "<p>collection caps</p>",
        &[
            "for(var i=0;i<1000;i++)document.getElementsByTagName('tag'+i);",
            "document.title='collection cap recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "collection cap recovered");
    assert_eq!(page.state.dom.collections.len(), 128);
    assert!(page.report().contains("tag-collection limit"));

    let markup = format!("<body>{}</body>", "<div>value</div>".repeat(1000));
    let (doc, _, page, outcome) = self::page(
        &markup,
        &[
            "var live=document.getElementsByTagName('div');for(var i=0;i<1000;i++)live.length;",
            "document.title='live work recovered';",
        ],
    );
    assert_eq!(outcome.errors, 1, "{}", page.report());
    assert_eq!(doc.title, "live work recovered");
    assert!(page.report().contains("DOM work limit"));
}

#[test]
fn dynamic_function_callbacks_keep_document_string_compilation_policy() {
    for (header, allowed) in [
        ("script-src 'nonce-original-function-fixture'", false),
        ("script-src 'nonce-original-function-fixture' 'unsafe-eval'", true),
        ("default-src 'none'; script-src-elem 'nonce-original-function-fixture' 'unsafe-eval'", false),
        ("script-src 'nonce-original-function-fixture' 'unsafe-eval', script-src 'nonce-original-function-fixture'", false),
    ] {
        let mut policy = crate::csp::PolicySet::default();
        policy.add_header(header);
        let mut doc = crate::html::parse("<p id=out></p>");
        doc.scripting_enabled = true;
        let mut forms = Forms::new(&doc);
        let mut page = Page::for_test();
        let location = crate::navigation::address(
            "http://127.0.0.1:1/function-policy",
            &crate::navigation::Location::home(),
        ).unwrap();
        page.set_location(&location);
        let source = r#"
var log='';
function attempt(label){
try{
var body="document.getElementById('out').textContent+=label;";
var compiled=label==='timer'?new Function('label',body):Function('label',body);
compiled(label);log+=label+':allowed;';
}catch(error){if(error.name!=='EvalError')throw error;log+=label+':blocked;';}
document.title=log;
}
attempt('script');
Promise.resolve().then(function(){attempt('promise');});
setTimeout(function(){attempt('timer');},0);
"#;
        assert!(policy.allows_inline(0, Some("original-function-fixture"), source).is_ok());
        let scripts = [Script { node: 0, label: "original Function CSP fixture".into(), source: source.into() }];
        let outcome = page.start_with_policy(&mut doc, &mut forms, &scripts, &policy);
        assert_eq!(outcome.errors, 0, "{header}: {}", page.report());
        let result = if allowed { "allowed" } else { "blocked" };
        assert_eq!(doc.title, format!("script:{result};promise:{result};"));
        let outcome = page.poll_timer_at(&mut doc, &mut forms, Instant::now() + Duration::from_secs(1)).unwrap();
        assert_eq!(outcome.errors, 0, "{header}: {}", page.report());
        assert_eq!(doc.title, format!("script:{result};promise:{result};timer:{result};"));
        let out = doc.nodes.iter().enumerate().find_map(|(id, _)| (doc.attr(id, "id") == Some("out")).then_some(id)).unwrap();
        let text = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state }.text_content(out).unwrap();
        assert_eq!(text, if allowed { "scriptpromisetimer" } else { "" });
    }
}


#[test]
fn dynamic_crossorigin_mode_is_snapshotted_and_cross_origin_is_still_denied() {
    let (_, _, mut page, outcome) = page("<html><head></head><body></body></html>", &[r#"
var first=document.createElement('script');first.async=false;first.crossOrigin='anonymous';first.src='/one.js';
document.head.appendChild(first);first.removeAttribute('crossorigin');
var second=document.createElement('script');second.async=false;second.crossOrigin='use-credentials';second.src='/two.js';document.head.appendChild(second);
var third=document.createElement('script');third.async=false;third.crossOrigin='anonymous';third.src='https://other.test/blocked.js';document.head.appendChild(third);
"#]);
    assert_eq!(outcome.errors, 0, "{}", page.report());
    let requests = page.take_dynamic_scripts();
    assert_eq!(requests.len(), 3);
    assert!(requests[0].same_origin_only && requests[0].error.is_none());
    assert!(requests[1].same_origin_only && requests[1].error.is_none());
    assert!(requests[2].same_origin_only);
    assert!(requests[2].error.as_deref().unwrap().contains("CORS-mode"));
}

#[test]
fn loaded_svg_naming_under_custom_html_uses_namespace_separate_from_drawing_and_handlers() {
    let html = r#"<html><body><fixture-shell><fixture-icon>
<svg id="glyph" viewBox="0 0 24 24" preserveAspectRatio="xMidYMid" style="width:24px" aria-hidden="true" focusable="false">
<defs><g><path id="stroke" d="M1 2 L5 6" pathLength="8" fill="url(#unimplemented)" onclick="document.title='unexpected handler'"/></g></defs></svg>
<math id="uncertain"></math></fixture-icon></fixture-shell></body></html>"#;
    let (mut doc, mut forms, mut page, result) = page(html, &[r#"
var v=document.getElementById('glyph'), s=document.getElementById('stroke');
var icon=v.parentElement, ns='http://www.w3.org/2000/svg';
if(v.localName!=='svg'||v.nodeName!=='svg'||v.namespaceURI!==ns||v.prefix!==null||
   !(v instanceof SVGElement)||v instanceof HTMLElement||v.ownerDocument!==document||
   s.localName!=='path'||s.namespaceURI!==ns||s.parentNode.localName!=='g'||
   s.parentNode.parentNode.localName!=='defs'||icon.localName!=='fixture-icon'||
   icon.parentElement.localName!=='fixture-shell'||icon.parentElement.namespaceURI===ns)
    throw 'loaded SVG namespace and topology';
if(v.getAttribute('viewBox')!=='0 0 24 24'||v.getAttribute('viewbox')!==null||
   v.getAttribute('preserveAspectRatio')!=='xMidYMid'||v.getAttribute('style')!=='width:24px'||
   v.getAttribute('aria-hidden')!=='true'||v.getAttribute('focusable')!=='false'||
   s.getAttribute('pathLength')!=='8'||s.getAttribute('fill')!=='url(#unimplemented)'||
   s.getAttribute('onclick')===null)
    throw 'raw naming independent from drawing support';
document.title='loaded SVG naming verified';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "loaded SVG naming verified");
    assert!(page.state.listeners.is_empty());
    let failure = page.console_eval(&mut doc, &mut forms, "document.getElementById('uncertain').localName;");
    assert_eq!(failure.errors, 1);
    assert!(page.report().contains("Receiver tag=math, namespace=unknown parser context, owner=main document, parent=fixture-icon, grandparent=fixture-shell."));
    assert!(page.take_dynamic_scripts().is_empty() && page.take_dynamic_stylesheets().is_empty());
    let (added, removed, changed) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
    assert!(!page.has_navigation_request());
}

#[test]
fn template_svg_insertion_keeps_the_original_checked_foreign_nodes() {
    let (doc, _, mut page, result) = page("<html><head><title>Fixture</title></head><body><main id=app></main></body></html>", &[r#"
var template=document.createElement('template'), panel=document.getElementById('app');
template.innerHTML='<fixture-atlas>\n<svg id="vector" xmlns="http://www.w3.org/2000/svg"><defs><g id="shape"><path d="M2 3 L4 7"/></g></defs></svg></fixture-atlas>';
var donor=template.content, owner=donor.ownerDocument, atlas=donor.firstChild, svg=atlas.lastChild;
var defs=svg.firstChild, shape=defs.firstChild, path=shape.firstChild;
var before=svg.childNodes, held=donor.childNodes, leading=atlas.firstChild;
var marker={};svg.originalMarker=marker;
if(path.isConnected||path.ownerDocument!==owner||document.getElementById('vector')!==null)
    throw 'inert SVG identities';
if(panel.appendChild(donor)!==donor||panel.firstChild!==atlas||atlas.lastChild!==svg||
   document.getElementById('vector')!==svg||document.getElementById('shape')!==shape||
   svg.firstChild!==defs||shape.firstChild!==path||atlas.firstChild!==leading)
    throw 'transferred tree identity';
if(!(svg instanceof SVGElement)||svg instanceof HTMLElement||svg.namespaceURI!=='http://www.w3.org/2000/svg'||
   path.namespaceURI!==svg.namespaceURI||path.ownerDocument!==document||leading.ownerDocument!==document||
   !path.isConnected||path.parentNode!==shape||path.getAttribute('d')!=='M2 3 L4 7'||svg.originalMarker!==marker||
   svg.childNodes!==before||before[0]!==defs||held!==donor.childNodes||held.length!==0||
   donor.ownerDocument!==owner||template.content!==donor)
    throw 'namespace, ownership or child lists';
document.title='checked SVG attached';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "checked SVG attached");
    assert!(page.take_dynamic_scripts().is_empty() && page.take_dynamic_stylesheets().is_empty());
    let (added, removed, changed) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
    assert!(!page.has_navigation_request());
}

#[test]
fn template_svg_presentation_attributes_keep_real_values_aliases_clone_and_guards() {
    let(_,_,mut page,outcome)=page("<html><body><main id=mount></main></body></html>",&[r##"
var seed=document.createElement('template');seed.innerHTML='<svg><defs><path id="shape" d="M0 0 L2 2"/></defs><use id="presented" href="#shape" overflow="visible" stroke-linecap="round" stroke-linejoin="bevel" stroke-miterlimit="4" stroke-dasharray="1 2" stroke-dashoffset=".5" pointer-events="none" vector-effect="non-scaling-stroke" color-interpolation="sRGB" font-family="serif" font-size="12" text-anchor="middle" transform-origin="0 0"/></svg>';
var content=seed.content,svg=content.firstChild,use=svg.children[1],map=use.attributes,overflow=use.getAttributeNode('overflow'),owner=use.ownerDocument;
if(map.getNamedItem('overflow')!==overflow||overflow.ownerElement!==use||overflow.value!=='visible'||use.getAttributeNames().join(',')!=='id,href,overflow,stroke-linecap,stroke-linejoin,stroke-miterlimit,stroke-dasharray,stroke-dashoffset,pointer-events,vector-effect,color-interpolation,font-family,font-size,text-anchor,transform-origin')throw 'real ordered presentation attributes';
var clone=content.cloneNode(true).firstChild.children[1];
if(clone===use||clone.ownerDocument!==owner||clone.getAttribute('overflow')!=='visible'||clone.getAttribute('stroke-dasharray')!=='1 2'||clone.getAttribute('font-family')!=='serif'||clone.getAttribute('vector-effect')!=='non-scaling-stroke'||seed.innerHTML.indexOf('overflow="visible"')<0)throw 'presentation clone and serialization';
document.getElementById('mount').appendChild(content);
if(document.getElementById('presented')!==use||!use.isConnected||overflow.ownerDocument!==document||overflow.ownerElement!==use||overflow.value!=='visible'||use.getAttribute('stroke-linejoin')!=='bevel'||use.getAttribute('pointer-events')!=='none'||use.getAttribute('text-anchor')!=='middle'||clone.isConnected||content.childNodes.length!==0)throw 'presentation owner promotion';
"##]);
    assert_eq!(outcome.errors,0,"{}",page.report());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());let(added,removed,changed)=page.take_image_changes();assert!(added.is_empty()&&removed.is_empty()&&changed.is_empty());assert!(!page.has_navigation_request());
}

#[test]
fn template_svg_local_use_keeps_real_xlink_attributes_clone_and_owner_promotion() {
    let(_,_,mut page,outcome)=page("<html><body><main id=mount></main></body></html>",&[r##"
var seed=document.createElement('template');seed.innerHTML='<svg xmlns:xlink="http://www.w3.org/1999/xlink"><defs><g id="shape"><path d="M0 0 L2 2"/></g><clipPath id="cut"><rect width="2" height="2"/></clipPath><linearGradient id="base-paint"><stop offset="0" stop-color="red"/></linearGradient><linearGradient id="paint" href="#base-paint"/></defs><use id="qualified" xlink:href="#shape" clip-path="url(#cut)" mask="none" fill="url(#paint)"/><use id="plain" href="#shape" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"/></svg>';
var content=seed.content,svg=content.firstChild,qualified=svg.children[1],plain=svg.children[2],uri='http://www.w3.org/1999/xlink',xmlns='http://www.w3.org/2000/xmlns/',owner=qualified.ownerDocument;
var href=qualified.attributes.getNamedItemNS(uri,'href'),decl=svg.attributes.getNamedItemNS(xmlns,'xlink');
if(qualified.nodeName!=='use'||qualified.namespaceURI!==svg.namespaceURI||!(qualified instanceof SVGElement)||qualified.getAttribute('href')!==null||qualified.getAttribute('xlink:href')!=='#shape'||plain.getAttribute('href')!=='#shape'||qualified.getAttributeNames().join(',')!=='id,xlink:href,clip-path,mask,fill')throw 'real local use fields';
if(href!==qualified.getAttributeNode('xlink:href')||href.namespaceURI!==uri||href.prefix!=='xlink'||href.localName!=='href'||href.name!=='xlink:href'||href.ownerElement!==qualified||href.ownerDocument!==owner||decl.prefix!=='xmlns'||decl.localName!=='xlink'||decl.value!==uri)throw 'actual foreign Attr namespaces';
if(plain.attributes.getNamedItemNS(xmlns,'xmlns').value!==svg.namespaceURI||plain.attributes.getNamedItemNS(xmlns,'xlink').value!==uri||svg.firstChild.children[3].getAttribute('href')!=='#base-paint')throw 'local gradient reference and subtree declarations';
var clone=content.cloneNode(true).firstChild.children[1],copy=clone.attributes.getNamedItemNS(uri,'href');
if(clone===qualified||copy===href||copy.ownerElement!==clone||copy.value!=='#shape'||clone.getAttribute('clip-path')!=='url(#cut)'||seed.innerHTML.indexOf('xmlns:xlink="http://www.w3.org/1999/xlink"')<0||seed.innerHTML.indexOf('xlink:href="#shape"')<0)throw 'real local reference clone and serialization';
document.getElementById('mount').appendChild(content);
if(document.getElementById('qualified')!==qualified||!plain.isConnected||qualified.getAttribute('xlink:href')!=='#shape'||qualified.getAttribute('fill')!=='url(#paint)'||href.ownerElement!==qualified||href.ownerDocument!==document||href.value!=='#shape'||href.namespaceURI!==uri||decl.ownerDocument!==document||content.childNodes.length!==0||clone.isConnected)throw 'real reference owner promotion';
"##]);
    assert_eq!(outcome.errors,0,"{}",page.report());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());let(added,removed,changed)=page.take_image_changes();assert!(added.is_empty()&&removed.is_empty()&&changed.is_empty());assert!(!page.has_navigation_request());
}

#[test]
fn template_svg_definition_containers_keep_adjusted_names_clone_and_owner_promotion() {
    let(_,_,mut page,outcome)=page("<html><body><main id=mount></main></body></html>",&[r##"
var seed=document.createElement('template');seed.innerHTML='<svg><defs><clipPath id="cut" clipPathUnits="userSpaceOnUse"><path d="M0 0 L2 2"/></clipPath><linearGradient id="linear" gradientUnits="userSpaceOnUse" gradientTransform="rotate(5)" spreadMethod="pad"><stop offset="0" stop-color="#fff" stop-opacity=".5"/></linearGradient><radialGradient id="radial" fx="1" fy="2" fr="0"><stop offset="1" stop-color="red"/></radialGradient><symbol id="symbol" viewBox="0 0 2 2"><rect width="2" height="2"/></symbol></defs></svg>';
var content=seed.content,svg=content.firstChild,defs=svg.firstChild,view=defs.children,clip=view[0],linear=view[1],radial=view[2],symbol=view[3],owner=clip.ownerDocument,units=clip.getAttributeNode('clipPathUnits');
if(view.length!==4||clip.nodeName!=='clipPath'||clip.localName!=='clipPath'||linear.nodeName!=='linearGradient'||radial.nodeName!=='radialGradient'||symbol.nodeName!=='symbol'||linear.firstChild.nodeName!=='stop'||clip.namespaceURI!==svg.namespaceURI||!(clip instanceof SVGElement)||clip instanceof HTMLElement)throw 'real definition namespace and tag adjustment';
if(clip.getAttribute('clipPathUnits')!=='userSpaceOnUse'||clip.getAttribute('clippathunits')!==null||linear.getAttributeNames().join(',')!=='id,gradientUnits,gradientTransform,spreadMethod'||linear.getAttribute('gradientTransform')!=='rotate(5)'||linear.firstChild.getAttribute('stop-opacity')!=='.5'||radial.getAttribute('fr')!=='0'||symbol.getAttribute('viewBox')!=='0 0 2 2')throw 'actual definition attribute values';
var clone=content.cloneNode(true).firstChild.firstChild;
if(clone.firstChild===clip||clone.firstChild.firstChild===clip.firstChild||clone.firstChild.ownerDocument!==owner||clone.children[1].nodeName!=='linearGradient'||clone.children[1].getAttribute('spreadMethod')!=='pad'||seed.innerHTML.indexOf('<clipPath ')<0||seed.innerHTML.indexOf('gradientTransform="rotate(5)"')<0)throw 'definition clone and serialization';
document.getElementById('mount').appendChild(content);
if(document.getElementById('cut')!==clip||!radial.isConnected||symbol.firstChild.ownerDocument!==document||defs.children!==view||view[0]!==clip||units.ownerElement!==clip||units.ownerDocument!==document||units.value!=='userSpaceOnUse'||clone.isConnected||content.childNodes.length!==0)throw 'definition native owner promotion';
"##]);
    assert_eq!(outcome.errors,0,"{}",page.report());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());let(added,removed,changed)=page.take_image_changes();assert!(added.is_empty()&&removed.is_empty()&&changed.is_empty());assert!(!page.has_navigation_request());
}

#[test]
fn template_svg_mask_container_keeps_namespace_children_clone_and_real_owners() {
    let(_,_,mut page,outcome)=page("<html><body><main id=mount></main></body></html>",&[r#"
var seed=document.createElement('template');seed.innerHTML='<svg><defs><mask id="mask" maskUnits="userSpaceOnUse" maskContentUnits="objectBoundingBox" mask-type="alpha"><rect width="1" height="1" fill="white"/></mask></defs></svg>';
var content=seed.content,svg=content.firstChild,defs=svg.firstChild,mask=defs.firstChild,rect=mask.firstChild,owner=mask.ownerDocument,units=mask.getAttributeNode('maskUnits'),children=mask.children;
if(mask.nodeName!=='mask'||!(mask instanceof SVGElement)||mask.namespaceURI!=='http://www.w3.org/2000/svg'||rect.namespaceURI!==mask.namespaceURI||mask.parentNode!==defs||rect.ownerDocument!==owner||mask.getAttributeNames().join(',')!=='id,maskUnits,maskContentUnits,mask-type'||mask.getAttribute('maskunits')!==null||mask.getAttribute('maskContentUnits')!=='objectBoundingBox'||mask.getAttribute('mask-type')!=='alpha')throw 'actual SVG mask metadata';
var clone=content.cloneNode(true).firstChild.firstChild.firstChild;
if(clone===mask||clone.firstChild===rect||clone.ownerDocument!==owner||clone.getAttribute('maskUnits')!=='userSpaceOnUse'||seed.innerHTML.indexOf('maskContentUnits="objectBoundingBox"')<0)throw 'real mask clone and serialization';
document.getElementById('mount').appendChild(content);
if(document.getElementById('mask')!==mask||!mask.isConnected||rect.ownerDocument!==document||mask.children!==children||children[0]!==rect||units.ownerElement!==mask||units.ownerDocument!==document||units.value!=='userSpaceOnUse'||content.childNodes.length!==0||clone.isConnected)throw 'real mask promotion';
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());let(added,removed,changed)=page.take_image_changes();assert!(added.is_empty()&&removed.is_empty()&&changed.is_empty());assert!(!page.has_navigation_request());
}

#[test]
fn template_svg_style_preserves_actual_attribute_aliases_and_activation_guards() {
    let(mut doc,mut forms,mut page,outcome)=page("<html><body><main id=mount></main></body></html>",&[r#"
var seed=document.createElement('template');seed.innerHTML='<svg id="styled" style="display:none; width:24px" aria-hidden="true" role="img" viewBox="0 0 24 24" data-href="/ordinary"><rect width="2" height="3"/></svg>';
var content=seed.content,svg=content.firstChild,style=svg.getAttributeNode('style'),aria=svg.getAttributeNode('aria-hidden'),map=svg.attributes,children=svg.childNodes;
if(map.getNamedItem('style')!==style||style.ownerElement!==svg)throw 'real inert SVG style Attr';
document.getElementById('mount').appendChild(content);
if(!svg.isConnected||svg.ownerDocument!==document||style.ownerDocument!==document||style.ownerElement!==svg||style.value!=='display:none; width:24px'||svg.childNodes!==children||svg.namespaceURI!=='http://www.w3.org/2000/svg'||content.childNodes.length!==0)throw 'genuine SVG style transfer';
if(aria.ownerElement!==svg||aria.ownerDocument!==document||aria.value!=='true'||svg.getAttribute('viewBox')!=='0 0 24 24'||svg.getAttribute('viewbox')!==null||svg.getAttribute('data-href')!=='/ordinary')throw 'real SVG metadata ownership and adjusted names';
var blocked=document.createElement('template');blocked.innerHTML='<svg style="color:url(/guard)"><rect width="1" height="1"/></svg>';
var donor=blocked.content,unsafe=donor.firstChild,owner=unsafe.ownerDocument,mount=document.getElementById('mount'),view=mount.childNodes;
"#]);
    assert_eq!(outcome.errors,0,"{}",page.report());
    let nodes=doc.nodes.len();let generation=page.state.dom_commits;let retained=page.state.retained;
    let failed=page.console_eval(&mut doc,&mut forms,"mount.appendChild(donor);");
    assert_eq!(failed.errors,1,"{}",page.report());assert_eq!(doc.nodes.len(),nodes);assert_eq!(page.state.dom_commits,generation);assert_eq!(page.state.retained,retained);
    assert!(page.take_dynamic_scripts().is_empty());assert!(page.take_dynamic_stylesheets().is_empty());let(added,removed,changed)=page.take_image_changes();assert!(added.is_empty()&&removed.is_empty()&&changed.is_empty());
    let recovered=page.console_eval(&mut doc,&mut forms,"if(donor.firstChild!==unsafe||unsafe.ownerDocument!==owner||unsafe.isConnected||mount.childNodes!==view||view.length!==1||view[0]!==svg||unsafe.getAttribute('style')!=='color:url(/guard)')throw 'SVG style refusal leaked';unsafe.removeAttribute('style');mount.appendChild(donor);if(!unsafe.isConnected||unsafe.ownerDocument!==document||donor.childNodes.length!==0||view.length!==2)throw 'SVG style safe recovery';");
    assert_eq!(recovered.errors,0,"{}",page.report());
}

#[test]
fn template_svg_keeps_checked_inert_namespaces_and_rolls_back_unsafe_markup() {
    let (mut doc, mut forms, mut page, result) = page("<html><body><main id=app></main></body></html>", &[r#"
var template=document.createElement('template'), uri='http://www.w3.org/2000/svg';
template.innerHTML='<fixture-atlas><svg xmlns="http://www.w3.org/2000/svg" style="display: none; width: 24px" aria-hidden="true" role="img" focusable="false" viewBox="0 0 24 24" preserveAspectRatio="xMidYMid meet" data-href="/ordinary"><defs><g id="mark"><path d="M1 2 L3 4" pathLength="20"/></g><g id="next"><rect x="1" y="2" width="3" height="4"/></g></defs></svg><span id="tail"></span></fixture-atlas>';
var content=template.content, host=content.firstChild, svg=host.firstChild, defs=svg.firstChild;
var mark=defs.firstChild, path=mark.firstChild, owner=content.ownerDocument;
if(svg.nodeName!=='svg'||!(svg instanceof SVGElement)||svg.namespaceURI!==uri||
   defs.namespaceURI!==uri||mark.namespaceURI!==uri||path.namespaceURI!==uri||
   path.getAttribute('d')!=='M1 2 L3 4'||path.getAttribute('pathLength')!=='20'||svg.getAttribute('style')!=='display: none; width: 24px'||mark.nextSibling.getAttribute('id')!=='next'||
   host.lastChild.namespaceURI!=='http://www.w3.org/1999/xhtml'||path.ownerDocument!==owner||
   owner.defaultView!==null||path.isConnected||document.querySelector('svg')!==null)
    throw 'actual private SVG topology';
if(svg.getAttributeNames().join(',')!=='xmlns,style,aria-hidden,role,focusable,viewBox,preserveAspectRatio,data-href'||svg.getAttribute('aria-hidden')!=='true'||svg.getAttribute('role')!=='img'||svg.getAttribute('focusable')!=='false'||svg.getAttribute('data-href')!=='/ordinary')throw 'real ordered SVG declarative attributes';
var copy=content.cloneNode(true).firstChild.firstChild;
if(copy===svg||copy.namespaceURI!==uri||copy.getAttribute('style')!==svg.getAttribute('style')||copy.getAttribute('aria-hidden')!=='true'||copy.getAttribute('viewBox')!=='0 0 24 24'||copy.getAttribute('preserveAspectRatio')!=='xMidYMid meet'||copy.firstChild.firstChild.firstChild===path||
   template.innerHTML.indexOf('<defs><g id="mark"><path')<0)
    throw 'retained SVG serialization or clone';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    let retained = page.state.retained;
    for markup in [
        "<svg><foreignObject><div></div></foreignObject></svg>",
        "<svg><path onclick='trigger()'/></svg>",
        "<svg aria-hidden='true' data-href='/ordinary' onclick='trigger()'></svg>",
        "<svg role='img' href='/guard'></svg>",
        "<svg aria-hidden='true' xlink:href='/guard'></svg>",
        "<svg><defs><mask onclick='trigger()'></mask></defs></svg>",
        "<svg><defs><mask href='/guard'></mask></defs></svg>",
        "<svg><defs><clipPath onclick='trigger()'></clipPath></defs></svg>",
        "<svg><defs><linearGradient href='/guard'></linearGradient></defs></svg>",
        "<svg><filter><feImage href='/guard'/></filter></svg>",
        "<svg><path fill='url(/guard#foreign)'/></svg>",
        "<svg><use href='https://guard.invalid/icon.svg#shape'/></svg>",
        "<svg><use xlink:href='javascript:trigger()'/></svg>",
        "<svg xmlns:xlink='urn:wrong'><use xlink:href='#local'/></svg>",
        "<svg><use other:href='#local'/></svg>",
        "<svg><path clip-path='url(https://guard.invalid/icon.svg#cut)'/></svg>",
        "<svg><path mask='url(/guard#mask)'/></svg>",
        "<svg><use href='#local' overflow='visible' cursor='url(https://guard.invalid/cursor)'/></svg>",
        "<svg><use href='#local' stroke-linecap='round' filter='url(/guard#filter)'/></svg>",
        "<svg><use href='#local' overflow='visible' marker-start='url(#marker)'/></svg>",
        "<svg><image href='/guard'/></svg>",
        "<svg><g></svg>",
        "<svg><g/>",
    ] {
        let result = page.console_eval(&mut doc, &mut forms, &format!("template.innerHTML=\"{markup}\";"));
        assert_eq!(result.errors, 1, "{markup}");
        assert_eq!(page.state.retained, retained, "{markup}");
    }
    let result = page.console_eval(&mut doc, &mut forms, "if(template.content!==content||content.firstChild!==host||path.parentNode!==mark||path.ownerDocument!==owner)throw 'rollback';");
    assert_eq!(result.errors, 0, "{}", page.report());
}

#[test]
fn template_fragment_promotes_original_nodes_and_checks_stylesheet_policy() {
    for blocked in [false, true] {
        let html = "<html><head><title>Original</title></head><body><iframe hidden></iframe><main id=app>Fixture</main></body></html>";
        let source = r#"
var template=document.createElement('template');
template.innerHTML='<theme-scope id="scope"><style is="fixture-sheet">#app{opacity:0.5}</style></theme-scope>';
var donor=template.content, theme=donor.firstChild, style=theme.firstChild, text=style.firstChild;
var owner=donor.ownerDocument, marker={}, head=document.head, anchor=head.lastChild;
var roots=donor.childNodes, nested=theme.childNodes, styleChildren=style.childNodes;
var observer=new MutationObserver(function(){throw 'records should have been taken';});
observer.observe(head,{childList:true});
theme.fixtureMarker=marker;
if(theme.isConnected||style.isConnected||document.getElementById('scope')!==null)
    throw 'inert stylesheet attached';
if(head.insertBefore(donor,anchor)!==donor||theme!==document.getElementById('scope')||
   theme.fixtureMarker!==marker||theme.parentNode!==head||theme.nextSibling!==anchor||
   theme.firstChild!==style||style.firstChild!==text||!style.isConnected||style.getAttribute('is')!=='fixture-sheet'||
   theme.ownerDocument!==document||style.ownerDocument!==document||text.ownerDocument!==document)
    throw 'real transferred nodes';
if(donor.ownerDocument!==owner||template.content!==donor||donor.firstChild!==null||
   roots!==donor.childNodes||roots.length!==0||nested!==theme.childNodes||nested[0]!==style||
   styleChildren!==style.childNodes||styleChildren[0]!==text)
    throw 'live collection or donor changed';
var records=observer.takeRecords(), record=records[0];
if(records.length!==1||record.target!==head||record.addedNodes[0]!==theme||record.nextSibling!==anchor)
    throw 'native mutation record';
observer.disconnect();
document.title='stylesheet nodes promoted';
"#;
        let (mut doc, mut forms, mut page, _) = page(html, &[]);
        if blocked { page.state.dynamic.policy.add_header("style-src 'none'"); }
        let result = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(result.errors, 0, "{}", page.report());
        assert_eq!(doc.title, "stylesheet nodes promoted");
        assert_eq!(page.state.stylesheets.source_bytes > 0, !blocked);
        assert_eq!(page.report().contains("Content Security Policy blocked an inline style element"), blocked);
    }
}

#[test]
fn template_fragment_promotion_rejects_nested_active_resources_atomically() {
    let (mut doc, mut forms, mut page, initial) = page("<html><head><title>Fixture</title></head><body><main></main></body></html>", &[r#"
var holder=document.createElement('template');
holder.innerHTML='<section><style>main{opacity:0.5}</style><iframe src="/private"></iframe></section>';
var content=holder.content, root=content.firstChild, sheet=root.firstChild, frame=root.lastChild;
var owner=content.ownerDocument, first=document.head.firstChild;
"#]);
    assert_eq!(initial.errors, 0, "{}", page.report());
    let failed = page.console_eval(&mut doc, &mut forms, "document.head.appendChild(content);");
    assert_eq!(failed.errors, 1, "{}", page.report());
    let verified = page.console_eval(&mut doc, &mut forms, r#"
if(document.head.firstChild!==first||document.head.lastChild!==first||content.firstChild!==root||
   root.firstChild!==sheet||root.lastChild!==frame||root.ownerDocument!==owner||
   sheet.isConnected||frame.getAttribute('src')!=='/private'||document.querySelector('section')!==null)
    throw 'blocked mutation leaked';
"#);
    assert_eq!(verified.errors, 0, "{}", page.report());
    assert_eq!(page.state.stylesheets.source_bytes, 0);
}

#[test]
fn generated_inert_elements_keep_author_state_and_detach_without_leaking() {
    let (doc, _, page, result) = page("<html><head></head><body><main id=app></main></body></html>", &[r#"
var template=document.createElement('template');
template.innerHTML='<theme-scope id="private-theme"><style>main{color:purple}</style></theme-scope>';
var content=template.content, theme=content.firstChild, style=theme.firstChild, text=style.firstChild;
var owner=content.ownerDocument, marker={};
theme.testMarker=marker;style.testMarker=theme;
if(theme.testMarker!==marker||style.testMarker!==theme||theme.getAttribute('id')!=='private-theme'||
   theme.ownerDocument!==owner||style.ownerDocument!==owner||text.ownerDocument!==owner||
   theme.isConnected||style.isConnected||text.data!=='main{color:purple}'||
   document.getElementById('private-theme')!==null||document.querySelector('theme-scope')!==null)
    throw 'private identities and actual markup';
var copy=content.cloneNode(true).firstChild;
if(copy===theme||copy.testMarker!==undefined||copy.firstChild===style||
   copy.getAttribute('id')!=='private-theme'||copy.ownerDocument!==owner)
    throw 'independent inert clone';
template.innerHTML='<div id="new-inert"></div>';
if(theme.parentNode!==null||theme.firstChild!==style||style.firstChild!==text||
   theme.testMarker!==marker||theme.ownerDocument!==owner||content.firstChild===theme||
   content.firstChild.ownerDocument!==owner||document.querySelector('#new-inert')!==null)
    throw 'detached inert subtree';
document.title='inert author state verified';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "inert author state verified");
}

#[test]
fn inert_elements_get_attribute_from_real_markup_without_active_attachment() {
    let (doc, _, page, result) = page(
        "<html><body><iframe hidden></iframe><main id=app></main></body></html>",
        &[r#"
var markup=document.createElement('template');
markup.innerHTML='<theme-scope class="from-template" data-raw="&amp;" empty><link href="/private.css"></theme-scope>';
var donor=markup.content, theme=donor.firstChild, main=document.getElementById('app');
var get=Element.prototype.getAttribute, owner=donor.ownerDocument;
var once=0, qualified={toString:function(){once++;return 'CLASS';}};
if(!(theme instanceof Element)||theme.nodeType!==1||theme.ownerDocument!==owner||
   owner===document||owner.defaultView!==null||theme.isConnected||theme.parentNode!==donor)
    throw 'actual inert element';
if(theme.getAttribute!==get||theme.getAttribute(qualified)!=='from-template'||once!==1||
   theme.getAttribute('data-raw')!=='&'||theme.getAttribute('empty')!==''||
   theme.getAttribute('missing')!==null||theme.firstChild.getAttribute('href')!=='/private.css')
    throw 'retained attribute values';
var built=owner.createElement('theme-scope'), copied=donor.cloneNode(true).firstChild;
var adopted=document.createElement('div');
adopted.setAttribute('class','author');
donor.appendChild(adopted);
if(get.call(built,'class')!==null||get.call(copied,'class')!=='from-template'||
   get.call(adopted,'class')!=='author'||main.getAttribute.call(theme,'class')!=='from-template'||
   get.call(main,'id')!=='app'||document.querySelector('theme-scope')!==null)
    throw 'receiver and source ownership';
var failures=0;
for(var run of [function(){theme.getAttribute();},function(){get.call(donor,qualified);},
               function(){get.call({},qualified);},function(){get.call(theme,Symbol('class'));}]){
    try{run();}catch(e){if(e.name==='TypeError'){failures++;continue;}throw e;}
}
if(failures!==4||once!==1||theme.ownerDocument!==owner||theme.parentNode!==donor||
   donor.lastChild!==adopted||adopted.ownerDocument!==owner||main.childNodes.length!==0)
    throw 'failed reads changed topology or arguments';
document.title='inert attributes verified';
"#],
    );
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "inert attributes verified");
}

#[test]
fn empty_inert_fragment_insertion_preserves_owner_and_checks_destination() {
    let (doc, _, page, result) = page(
        "<html><head><title>Fixture</title></head><body><iframe hidden></iframe><main>Text</main></body></html>",
        &[r#"
var template=document.createElement('template'), fragment=template.content;
var owner=fragment.ownerDocument, head=document.head, first=head.firstChild;
var main=document.querySelector('main'), clone=fragment.cloneNode(false);
if(owner===document||owner.defaultView!==null||clone===fragment||clone.ownerDocument!==owner)
    throw 'inert provenance';
if(head.appendChild(fragment)!==fragment||head.insertBefore(clone,first)!==clone||
   head.insertBefore(fragment,null)!==fragment||head.firstChild!==first||head.lastChild!==first)
    throw 'empty insertion';
if(fragment.ownerDocument!==owner||template.content!==fragment||fragment.parentNode!==null||
   fragment.isConnected||fragment.getRootNode()!==fragment||fragment.firstChild!==null)
    throw 'fragment identity changed';
function refuses(name, run) {
    try{run();}catch(e){if(e.name===name)return;throw e;}
    throw 'missing '+name;
}
refuses('NotFoundError',function(){head.insertBefore(fragment,main);});
refuses('TypeError',function(){head.insertBefore(fragment,{});});
refuses('HierarchyRequestError',function(){Node.prototype.appendChild.call(main.firstChild,fragment);});
refuses('HierarchyRequestError',function(){template.appendChild(fragment);});
template.innerHTML='<script src="/guard"></script><iframe src="/guard"></iframe>';
refuses('TypeError',function(){head.appendChild(fragment);});
if(head.firstChild!==first||head.lastChild!==first||fragment.ownerDocument!==owner||
   fragment.firstChild.nodeName!=='SCRIPT'||fragment.lastChild.nodeName!=='IFRAME')
    throw 'failed insertion changed tree';
document.title='empty fragment verified';
"#],
    );
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "empty fragment verified");
}

#[test]
fn template_content_append_keeps_real_identity_and_adopts_into_inert_ownership() {
    let (mut doc, mut forms, mut page, _) = page(r#"<html><body><iframe hidden srcdoc="early" id=early></iframe><div id=app></div></body></html>"#, &[]);
    let result = page.console_eval(&mut doc, &mut forms, r#"
var f=document.getElementById('early'), app=document.getElementById('app');
if(f.nextElementSibling!==app||f.contentWindow!==null)throw 'parsed iframe guard';
var p=document.createElement('template'), r=document.createElement('template'), n=document.createElement('div');
var pc=p.content, rc=r.content, owner=pc.ownerDocument, proof={};
n.id='adopted-leaf';n.proof=proof;
if(n.ownerDocument!==document||n.parentNode!==null||n.getRootNode()!==n||owner===document||owner.defaultView!==null)
    throw 'before adoption';
if(rc.appendChild(n)!==n||rc.firstChild!==n||n.ownerDocument!==owner||n.parentNode!==rc||
   n.getRootNode()!==rc||n.isConnected||n.id!=='adopted-leaf'||n.proof!==proof)
    throw 'ordinary leaf adopted';
if(pc.appendChild(r)!==r||pc.firstChild!==r||r.ownerDocument!==owner||r.parentNode!==pc||
   r.content!==rc||r.getRootNode()!==pc||r.content.firstChild!==n||n.parentNode!==rc)
    throw 'nested template adopted';
var shallow=document.importNode(n,true);
if(shallow===n||shallow.ownerDocument!==document||shallow.parentNode!==null||shallow.id!=='adopted-leaf')
    throw 'checked copy after adoption';
if(document.getElementById('adopted-leaf')!==null||p.firstChild!==null||r.firstChild!==null)
    throw 'inert nodes leaked into document';
p.innerHTML='<span id="replacement"></span>';
if(pc.firstChild===r||pc.firstChild.id!=='replacement'||r.parentNode!==null||
   r.ownerDocument!==owner||r.content!==rc||rc.firstChild!==n)
    throw 'replace detaches retained adopted child';
document.title='inert-adoption-ok';
"#);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "inert-adoption-ok");
}

#[test]
fn template_content_append_refuses_active_and_cyclic_transfers_atomically() {
    let (mut doc, mut forms, mut page, _) = page("<html><body><div id=guard></div></body></html>", &[]);
    let result = page.console_eval(&mut doc, &mut forms, r#"
var outer=document.createElement('template'), inner=document.createElement('template');
var outerFragment=outer.content, innerFragment=inner.content;
outerFragment.appendChild(inner);
var connected=document.createElement('div'), image=document.createElement('img'), bad=0;
document.body.appendChild(connected);
for(var node of [connected,image,{}]){
  try{innerFragment.appendChild(node);}catch(e){
    if(e.name==='TypeError'||e.name==='NotSupportedError')bad++;else throw e;
  }
}
var separate=document.implementation.createHTMLDocument().createElement('template').content;
try{separate.appendChild(outer);}catch(e){if(e.name==='NotSupportedError')bad++;else throw e;}
if(bad!==4||innerFragment.firstChild!==null||outerFragment.firstChild!==inner||
   connected.parentNode!==document.body||image.parentNode!==null||separate.firstChild!==null)
    throw 'rejected transfer';
"#);
    assert_eq!(result.errors, 0, "{}", page.report());
    let retained = page.state.retained;
    let result = page.console_eval(&mut doc, &mut forms, "var cycle=false;try{innerFragment.appendChild(outer);}catch(e){cycle=e instanceof DOMException&&e.name==='HierarchyRequestError'&&e.message.indexOf('ancestor cycle')>=0;}if(!cycle)throw 'actual typed ancestor cycle refusal';");
    assert_eq!(result.errors,0,"{}",page.report());
    assert_eq!(page.state.retained, retained);
    let result = page.console_eval(&mut doc, &mut forms, "if(outer.parentNode!==null||innerFragment.firstChild!==null||outerFragment.firstChild!==inner)throw 'cycle changed tree';");
    assert_eq!(result.errors, 0, "{}", page.report());
}

#[test]
fn template_deep_clone_copies_materialized_nested_contents_with_real_owners() {
    let (doc, _, page, result) = page(r#"<html><body><iframe id=early hidden srcdoc="early"></iframe><div id=app></div></body></html>"#, &[r#"
var frame=document.getElementById('early');
if(frame.nextElementSibling!==document.getElementById('app')||frame.contentWindow!==null)
    throw 'parsed iframe topology';
var outer=document.createElement('template'), inner=document.createElement('template');
var leaf=document.createElement('div'), original=outer.content, innerOriginal=inner.content;
outer.id='original'; leaf.id='leaf';
innerOriginal.appendChild(leaf);
original.appendChild(inner);
var shallow=outer.cloneNode(false), copy=outer.cloneNode(true), nested=copy.content.firstChild, cloneLeaf=nested.content.firstChild;
outer.id='changed';
if(copy===outer||!(copy instanceof HTMLTemplateElement)||copy.ownerDocument!==document||
   copy.parentNode!==null||copy.firstChild!==null||copy.id!=='original'||shallow.content.firstChild!==null)
   throw 'outer template clone';
if(copy.content===original||copy.content.ownerDocument!==original.ownerDocument||nested===inner||
   nested.ownerDocument!==inner.ownerDocument||nested.parentNode!==copy.content||
   nested.content===innerOriginal||cloneLeaf===leaf||cloneLeaf.id!=='leaf'||
   cloneLeaf.ownerDocument!==leaf.ownerDocument||cloneLeaf.parentNode!==nested.content)
   throw 'deep inert structure';
copy.innerHTML='<section id="changed-copy"></section>';
if(original.firstChild!==inner||innerOriginal.firstChild!==leaf||nested.parentNode!==null||
   nested.content.firstChild!==cloneLeaf||copy.content.firstChild.id!=='changed-copy')
   throw 'independent clone and retained references';
document.title='nested-template-clone-ok';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "nested-template-clone-ok");
}

#[test]
fn template_deep_clone_handles_unmaterialized_markup_and_bounded_failure() {
    let (mut doc, mut forms, mut page, result) = page("<html><body><template id=literal><section><template><span>original</span></template></section></template></body></html>", &[r#"
var literal=document.getElementById('literal');
var deep=literal.cloneNode(true), shallow=literal.cloneNode();
var deepInner=deep.content.firstChild.firstChild;
if(shallow.content.firstChild!==null||deepInner.content.firstChild.textContent!=='original'||deep.id!=='literal'||
   deepInner===literal.content.firstChild.firstChild||deepInner.content.ownerDocument!==literal.content.ownerDocument)
   throw 'unmaterialized template markup';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    let retained = page.state.retained;
    let limit = page.state.limits.host.extra_nodes;
    page.state.limits.host.extra_nodes = 0;
    let result = page.console_eval(&mut doc, &mut forms, "deep.cloneNode(true);");
    assert_eq!(result.errors, 1);
    assert_eq!(page.state.retained, retained);
    page.state.limits.host.extra_nodes = limit;
    let result = page.console_eval(&mut doc, &mut forms, "if(deepInner.content.firstChild.textContent!=='original'||deep.cloneNode(true).content.firstChild.firstChild.content.firstChild.textContent!=='original')throw 'clone retry';");
    assert_eq!(result.errors, 0, "{}", page.report());
}

#[test]
fn comment_inherited_native_getter_alias_keeps_the_real_receiver() {
    let (doc, _, page, result) = page("<!doctype html><html><head><title>Original comment test</title></head><body><iframe hidden></iframe><div id=app></div></body></html>", &[r#"
var host=document.head, first=document.createComment('first'), second=document.createComment('second');
var before=Object.getOwnPropertyDescriptor(Node.prototype,'parentNode');
var originalInsert=Node.prototype.insertBefore;
var reads=0;
Object.defineProperty(Node.prototype,'savedParent',before);
Object.defineProperty(CharacterData.prototype,'receiverProof',{
    get:function(){reads++;return this;},configurable:true
});
Node.prototype.insertBefore=function(node,reference){
    if(reference!==null&&reference.savedParent!==this)throw Error('bad insertion parent');
    return originalInsert.call(this,node,reference);
};
if(first.savedParent!==null||!('savedParent' in first)||first.receiverProof!==first||reads!==1||
   !(first instanceof Comment)||first.ownerDocument!==document||first.data!=='first'||first.length!==5)
    throw 'detached inherited receiver';
originalInsert.call(host,first,host.firstChild);
if(first.savedParent!==host||host.firstChild!==first)throw 'physical parent';
if(host.insertBefore(second,first)!==second||second.savedParent!==host||
   host.firstChild!==second||second.nextSibling!==first)
   throw 'ordinary insertion through inherited getter';
host.removeChild(second);
if(second.savedParent!==null||second.isConnected||'neverDefined' in second||
   second.neverDefined!==undefined||second.nodeValue!=='second')
   throw 'retained Comment lookup';
delete Node.prototype.savedParent;
if('savedParent' in second||second.savedParent!==undefined)throw 'live inherited descriptor';
document.title='comment-native-alias-ok';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "comment-native-alias-ok");
}

#[test]
fn html_fullscreen_feature_detection_uses_ordinary_absent_property_lookup() {
    let (doc, _, page, result) = page("<!doctype html><html><body><iframe id=early hidden></iframe><div id=app></div></body></html>", &[r#"
var html=document.documentElement, frame=document.getElementById('early'), app=document.getElementById('app');
var fullscreen=html.requestFullscreen?'standard':html.webkitRequestFullscreen?'webkit':
    html.mozRequestFullScreen?'moz':html.msRequestFullscreen?'ms':'absent';
if(fullscreen!=='absent'||'requestFullscreen' in html||html.requestFullscreen!==undefined||
   frame.nextElementSibling!==app||frame.contentWindow!==null)
    throw 'unsupported fullscreen must be absent';
try{html.requestFullscreen();throw 'unsupported method appeared';}
catch(e){if(e.name!=='TypeError')throw e;}
Object.defineProperty(app,'requestFullscreen',{value:function(){return this===app;},configurable:true});
if(!app.requestFullscreen()||html.requestFullscreen!==undefined)throw 'ordinary author property';
delete app.requestFullscreen;
if('requestFullscreen' in app||app.requestFullscreen!==undefined||frame.contentWindow!==null)
    throw 'ordinary absence and native frame boundary';
document.title='fullscreen-feature-absent';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "fullscreen-feature-absent");
}

#[test]
fn html_element_prototype_keeps_real_native_receiver_and_ordinary_ancestry() {
    let (mut doc, mut forms, mut page, result) = page("<!doctype html><html><body><iframe id=guard hidden></iframe><original-card id=card></original-card><original-card id=other></original-card></body></html>", &[r#"
var card=document.getElementById('card'),other=document.getElementById('other'),guard=document.getElementById('guard');
var canonical=Object.getPrototypeOf(card), proof={}, reads=0, parent=Object.getOwnPropertyDescriptor(Node.prototype,'parentNode').get;
var wrapped=Object.create(canonical,{
    report:{get:function(){reads++;if(this!==card)throw 'receiver';return this.marker;}},
    parentAlias:{get:parent}
});
function Author(){}
Author.prototype=wrapped;
card.marker=proof;
card.__proto__=wrapped;
if(Object.getPrototypeOf(card)!==wrapped||card.__proto__!==wrapped||!wrapped.isPrototypeOf(card)||
   !(card instanceof Author)||!(card instanceof HTMLElement)||card.report!==proof||reads!==1||
   card.parentAlias!==document.body||card.ownerDocument!==document||!card.isConnected||
   Object.getPrototypeOf(other)!==canonical||other.report!==undefined||guard.contentWindow!==null)
    throw 'overlay identity and native state';
var replacement=Object.create(wrapped,{flag:{value:42}});
if(Object.setPrototypeOf(card,replacement)!==card||Object.getPrototypeOf(card)!==replacement||card.flag!==42||
   card.report!==proof||reads!==2)throw 'ordinary replacement';
var rejected=0, trapCalls=0;
var proxy=new Proxy(Object.create(canonical),{getPrototypeOf:function(){trapCalls++;return canonical;}});
for(var invalid of [Array.prototype,Object.prototype,null,proxy,Object.create(proxy)]){
    try{Object.setPrototypeOf(card,invalid);}
    catch(e){if(e.name==='TypeError')rejected++;else throw e;}
}
if(rejected!==5||trapCalls!==0||Object.getPrototypeOf(card)!==replacement||card.flag!==42||
   guard.nextElementSibling!==card)throw 'protected chain';
Object.setPrototypeOf(card,canonical);
if(card instanceof Author||card.flag!==undefined||card.marker!==proof||card.parentNode!==document.body)
    throw 'restore canonical prototype';
document.title='element-prototype-ok';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "element-prototype-ok");
    let denied = page.console_eval(&mut doc, &mut forms, "Object.setPrototypeOf(guard, canonical);");
    assert_eq!(denied.errors, 1);
    assert!(page.report().contains("Native host object prototypes cannot be changed"));
    let result = page.console_eval(&mut doc, &mut forms, "if(guard.contentWindow!==null||guard.nextElementSibling!==card)throw 'frame changed';");
    assert_eq!(result.errors, 0, "{}", page.report());
}

#[test]
fn base_event_prototype_patch_preserves_trust_dispatch_and_lifetime() {
    let (mut doc, mut forms, mut page, result) = page("<html><body><p>Original event</p></body></html>", &[r#"
var exposed, delivered=0, native=Event.prototype;
var patched=Object.create(native,{alias:{get:function(){return this.type;}}});
document.addEventListener('DOMContentLoaded',function(event){
    event.__proto__=patched;
    var flag=Object.getOwnPropertyDescriptor(event,'isTrusted');
    if(event.alias!=='DOMContentLoaded'||!event.isTrusted||!flag||flag.configurable||
       !(event instanceof Event)||Object.getPrototypeOf(event)!==patched||
       !patched.isPrototypeOf(event)||event.target!==document||event.currentTarget!==document)
        throw 'trusted event wrapper and dispatch';
    exposed=event;
    delivered++;
});
window.addEventListener('load',function(){
    var author=new Event('original');
    Object.setPrototypeOf(author,patched);
    if(delivered!==1||author.isTrusted||author.alias!=='original'||Object.getPrototypeOf(author)!==patched)
        throw 'authored event retains its own trust';
    document.title='event-prototype-ok';
});
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "event-prototype-ok");
    let result = page.console_eval(&mut doc, &mut forms, "Object.getPrototypeOf(exposed);");
    assert_eq!(result.errors, 1);
    let result = page.console_eval(&mut doc, &mut forms, "if(delivered!==1)throw 'later event effects';");
    assert_eq!(result.errors, 0, "{}", page.report());
}

#[test]
fn text_inherited_native_traversal_aliases_reach_an_element_after_stylesheet_text() {
    let (doc, _, page, result) = page("<html><head><title>Text traversal</title></head><body><iframe hidden></iframe><original-card id=target></original-card></body></html>", &[r#"
var sheet=document.createElement('style'), target=document.getElementById('target');
sheet.textContent='#target{color:#123456}';
document.head.insertBefore(sheet,document.head.firstChild);
var parent=Object.getOwnPropertyDescriptor(Node.prototype,'parentNode');
var sibling=Object.getOwnPropertyDescriptor(Node.prototype,'nextSibling');
Object.defineProperty(Node.prototype,'savedParent',parent);
Object.defineProperty(Node.prototype,'savedSibling',sibling);
var raw=sheet.firstChild, detached=document.createTextNode('original');
if(!(raw instanceof Text)||raw.nodeType!==3||raw.data!=='#target{color:#123456}'||
   raw.savedParent!==sheet||raw.savedSibling!==null||raw.ownerDocument!==document||
   !('savedParent' in raw)||detached.savedParent!==null||detached.savedSibling!==null||
   detached.data!=='original')
    throw 'real Text getters and native slots';
var cursor=document, count=0, found=false;
while(cursor&&count++<40){
    if(cursor===target){found=true;break;}
    if(cursor.firstChild){cursor=cursor.firstChild;continue;}
    while(cursor&&cursor!==document&&!cursor.savedSibling)cursor=cursor.savedParent;
    cursor=cursor&&cursor!==document?cursor.savedSibling:null;
}
if(!found||target.previousElementSibling.contentWindow!==null)
    throw 'ordinary traversal below the stylesheet';
delete Node.prototype.savedParent;
delete Node.prototype.savedSibling;
if(raw.savedParent!==undefined||'savedSibling' in raw||raw.parentNode!==sheet)
    throw 'live prototype lookup';
document.title='text-traversal-ok';
"#]);
    assert_eq!(result.errors, 0, "{}", page.report());
    assert_eq!(doc.title, "text-traversal-ok");
}
