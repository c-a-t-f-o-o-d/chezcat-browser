//! Ordinary concrete HTML interfaces, string tags, and owner-realm access.
use super::*;

#[path = "script_content_editable_tests.rs"]
mod content_editable_tests;

fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
}
fn id(doc: &Document, name: &str) -> usize {
    doc.nodes
        .iter()
        .position(|node| matches!(&node.kind,NodeKind::Element{id:Some(id),..} if id==name))
        .unwrap()
}
fn fixture() -> (Document, Forms, Page) {
    fixture_with("")
}
fn fixture_with(extra: &str) -> (Document, Forms, Page) {
    let markup = concat!(
        "<!doctype html><title>Original DOM string protocol</title><body>",
        "<form id=form><button id=button type=button>Original button</button>",
        "<input id=neighbor name=keep value=original></form>",
        "<ul id=list><li>Original item</li></ul><section id=opaque>Retained opaque element</section>",
        "<a id=anchor href='../destination?q=one#part'>Original link</a>"
    );
    let mut doc = crate::html::parse(&[markup, extra].concat());
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let source = location("https://dom-string.test/path/page");
    page.set_location(&source);
    page.set_resource_base(&source);
    assert_eq!(
        page.start(&mut doc, &mut forms, &[]).errors,
        0,
        "{}",
        page.report()
    );
    let neighbor = id(&doc, "neighbor");
    forms.script_set_value(neighbor, "edited neighbor").unwrap();
    {
        let e = &mut forms.controls.get_mut(&neighbor).unwrap().value;
        e.cursor = 5;
        e.anchor = 2;
        e.offset = 1;
    }
    run(&mut doc,&mut forms,&mut page,"var button=document.getElementById('button'),list=document.getElementById('list'),neighbor=document.getElementById('neighbor'),stringify=Object.prototype.toString,tag=Symbol.toStringTag;");
    (doc, forms, page)
}
fn kept(doc: &Document, forms: &Forms, page: &mut Page) {
    let node = id(doc, "neighbor");
    let c = &forms.controls[&node];
    assert_eq!(
        (
            c.value.text(),
            c.value.cursor,
            c.value.anchor,
            c.value.offset,
            c.dirty_value,
            c.owner
        ),
        (
            "edited neighbor".into(),
            5,
            2,
            1,
            true,
            Some(id(doc, "form"))
        )
    );
    assert_eq!(forms.text_default_value(node).unwrap(), "original");
    assert_eq!(doc.attr(node, "value"), Some("original"));
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
    kept(doc, forms, page);
}

#[test]
fn autonomous_element_identity_uses_the_html_and_dom_name_rules() {
    use crate::javascript::valid_custom_element_name;
    for name in ["a-", "data-panel", "x-α", "note-🙂", "x-@", "x-=", "x-:", "x-\u{b}"] {
        assert!(valid_custom_element_name(name), "{name:?}");
    }
    for name in [
        "", "x", "-x", "1-name", "α-name", "X-name", "x-Name", "x-\0", "x-/",
        "x->", "x- ", "x-\t", "x-\n", "x-\r", "x-\u{c}",
        "annotation-xml", "color-profile", "font-face", "font-face-src", "font-face-uri",
        "font-face-format", "font-face-name", "missing-glyph",
    ] {
        assert!(!valid_custom_element_name(name), "{name:?}");
    }
}

