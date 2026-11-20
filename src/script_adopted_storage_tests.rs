//! Original owned-record index admission, transfer and retained-alias checks.
use super::*;

fn page(markup: &str) -> (Document, Forms, Page, Vec<u32>, usize) {
    let mut doc = crate::html::parse_loaded_document(markup, &|| Ok(())).unwrap();
    doc.scripting_enabled = true;
    let templates = doc.nodes.iter().enumerate().filter_map(|(index, node)|
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "template").then_some(node_handle(index))).collect();
    let body = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "body")).unwrap();
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page, templates, body)
}

#[test]
fn browser_adopted_storage_append_joint_exact_backing_and_copy_refusal_keep_owner() {
    let (mut doc, mut forms, mut page, templates, _) = page("<html><body><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let Value::Host(fragment) = bridge.template_content(templates[0]).unwrap() else { panic!("content") };
    let Value::Host(child) = bridge.create_dom_node(NodeKind::Element { tag: "b".into(), id: None, href: None, attrs: Vec::new() }).unwrap() else { panic!("author node") };
    let before = bridge.state.retained;
    let bundle = size_of::<(u32, InertNode)>() + size_of::<u32>();
    bridge.state.retained_limit = before + bundle - 1;
    assert!(bridge.append_template_child(fragment, &[Value::Host(child)]).is_err());
    bridge.state.retained_limit += 1;
    bridge.state.copy_work = 0;
    assert!(bridge.append_template_child(fragment, &[Value::Host(child)]).is_err());
    assert_eq!(bridge.state.inert_documents.adopted.storage_bytes(), 0);
    assert_eq!(bridge.state.inert_documents.node(fragment).unwrap().children.capacity(), 0);
    assert_eq!(bridge.state.retained, before);
    bridge.state.copy_work = bundle;
    bridge.append_template_child(fragment, &[Value::Host(child)]).unwrap();
    assert_eq!(bridge.state.retained, before + bundle);
    assert_eq!(bridge.state.copy_work, 0);
    assert_eq!(bridge.inert_children(fragment).unwrap(), [child]);
    let owner = bridge.state.inert_documents.node(fragment).unwrap().owner;
    assert_eq!(bridge.state.inert_documents.node(child).unwrap().owner, owner);
}

#[test]
fn browser_adopted_storage_parked_source_and_owned_record_fit_as_one_bundle() {
    let (mut doc, mut forms, mut page, templates, _) = page("<html><body><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let Value::Host(fragment) = bridge.template_content(templates[0]).unwrap() else { panic!("content") };
    let before = bridge.state.retained;
    let nodes = bridge.doc.nodes.len();
    let backing = bridge.state.inert_documents.backing_bytes();
    assert!(bridge.state.inert_documents.entries.capacity() > bridge.state.inert_documents.entries.len());
    // Existing entry backing has one spare slot. New storage owns one source
    // Rc<Node>, its tag and root link, a future main Node, and both new slots.
    let bundle = 2 * size_of::<Node>() + 2 * size_of::<usize>() + 1 + size_of::<u32>()+2+size_of::<Option<usize>>()
        + size_of::<GeneratedNode>() + size_of::<(u32, InertNode)>();
    bridge.state.retained_limit = before + bundle - 1;
    assert!(bridge.set_template_html(templates[0], "<i></i>").is_err());
    assert_eq!(bridge.state.inert_documents.backing_bytes(), backing);
    assert_eq!(bridge.doc.nodes.len(), nodes);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.set_template_html(templates[0], "<i></i>").unwrap();
    assert_eq!(bridge.state.retained, before + bundle);
    assert_eq!(bridge.doc.nodes.len(), nodes + 1);
    let saved = bridge.inert_children(fragment).unwrap()[0];
    assert_eq!(saved, node_handle(nodes));
    assert!(bridge.parked_template_source(saved).is_some());
    assert_eq!(bridge.state.inert_documents.adopted.storage_bytes(), size_of::<(u32, InertNode)>());
}

#[test]
fn browser_adopted_storage_large_promotion_refunds_dropped_links_and_keeps_paid_backing() {
    let (mut doc, mut forms, mut page, templates, body) = page("<html><body><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    bridge.set_template_html(templates[0], &"<div><b></b></div>".repeat(6000)).unwrap();
    let Value::Host(fragment) = bridge.template_content(templates[0]).unwrap() else { panic!("content") };
    let roots = bridge.inert_children(fragment).unwrap().to_vec();
    assert_eq!(roots.len(), 6000);
    let before = bridge.state.retained;
    let backing = bridge.state.inert_documents.adopted.storage_bytes();
    assert_eq!(bridge.state.inert_documents.adopted.len(), 12_000);
    // Each real copied pair owns 'div' + 'b' and one usize link; dropping the
    // inert div's u32 link refunds its original allocation. Sources stay saved.
    let destination = (bridge.doc.nodes[body].children.len() + roots.len())
        .saturating_sub(bridge.doc.nodes[body].children.capacity()) * size_of::<usize>();
    let growth = 6000 * (4 + size_of::<usize>() - size_of::<u32>()) + destination;
    bridge.state.retained_limit = before + growth - 1;
    assert!(bridge.splice_checked_inert_fragment(body, fragment, None).is_err());
    assert_eq!(bridge.inert_children(fragment).unwrap(), roots);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.state.host_work = bridge.state.limits.host_work;
    bridge.splice_checked_inert_fragment(body, fragment, None).unwrap();
    assert_eq!(bridge.state.retained, before + growth);
    assert!(bridge.state.inert_documents.adopted.is_empty());
    assert_eq!(bridge.state.inert_documents.adopted.storage_bytes(), backing);
    assert!(bridge.inert_children(fragment).unwrap().is_empty());
    for saved in [roots[0], roots[5999]] {
        let index = node_index(saved).unwrap();
        assert!(matches!(&bridge.doc.nodes[index].kind, NodeKind::Element { tag, .. } if tag == "div"));
        assert_eq!(bridge.state.tree.parents[index], Some(body));
        assert!(bridge.state.dom.connected[index]);
    }
}

#[test]
fn browser_adopted_storage_selective_bulk_promotion_preserves_other_saved_owners() {
    let (mut doc, mut forms, mut page, templates, body) = page("<html><body><template></template><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    for &template in &templates { bridge.set_template_html(template, "<div><b>one</b></div>").unwrap(); }
    let Value::Host(first) = bridge.template_content(templates[0]).unwrap() else { panic!("first content") };
    let Value::Host(second) = bridge.template_content(templates[1]).unwrap() else { panic!("second content") };
    let saved = bridge.inert_children(second).unwrap()[0];
    let before = bridge.state.retained;
    let storage = bridge.state.inert_documents.adopted.storage_bytes();
    bridge.splice_checked_inert_fragment(body, first, None).unwrap();
    assert_eq!(bridge.state.retained, before + 7 + 2 * size_of::<usize>() - 2 * size_of::<u32>());
    assert_eq!(bridge.state.inert_documents.adopted.len(), 3);
    assert_eq!(bridge.state.inert_documents.adopted.storage_bytes(), storage);
    assert_eq!(bridge.state.inert_documents.node(saved).unwrap().parent, Some(second));
    assert_eq!(bridge.inert_children(second).unwrap(), [saved]);
    assert!(bridge.parked_template_source(saved).is_some());
}
