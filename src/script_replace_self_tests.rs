//! Same-node replacement must retain identity while performing real effects.
use super::*;
fn fixture() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<html><body><div id=p><span id=before></span><b id=child>abcd</b><i id=after></i></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
#[test]
fn replacement_self_main_keeps_identity_but_queues_remove_insert_and_updates_live_ranges() {
    let (mut doc, mut forms, mut page) = fixture();
    let outcome=page.console_eval(&mut doc,&mut forms,r#"
var p=document.getElementById('p'),child=document.getElementById('child'),before=document.getElementById('before'),after=document.getElementById('after'),list=p.childNodes,range=document.createRange();range.setStart(child.firstChild,1);range.setEnd(child.firstChild,3);var observer=new MutationObserver(function(){});observer.observe(p,{childList:true});
if(p.replaceChild(child,child)!==child||p.childNodes!==list||list.length!==3||list[0]!==before||list[1]!==child||list[2]!==after||child.parentNode!==p)throw 'self tree identity';var records=observer.takeRecords();if(records.length!==2||records[0].target!==p||records[0].removedNodes[0]!==child||records[0].addedNodes.length||records[0].previousSibling!==before||records[0].nextSibling!==after||records[1].addedNodes[0]!==child||records[1].removedNodes.length||records[1].previousSibling!==before||records[1].nextSibling!==after)throw 'self removal and insertion records';
if(range.startContainer!==p||range.endContainer!==p||range.startOffset!==1||range.endOffset!==1||!range.collapsed)throw 'real live range remove/insert effects';
"#);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn replacement_self_native_work_refusal_keeps_ranges_tree_focus_and_generation_then_recovers() {
    let (mut doc, mut forms, mut page) = fixture();
    assert_eq!(page.console_eval(&mut doc,&mut forms,"var p=document.getElementById('p'),child=document.getElementById('child'),range=document.createRange();range.setStart(child.firstChild,1);range.setEnd(child.firstChild,3);").errors,0,"{}",page.report());
    let parent = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("p"))
        .unwrap();
    let child = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("child"))
        .unwrap();
    doc.native_focus.set(Some(child));
    let focus = doc.native_focus.get();
    let generation = page.state.dom_commits;
    let before = page
        .state
        .ranges
        .iter()
        .map(|r| (r.start.node, r.start.offset, r.end.node, r.end.offset))
        .collect::<Vec<_>>();
    let links = doc.nodes[parent].children.clone();
    let work = page.state.host_work;
    page.state.host_work = 0;
    let failure = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    }
    .dom_call(
        node_handle(parent),
        "replaceChild",
        &[
            Value::Host(node_handle(child)),
            Value::Host(node_handle(child)),
        ],
    )
    .unwrap_err();
    assert!(failure.contains("work"), "{failure}");
    assert_eq!(doc.nodes[parent].children, links);
    assert_eq!(doc.native_focus.get(), focus);
    assert_eq!(page.state.dom_commits, generation);
    assert_eq!(
        page.state
            .ranges
            .iter()
            .map(|r| (r.start.node, r.start.offset, r.end.node, r.end.offset))
            .collect::<Vec<_>>(),
        before
    );
    page.state.host_work = work;
    assert_eq!(page.console_eval(&mut doc,&mut forms,"p.replaceChild(child,child);if(range.startContainer!==p||range.startOffset!==1||!range.collapsed)throw 'real recovery';").errors,0,"{}",page.report());
    assert_eq!(doc.native_focus.get(), None);
}
