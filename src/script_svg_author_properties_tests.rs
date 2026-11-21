//! Original namespace/owner witnesses for component binding metadata.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><div id=mount></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://svg-properties.test/source",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    run(&mut doc,&mut forms,&mut page,"var template=document.createElement('template');template.innerHTML='<svg width=24 height=24><rect id=shape width=12 height=12 fill=red></rect></svg>';var root=template.content.firstChild,shape=root.firstChild,owner=shape.ownerDocument;if(root.namespaceURI!=='http://www.w3.org/2000/svg'||shape.namespaceURI!==root.namespaceURI||owner===document||root.ownerDocument!==owner)throw 'actual inert SVG ownership';");
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn svg_author_properties_inert_parsed_and_created_metadata_keeps_native_namespace_attributes() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var created=owner.createElementNS('http://www.w3.org/2000/svg','rect'),symbol=Symbol('binding'),hits=0,observer=new MutationObserver(function(){});observer.observe(shape,{attributes:true,attributeOldValue:true});
for(var node of [root,shape,created]){var state={node:node};node.bindingState=state;node[symbol]=17;Object.defineProperty(node,'bindingValue',{configurable:true,get:function(){return this.bindingState;},set:function(value){hits++;this.setAttribute('fill',value);}});if(node.bindingValue!==state||node.bindingState.node!==node||node[symbol]!==17||Object.keys(node).indexOf('bindingState')<0||Object.getOwnPropertySymbols(node).indexOf(symbol)<0||Object.getPrototypeOf(node)!==SVGElement.prototype||node.namespaceURI!=='http://www.w3.org/2000/svg')throw 'real native SVG author store';}
if(observer.takeRecords().length||shape.getAttribute('fill')!=='red'||shape.id!=='shape'||shape.localName!=='rect')throw 'metadata changed native attrs';shape.bindingValue='blue';var records=observer.takeRecords();if(hits!==1||records.length!==1||records[0].target!==shape||records[0].attributeName!=='fill'||records[0].oldValue!=='red'||shape.getAttribute('fill')!=='blue')throw 'author setter keeps real mutation';
"#,
    );
    let commits = page.state.dom_commits;
    let out = page.console_eval(
        &mut doc,
        &mut forms,
        "shape.bindingValue='url(https://external-svg-properties.test/paint.svg#p)';",
    );
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    run(&mut doc,&mut forms,&mut page,"if(hits!==2||shape.getAttribute('fill')!=='blue'||observer.takeRecords().length)throw 'author setter preserves SVG resource refusal';");
}
#[test]
fn svg_author_properties_clone_import_and_supported_promotion_keep_independent_state() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var state={held:shape};shape.bindingState=state;root.bindingState={held:root};var clone=shape.cloneNode(true),imported=document.importNode(template.content,true),copy=imported.querySelector('#shape');if(clone.bindingState!==undefined||copy.bindingState!==undefined||copy.getAttribute('fill')!=='red'||clone.namespaceURI!==shape.namespaceURI)throw 'DOM copying is not metadata copying';copy.bindingState={held:copy};document.getElementById('mount').appendChild(imported);if(copy.ownerDocument!==document||copy.bindingState.held!==copy||shape.ownerDocument!==owner||shape.bindingState!==state)throw 'independent imported owner';document.body.appendChild(template.content);if(shape.ownerDocument!==document||root.ownerDocument!==document||shape.bindingState!==state||state.held!==shape||root.bindingState.held!==root||shape.getAttribute('fill')!=='red')throw 'staged alias promotion';
"#,
    );
}
#[test]
fn svg_author_properties_quota_failure_and_late_foreign_access_preserve_state() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var hits=0,state={};Object.defineProperty(shape,'bindingState',{get:function(){hits++;return state;},set:function(v){hits++;state=v;},configurable:true});var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var read=frame.contentWindow.Function('node','return node.bindingState;');if(read(shape)!==state||hits!==1)throw 'same origin';");
    let commits = page.state.dom_commits;
    let limit = page.runtime.event_property_allowance_for_test(0);
    let out = page.console_eval(&mut doc, &mut forms, "shape.unpublishedBinding=17;");
    page.runtime.event_property_allowance_for_test(limit);
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(page.state.dom_commits, commits);
    run(&mut doc,&mut forms,&mut page,"if(Object.hasOwn(shape,'unpublishedBinding')||shape.bindingState!==state)throw 'failed publication';shape.recoveredBinding=37;if(shape.recoveredBinding!==37)throw 'fresh job recovery';");
    page.set_location(
        &crate::navigation::address(
            "https://other-svg-properties.test/",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    let out = page.console_eval(&mut doc, &mut forms, "read(shape);");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert!(page.report().contains("origin") || page.report().contains("Security"));
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if(hits!==2||shape.getAttribute('fill')!=='red')throw 'foreign getter effects';",
    );
}
