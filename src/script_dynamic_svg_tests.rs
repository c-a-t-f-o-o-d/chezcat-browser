//! Original dynamic-svg construction, coercion, owner and storage checks.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(
        "<html><body><div id=outer><div id=target>Original</div></div></body></html>",
    );
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://dynamic-svg-owned.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn dynamic_svg_creation_attributes_namespace_identity_and_clone_are_real() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var ns='http://www.w3.org/2000/svg',xl='http://www.w3.org/1999/xlink',xn='http://www.w3.org/2000/xmlns/',svg=document.createElementNS(ns,'svg'),defs=document.createElementNS(ns,'defs'),path=document.createElementNS(ns,'path'),use=document.createElementNS(ns,'use');
svg.setAttributeNS(xn,'xmlns',ns);svg.setAttributeNS(xn,'xmlns:xlink',xl);svg.setAttribute('viewBox','0 0 20 20');svg.setAttribute('width','80');svg.setAttribute('height','80');path.setAttribute('id','shape');path.setAttribute('d','M0 0 H20 V20 H0 Z');path.setAttribute('fill','#e03040');use.setAttributeNS(xl,'xlink:href','#shape');defs.appendChild(path);svg.appendChild(defs);svg.appendChild(use);svg.classList='first';svg.classList.add('built');
if(!(svg instanceof SVGElement)||!(svg instanceof Element)||svg instanceof HTMLElement||svg.namespaceURI!==ns||svg.localName!=='svg'||svg.tagName!=='svg'||svg.prefix!==null||svg.ownerDocument!==document||svg.getAttribute('viewBox')!=='0 0 20 20'||svg.getAttribute('viewbox')!==null||!svg.classList.contains('built')||svg.attributes.getNamedItem('viewBox').value!=='0 0 20 20')throw 'real SVG namespace/attribute metadata';
var attr=use.getAttributeNodeNS(xl,'href');if(attr.namespaceURI!==xl||attr.prefix!=='xlink'||attr.localName!=='href'||attr.value!=='#shape'||use.getAttribute('xlink:href')!=='#shape'||use.getAttribute('href')!==null)throw 'real expanded names';
var clone=svg.cloneNode(true);if(clone===svg||clone.namespaceURI!==ns||clone.querySelector('path')===path||clone.querySelector('use').getAttributeNS(xl,'href')!=='#shape'||clone.querySelector('path').getAttribute('d')!=='M0 0 H20 V20 H0 Z')throw 'real namespaced clone';
document.getElementById('target').appendChild(clone);if(!clone.isConnected||svg.isConnected||clone.querySelector('path').ownerDocument!==document)throw 'actual SVG connection';
var look=document.createElement('svg');if(look.namespaceURI===ns||look instanceof SVGElement||!(look instanceof HTMLElement))throw 'HTML local name fabricated SVG';
"##,
    );
    assert!(!page.has_host_tasks());
}
#[test]
fn dynamic_svg_inert_factories_attribute_aliases_and_deep_import_preserve_owners() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var ns='http://www.w3.org/2000/svg',owner=document.implementation.createHTMLDocument('icons'),svg=owner.createElementNS(ns,'svg'),group=owner.createElementNS(ns,'g'),rect=owner.createElementNS(ns,'rect');svg.setAttribute('viewBox','0 0 10 10');rect.setAttribute('width','10');rect.setAttribute('height','10');rect.setAttribute('fill','#20a040');group.appendChild(rect);svg.appendChild(group);var saved=rect.getAttributeNode('fill');rect.setAttribute('fill','#3040a0');if(saved.value!=='#3040a0'||saved.ownerElement!==rect||rect.ownerDocument!==owner||rect.namespaceURI!==ns||svg.querySelector('rect')!==rect)throw 'real inert SVG data';
var clone=svg.cloneNode(true),copy=document.importNode(svg,true);if(clone.ownerDocument!==owner||clone===svg||clone.querySelector('rect')===rect||copy.ownerDocument!==document||copy.namespaceURI!==ns||copy.querySelector('rect').getAttribute('fill')!=='#3040a0'||saved.ownerDocument!==owner)throw 'namespace/owner independent copies';
var fragment=document.createDocumentFragment();fragment.appendChild(copy);document.getElementById('target').appendChild(fragment);if(!copy.isConnected||svg.ownerDocument!==owner||svg.querySelector('rect')!==rect)throw 'real namespace fragment splice';
"##,
    );
}
#[test]
fn dynamic_svg_resource_namespace_and_hierarchy_refusals_are_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg'),path=document.createElementNS(ns,'path'),use=document.createElementNS(ns,'use');path.setAttribute('d','M0 0 H1 V1 Z');use.setAttribute('href','#safe');");
    for code in [
        "document.createElementNS(ns,'script');",
        "document.createElementNS(ns,'foreignObject');",
        "document.createElementNS(ns,'image');",
        "document.createElementNS(ns,'animate');",
        "document.createElementNS('urn:unrelated','xml:svg');",
        "path.setAttribute('onclick','forbidden()');",
        "use.setAttribute('href','https://unrequested.test/icon.svg#x');",
        "use.setAttributeNS('http://www.w3.org/1999/xlink','xlink:href','javascript:forbidden()');",
        "svg.appendChild(document.createElement('span'));",
        "document.getElementById('target').appendChild(path);",
    ] {
        let nodes = doc.nodes.len();
        let out = page.console_eval(&mut doc, &mut forms, code);
        assert_eq!(out.errors, 1, "{code}\n{}", page.report());
        if !code.contains("createElement('span')") {
            assert_eq!(doc.nodes.len(), nodes);
        }
    }
    run(&mut doc,&mut forms,&mut page,"if(path.parentNode!==null||path.getAttribute('onclick')!==null||use.getAttribute('href')!=='#safe'||svg.childNodes.length)throw 'refusal changed actual data';svg.appendChild(path);document.getElementById('target').appendChild(svg);if(!path.isConnected||path.namespaceURI!==ns)throw 'recovery';");
    run(&mut doc,&mut forms,&mut page,"var prefixed=document.createElementNS(ns,'svg:svg');if(prefixed.namespaceURI!==ns||prefixed.prefix!=='svg'||prefixed.localName!=='svg'||prefixed.tagName!=='svg:svg'||prefixed.cloneNode().prefix!=='svg')throw 'real prefixed SVG';");
    assert!(page.take_image_changes().2.is_empty());
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
}
#[test]
fn dynamic_svg_native_allocation_and_copy_refusal_do_not_publish_nodes() {
    let (mut doc, mut forms, mut page) = fixture();
    page.state.rebuild(&doc).unwrap();
    let before = page.state.retained;
    let nodes = doc.nodes.len();
    let limit = page.state.retained_limit;
    let copies = page.state.copy_work;
    page.state.retained_limit = before;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .create_dom_node_ns(
        NodeKind::Element {expanded_name:Vec::new(),
            tag: "svg".into(),
            id: None,
            href: None,
            attrs: Vec::new()
        },
        crate::html::Namespace::Svg
    )
    .is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(doc.nodes.len(), nodes);
    page.state.retained_limit = limit;
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .create_dom_node_ns(
        NodeKind::Element {expanded_name:Vec::new(),
            tag: "path".into(),
            id: None,
            href: None,
            attrs: Vec::new()
        },
        crate::html::Namespace::Svg
    )
    .is_err());
    assert_eq!(page.state.retained, before);
    assert_eq!(doc.nodes.len(), nodes);
    page.state.copy_work = copies;
    run(&mut doc,&mut forms,&mut page,"var svg=document.createElementNS('http://www.w3.org/2000/svg','svg');if(svg.namespaceURI!=='http://www.w3.org/2000/svg')throw 'real namespace recovery';");
}