#[test]
fn autonomous_element_identity_preserves_namespace_owner_and_native_guards() {
    let (mut doc, mut forms, mut page) = fixture_with(concat!(
        "<notice-line id='custom'></notice-line><font-face id='reserved'></font-face>",
        "<svg><notice-line id='foreign'/></svg><template><notice-line id='hidden'></notice-line></template>",
    ));
    for name in ["reserved", "foreign", "hidden"] {
        let handle = node_handle(id(&doc, name));
        let bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        assert_eq!(bridge.concrete_html_interface(handle), None, "{name}");
        assert!(!bridge.element_expando_receiver(handle), "{name}");
    }
    run(&mut doc, &mut forms, &mut page, r#"
var notice=document.getElementById('custom'), C=HTMLElement, proto=C.prototype;
var label=Symbol('notice');
Object.defineProperty(notice,'entry',{value:3,configurable:true});
notice[label]=notice;
if(notice.entry!==3||notice[label]!==notice||!delete notice.entry||Object.hasOwn(notice,'entry'))
    throw 'ordinary own data';
var normalized=document.createElement('Notice-Line');
if(normalized.localName!=='notice-line'||Object.getPrototypeOf(normalized)!==proto||
   stringify.call(notice)!=='[object HTMLElement]')throw 'normalized factory or interface tag';
class Unregistered extends HTMLElement {}
for(var construct of [function(){new C();},function(){new Unregistered();}]){
    var rejected=false;try{construct();}catch(e){if(e.name!=='TypeError')throw e;rejected=true;}
    if(!rejected)throw 'unregistered construction';
}
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);
var w=f.contentWindow, read=w.Function('value','return value.constructor;');
if(read(notice)!==C||w.HTMLElement===C||notice instanceof w.HTMLElement||
   Object.getPrototypeOf(w.document.body)!==w.HTMLBodyElement.prototype)
    throw 'genuine owner prototype';
"#);
    for source in [
        "Object.getPrototypeOf(document.createElementNS('http://www.w3.org/1999/xhtml','notice-Line'));",
        "Object.setPrototypeOf(notice,{});",
        "Object.defineProperty(notice,'ownerDocument',{value:null});",
    ] {
        assert_eq!(page.console_eval(&mut doc, &mut forms, source).errors, 1, "{source}");
    }
    page.set_location(&location("https://other-dom-string.test/changed"));
    assert_eq!(page.console_eval(&mut doc, &mut forms, "read(notice);").errors, 1);
    assert!(page.report().contains("same-origin"), "{}", page.report());
    page.set_location(&location("https://dom-string.test/path/page"));
    run(&mut doc, &mut forms, &mut page,
        "if(read(notice)!==C||notice[label]!==notice||notice.ownerDocument!==document)throw 'restored owner and state';");
}

#[test]
fn document_interface_script_uses_the_owner_realm_constructor_and_node_prototype() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var C=Document,P=C.prototype;
if(C!==window.Document||C.length!==0||C.name!=='Document'||Object.getPrototypeOf(C)!==Node||
   Object.getPrototypeOf(P)!==Node.prototype||P.constructor!==C)throw 'Document interface chain';
if(!(document instanceof C)||!(document instanceof Node)||button instanceof C||
   !P.isPrototypeOf(document)||document.body.ownerDocument!==document)throw 'Document instance identity';
var d=Object.getOwnPropertyDescriptor(P,Symbol.toStringTag);
if(d.value!=='Document'||d.enumerable||d.writable||!d.configurable)throw 'Document class tag';
var seen=null;Object.defineProperty(P,'reportOwner',{configurable:true,get:function(){seen=this;return 29;}});
if(document.reportOwner!==29||seen!==document||Object.hasOwn(document,'reportOwner'))throw 'Document ordinary inherited lookup';
delete P.reportOwner;
"#);
}

#[test]
fn document_interface_global_alias_and_branded_getter_use_the_real_tree() {
    let (mut doc, mut forms, mut page) = fixture();
    let before = doc.nodes.len();
    let commits = page.state.dom_commits;
    run(&mut doc,&mut forms,&mut page,r#"
var C=Document,P=C.prototype,html=document.documentElement;
for(var name of ['Document','HTMLDocument']){
    var g=Object.getOwnPropertyDescriptor(window,name);
    if(g.value!==C||g.enumerable||!g.configurable||!g.writable)throw 'interface Window descriptor';
}
if(document.constructor!==C||Object.getPrototypeOf(document)!==P||stringify.call(document)!=='[object Document]')
    throw 'original Document identity';
var field=Object.getOwnPropertyDescriptor(P,'documentElement'),get=field.get;
if(get.name!=='get documentElement'||get.length!==0||field.set!==undefined||!field.enumerable||!field.configurable||
   get.call(document)!==html||html.ownerDocument!==document||Object.hasOwn(document,'documentElement'))
    throw 'Document root getter';
for(var invalid of [button,P,{},null]){
    var denied=false;try{get.call(invalid);}catch(e){denied=e instanceof TypeError;}if(!denied)throw 'getter brand';
}
window.Document=Node;
if(HTMLDocument!==C||document.constructor!==C||!(document instanceof C))throw 'interface alias or saved constructor';
window.Document=C;
var receiver=null;
Object.defineProperty(P,'documentElement',{configurable:true,get:function(){receiver=this;return 47;}});
if(document.documentElement!==47||receiver!==document||get.call(document)!==html)throw 'live prototype versus saved native getter';
Object.defineProperty(P,'documentElement',field);
var ordinaryCall=false,allocation=false;
try{C();}catch(e){ordinaryCall=e instanceof TypeError;}
try{new C();}catch(e){allocation=e.name==='NotSupportedError';}
if(!ordinaryCall||!allocation||document.documentElement!==html)throw 'independent allocation boundary';
"#);
    assert_eq!(doc.nodes.len(),before);
    assert_eq!(page.state.dom_commits,commits);
}

#[test]
fn document_interface_blank_child_realms_revalidate_saved_objects_and_getters() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,r#"
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);
var w=f.contentWindow,child=f.contentDocument,C=w.Document,P=C.prototype,root=child.documentElement;
var get=Object.getOwnPropertyDescriptor(Document.prototype,'documentElement').get;
var childGet=Object.getOwnPropertyDescriptor(P,'documentElement').get;
if(C===Document||C!==w.HTMLDocument||child.constructor!==C||Object.getPrototypeOf(C)!==w.Node||
   Object.getPrototypeOf(P)!==w.Node.prototype||Object.getPrototypeOf(child)!==P||
   !(child instanceof C)||child instanceof Document||document instanceof C||stringify.call(child)!=='[object Document]')
    throw 'child document realm';
