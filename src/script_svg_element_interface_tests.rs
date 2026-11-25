//! Genuine namespace-backed SVGElement ancestry and its finite interface surface.
//! Specific SVG prototypes, SVG mixins, factories and rendering are separate.
use super::*;

fn location(source: &str) -> crate::navigation::Location {
    crate::navigation::address(source, &crate::navigation::Location::home()).unwrap()
}
fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn editor(forms: &Forms, node: usize) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &forms.controls[&node];
    (
        control.value.text(),
        control.value.cursor,
        control.value.anchor,
        control.value.offset,
        control.dirty_value,
        control.owner,
    )
}
fn no_effects(page: &mut Page) {
    assert!(!page.has_navigation_request());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (added, removed, changed) = page.take_image_changes();
    assert!(added.is_empty() && removed.is_empty() && changed.is_empty());
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{source}\n{}", page.report());
    no_effects(page);
}
fn fixture() -> (Document, Forms, Page) {
    fixture_with_page(Page::for_test())
}
fn fixture_with_page(mut page:Page)->(Document,Forms,Page) {
    let mut doc = crate::html::parse(concat!(
        "<!doctype html><title>Original SVG interface</title><body>",
        "<div id=html>Original HTML</div>",
        "<div id=spoof xmlns='http://www.w3.org/2000/svg'>HTML namespace witness</div>",
        "<svg id=svg xmlns='http://www.w3.org/2000/svg'><g id=group><rect id=shape width=8 height=4></rect></g></svg>",
        "<math id=unknown></math>",
        "<form id=form><input id=neighbor name=neighbor value=original></form>"
    ));
    for name in ["svg", "group", "shape"] {
        assert_eq!(
            doc.nodes[id(&doc, name)].namespace,
            crate::html::Namespace::Svg
        );
    }
    for name in ["html", "spoof", "neighbor"] {
        assert_eq!(
            doc.nodes[id(&doc, name)].namespace,
            crate::html::Namespace::Html
        );
    }
    assert_eq!(
        doc.nodes[id(&doc, "unknown")].namespace,
        crate::html::Namespace::UnknownForeign
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let source = location("https://svg-interface.test/source");
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
    let value = &mut forms.controls.get_mut(&neighbor).unwrap().value;
    value.cursor = 5;
    value.anchor = 2;
    value.offset = 1;
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var svg=document.getElementById('svg'),group=document.getElementById('group'),shape=document.getElementById('shape');
var html=document.getElementById('html'),spoof=document.getElementById('spoof'),unknown=document.getElementById('unknown');
var neighbor=document.getElementById('neighbor'),S=SVGElement,P=S.prototype;
if(svg.nodeType!==1||svg.ownerDocument!==document||group.parentNode!==svg||shape.parentNode!==group||
   svg.namespaceURI!=='http://www.w3.org/2000/svg'||group.localName!=='g'||shape.nodeName!=='rect'||shape.tagName!=='rect')
    throw 'genuine parsed SVG physical and naming facts';
if(neighbor.value!=='edited neighbor'||neighbor.defaultValue!=='original'||neighbor.ownerDocument!==document)
    throw 'actual edited native neighbor';
var htmlWord=document.createElement('svg');
if(htmlWord.namespaceURI!=='http://www.w3.org/1999/xhtml'||htmlWord.nodeName!=='SVG')throw 'HTML factory namespace';
if(!(svg instanceof S)||!(group instanceof S)||!(shape instanceof S)||!(svg instanceof Element)||!(svg instanceof Node)||
   svg instanceof HTMLElement||html instanceof S||spoof instanceof S||htmlWord instanceof S||unknown instanceof S)
    throw 'namespace-backed SVG and HTML ancestry';
if(!P.isPrototypeOf(svg)||!Element.prototype.isPrototypeOf(svg))throw 'ordinary ancestry control';
"#,
    );
    (doc, forms, page)
}

