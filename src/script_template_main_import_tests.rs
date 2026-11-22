//! Real main-template identity, appropriate document ownership and rollback.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<html><head></head><body><div id=mount></div><template id=loaded><span id=inside>Loaded</span><template><b>Nested</b></template></template></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn run(doc: &mut Document, forms: &mut Forms, page: &mut Page, source: &str) {
    let out = page.console_eval(doc, forms, source);
    assert_eq!(out.errors, 0, "{source}\n{}", page.report());
}
#[test]
fn template_main_import_nested_contents_keep_real_owners_and_separate_trees() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var source=document.createElement('template');source.innerHTML='<original-shell><template id="nested"><style>.inert{color:red}</style><input value="seed"><img src="https://unrequested.test/inert.png"><span id="label">Original</span><template><b>Deep</b></template></template></original-shell>';var original=source.content,owner=original.ownerDocument,copy=document.importNode(source,true),content=copy.content,nested=content.querySelector('#nested'),originalNested=original.querySelector('#nested');
if(copy===source||!(copy instanceof HTMLTemplateElement)||copy.ownerDocument!==document||copy.childNodes.length||content===original||content.ownerDocument!==owner||content.parentNode!==null||nested===originalNested||nested.ownerDocument!==owner||nested.childNodes.length||nested.content===originalNested.content||nested.content.ownerDocument!==owner||nested.content.querySelector('#label').textContent!=='Original'||nested.content.querySelector('template').content.textContent!=='Deep')throw 'genuine template contents and owners';
document.getElementById('mount').appendChild(copy);if(!copy.isConnected||content.isConnected||nested.isConnected||document.querySelector('#label')!==null||document.querySelector('#nested')!==null)throw 'content did not stay inert';
source.innerHTML='<span>Changed source</span>';if(nested.content.querySelector('#label').textContent!=='Original'||copy.content!==content)throw 'copy aliased source content';
"##,
    );
    assert!(page.take_image_changes().2.is_empty());
    assert_eq!(page.state.stylesheets.source_bytes, 0);
    assert!(forms.controls.is_empty());
}
#[test]
fn template_main_import_cross_inert_documents_use_destination_appropriate_owner() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var other=document.implementation.createHTMLDocument('source'),source=other.createElement('template');source.innerHTML='<span>Outer</span><template><i>Inner</i></template>';var oldOwner=source.content.ownerDocument,reference=document.createElement('template').content.ownerDocument,copy=document.importNode(source,true),nested=copy.content.querySelector('template');
if(copy.ownerDocument!==document||copy.content.ownerDocument!==reference||reference===oldOwner||copy.content.firstChild.ownerDocument!==reference||nested.ownerDocument!==reference||nested.content.ownerDocument!==reference||nested.content.firstChild.ownerDocument!==reference||source.ownerDocument!==other||source.content.ownerDocument!==oldOwner||copy.content.firstChild===source.content.firstChild)throw 'appropriate target template document';
var old=copy.content.firstChild;nested.innerHTML='<b>Changed destination</b>';if(source.content.lastChild.content.textContent!=='Inner'||old.parentNode!==copy.content)throw 'independent target owner state';
"##,
    );
}
#[test]
fn template_main_clone_returns_movable_main_host_and_shallow_empty_content() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r##"
var source=document.getElementById('loaded'),original=source.content,deep=source.cloneNode(true),shallow=source.cloneNode(false),mount=document.getElementById('mount');var savedContent=deep.content;
if(deep.ownerDocument!==document||shallow.ownerDocument!==document||deep.content===original||deep.content.ownerDocument!==original.ownerDocument||deep.content.firstChild===original.firstChild||deep.content.textContent!=='Loaded'||shallow.content.childNodes.length||shallow.childNodes.length||deep.childNodes.length)throw 'actual main template clone';
mount.appendChild(deep);var fragment=document.createDocumentFragment();fragment.appendChild(shallow);mount.appendChild(fragment);if(!deep.isConnected||!shallow.isConnected||deep.parentNode!==mount||shallow.parentNode!==mount||deep.content.isConnected||mount.querySelector('#inside')!==null||deep.content.lastChild.content.textContent!=='Nested')throw 'movable main hosts without content activation';
mount.removeChild(deep);if(deep.isConnected||deep.content!==savedContent||deep.content.firstChild.textContent!=='Loaded')throw 'host removal preserved contents';mount.textContent='Cleared';if(shallow.isConnected||shallow.parentNode!==null||shallow.content.childNodes.length||deep.content!==savedContent)throw 'text replacement retired only actual hosts';
"##,
    );
}
#[test]
fn template_main_import_rolls_back_new_content_links_and_keeps_reusable_backing() {
    let (mut doc, mut forms, mut page) = fixture();
    run(&mut doc,&mut forms,&mut page,"var t=document.createElement('template');t.innerHTML='<template><span>Kept</span></template>';var source=t.content.firstChild,old=source.content;");
    page.state.rebuild(&doc).unwrap();
    let Value::Host(source) = page
        .runtime
        .execute(
            "source;",
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
    let entries = page.state.inert_documents.entries.len();
    let contents = page.state.inert_documents.template_contents.len();
    let documents = page.state.inert_documents.template_documents.len();
    let retained = page.state.retained;
    let checkpoint = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .template_import_checkpoint();
    let backing=checkpoint.backing;
    let destination = node_handle(doc.nodes.len());
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    assert!(bridge
        .prepare_main_template_contents(&[(source, destination), (0, destination + 1)], true)
        .is_err());
    assert!(bridge.state.inert_documents.entries.len() > entries);
    bridge.rollback_template_import(checkpoint);
    assert_eq!(bridge.state.inert_documents.entries.len(), entries);
    assert_eq!(
        bridge.state.inert_documents.template_contents.len(),
        contents
    );
    assert_eq!(
        bridge.state.inert_documents.template_documents.len(),
        documents
    );
    assert_eq!(
        bridge.state.retained,
        retained + bridge.template_import_backing() - backing
    );
    assert!(!bridge
        .state
        .inert_documents
        .template_contents
        .contains_key(&destination));
    drop(bridge);
    run(&mut doc,&mut forms,&mut page,"if(source.content!==old||old.textContent!=='Kept')throw 'failed import altered source identity';var copy=document.importNode(t,true);if(copy.content.firstChild.content.textContent!=='Kept'||copy.ownerDocument!==document)throw 'real recovery';");
}

#[test]
fn template_import_identity_late_copy_failure_rolls_back_native_bundle_and_recovers() {
    fn prepared() -> (Document,Forms,Page,u32,u32) {
        let (mut doc,mut forms,mut page)=fixture();
        let source=node_handle(doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element {id:Some(id),..} if id=="loaded")).unwrap());
        let Value::Host(content)= (Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state}).template_content(source).unwrap() else {panic!("content")};
        page.begin_job();
        (doc,forms,page,source,content)
    }
    let (mut baseline,mut baseline_forms,mut baseline_page,source,_)=prepared();
    let allowance=baseline_page.state.copy_work;
    let mut bridge=Bridge {doc:&mut baseline,forms:&mut baseline_forms,state:&mut baseline_page.state};
    bridge.import_node(&[Value::Host(source),Value::Bool(true)],None).unwrap();
    let needed=allowance-bridge.state.copy_work;
    let (mut doc,mut forms,mut page,source,content)=prepared();
    let before=(doc.nodes.len(),page.state.retained,page.state.inert_documents.entries.len(),
        page.state.inert_documents.adopted.len(),page.state.dom.created.len(),page.state.dom_commits);
    let mut bridge=Bridge {doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let backing=bridge.template_import_backing();
    bridge.state.copy_work=needed-1;
    assert!(bridge.import_node(&[Value::Host(source),Value::Bool(true)],None).is_err());
    assert_eq!(bridge.doc.nodes.len(),before.0);
    assert_eq!(bridge.state.inert_documents.entries.len(),before.2);
    assert_eq!(bridge.state.inert_documents.adopted.len(),before.3);
    assert_eq!(bridge.state.dom.created.len(),before.4);
    assert_eq!(bridge.state.dom_commits,before.5);
    assert!(bridge.template_import_backing()>backing,"late failure must exercise acquired bundle backing");
    assert_eq!(bridge.state.retained,before.1+bridge.template_import_backing()-backing);
    assert_eq!(bridge.state.inert_documents.template_contents.get(&source),Some(&content));
    drop(bridge);page.begin_job();
    run(&mut doc,&mut forms,&mut page,"var source=document.getElementById('loaded'),copy=document.importNode(source,true),content=copy.content,nested=content.lastChild,inner=nested.content.firstChild,span=content.firstChild;document.getElementById('mount').appendChild(copy);document.body.appendChild(nested.content);content.removeChild(nested);document.body.appendChild(content);if(span.ownerDocument!==document||inner.ownerDocument!==document||source.content.firstChild===span||source.content.textContent!=='Loaded')throw 'late import recovery';");
}
