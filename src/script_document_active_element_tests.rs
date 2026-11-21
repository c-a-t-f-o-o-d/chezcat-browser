//! Real committed focus readback, canonical Document accessor and owner guards.
use super::*;
fn fixture(markup: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://active-owned.test/main",
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
fn document_active_element_committed_focus_readonly_brands_author_shadow_and_removal_are_real() {
    let (mut doc, mut forms, mut page) = fixture(
        "<html><body><input id=a value=seed><input id=b><div id=activeElement></div></body></html>",
    );
    run(&mut doc,&mut forms,&mut page,"var a=document.getElementById('a'),b=document.getElementById('b'),d=Object.getOwnPropertyDescriptor(Document.prototype,'activeElement'),get=d.get,hits=0,hostile={toString:function(){hits++;throw 'coercion';}};if(document.activeElement!==document.body||d.set!==undefined||!d.enumerable||!d.configurable||get.length!==0||get.name!=='get activeElement'||Object.getPrototypeOf(get)!==Function.prototype)throw 'document default or descriptor';");
    let a = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("a"))
        .unwrap();
    page.begin_job();
    assert_eq!(
        page.focus_transition(&mut doc, &mut forms, None, Some(a))
            .errors,
        0
    );
    run(&mut doc,&mut forms,&mut page,"if(document.activeElement!==a||get.call(document,hostile)!==a)throw 'committed focus';b.dispatchEvent(new FocusEvent('focusin',{bubbles:true}));document.activeElement=hostile;if(document.activeElement!==a||hits!==0||Object.prototype.hasOwnProperty.call(document,'activeElement'))throw 'synthetic focus or readonly mutation';var strict=0;try{(function(){'use strict';document.activeElement=hostile;})();}catch(e){if(!(e instanceof TypeError))throw e;strict++;}if(strict!==1||hits!==0)throw 'readonly strict';for(var receiver of [{},document.body,document.createDocumentFragment()]){var bad=0;try{get.call(receiver,hostile);}catch(e){if(!(e instanceof TypeError))throw e;bad++;}if(bad!==1)throw 'document brand';}Object.defineProperty(document,'activeElement',{value:b,writable:true,configurable:true});if(document.activeElement!==b||get.call(document)!==a)throw 'author shadow versus saved native';delete document.activeElement;delete Document.prototype.activeElement;if(document.activeElement!==undefined||'activeElement' in document||get.call(document)!==a)throw 'deleted prototype';Object.defineProperty(Document.prototype,'activeElement',d);a.parentNode.removeChild(a);if(document.activeElement!==document.body)throw 'removed focus';document.body.appendChild(a);if(document.activeElement!==document.body)throw 'reinserted focus';");
}
#[test]
fn document_active_element_inert_and_child_documents_keep_owner_and_origin_scope() {
    let (mut doc, mut forms, mut page) = fixture("<html><body></body></html>");
    run(&mut doc,&mut forms,&mut page,"var get=Object.getOwnPropertyDescriptor(Document.prototype,'activeElement').get,inert=document.implementation.createHTMLDocument('inert'),t=document.createElement('template');if(get.call(inert)!==inert.body||inert.activeElement!==inert.body||get.call(t.content.ownerDocument)!==null)throw 'real inert viewport fallback';var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,cd=w.document,cg=Object.getOwnPropertyDescriptor(w.Document.prototype,'activeElement').get;if(cd.activeElement!==cd.body||get.call(cd)!==cd.body||cg.call(cd)!==cd.body||Object.getPrototypeOf(cg)!==w.Function.prototype)throw 'child owner';");
    page.set_location(
        &crate::navigation::address(
            "https://other-active.test/",
            &crate::navigation::Location::home(),
        )
        .unwrap(),
    );
    for source in [
        "cd.activeElement;",
        "cg.call(cd);",
        "get.call(cd);",
        "cg.call(document);",
    ] {
        let out = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(out.errors, 1, "{source} {}", page.report());
        assert!(
            page.report().contains("origin") || page.report().contains("Security"),
            "{}",
            page.report()
        );
    }
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "if(get.call(document)!==document.body)throw 'main recovery';",
    );
}
#[test]
fn document_active_element_document_element_and_empty_fallbacks_are_actual_nodes() {
    for markup in ["<html><head></head></html>", ""] {
        let (mut doc, mut forms, mut page) = fixture(markup);
        run(
            &mut doc,
            &mut forms,
            &mut page,
            "if(document.activeElement!==document.documentElement)throw 'actual root fallback';",
        );
    }
}