if(get.call(child)!==root||childGet.call(document)!==document.documentElement||child.defaultView!==w)
    throw 'borrowed root getter';
var hits=0;Object.defineProperty(P,'whichDocument',{configurable:true,get:function(){hits++;return this;}});
if(child.whichDocument!==child||document.whichDocument!==undefined||hits!==1)throw 'child prototype receiver';
var local=w.Function('return Document===HTMLDocument && document.documentElement.ownerDocument===document;');
if(!local())throw 'child script interface';
"#);
    page.set_location(&location("https://other-dom-string.test/changed"));
    for source in ["child.whichDocument;", "child instanceof C;", "get.call(child);", "childGet.call(document);"] {
        page.clear_console();
        let out=page.console_eval(&mut doc,&mut forms,source);
        assert_eq!(out.errors,1,"{source}\n{}",page.report());
        assert!(page.report().contains("same-origin"),"{source}\n{}",page.report());
    }
    run(&mut doc,&mut forms,&mut page,"if(!local()||hits!==1)throw 'owner-local recovery';");
    page.set_location(&location("https://dom-string.test/path/page"));
    run(&mut doc,&mut forms,&mut page,r#"
if(child.whichDocument!==child||hits!==2)throw 'same-origin recovery';
document.body.removeChild(f);
if(f.contentDocument!==null||!w.closed)throw 'original frame detached';
document.body.appendChild(f);
if(f.contentDocument===child||f.contentWindow.Document===C||f.contentDocument instanceof C)throw 'new browsing context';
"#);
    let out=page.console_eval(&mut doc,&mut forms,"get.call(child);");
    assert_eq!(out.errors,1,"{}",page.report());
    kept(&doc,&forms,&mut page);
}

