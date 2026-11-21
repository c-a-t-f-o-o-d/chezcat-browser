//! Original image clone/import identity, activation and transactional refusal.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc = crate::html::parse("<html><body><div id=target></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://image-import-owned.test/root",
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
fn image_import_clone_keep_real_owners_and_start_no_detached_requests() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<original-images><img id="first" src="one.ppm" alt="real" width="20" height="10" on-click="data"><img id="second" src="two.ppm"></original-images>';var source=t.content.firstChild,fragment=document.importNode(t.content,true),first=fragment.querySelector('#first'),second=fragment.querySelector('#second'),copy=first.cloneNode(true);
if(!(first instanceof HTMLImageElement)||!(copy instanceof HTMLImageElement)||!(copy instanceof Image)||copy===first||first.ownerDocument!==document||copy.ownerDocument!==document||first.isConnected||copy.parentNode!==null||copy.getAttribute('src')!=='one.ppm'||copy.getAttribute('on-click')!=='data'||copy.alt!=='real'||copy.width!==20||copy.height!==10||first.naturalWidth!==0||copy.naturalWidth!==0||first.complete||copy.complete||copy.currentSrc!==''||source.ownerDocument===document)throw 'actual detached image copies';
"##,
    );
    assert!(page.take_image_changes().2.is_empty());
    assert!(page.state.images.entries.is_empty());
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "document.getElementById('target').appendChild(fragment);",
    );
    let (_, _, requests) = page.take_image_changes();
    assert_eq!(requests.len(), 2);
    assert!(requests.iter().any(|r| r.source.ends_with("one.ppm")));
    assert!(requests.iter().any(|r| r.source.ends_with("two.ppm")));
    assert!(requests.iter().all(|r| page.current_image_request(r)));
    run(&mut doc,&mut forms,&mut page,"if(!first.isConnected||!second.isConnected||copy.isConnected||first.complete||copy.complete)throw 'real pending requests';");
}
#[test]
fn image_import_native_attempt_refusal_preserves_fragment_and_queues_nothing() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<img src=one.ppm><img src=two.ppm>';var fragment=document.importNode(t.content,true),first=fragment.firstChild,second=fragment.lastChild;");
    let attempts = page.state.images.attempts;
    page.state.images.attempts = MAX_IMAGE_REQUESTS;
    let out = page.console_eval(
        &mut doc,
        &mut forms,
        "document.getElementById('target').appendChild(fragment);",
    );
    assert_eq!(out.errors, 1);
    assert!(page.take_image_changes().2.is_empty());
    assert!(page.state.images.entries.is_empty());
    page.state.images.attempts = attempts;
    run(&mut doc,&mut forms,&mut page,"if(fragment.firstChild!==first||fragment.lastChild!==second||first.parentNode!==fragment||second.parentNode!==fragment||first.isConnected||document.getElementById('target').childNodes.length)throw 'failed connection changed actual topology';document.getElementById('target').appendChild(fragment);");
    assert_eq!(page.take_image_changes().2.len(), 2);
}
#[test]
fn image_import_responsive_and_handler_guards_refuse_without_partial_nodes() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var t=document.createElement('template');",
    );
    for markup in ["<original-images><img src=one.ppm><img src=two.ppm srcset='two.ppm 1x'></original-images>","<original-images><img src=one.ppm><img src=two.ppm onload='forbidden()'></original-images>","<original-images><img src=one.ppm><iframe></iframe></original-images>"]{
  run(&mut doc,&mut forms,&mut page,&format!("t.innerHTML={markup:?};"));page.state.rebuild(&doc).unwrap();let count=doc.nodes.len();let retained=page.state.retained;let out=page.console_eval(&mut doc,&mut forms,"document.importNode(t.content,true);");assert_eq!(out.errors,1);assert_eq!(doc.nodes.len(),count);assert_eq!(page.state.retained,retained);assert!(page.take_image_changes().2.is_empty());
 }
    run(&mut doc,&mut forms,&mut page,"t.innerHTML='<img src=one.ppm>';var copy=document.importNode(t.content,true);if(!(copy.firstChild instanceof HTMLImageElement)||copy.firstChild.getAttribute('src')!=='one.ppm')throw 'recovery';");
}