#[test]
fn dynamic_svg_shape_reorder_replacement_fragment_splice_and_attr_detach_are_real() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var ns='http://www.w3.org/2000/svg',svg=document.createElementNS(ns,'svg'),g=document.createElementNS(ns,'g'),rect=document.createElementNS(ns,'rect'),circle=document.createElementNS(ns,'circle'),path=document.createElementNS(ns,'path');
rect.setAttribute('width','12');rect.setAttribute('height','14');circle.setAttribute('r','5');path.setAttribute('d','M0 0 H8 V8 Z');svg.appendChild(g);g.appendChild(rect);g.insertBefore(circle,rect);if(g.firstChild!==circle||circle.nextSibling!==rect)throw 'actual SVG insertBefore';
var fragment=document.createDocumentFragment();fragment.appendChild(path);if(g.replaceChild(fragment,circle)!==circle||circle.parentNode!==null||g.firstChild!==path||path.nextSibling!==rect||fragment.childNodes.length)throw 'actual SVG fragment replacement';
g.appendChild(path);if(g.firstChild!==rect||g.lastChild!==path)throw 'actual SVG reorder';var attr=rect.getAttributeNode('width');if(rect.removeAttributeNode(attr)!==attr||attr.ownerElement!==null||rect.getAttribute('width')!==null)throw 'real SVG detached Attr';attr.value='18';rect.setAttributeNode(attr);if(rect.getAttribute('width')!=='18'||attr.ownerElement!==rect)throw 'real SVG Attr reattachment';
document.getElementById('target').appendChild(svg);g.removeChild(path);if(path.isConnected||!rect.isConnected||g.childNodes.length!==1||svg.namespaceURI!==ns)throw 'actual SVG removal';
"##,
    );
}

#[test]
fn dynamic_svg_import_revalidates_inert_style_before_main_publication() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var seed=document.createElement('template');seed.innerHTML='<svg style=\"color:url(/guard)\"><rect width=\"2\" height=\"3\"/></svg>';var inert=seed.content.firstChild;");
    let nodes = doc.nodes.len();
    let commits = page.state.dom_commits;
    let retained = page.state.retained;
    let failed = page.console_eval(&mut doc, &mut forms, "document.importNode(inert,true);");
    assert_eq!(failed.errors, 1);
    assert_eq!(doc.nodes.len(), nodes);
    assert_eq!(page.state.dom_commits, commits);
    assert_eq!(page.state.retained, retained);
    run(&mut doc,&mut forms,&mut page,"inert.removeAttribute('style');var copy=document.importNode(inert,true);document.getElementById('target').appendChild(copy);if(!copy.isConnected||copy.namespaceURI!=='http://www.w3.org/2000/svg')throw 'guarded import recovery';");
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
    let (a, b, c) = page.take_image_changes();
    assert!(a.is_empty() && b.is_empty() && c.is_empty());
}