#[test]
fn svg_element_interface_canonical_metadata_and_illegal_class_construction() {
    let (mut doc, mut forms, mut page) = fixture();
    let preserved = editor(&forms, id(&doc, "neighbor"));
    let nodes = format!("{:?}", doc.nodes);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
if(typeof S!=='function'||S.name!=='SVGElement'||S.length!==0||Object.getPrototypeOf(S)!==Element||
   Object.getPrototypeOf(P)!==Element.prototype||P.constructor!==S||P===HTMLElement.prototype)throw 'canonical SVG interface';
var descriptor=Object.getOwnPropertyDescriptor(S,'prototype'),tag=Object.getOwnPropertyDescriptor(P,Symbol.toStringTag);
if(descriptor.value!==P||descriptor.writable||descriptor.enumerable||descriptor.configurable||
   tag.value!=='SVGElement'||tag.writable||tag.enumerable||!tag.configurable)throw 'SVG interface descriptors';
if(typeof SVGSVGElement!=='undefined'||typeof SVGGElement!=='undefined'||typeof SVGGraphicsElement!=='undefined'||
   Object.hasOwn(P,'getBBox')||Object.hasOwn(P,'ownerSVGElement')||Object.hasOwn(P,'viewportElement'))throw 'unimplemented SVG surface advertised';
var effects=0,input={toString(){effects++;return 'unused';}},rejected=0;
class Panel extends S {}
class Explicit extends S {constructor(){super(input);}}
if(Object.getPrototypeOf(Panel)!==S||Object.getPrototypeOf(Panel.prototype)!==P)throw 'real SVG class heritage';
for(var Constructor of [S,Panel,Explicit]){
    try{new Constructor();}catch(error){if(!(error instanceof TypeError))throw error;rejected++;}
}
try{S();}catch(error){if(!(error instanceof TypeError))throw error;rejected++;}
class ReturnObject extends S {constructor(){return {plain:true};}}
var plain=new ReturnObject();
if(!plain.plain||plain instanceof S||Object.getPrototypeOf(plain)!==Object.prototype||rejected!==4||effects!==0)
    throw 'unregistered SVG creation or argument conversion';
"#,
    );
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), preserved);
    assert_eq!(
        forms.text_default_value(id(&doc, "neighbor")).unwrap(),
        "original"
    );
}

#[test]
fn svg_element_interface_ancestry_reads_do_not_allocate_and_follow_live_prototypes() {
    let (mut doc, mut forms, mut page) = fixture();
    let nodes = format!("{:?}", doc.nodes);
    let preserved = editor(&forms, id(&doc, "neighbor"));
    let allocations = page.runtime.window_allocation_state_for_test();
    for source in [
        "if(!(svg instanceof S)||!(group instanceof S)||!(shape instanceof S))throw 'SVG read';",
        "if(html instanceof S||spoof instanceof S||htmlWord instanceof S||unknown instanceof S)throw 'namespace-negative read';",
        "if(!P.isPrototypeOf(svg)||!Element.prototype.isPrototypeOf(svg)||HTMLElement.prototype.isPrototypeOf(svg))throw 'ancestor read';",
    ] {
        run(&mut doc, &mut forms, &mut page, source);
        assert_eq!(page.runtime.window_allocation_state_for_test(), allocations);
    }
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var parent=Object.getPrototypeOf(P),htmlParent=Object.getPrototypeOf(HTMLElement.prototype);
Object.setPrototypeOf(P,null);
if(!(svg instanceof S)||svg instanceof Element)throw 'SVG live parent deletion';
Object.setPrototypeOf(P,parent);
Object.setPrototypeOf(HTMLElement.prototype,P);
if(!(html instanceof S)||!(spoof instanceof S)||!(htmlWord instanceof S))throw 'author prototype ancestry bypassed';
Object.setPrototypeOf(HTMLElement.prototype,htmlParent);
if(!(svg instanceof S)||!(svg instanceof Element)||html instanceof S||spoof instanceof S||htmlWord instanceof S)
    throw 'restored interface ancestry';
"#,
    );
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), preserved);
}