#[test]
fn iframe_interface_parsed_and_created_elements_share_an_ordinary_canonical_prototype() {
    let (mut doc, mut forms, mut page) = fixture_with(
        "<iframe id=parsed hidden>fallback</iframe><template><iframe id=inactive></iframe></template>",
    );
    run(&mut doc, &mut forms, &mut page, r#"
var C=window.HTMLIFrameElement,P=C.prototype,parsed=document.getElementById('parsed'),fresh=document.createElement('iframe');
if(parsed===null||document.querySelector('iframe')!==parsed||!parsed.isConnected||fresh.isConnected||
   parsed.contentWindow!==null||parsed.contentDocument!==null||document.getElementById('inactive')!==null)
    throw 'iframe host discovery or browsing context';
if(typeof C!=='function'||C.name!=='HTMLIFrameElement'||C.length!==0||Object.getPrototypeOf(C)!==HTMLElement||
   Object.getPrototypeOf(P)!==HTMLElement.prototype||neighbor instanceof C||document.body instanceof C)
    throw 'frame constructor or ancestry';
var tagDescriptor=Object.getOwnPropertyDescriptor(P,tag),constructorDescriptor=Object.getOwnPropertyDescriptor(P,'constructor');
var protoDescriptor=Object.getOwnPropertyDescriptor(C,'prototype');
if(protoDescriptor.value!==P||protoDescriptor.writable||protoDescriptor.enumerable||protoDescriptor.configurable||
   constructorDescriptor.value!==C||!constructorDescriptor.writable||constructorDescriptor.enumerable||!constructorDescriptor.configurable||
   tagDescriptor.value!=='HTMLIFrameElement'||tagDescriptor.writable||tagDescriptor.enumerable||!tagDescriptor.configurable)
    throw 'frame interface descriptor flags';
for(var frame of [parsed,fresh]){
    if(Object.getPrototypeOf(frame)!==P||frame.constructor!==C||!(frame instanceof C)||!(frame instanceof HTMLElement)||
       !P.isPrototypeOf(frame)||Object.hasOwn(frame,'constructor')||stringify.call(frame)!=='[object HTMLIFrameElement]')
        throw 'frame object identity';
}
"#);
    let commits = page.state.dom_commits;
    let nodes = doc.nodes.len();
    run(&mut doc, &mut forms, &mut page, r#"
var originalGlobal=Object.getOwnPropertyDescriptor(window,'HTMLIFrameElement');
window.HTMLIFrameElement=HTMLInputElement;
if(window.HTMLIFrameElement!==HTMLInputElement||parsed.constructor!==C||!(fresh instanceof C)||fresh instanceof window.HTMLIFrameElement)
    throw 'global replacement changed original interface';
Object.defineProperty(window,'HTMLIFrameElement',originalGlobal);
var seen=null;
Object.defineProperty(P,'constructor',{configurable:true,get:function(){seen=this;return neighbor;}});
if(parsed.constructor!==neighbor||seen!==parsed||fresh.constructor!==neighbor||seen!==fresh||!(parsed instanceof C))
    throw 'inherited frame constructor getter';
Object.defineProperty(P,'constructor',constructorDescriptor);
Object.defineProperty(P,tag,{configurable:true,value:'FrameMarker'});
if(stringify.call(fresh)!=='[object FrameMarker]')throw 'live frame prototype tag';
Object.defineProperty(P,tag,tagDescriptor);
var rejected=0;
for(var invoke of [function(){C();},function(){new C();}]){
    try{invoke();}catch(error){if(!(error instanceof TypeError))throw error;rejected++;}
}
if(rejected!==2||HTMLIFrameElement!==C||parsed.constructor!==C||stringify.call(fresh)!=='[object HTMLIFrameElement]')
    throw 'illegal construction or interface restoration';
"#);
    assert_eq!(doc.nodes.len(), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert!(page.state.blank_frames.is_empty());
}

#[test]
fn iframe_interface_parsed_srcdoc_identity_keeps_instance_storage_restricted() {
    let (mut doc, mut forms, mut page) = fixture_with("<iframe id=authored srcdoc='retained'></iframe>");
    let node = id(&doc, "authored");
    run(&mut doc, &mut forms, &mut page, r#"
var authored=document.querySelector('iframe'),fresh=document.createElement('iframe'),C=HTMLIFrameElement;
if(authored!==document.getElementById('authored')||!(authored instanceof C)||authored.constructor!==C||
   Object.getPrototypeOf(authored)!==C.prototype||stringify.call(authored)!=='[object HTMLIFrameElement]'||
   authored.srcdoc!=='retained'||authored.contentWindow!==null||authored.contentDocument!==null)
    throw 'parsed frame identity or inert state';
fresh.privateNote=7;
if(Object.keys(fresh).length!==1||Object.getOwnPropertyDescriptor(fresh,'privateNote').value!==7||
   Object.hasOwn(fresh,'srcdoc')||Object.getPrototypeOf(fresh)!==C.prototype)
    throw 'created frame own storage or identity';
"#);
    let commits = page.state.dom_commits;
    for source in [
        "Object.hasOwn(authored,'srcdoc');",
        "Object.getOwnPropertyDescriptor(authored,'privateNote');",
        "Object.keys(authored);",
        "for(var key in authored){}",
        "authored.privateNote=1;",
        "Object.defineProperty(authored,'privateNote',{value:1});",
        "authored.srcdoc='changed';",
    ] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert_eq!(page.runtime.native_overlay_state_for_test(node_handle(node)), (None, None));
        assert_eq!(doc.attr(node, "srcdoc"), Some("retained"));
        assert_eq!(page.state.dom_commits, commits);
        assert!(page.state.blank_frames.is_empty());
    }
    run(&mut doc, &mut forms, &mut page,
        "if(authored.constructor!==C||authored.contentWindow!==null||fresh.privateNote!==7)throw 'frame state after refusals';");
}

#[test]
fn iframe_interface_blank_child_realm_uses_owner_identity_and_rechecks_saved_access() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var owner=document.createElement('iframe');owner.hidden=true;document.body.appendChild(owner);
var w=owner.contentWindow,F=w.Function,Child=w.HTMLIFrameElement,Parent=HTMLIFrameElement;
var local=F('return window.HTMLIFrameElement===HTMLIFrameElement && !(document.body instanceof HTMLIFrameElement);');
var read=F('node','return node.constructor;'),inChild=F('node','return node instanceof HTMLIFrameElement;');
if(!local()||Child!==F('return HTMLIFrameElement;')()||Child===Parent||Child.prototype===Parent.prototype||
   Object.getPrototypeOf(Child)!==w.HTMLElement||Object.getPrototypeOf(Child.prototype)!==w.HTMLElement.prototype||
   owner.contentDocument!==w.document||owner instanceof Child||inChild(owner)||!(owner instanceof Parent)||read(owner)!==Parent)
    throw 'child realm or element owner mismatch';
var typed=false;
try{new Child();}catch(error){typed=Object.getPrototypeOf(error)===w.TypeError.prototype;}
if(!typed)throw 'child realm illegal constructor error';
"#);
    let frames = page.state.blank_frames.len();
    let commits = page.state.dom_commits;
    assert_eq!(frames, 1);
    page.set_location(&location("https://other-dom-string.test/changed"));
    for source in ["read(owner);", "inChild(owner);", "new Child();"] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        assert_eq!(page.state.blank_frames.len(), frames);
        kept(&doc, &forms, &mut page);
    }
    page.set_location(&location("https://dom-string.test/path/page"));
    run(&mut doc, &mut forms, &mut page,
        "if(!local()||read(owner)!==Parent||inChild(owner)||owner.contentWindow!==w)throw 'restored frame access';");
}

#[test]
fn iframe_interface_unrepresented_fallback_refuses_child_handles_but_empty_frames_keep_live_lists() {
    let (mut doc, mut forms, mut page) = fixture_with(concat!(
        "<iframe id=plain>fallback</iframe><iframe id=markup><b id=buried>fallback</b></iframe>",
        "<iframe id=empty></iframe><div id=after><b id=visible>neighbor</b></div>"
    ));
    run(&mut doc, &mut forms, &mut page, r#"
var plain=document.getElementById('plain'),markup=document.getElementById('markup'),empty=document.getElementById('empty');
var fresh=document.createElement('iframe'),after=document.getElementById('after');
if(!(plain instanceof HTMLIFrameElement)||!(markup instanceof HTMLIFrameElement)||
   document.getElementById('buried')!==null||markup.querySelector('*')!==null||
   markup.getElementsByTagName('*').length!==0||after.querySelector('b')!==document.getElementById('visible'))
    throw 'iframe host, hidden fallback or neighboring ordinary selection';
if(('firstElementChild' in markup && markup.firstElementChild!==null)||
   ('lastElementChild' in markup && markup.lastElementChild!==null)||
   ('children' in markup && markup.children.length!==0))
    throw 'iframe fallback exposed as element children';
"#);
    let lists = page.state.node_lists.len();
    let retained = page.state.retained;
    let commits = page.state.dom_commits;
    for source in [
        "plain.childNodes;", "plain.firstChild;", "plain.lastChild;",
        "markup.childNodes;", "markup.firstChild;", "markup.lastChild;",
    ] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("Iframe fallback child topology"), "{}", page.report());
        assert_eq!(page.state.node_lists.len(), lists);
        assert_eq!(page.state.retained, retained);
        assert_eq!(page.state.dom_commits, commits);
        assert!(page.state.blank_frames.is_empty());
    }
    run(&mut doc, &mut forms, &mut page, r#"
for(var iframe of [empty,fresh]){
    var children=iframe.childNodes;
    if(children!==iframe.childNodes||children.length!==0||children[0]!==undefined||children.item(0)!==null||
       iframe.firstChild!==null||iframe.lastChild!==null)throw 'empty iframe child topology';
}
var saved=fresh.childNodes;fresh.hidden=true;document.body.appendChild(fresh);
if(fresh.childNodes!==saved||saved.length!==0||fresh.firstChild!==null||!fresh.contentWindow||
   markup.contentWindow!==null||markup.contentDocument!==null||after.childNodes[0]!==after.firstChild||
   after.firstChild.firstChild.data!=='neighbor')throw 'blank child context or neighboring physical tree';
"#);
    assert_eq!(page.state.blank_frames.len(), 1);
}

#[test]
fn iframe_interface_inspection_preserves_existing_frame_activation_limits() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var candidate=document.createElement('iframe');candidate.hidden=true;candidate.srcdoc='<p>retained</p>';
if(!(candidate instanceof HTMLIFrameElement)||candidate.srcdoc!=='<p>retained</p>'||
   candidate.contentDocument!==null||candidate.contentWindow!==null)throw 'unconnected frame inspection';
"#);
    let commits = page.state.dom_commits;
    let out = page.console_eval(&mut doc, &mut forms, "document.body.appendChild(candidate);");
    assert_eq!(out.errors, 1, "{}", page.report());
    run(&mut doc, &mut forms, &mut page, r#"
if(candidate.parentNode!==null||candidate.contentWindow!==null||candidate.contentDocument!==null||
   candidate.srcdoc!=='<p>retained</p>'||candidate.constructor!==HTMLIFrameElement)
    throw 'refused insertion changed frame identity or native state';
"#);
    assert_eq!(page.state.dom_commits, commits);
    assert!(page.state.blank_frames.is_empty());
}

#[test]
fn input_interface_identity_preserves_native_editor_and_follows_ordinary_descriptors() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var C = window.HTMLInputElement, P = C.prototype;
var fresh = document.createElement('input'), d = Object.getOwnPropertyDescriptor(P,'constructor');
if (neighbor.constructor !== C || fresh.constructor !== C || Object.getPrototypeOf(neighbor) !== P
    || Object.getPrototypeOf(fresh) !== P || Object.getPrototypeOf(P) !== HTMLElement.prototype
    || Object.getPrototypeOf(C) !== HTMLElement || !(neighbor instanceof C) || button instanceof C)
    throw 'wrong input interface';
if (!d || d.value !== C || d.enumerable || !d.writable || !d.configurable
    || Object.getOwnPropertyDescriptor(neighbor,'constructor') !== undefined
    || stringify.call(neighbor) !== '[object HTMLInputElement]') throw 'input interface descriptors';
var rejected = 0;
for (var invoke of [function(){ C(); },function(){ new C(); }]) {
    try { invoke(); } catch(e) { if(e.name === 'TypeError') rejected++; else throw e; }
}
if (rejected !== 2) throw 'illegal direct interface construction';
P.constructor = button.constructor;
if (fresh.constructor !== button.constructor || neighbor.constructor !== button.constructor
    || !(neighbor instanceof C)) throw 'input constructor did not follow ordinary lookup';
Object.defineProperty(P,'constructor',d);
if (neighbor.constructor !== C || neighbor.value !== 'edited neighbor'
    || neighbor.defaultValue !== 'original') throw 'restored constructor or editor';
"#);
}

