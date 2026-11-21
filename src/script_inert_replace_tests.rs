//! Original inert replacement owner, topology, observer and refusal checks.
use super::*;
fn page() -> (Document, Forms, Page) {
    let mut doc=crate::html::parse("<html><body><template id=seed><div><b>old</b></div></template><div id=mount></div></body></html>");
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    let location = crate::navigation::Location {
        url: Some(
            crate::url::parse("https://example.test/inert-replace")
                .unwrap()
                .0,
        ),
        path: None,
        fragment: String::new(),
        root: None,
    };
    page.set_location(&location);
    page.set_resource_base(&location);
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn execute(d: &mut Document, f: &mut Forms, p: &mut Page, s: &str) {
    assert_eq!(p.console_eval(d, f, s).errors, 0, "{}", p.report());
}
#[test]
fn inert_replace_normal_borrowed_sibling_orders_fragments_and_empty_donors_keep_live_identity() {
    let (mut d, mut f, mut p) = page();
    execute(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=new DOMParser().parseFromString('<html><body><!--marker--></body></html>','text/html'),parent=owner.createElement('div'),a=owner.createElement('a'),b=owner.createElement('b'),c=owner.createElement('i'),x=owner.createElement('em');parent.appendChild(a);parent.appendChild(b);parent.appendChild(c);var list=parent.childNodes,elements=parent.children,replace=Node.prototype.replaceChild;
if(parent.replaceChild(x,b)!==b||list.length!==3||list[0]!==a||list[1]!==x||list[2]!==c||b.parentNode!==null||x.parentNode!==parent)throw 'real normal replacement';
if(replace.call(parent,c,a)!==a||list.length!==2||list[0]!==c||list[1]!==x)throw 'incoming following old';if(replace.call(parent,c,x)!==x||list.length!==1||list[0]!==c)throw 'incoming preceding old';if(parent.replaceChild(c,c)!==c||list.length!==1||list[0]!==c)throw 'self identity';
var donor=owner.createDocumentFragment(),text=owner.createTextNode('left'),comment=owner.body.firstChild,em=owner.createElement('em');donor.appendChild(text);donor.appendChild(comment);donor.appendChild(em);var donorList=donor.childNodes;
if(replace.call(parent,donor,c)!==c||donorList.length!==0||list.length!==3||list[0]!==text||list[1]!==comment||list[2]!==em||elements.length!==1||elements[0]!==em||parent.childNodes!==list||parent.children!==elements||c.parentNode!==null||text.ownerDocument!==owner)throw 'real mixed fragment splice';
var empty=owner.createDocumentFragment();if(parent.replaceChild(empty,em)!==em||list.length!==2||elements.length!==0||em.parentNode!==null||comment.previousSibling!==text||text.nextSibling!==comment)throw 'empty fragment removes old';
"#,
    );
}
#[test]
fn inert_replace_parked_main_handles_and_descendant_sources_use_actual_owner_routes() {
    let (mut d, mut f, mut p) = page();
    execute(
        &mut d,
        &mut f,
        &mut p,
        r#"
var template=document.createElement('template');template.innerHTML='<div><b>old</b></div>';var parked=template.content.firstElementChild,owner=parked.ownerDocument,old=parked.firstChild,next=document.createElement('em'),attr;
next.setAttribute('aria-label','native');attr=next.getAttributeNode('aria-label');template.content.appendChild(next);var wrapper={node:parked,replace:function(n,o){return Node.prototype.replaceChild.call(this.node,n,o);}};
if(wrapper.replace(next,old)!==old||parked.firstChild!==next||old.parentNode!==null||next.parentNode!==parked||next.ownerDocument!==owner||attr.ownerElement!==next)throw 'actual parked HTMLDivElement owner';
var outer=owner.createElement('div'),removed=owner.createElement('section'),inside=owner.createElement('span');outer.appendChild(removed);removed.appendChild(inside);if(outer.replaceChild(inside,removed)!==removed||outer.firstChild!==inside||inside.parentNode!==outer||removed.parentNode!==null||removed.childNodes.length!==0||inside.getRootNode()!==outer||removed.getRootNode()!==removed)throw 'descendant removed from old subtree';
document.getElementById('mount').appendChild(template.content);if(!next.isConnected||next.ownerDocument!==document||attr.ownerDocument!==document||next.getAttributeNode('aria-label')!==attr||attr.value!=='native')throw 'promotion preserves replacement and Attr identities';
"#,
    );
}
#[test]
fn inert_replace_argument_brand_error_order_structural_and_cross_owner_refusal_are_atomic() {
    let (mut d, mut f, mut p) = page();
    execute(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=document.implementation.createHTMLDocument('owner'),parent=owner.createElement('div'),old=owner.createElement('b'),next=owner.createElement('em'),other=owner.createElement('span'),calls=0;parent.appendChild(old);
function fails(fn,name,code){var caught=false;try{fn();}catch(e){caught=e.name===name&&(name==='TypeError'?e instanceof TypeError:e instanceof DOMException&&e.code===code);}if(!caught)throw 'typed replacement error';}
fails(function(){parent.replaceChild(parent,null);},'TypeError');fails(function(){parent.replaceChild(parent,other);},'HierarchyRequestError',3);fails(function(){parent.replaceChild(next,other);},'NotFoundError',8);
fails(function(){parent.replaceChild({toString:function(){calls++;return 'fake';}},old);},'TypeError');fails(function(){parent.replaceChild(next);},'TypeError');
fails(function(){Node.prototype.replaceChild.call(owner.createTextNode('leaf'),next,old);},'HierarchyRequestError',3);fails(function(){parent.replaceChild(owner,old);},'HierarchyRequestError',3);
var foreign=document.implementation.createHTMLDocument('foreign').createElement('div');fails(function(){parent.replaceChild(foreign,old);},'NotSupportedError',9);fails(function(){owner.replaceChild(next,owner.documentElement);},'NotSupportedError',9);
if(calls||parent.childNodes.length!==1||parent.firstChild!==old||old.parentNode!==parent||next.parentNode!==null||foreign.parentNode!==null||owner.documentElement.localName!=='html')throw 'refusal changed topology';
"#,
    );
}
#[test]
fn inert_replace_observer_donor_combined_self_and_transient_subtree_records_are_real() {
    let (mut d, mut f, mut p) = page();
    execute(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=new DOMParser().parseFromString('<html><body><!--comment--></body></html>','text/html'),parent=owner.createElement('div'),before=owner.createElement('i'),old=owner.createElement('b'),after=owner.createElement('em'),donor=owner.createDocumentFragment(),text=owner.createTextNode('text'),comment=owner.body.firstChild;parent.appendChild(before);parent.appendChild(old);parent.appendChild(after);donor.appendChild(text);donor.appendChild(comment);
var observer=new MutationObserver(function(){});observer.observe(parent,{childList:true,subtree:true,attributes:true});observer.observe(donor,{childList:true});parent.replaceChild(donor,old);old.setAttribute('data-removed','still observed');var records=observer.takeRecords();
if(records.length!==3||records[0].target!==donor||records[0].removedNodes.length!==2||records[0].removedNodes[0]!==text||records[0].removedNodes[1]!==comment||records[1].target!==parent||records[1].addedNodes.length!==2||records[1].removedNodes[0]!==old||records[1].previousSibling!==before||records[1].nextSibling!==after||records[2].target!==old||records[2].attributeName!=='data-removed')throw 'real donor combined and transient records';
observer.disconnect();var single=owner.createElement('div'),same=owner.createElement('b');single.appendChild(same);observer.observe(single,{childList:true});single.replaceChild(same,same);records=observer.takeRecords();if(records.length!==2||records[0].removedNodes[0]!==same||records[0].addedNodes.length!==0||records[1].addedNodes[0]!==same||records[1].removedNodes.length!==0||single.firstChild!==same)throw 'adopt/remove/insert self records';
"#,
    );
}
#[test]
fn inert_replace_growth_and_copy_refusal_leave_both_trees_and_storage_unchanged() {
    let (mut doc, mut forms, mut page) = page();
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    let Value::Host(owner) = bridge.create_inert_document(&[]).unwrap() else {
        panic!()
    };
    let Value::Host(parent) = bridge
        .inert_create_node(owner, "createDocumentFragment", &[])
        .unwrap()
    else {
        panic!()
    };
    let Value::Host(donor) = bridge
        .inert_create_node(owner, "createDocumentFragment", &[])
        .unwrap()
    else {
        panic!()
    };
    let Value::Host(old) = bridge
        .inert_create_node(owner, "createElement", &[Value::String("b".into())])
        .unwrap()
    else {
        panic!()
    };
    bridge
        .mutate_inert_children(parent, "appendChild", &[Value::Host(old)])
        .unwrap();
    while {
        let c = &bridge.state.inert_documents.node(parent).unwrap().children;
        c.len() < c.capacity()
    } {
        let Value::Host(node) = bridge
            .inert_create_node(owner, "createTextNode", &[Value::String("tail".into())])
            .unwrap()
        else {
            panic!()
        };
        bridge
            .mutate_inert_children(parent, "appendChild", &[Value::Host(node)])
            .unwrap();
    }
    for _ in 0..2 {
        let Value::Host(node) = bridge
            .inert_create_node(owner, "createTextNode", &[Value::String("donor".into())])
            .unwrap()
        else {
            panic!()
        };
        bridge
            .mutate_inert_children(donor, "appendChild", &[Value::Host(node)])
            .unwrap();
    }
    let before = bridge.inert_children(parent).unwrap().to_vec();
    let incoming = bridge.inert_children(donor).unwrap().to_vec();
    let retained = bridge.state.retained;
    let limit = bridge.state.retained_limit;
    let generation = bridge.state.dom_commits;
    bridge.state.retained_limit = retained;
    assert!(bridge
        .replace_inert_child(parent, &[Value::Host(donor), Value::Host(old)])
        .is_err());
    assert_eq!(bridge.inert_children(parent).unwrap(), before);
    assert_eq!(bridge.inert_children(donor).unwrap(), incoming);
    assert_eq!(bridge.state.retained, retained);
    assert_eq!(bridge.state.dom_commits, generation);
    bridge.state.retained_limit = limit;
    let copies = bridge.state.copy_work;
    bridge.state.copy_work = 0;
    assert!(bridge
        .replace_inert_child(parent, &[Value::Host(donor), Value::Host(old)])
        .is_err());
    assert_eq!(bridge.inert_children(parent).unwrap(), before);
    assert_eq!(bridge.inert_children(donor).unwrap(), incoming);
    assert_eq!(bridge.state.retained, retained);
    assert_eq!(bridge.state.dom_commits, generation);
    bridge.state.copy_work = copies;
    assert!(
        matches!(bridge.replace_inert_child(parent,&[Value::Host(donor),Value::Host(old)]).unwrap(),Value::Host(h) if h==old)
    );
    assert!(bridge.inert_children(donor).unwrap().is_empty());
    assert_eq!(bridge.state.inert_documents.node(old).unwrap().parent, None);
}

#[test]
fn inert_replace_loaded_promotion_failure_restores_main_slot_and_inert_owner_then_recovers() {
    let (mut doc, mut forms, mut page) = page();
    let body = (0..doc.nodes.len())
        .find(|&n| tag(&doc, n) == "body")
        .unwrap();
    let mut bridge = Bridge {
        doc: &mut doc,
        forms: &mut forms,
        state: &mut page.state,
    };
    let Value::Host(template) = bridge
        .create_dom_node(NodeKind::Element {
            tag: "template".into(),
            id: None,
            href: None,
            attrs: Vec::new(),
        })
        .unwrap()
    else {
        panic!()
    };
    let Value::Host(fragment) = bridge.template_content(template).unwrap() else {
        panic!()
    };
    let Value::Host(child) = bridge
        .create_dom_node(NodeKind::Element {
            tag: "div".into(),
            id: None,
            href: None,
            attrs: Vec::new(),
        })
        .unwrap()
    else {
        panic!()
    };
    let index = node_index(child).unwrap();
    bridge
        .attribute(index, "data-preserved", Some("loaded".into()))
        .unwrap();
    bridge
        .append_template_child(fragment, &[Value::Host(child)])
        .unwrap();
    let Value::Host(button) = bridge
        .create_dom_node(NodeKind::Element {
            tag: "button".into(),
            id: None,
            href: None,
            attrs: Vec::new(),
        })
        .unwrap()
    else {
        panic!()
    };
    let button = node_index(button).unwrap();
    let retained = bridge.state.retained;
    let generation = bridge.state.dom_commits;
    let failure = bridge
        .splice_checked_inert_fragment(button, fragment, None)
        .unwrap_err();
    assert!(failure.to_ascii_lowercase().contains("button"), "{failure}");
    assert_eq!(tag(bridge.doc, index), "div");
    assert_eq!(bridge.doc.attr(index, "data-preserved"), Some("loaded"));
    assert!(bridge.doc.nodes[button].children.is_empty());
    assert_eq!(bridge.inert_children(fragment).unwrap(), [child]);
    assert_eq!(
        bridge.state.inert_documents.node(child).unwrap().parent,
        Some(fragment)
    );
    assert_eq!(bridge.state.retained, retained);
    assert_eq!(bridge.state.dom_commits, generation);
    bridge
        .splice_checked_inert_fragment(body, fragment, None)
        .unwrap();
    assert_eq!(bridge.state.tree.parents[index], Some(body));
    assert!(bridge.inert_children(fragment).unwrap().is_empty());
    assert_eq!(bridge.doc.attr(index, "data-preserved"), Some("loaded"));
    assert!(!bridge.inert_adopted_node(child));
}

#[test]
fn inert_replace_saved_child_realm_method_revalidates_origin_and_keeps_child_write_guard() {
    let (mut d, mut f, mut p) = page();
    execute(
        &mut d,
        &mut f,
        &mut p,
        r#"
var owner=document.implementation.createHTMLDocument('owner'),parent=owner.createElement('div'),old=owner.createElement('b'),next=owner.createElement('em');parent.appendChild(old);var frame=document.createElement('iframe');frame.hidden=true;document.body.appendChild(frame);var w=frame.contentWindow,saved=w.Node.prototype.replaceChild;if(saved===Node.prototype.replaceChild||Object.getPrototypeOf(saved)!==w.Function.prototype||saved.call(parent,next,old)!==old||parent.firstChild!==next)throw 'genuine borrowed owner method';
var caught=false;try{Node.prototype.replaceChild.call(w.document.body,next,w.document.documentElement);}catch(e){caught=e instanceof TypeError;}if(!caught||parent.firstChild!==next||next.parentNode!==parent)throw 'child mutation guard';var pending=owner.createElement('span');
"#,
    );
    p.set_location(&crate::navigation::Location {
        url: Some(crate::url::parse("https://other.test/").unwrap().0),
        path: None,
        fragment: String::new(),
        root: None,
    });
    assert_eq!(
        p.console_eval(&mut d, &mut f, "saved.call(parent,pending,next);")
            .errors,
        1,
        "{}",
        p.report()
    );
    assert!(p.report().contains("same-origin"), "{}", p.report());
    execute(&mut d,&mut f,&mut p,"if(parent.firstChild!==next||next.parentNode!==parent||pending.parentNode!==null)throw 'denied owner call mutated tree';");
}