#[test]
fn svg_element_ordinary_properties_keep_native_prototype_boundaries() {
    let (mut doc, mut forms, mut page) = fixture();
    let nodes = format!("{:?}", doc.nodes);
    let preserved = editor(&forms, id(&doc, "neighbor"));
    let handles = ["svg", "group", "shape"].map(|name| node_handle(id(&doc, name)));
    // The first read of either Object static method legitimately materializes
    // one native callable and its metadata object. Do that on the ordinary
    // interface prototype before measuring the refused native operations.
    run(&mut doc, &mut forms, &mut page, "if(Object.getPrototypeOf(P)!==Element.prototype||Object.setPrototypeOf(P,Element.prototype)!==P)throw 'ordinary prototype controls';");
    let overlays = handles.map(|handle| page.runtime.native_overlay_state_for_test(handle));
    run(&mut doc,&mut forms,&mut page,"if(Object.getPrototypeOf(svg)!==P||Object.getPrototypeOf(group)!==P||Object.getPrototypeOf(shape)!==P)throw 'canonical SVGElement suffix';");
    for (source, expected) in [
        (
            "Object.setPrototypeOf(svg,P);",
            "Native host object prototypes cannot be changed",
        ),
    ] {
        let allocations = page.runtime.window_allocation_state_for_test();
        page.clear_console();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(
            page.report().contains(expected),
            "{source}\n{}",
            page.report()
        );
        assert_eq!(
            page.runtime.window_allocation_state_for_test(),
            allocations,
            "{source}"
        );
        for (handle, original) in handles.into_iter().zip(overlays) {
            assert_eq!(
                page.runtime.native_overlay_state_for_test(handle),
                original,
                "{source}"
            );
        }
        assert_eq!(format!("{:?}", doc.nodes), nodes);
        assert_eq!(editor(&forms, id(&doc, "neighbor")), preserved);
        no_effects(&mut page);
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "if(!(svg instanceof S)||html instanceof S)throw 'native refusal changed ancestry';",
        );
    }
    run(&mut doc, &mut forms, &mut page, r#"
var missing='ordinaryFixtureKey', token={}, reads=0, key=Symbol('svg fixture');
if(svg[missing]!==undefined||missing in svg||Object.hasOwn(svg,missing))
    throw 'missing ordinary property';
Object.defineProperty(P,missing,{configurable:true,get:function(){
    if(this!==svg&&this!==shape)throw 'actual SVG receiver';
    reads++;
    return token;
}});
if(svg[missing]!==token||shape[missing]!==token||reads!==2||Object.hasOwn(svg,missing))
    throw 'ordinary inherited property';
Object.defineProperty(svg,missing,{value:17,writable:true,configurable:true});
svg[key]=shape;
if(svg[missing]!==17||shape[missing]!==token||svg[key]!==shape||
   Object.getOwnPropertyDescriptor(svg,missing).value!==17||!Object.hasOwn(svg,key))
    throw 'real own property';
delete svg[missing];delete P[missing];delete svg[key];
if(svg[missing]!==undefined||svg[key]!==undefined||missing in svg||
   svg.namespaceURI!=='http://www.w3.org/2000/svg'||svg.ownerDocument!==document)
    throw 'deletion or native identity';
"#);
    assert_eq!(format!("{:?}", doc.nodes), nodes);
    assert_eq!(editor(&forms, id(&doc, "neighbor")), preserved);
}

#[test]
fn svg_element_interface_owner_realms_and_saved_constructor_origin_guards() {
    let (mut doc, mut forms, mut page) = fixture_with_page(Page::with_limits(crate::javascript::limits::BROWSER));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);
var w=frame.contentWindow,F=w.Function,Foreign=w.SVGElement,foreignPrototype=Foreign.prototype;
var fromChild=F('node','return node instanceof SVGElement;');
var childOwn=F('return typeof SVGElement==="function"&&!(document.body instanceof SVGElement)&&document.body.ownerDocument===document;');
if(Foreign===S||foreignPrototype===P||Object.getPrototypeOf(Foreign)!==w.Element||
   Object.getPrototypeOf(foreignPrototype)!==w.Element.prototype||svg instanceof Foreign||fromChild(svg)||
   w.document.body instanceof Foreign||w.document.body instanceof S||!childOwn())throw 'owner realm identity';
var rejected=false;
try{new Foreign();}catch(error){rejected=Object.getPrototypeOf(error)===w.TypeError.prototype;}
if(!rejected)throw 'owner-realm illegal constructor TypeError';
"#,
    );
    let nodes = format!("{:?}", doc.nodes);
    let preserved = editor(&forms, id(&doc, "neighbor"));
    page.set_location(&location("https://different-svg-owner.test/changed"));
    for source in [
        "new Foreign();",
        "(class extends Foreign {});",
        "fromChild(svg);",
    ] {
        let allocations = page.runtime.window_allocation_state_for_test();
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(outcome.errors, 1, "{source}\n{}", page.report());
        assert!(page.report().contains("same-origin"), "{}", page.report());
        assert_eq!(
            page.runtime.window_allocation_state_for_test(),
            allocations,
            "{source}"
        );
        assert_eq!(format!("{:?}", doc.nodes), nodes);
        assert_eq!(editor(&forms, id(&doc, "neighbor")), preserved);
        no_effects(&mut page);
        run(&mut doc, &mut forms, &mut page, "if(!childOwn()||!(svg instanceof S)||html instanceof S||neighbor.value!=='edited neighbor')throw 'owner recovery';");
    }
}
