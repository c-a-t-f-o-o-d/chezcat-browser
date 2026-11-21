//! Original passive-html-import construction, coercion, owner and storage checks.
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
            "https://passive-html-import-owned.test/source",
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
fn passive_html_import_clones_preserve_real_tag_attributes_topology_and_owners() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var t=document.createElement('template');t.innerHTML='<original-shell id="shell"><original-search on-click="ordinary" on-keydown="keys"><span id="label">Real descendants</span></original-search><a href="#end">Link</a></original-shell>';var source=t.content.firstChild,owner=source.ownerDocument,imported=document.importNode(t.content,true),copy=imported.firstChild;
if(copy===source||copy.localName!=='original-shell'||copy.ownerDocument!==document||source.ownerDocument!==owner||copy.firstChild.localName!=='original-search'||copy.firstChild.getAttribute('on-click')!=='ordinary'||copy.firstChild.getAttribute('on-keydown')!=='keys'||copy.firstChild.firstChild.textContent!=='Real descendants'||!(copy instanceof HTMLElement)||!(copy instanceof Element)||copy.parentNode!==imported||copy.firstChild.parentNode!==copy||source.isConnected)throw 'real imported passive HTML';
var shallow=copy.cloneNode(false),deep=copy.cloneNode(true);if(shallow.childNodes.length||deep.firstChild===copy.firstChild||deep.firstChild.localName!=='original-search'||deep.ownerDocument!==document||deep.firstChild.getAttribute('on-click')!=='ordinary'||deep.querySelector('#label').textContent!=='Real descendants')throw 'native main clone family';
document.getElementById('target').appendChild(imported);if(!copy.isConnected||document.querySelector('original-shell > original-search > span')!==copy.firstChild.firstChild||source.parentNode!==t.content||source.querySelector('#label')===copy.querySelector('#label'))throw 'real connection and source independence';
if(typeof customElements!=='undefined'||'attachShadow' in HTMLElement.prototype)throw 'fake custom element or shadow feature detection';
"##,
    );
    assert!(!page.has_host_tasks());
}
#[test]
fn passive_html_import_late_native_handler_and_customized_builtin_refusals_are_atomic() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<original-shell><span>Kept</span></original-shell>';var source=t.content.firstChild,owner=source.ownerDocument;");
    for source in ["t.innerHTML='<original-shell><span onclick=forbidden()>Bad</span></original-shell>';","t.innerHTML='<original-shell is=unsupported><span>Bad</span></original-shell>';","t.innerHTML='<original-shell><span>Prefix</span><input type=radio></original-shell>';" ] {
  run(&mut doc,&mut forms,&mut page,source);let before=page.state.retained;let count=doc.nodes.len();let commits=page.state.dom_commits;
  let result=page.console_eval(&mut doc,&mut forms,"document.importNode(t.content,true);");assert_eq!(result.errors,1,"{}",page.report());assert_eq!(page.state.retained,before);assert_eq!(doc.nodes.len(),count);assert_eq!(page.state.dom_commits,commits);
 }
    run(&mut doc,&mut forms,&mut page,"t.innerHTML='<original-shell on-click=data><span>Recovered</span></original-shell>';var copy=document.importNode(t.content,true);if(copy.firstChild.localName!=='original-shell'||copy.firstChild.getAttribute('on-click')!=='data'||copy.firstChild.textContent!=='Recovered')throw 'recovery';");
    assert!(!page.has_host_tasks());
}
#[test]
fn passive_html_clone_active_native_handlers_still_refuse_before_publication() {
    let mut doc=crate::html::parse("<html><body><original-shell id=guarded><span onclick='forbidden()'>Kept</span></original-shell></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var parent=document.getElementById('guarded'),child=parent.firstChild;",
    );
    let before = page.state.retained;
    let count = doc.nodes.len();
    let result = page.console_eval(&mut doc, &mut forms, "parent.cloneNode(true);");
    assert_eq!(result.errors, 1);
    assert_eq!(page.state.retained, before);
    assert_eq!(doc.nodes.len(), count);
    run(&mut doc,&mut forms,&mut page,"if(document.getElementById('guarded')!==parent||parent.firstChild!==child||child.getAttribute('onclick')!=='forbidden()')throw 'refusal changed source';var safe=document.createElement('original-shell'),leaf=document.createElement('span');leaf.setAttribute('on-click','ordinary');safe.appendChild(leaf);var cloned=safe.cloneNode(true);if(cloned===safe||cloned.firstChild===leaf||cloned.firstChild.getAttribute('on-click')!=='ordinary')throw 'native clone recovery';");
}

#[test]
fn passive_html_import_payload_refusal_leaves_source_and_destinations_unchanged() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<original-shell><original-content data-note=real>Text</original-content></original-shell>';var f=t.content,source=f.firstChild;");
    let Value::Host(handle) = page
        .runtime
        .execute(
            "f;",
            &mut Bridge {
                doc: &mut doc,
                forms: &mut forms,
                state: &mut page.state,
            },
        )
        .unwrap()
    else {
        panic!("real source")
    };
    let before = page.state.retained;
    let count = doc.nodes.len();
    let limit = page.state.retained_limit;
    let copies = page.state.copy_work;
    page.state.retained_limit = before;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .import_node(&[Value::Host(handle), Value::Bool(true)], None)
    .is_err());
    assert_eq!(doc.nodes.len(), count);
    assert_eq!(page.state.retained, before);
    page.state.retained_limit = limit;
    page.state.copy_work = 0;
    assert!(Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state
    }
    .import_node(&[Value::Host(handle), Value::Bool(true)], None)
    .is_err());
    assert_eq!(doc.nodes.len(), count);
    assert_eq!(page.state.retained, before);
    page.state.copy_work = copies;
    run(&mut doc,&mut forms,&mut page,"var copy=document.importNode(f,true);if(copy.firstChild===source||source.parentNode!==f||copy.querySelector('original-content').getAttribute('data-note')!=='real')throw 'native recovery';");
}