#[test]
fn input_interface_identity_uses_current_owner_and_distinct_realm_prototypes() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc, &mut forms, &mut page, r#"
var f = document.createElement('iframe');
f.hidden = true;
document.body.appendChild(f);
var w = f.contentWindow, childInput = w.Function('return HTMLInputElement;')();
var read = w.Function('element','return element.constructor;');
if (childInput === HTMLInputElement || childInput.prototype === HTMLInputElement.prototype
    || Object.getPrototypeOf(childInput) !== w.HTMLElement || read(neighbor) !== HTMLInputElement)
    throw 'input interface realm identity';
"#);
    page.set_location(&location("https://other-dom-string.test/changed"));
    let denied = page.console_eval(&mut doc, &mut forms, "read(neighbor);");
    assert_eq!(denied.errors, 1, "{}", page.report());
    assert!(page.report().contains("same-origin"), "{}", page.report());
    kept(&doc, &forms, &mut page);
    page.set_location(&location("https://dom-string.test/path/page"));
    run(&mut doc, &mut forms, &mut page,
        "if (read(neighbor) !== HTMLInputElement) throw 'restored owner access';");
}

#[test]
fn dom_object_string_protocol_concrete_prototypes_tag_descriptors_and_ordinary_overrides() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var BP=Object.getPrototypeOf(button),UP=Object.getPrototypeOf(list),BD=Object.getOwnPropertyDescriptor(BP,tag),UD=Object.getOwnPropertyDescriptor(UP,tag),OD=Object.getOwnPropertyDescriptor(Object.prototype,'toString');
if(BP===UP||Object.getPrototypeOf(BP)!==HTMLElement.prototype||Object.getPrototypeOf(UP)!==HTMLElement.prototype)throw 'distinct real concrete prototype suffixes';
if(HTMLButtonElement.prototype!==BP||HTMLUListElement.prototype!==UP||button.constructor!==HTMLButtonElement||list.constructor!==HTMLUListElement)throw 'concrete constructor identity';
if(!(button instanceof HTMLButtonElement)||list instanceof HTMLButtonElement||!(list instanceof HTMLUListElement))throw 'concrete instance identity';
for(var C of [HTMLButtonElement,HTMLUListElement]){try{new C();throw 'illegal construction succeeded';}catch(e){if(!(e instanceof TypeError))throw e;}}
for(var pair of [[BD,'HTMLButtonElement'],[UD,'HTMLUListElement']]){var d=pair[0];if(!d||d.value!==pair[1]||d.writable||d.enumerable||!d.configurable||d.get!==undefined||d.set!==undefined)throw 'standard concrete tag descriptor';}
if(Object.getOwnPropertyDescriptor(BP,'toString')!==undefined||Object.getOwnPropertyDescriptor(UP,'toString')!==undefined||Object.hasOwn(button,'toString')||Object.hasOwn(list,'toString'))throw 'Object stringifier must be inherited';
if(typeof stringify!=='function'||stringify.name!=='toString'||stringify.length!==0||Object.getPrototypeOf(stringify)!==Function.prototype||OD.value!==stringify||!OD.writable||OD.enumerable||!OD.configurable)throw 'ordinary canonical Object method';
if(button.toString!==stringify||list.toString!==stringify||button[tag]!=='HTMLButtonElement'||list[tag]!=='HTMLUListElement')throw 'canonical method/tag identity';
button[tag]='ignored';list[tag]='ignored';if(button[tag]!==BD.value||list[tag]!==UD.value||Object.hasOwn(button,tag)||Object.hasOwn(list,tag))throw 'inherited readonly tags';
var own=function(){return this===button?'own-button':'wrong';};button.toString=own;
if(button.toString()!=='own-button'||Object.getOwnPropertyDescriptor(button,'toString').value!==own||list.toString!==stringify||stringify.call(button)!=='[object HTMLButtonElement]')throw 'ordinary own stringifier shadow';
delete button.toString;if(button.toString!==stringify||!delete button.toString||button.toString()!=='[object HTMLButtonElement]')throw 'own deletion restores actual inheritance';
BP.toString=function(){return this===button?'prototype-button':'wrong';};if(button.toString()!=='prototype-button'||list.toString!==stringify)throw 'concrete prototype override leaked';
delete BP.toString;if(button.toString!==stringify)throw 'concrete prototype deletion bypassed Object method';
var tagReads=0;Object.defineProperty(button,tag,{configurable:true,get:function(){tagReads++;return 'AuthorButton';}});
if(button.toString()!=='[object AuthorButton]'||stringify.call(button)!=='[object AuthorButton]'||tagReads!==2||list.toString()!=='[object HTMLUListElement]')throw 'observable own tag getter';
delete button[tag];if(button.toString()!=='[object HTMLButtonElement]')throw 'own tag deletion';
delete BP[tag];if(button.toString()!=='[object HTMLElement]'||button[tag]!=='HTMLElement'||list[tag]!=='HTMLUListElement')throw 'concrete tag deletion must expose parent tag';
Object.defineProperty(BP,tag,BD);if(button.toString()!=='[object HTMLButtonElement]')throw 'concrete descriptor restoration';
var marker={},caught=false;Object.defineProperty(list,tag,{configurable:true,get:function(){throw marker;}});
try{stringify.call(list);}catch(e){caught=e===marker;}if(!caught)throw 'tag getter abrupt value changed';delete list[tag];
try{new stringify();throw 'Object method constructed';}catch(e){if(!(e instanceof TypeError))throw e;}
"#,
    );
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_getter_before_arguments_real_receiver_and_abrupt_order() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var BP=Object.getPrototypeOf(button),order='',marker={};
Object.defineProperty(BP,'toString',{configurable:true,get:function(){order+='G';if(this!==button)throw 'getter receiver';return function(value){order+='C';if(this!==button||value!==37)throw 'call receiver/argument';return 'observed';};}});
function argument(){order+='A';return 37;}
if(button.toString(argument())!=='observed'||order!=='GAC')throw 'inherited getter/argument/call order';
order='';Object.defineProperty(BP,'toString',{configurable:true,get:function(){order+='G';throw marker;}});
var caught=false;try{button.toString(argument());}catch(e){caught=e===marker;}if(!caught||order!=='G')throw 'abrupt member Get must precede argument';
delete BP.toString;order='';Object.defineProperty(button,'toString',{configurable:true,get:function(){order+='G';return stringify;}});
if(button.toString(argument())!=='[object HTMLButtonElement]'||order!=='GA')throw 'canonical builtin ignored args or lookup order';
order='';Object.defineProperty(button,'toString',{configurable:true,get:function(){order+='G';return 17;}});
try{button.toString(argument());throw 'noncallable accepted';}catch(e){if(!(e instanceof TypeError))throw e;}if(order!=='GA')throw 'noncallable member did not evaluate argument';
delete button.toString;if(button.toString!==stringify||list.toString!==stringify)throw 'getter deletion left ghost method';
"#,
    );
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_window_tag_override_delete_readonly_and_location_stringifier() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if(window.toString!==stringify||window.toString()!=='[object Window]'||window[tag]!=='Window')throw 'ordinary real Window stringifier/tag';
var originalWindowTag=Object.getOwnPropertyDescriptor(window,tag),tagReads=0;
if(originalWindowTag!==undefined||!(tag in window))throw 'Window class string must be inherited';
window[tag]='ignored';if(window[tag]!=='Window'||window.toString()!=='[object Window]')throw 'readonly Window tag assignment';
Object.defineProperty(window,tag,{configurable:true,get:function(){tagReads++;return 'AuthorWindow';}});
if(window.toString()!=='[object AuthorWindow]'||stringify.call(window)!=='[object AuthorWindow]'||tagReads!==2)throw 'Window own tag getter ignored';
delete window[tag];
if(window[tag]!=='Window'||window.toString()!=='[object Window]')throw 'Window override deletion/default restoration';
var anchor=document.getElementById('anchor'),savedLocation=location.toString;
if(typeof savedLocation!=='function'||savedLocation===stringify||savedLocation.call(location)!==location.href||location.toString()!=='https://dom-string.test/path/page')throw 'existing native Location stringifier changed';
if(anchor.href!=='https://dom-string.test/destination?q=one#part'||anchor.getAttribute('href')!=='../destination?q=one#part')throw 'existing anchor projection/raw attribute changed';
"#,
    );
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_same_owner_method_identity_and_cached_foreign_guards() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var f=document.createElement('iframe');f.hidden=true;document.body.appendChild(f);var w=f.contentWindow,F=w.Function;
var childStringify=F('return Object.prototype.toString;')(),childRead=F('node','return node.toString;'),childInvoke=F('method','node','return method.call(node);'),own=F('return window===self&&document.body.nodeType===1;');
if(childStringify===stringify||childRead(button)!==stringify||childInvoke(childStringify,button)!=='[object HTMLButtonElement]'||childInvoke(stringify,list)!=='[object HTMLUListElement]')throw 'real per-realm Object intrinsic and target owner lookup';
var reads=0,tags=0,BP=Object.getPrototypeOf(button);
Object.defineProperty(BP,'toString',{configurable:true,get:function(){reads++;return stringify;}});
Object.defineProperty(button,tag,{configurable:true,get:function(){tags++;return 'OwnerButton';}});
"#,
    );
    let commits = page.state.dom_commits;
    page.set_location(&location("https://other-dom-string.test/changed"));
    for source in [
        "childRead(button);",
        "childInvoke(stringify,button);",
        "childInvoke(childStringify,button);",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        kept(&doc, &forms, &mut page);
        run(&mut doc,&mut forms,&mut page,"if(!own()||reads!==0||tags!==0||neighbor.value!=='edited neighbor')throw 'refusal ran getter or lost owner context';");
    }
    page.set_location(&location("https://dom-string.test/path/page"));
    run(&mut doc,&mut forms,&mut page,"if(childRead(button)!==stringify||childInvoke(childStringify,button)!=='[object OwnerButton]'||reads!==1||tags!==1)throw 'same-origin recovery';delete BP.toString;delete button[tag];if(button.toString()!=='[object HTMLButtonElement]')throw 'restored owner tag';");
    assert_eq!(page.state.dom_commits, commits);
}

