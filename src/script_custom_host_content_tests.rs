//! Original passive custom-host content and native mutation boundaries.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<html><body><original-card id=host><span id=old>old</span><nested-card id=nested>nested</nested-card></original-card><div id=ordinary></div><protected-card id=protected><input id=field value=seed></protected-card></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::with_limits(crate::javascript::limits::BROWSER);
    page.set_location(
        &crate::navigation::address(
            "https://custom-content.test/source",
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
fn custom_host_content_native_text_replacement_records_range_and_saved_identity() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var host=document.getElementById('host'),old=host.firstChild,nested=old.nextSibling,list=host.childNodes,range=new Range(),observer=new MutationObserver(function(){}),set=Object.getOwnPropertyDescriptor(Node.prototype,'textContent').set;range.setStart(old.firstChild,1);range.setEnd(nested.firstChild,3);observer.observe(host,{childList:true});var conversions=0;set.call(host,{toString:function(){conversions++;return 'A<&';}});var records=observer.takeRecords();if(conversions!==1||host.textContent!=='A<&'||host.innerHTML!=='A&lt;&amp;'||list.length!==1||host.firstChild.nodeType!==3||old.parentNode!==null||nested.parentNode!==null||old.textContent!=='old'||nested.textContent!=='nested'||old.isConnected||nested.isConnected)throw 'actual native replacement';if(records.length!==1||records[0].target!==host||records[0].removedNodes.length!==2||records[0].removedNodes[0]!==old||records[0].removedNodes[1]!==nested||records[0].addedNodes.length!==1||records[0].addedNodes[0]!==host.firstChild||records[0].previousSibling!==null||records[0].nextSibling!==null)throw 'whole-content mutation';if(range.startContainer!==host||range.startOffset||range.endContainer!==host||range.endOffset||!range.collapsed)throw 'actual Range retirement';
var made=document.createElement('created-card');made.textContent='detached';if(made.firstChild.data!=='detached'||made.isConnected)throw 'detached native';document.body.appendChild(made);made.textContent=null;if(made.childNodes.length||made.textContent!=='')throw 'empty replacement';
"#,
    );
}
#[test]
fn custom_host_content_inert_import_promotion_and_ordinary_parent_remove_passive_custom_tree() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        r#"
var template=document.createElement('template');template.innerHTML='<staged-card><nested-card>staged</nested-card></staged-card>';var staged=template.content.firstChild,owner=staged.ownerDocument;staged.textContent='inert';if(staged.textContent!=='inert'||staged.ownerDocument!==owner)throw 'inert native';var imported=document.importNode(template.content,true),copy=imported.firstChild;copy.textContent='imported';document.getElementById('ordinary').appendChild(imported);if(copy.ownerDocument!==document||copy.textContent!=='imported')throw 'import owner';document.body.appendChild(template.content);staged.textContent='promoted';if(staged.ownerDocument!==document||staged.firstChild.data!=='promoted')throw 'promoted alias';var old=copy;document.getElementById('ordinary').textContent='replacement';if(old.parentNode!==null||old.textContent!=='imported'||document.getElementById('ordinary').firstChild.data!=='replacement')throw 'ordinary parent passive custom retirement';
"#,
    );
}
#[test]
fn custom_host_content_protected_controls_and_resources_refuse_without_publication() {
    let (mut doc, mut forms, mut page) = fixture();
    let field = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("field"))
        .unwrap();
    forms.script_set_value(field, "edited").unwrap();
    let before = (
        doc.nodes.len(),
        page.state.dom_commits,
        forms.controls[&field].value.text(),
    );
    run(&mut doc,&mut forms,&mut page,"var protectedHost=document.getElementById('protected'),child=protectedHost.firstChild,observer=new MutationObserver(function(){}),range=new Range();range.selectNode(child);observer.observe(protectedHost,{childList:true});");
    let out = page.console_eval(&mut doc, &mut forms, "protectedHost.textContent='blocked';");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(
        (
            doc.nodes.len(),
            page.state.dom_commits,
            forms.controls[&field].value.text()
        ),
        before
    );
    run(&mut doc,&mut forms,&mut page,"if(protectedHost.firstChild!==child||child.parentNode!==protectedHost||child.value!=='edited'||observer.takeRecords().length||range.startContainer!==protectedHost||range.startOffset!==0||range.endOffset!==1)throw 'protected native prefix publication';");
    assert!(page.take_dynamic_scripts().is_empty());
    assert!(page.take_dynamic_stylesheets().is_empty());
}
#[test]
fn custom_host_content_node_admission_and_foreign_namespace_keep_real_tree() {
    let (mut doc, mut forms, mut page) = fixture();
    run(
        &mut doc,
        &mut forms,
        &mut page,
        "var host=document.getElementById('host'),first=host.firstChild,list=host.childNodes;",
    );
    let count = doc.nodes.len();
    let commits = page.state.dom_commits;
    let limit = page.state.node_limit;
    page.state.node_limit = count;
    let out = page.console_eval(&mut doc, &mut forms, "host.textContent='unpublished';");
    assert_eq!(out.errors, 1, "{}", page.report());
    assert_eq!(doc.nodes.len(), count);
    assert_eq!(page.state.dom_commits, commits);
    page.state.node_limit = limit;
    run(&mut doc,&mut forms,&mut page,"if(host.firstChild!==first||list.length!==2||first.textContent!=='old')throw 'failed prefix';host.textContent='recovered';if(host.firstChild.data!=='recovered')throw 'fresh job';");
    let node = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("host"))
        .unwrap();
    doc.nodes[node].namespace = crate::html::Namespace::Svg;
    let children = doc.nodes[node].children.clone();
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    assert!(bridge.set_text_content(node, "foreign".into()).is_err());
    assert_eq!(bridge.doc.nodes[node].children, children);
}
