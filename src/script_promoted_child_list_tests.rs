//! Original bulk SameObject transfer, exact backing and unrelated-owner cases.
use super::*;
fn host(value: Result<Value, String>) -> u32 {
    let Value::Host(handle) = value.unwrap() else { panic!("genuine native handle") };
    handle
}
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
fn browser_promoted_lists_twelve_thousand_saved_identities_fit_linear_logarithmic_work() {
    let (mut doc, mut forms, mut page, templates, body) = page("<html><body><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let body_list = host(bridge.create_child_list(node_handle(body)));
    bridge.set_template_html(templates[0], &"<div><b></b></div>".repeat(6000)).unwrap();
    let fragment = host(bridge.template_content(templates[0]));
    let fragment_list = host(bridge.create_inert_child_list(fragment));
    let roots = bridge.inert_children(fragment).unwrap().to_vec();
    let mut saved = Vec::new();
    for &root in &roots {
        let child = bridge.inert_children(root).unwrap()[0];
        for node in [root, child] { saved.push((node, host(bridge.create_inert_child_list(node)))); }
    }
    assert_eq!(saved.len(), 12_000);
    let before = bridge.state.retained;
    let source_backing = bridge.state.dom.inert_child_lists.storage_bytes();
    let destination_backing = bridge.state.dom.child_lists.storage_bytes();
    let records = bridge.state.node_lists.len();
    // These are explicit test allowances for this one operation. The previous
    // per-record shifts need over 100 million work units on this same input.
    bridge.state.host_work = 2_500_000;
    bridge.state.copy_work = 32 * 1024 * 1024;
    bridge.splice_checked_inert_fragment(body, fragment, None).unwrap();
    assert!(bridge.state.host_work > 0);
    assert!(bridge.state.copy_work > 0);
    assert_eq!(bridge.state.node_lists.len(), records);
    assert_eq!(bridge.state.dom.child_lists.len(), 12_001);
    assert_eq!(bridge.state.dom.inert_child_lists.len(), 1);
    assert_eq!(bridge.state.dom.inert_child_lists.storage_bytes(), source_backing);
    let added = bridge.state.dom.child_lists.storage_bytes() - destination_backing;
    assert_eq!(bridge.state.retained, before + added + 6000 * (4 + size_of::<usize>() - size_of::<u32>()));
    for index in [0, 1, 11_998, 11_999] {
        let (node, list) = saved[index];
        assert_eq!(host(bridge.create_child_list(node)), list);
        assert!(matches!(bridge.state.node_lists.get(queries::node_list_index(list).unwrap()), Some(queries::NodeList::Children(owner)) if *owner==node_index(node).unwrap()));
    }
    assert_eq!(host(bridge.create_child_list(node_handle(body))), body_list);
    assert_eq!(host(bridge.create_inert_child_list(fragment)), fragment_list);
    assert!(bridge.inert_children(fragment).unwrap().is_empty());
}

#[test]
fn browser_promoted_lists_exact_destination_backing_refusal_and_recovery_preserve_aliases() {
    let (mut doc, mut forms, mut page, templates, body) = page("<html><body><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    bridge.set_template_html(templates[0], "<b></b>").unwrap();
    let fragment = host(bridge.template_content(templates[0]));
    let child = bridge.inert_children(fragment).unwrap()[0];
    let list = host(bridge.create_inert_child_list(child));
    let before = bridge.state.retained;
    let source = bridge.state.dom.inert_child_lists.storage_bytes();
    let bundle = 1 + size_of::<(usize, u32)>(); // copied tag plus one destination pair
    bridge.state.retained_limit = before + bundle - 1;
    assert!(bridge.splice_checked_inert_fragment(body, fragment, None).is_err());
    bridge.state.retained_limit += 1;
    bridge.state.copy_work = 0;
    assert!(bridge.splice_checked_inert_fragment(body, fragment, None).is_err());
    assert_eq!(bridge.state.retained, before);
    assert_eq!(bridge.state.dom.child_lists.storage_bytes(), 0);
    assert_eq!(bridge.state.dom.inert_child_lists.storage_bytes(), source);
    assert_eq!(host(bridge.create_inert_child_list(child)), list);
    assert!(matches!(bridge.state.node_lists.get(queries::node_list_index(list).unwrap()), Some(queries::NodeList::InertChildren(owner)) if *owner==child));
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.splice_checked_inert_fragment(body, fragment, None).unwrap();
    assert_eq!(bridge.state.retained, before + bundle);
    assert_eq!(host(bridge.create_child_list(child)), list);
    assert_eq!(bridge.state.dom.child_lists.storage_bytes(), size_of::<(usize, u32)>());
    assert!(bridge.state.dom.inert_child_lists.is_empty());
    assert_eq!(bridge.state.dom.inert_child_lists.storage_bytes(), source);
}

#[test]
fn browser_promoted_lists_selective_transfer_preserves_other_inert_and_main_cached_owners() {
    let (mut doc, mut forms, mut page, templates, body) = page("<html><body><template></template><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let body_list = host(bridge.create_child_list(node_handle(body)));
    for &template in &templates { bridge.set_template_html(template, "<div><b></b></div>").unwrap(); }
    let first = host(bridge.template_content(templates[0]));
    let second = host(bridge.template_content(templates[1]));
    let a = bridge.inert_children(first).unwrap()[0];
    let b = bridge.inert_children(second).unwrap()[0];
    let a_list = host(bridge.create_inert_child_list(a));
    let b_list = host(bridge.create_inert_child_list(b));
    let source = bridge.state.dom.inert_child_lists.storage_bytes();
    bridge.splice_checked_inert_fragment(body, first, None).unwrap();
    assert_eq!(host(bridge.create_child_list(a)), a_list);
    assert_eq!(host(bridge.create_inert_child_list(b)), b_list);
    assert_eq!(host(bridge.create_child_list(node_handle(body))), body_list);
    assert!(matches!(bridge.state.node_lists.get(queries::node_list_index(b_list).unwrap()), Some(queries::NodeList::InertChildren(owner)) if *owner==b));
    assert_eq!(bridge.state.inert_documents.node(b).unwrap().parent, Some(second));
    assert_eq!(bridge.state.dom.inert_child_lists.len(), 1);
    assert_eq!(bridge.state.dom.child_lists.len(), 2);
    assert_eq!(bridge.state.dom.inert_child_lists.storage_bytes(), source);
}