#[test]
fn dom_object_string_protocol_unrelated_opaque_types_and_shared_copy_failure_recover() {
    let (mut doc, mut forms, mut page) = fixture();
    let commits = page.state.dom_commits;
    run(&mut doc,&mut forms,&mut page,"var opaque=document.getElementById('opaque');if(button.toString()!=='[object HTMLButtonElement]'||list.toString()!=='[object HTMLUListElement]')throw 'admitted concrete controls';");
    for source in [
        "Object.getPrototypeOf(opaque);",
        "opaque[Symbol.toStringTag];",
        "stringify.call(opaque);",
        "opaque.toString();",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source}\n{}", page.report());
        assert_eq!(page.state.dom_commits, commits);
        kept(&doc, &forms, &mut page);
    }
    run(&mut doc,&mut forms,&mut page,"var longTag='x',budgetGets=0;while(longTag.length<2048)longTag+=longTag;if(longTag.length!==2048)throw 'tag fixture length';Object.defineProperty(button,tag,{configurable:true,get:function(){budgetGets++;return longTag;}});");
    page.begin_job();
    page.runtime.constrain_job(
        usize::MAX, 512, usize::MAX, Instant::now() + Duration::from_secs(10),
    );
    let fault = page.runtime.execute(
        "button.toString();",
        &mut Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state },
    ).unwrap_err();
    assert!(fault.contains("String/copy work limit reached"), "{fault}");
    assert!(page.runtime.invocation_failed_fatally());
    assert_eq!(page.state.dom_commits, commits);
    kept(&doc, &forms, &mut page);
    run(&mut doc,&mut forms,&mut page,"if(budgetGets!==1||button.toString()!=='[object '+longTag+']'||budgetGets!==2)throw 'shared copy bound or later job recovery';delete button[tag];if(button.toString()!=='[object HTMLButtonElement]')throw 'tag recovery';");
    assert_eq!(page.state.dom_commits, commits);
}
