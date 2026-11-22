//! Original native inert identity, joint backing and rollback acceptance.
use super::*;

fn page(markup: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse_loaded_document(markup, &|| Ok(())).unwrap();
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn backing(arena: &Arena) -> usize {
    arena.backing_bytes()
}

#[test]
fn inert_leaf_identity_joint_storage_copy_refusal_and_paid_native_slot() {
    let (mut doc, mut forms, mut page) = page("<html><body></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let Value::Host(owner) = bridge.create_inert_document(&[]).unwrap() else { panic!("owner") };
    let snapshot = |bridge: &Bridge<'_>| (
        bridge.state.retained, bridge.doc.nodes.len(), bridge.doc.nodes.capacity(),
        bridge.state.inert_documents.backing_bytes(), bridge.state.node_state_bytes(),
        bridge.state.node_sets_storage_bytes(), bridge.state.inert_documents.entries.len(),
        bridge.state.inert_documents.generated.len(), bridge.state.inert_documents.adopted.len(),
    );
    let before = snapshot(&bridge);
    let limit = bridge.state.retained_limit;
    bridge.state.retained_limit = bridge.state.retained;
    assert!(bridge.append_inert_text(owner, None, "payload".into(), 0).is_err());
    assert_eq!(snapshot(&bridge), before);
    bridge.state.retained_limit = limit;
    bridge.state.copy_work = 0;
    assert!(bridge.append_inert_text(owner, None, "payload".into(), 0).is_err());
    assert_eq!(snapshot(&bridge), before);
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    let text = String::from("payload");
    let payload = text.capacity() + size_of::<Node>() + 2 * size_of::<usize>();
    let handle = bridge.append_inert_text(owner, None, text, 0).unwrap();
    let after = snapshot(&bridge);
    assert_eq!(node_index(handle), Some(before.1));
    assert_eq!(after.1, before.1 + 1);
    assert_eq!(after.0 - before.0, payload + (after.2-before.2)*size_of::<Node>()
        + after.3-before.3 + after.4-before.4 + after.5-before.5);
    assert_eq!(bridge.state.inert_documents.node(handle).unwrap().owner, owner);
    assert_eq!(bridge.inert_text_value(handle), Some("payload"));
    assert!(!bridge.state.dom.connected[before.1]);
}

#[test]
fn browser_inert_storage_seventy_thousand_parsers_and_leaf_identities_keep_owners() {
    for parsers in [true, false] {
        let (mut doc, mut forms, mut page) = page("<html><body></body></html>");
        let before = page.state.retained;
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let owner = if parsers { None } else {
            let Value::Host(owner) = bridge.create_inert_document(&[]).unwrap() else { panic!("inert document") };
            Some(owner)
        };
        let initial = bridge.state.inert_documents.entries.len();
        let mut first = None;
        let mut last = 0;
        let mut identities = std::collections::HashSet::new();
        let mut native_previous = None;
        let mut inert_previous = None;
        for _ in 0..70_000 {
            let Value::Host(handle) = (if let Some(owner) = owner {
                bridge.inert_create_node(owner, "createTextNode", &[Value::String("x".into())])
            } else { bridge.create_dom_parser() }).unwrap() else { panic!("native handle") };
            first.get_or_insert(handle);
            last = handle;
            assert!(identities.insert(handle), "new leaf reused an identity");
            let previous = if handle < FIRST_ENTRY { &mut native_previous } else { &mut inert_previous };
            if let Some(old) = *previous { assert_eq!(handle, old + 1); }
            *previous = Some(handle);
        }
        assert_eq!(identities.len(), 70_000);
        if parsers { assert_eq!(last - first.unwrap(), 69_999); }
        else { assert!(native_previous.is_some(), "available native slots were not reserved"); }
        assert!(last < ENTRY_END);
        assert_eq!(bridge.state.inert_documents.entries.len(), initial + 70_000);
        assert!(bridge.state.retained >= before + backing(&bridge.state.inert_documents));
        assert!(bridge.state.retained <= bridge.state.retained_limit);
        if let Some(owner) = owner {
            for handle in [first.unwrap(), last] {
                assert_eq!(bridge.state.inert_documents.node(handle).unwrap().owner, owner);
                assert!(matches!(bridge.inert_get(handle, "data").unwrap(), Value::String(text) if text == "x"));
            }
        } else {
            assert!(matches!(bridge.state.inert_documents.entry(last), Some(Entry::Parser)));
        }
    }
}

#[test]
fn browser_inert_storage_joint_exact_backing_and_copy_refusal_publish_neither_vector() {
    let (mut doc, mut forms, mut page) = page("<html><body></body></html>");
    let before = page.state.retained;
    let payload = size_of::<Node>() + 2 * size_of::<usize>() + 1;
    let bundle = size_of::<Entry>() + size_of::<GeneratedNode>() + payload;
    page.state.retained_limit = before + bundle - 1;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.reserve_inert_entries(1, payload, 1).is_err());
    assert_eq!(backing(&bridge.state.inert_documents), 0);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    bridge.state.copy_work = bundle - 1;
    assert!(bridge.reserve_inert_entries(1, payload, 1).is_err());
    assert_eq!(backing(&bridge.state.inert_documents), 0);
    assert_eq!(bridge.state.retained, before);
    bridge.state.copy_work = bundle;
    assert_eq!(bridge.reserve_inert_entries(1, payload, 1).unwrap(), FIRST_ENTRY);
    assert_eq!(bridge.state.retained, before + bundle);
    assert_eq!(backing(&bridge.state.inert_documents), bundle - payload);
    bridge.state.inert_documents.entries.push(Entry::Parser);
    bridge.state.inert_documents.generated.push(GeneratedNode {
        node: Rc::new(Node { kind: NodeKind::Text("x".into()), children: Vec::new(), namespace: crate::html::Namespace::None }),
        unsafe_nonce: false,
    });
    assert_eq!(bridge.state.copy_work, 0);
}

#[test]
fn browser_inert_storage_parsed_sources_cross_old_count_with_real_order_and_owner() {
    let (mut doc, mut forms, mut page) = page("<html><body></body></html>");
    let markup = "<i></i>".repeat(70_000);
    let before = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let Value::Host(owner) = bridge.parse_inert_document(&[Value::String(markup)]).unwrap() else { panic!("real parsed document") };
    let Kind::Document(document) = &bridge.state.inert_documents.node(owner).unwrap().kind else { panic!("document record") };
    let body = document.body;
    let children = bridge.inert_children(body).unwrap();
    assert_eq!(children.len(), 70_000);
    for handle in [children[0], children[69_999]] {
        assert_eq!(bridge.state.inert_documents.node(handle).unwrap().owner, owner);
        assert_eq!(bridge.state.inert_documents.node(handle).unwrap().parent, Some(body));
        assert!(matches!(bridge.inert_import_source(handle), Some(ImportSource::Parsed { node: Node { kind: NodeKind::Element { tag, .. }, .. }, .. }) if tag == "i"));
    }
    assert!(bridge.state.inert_documents.generated.len() > 65_536);
    assert!(bridge.state.retained >= before + backing(&bridge.state.inert_documents));
    assert!(bridge.state.retained <= bridge.state.retained_limit);
}

#[test]
fn browser_inert_storage_failed_template_write_keeps_paid_backing_and_recovers() {
    let (mut doc, mut forms, mut page) = page("<html><body><template></template></body></html>");
    let template = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "template")).unwrap();
    let before = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    // The first three content identities and exact map backing fit; the replacement
    // needs two further identities and must fail after content materialization.
    bridge.state.retained_limit = before + 3 * size_of::<Entry>() + 2 * size_of::<(u32, u32)>();
    assert!(bridge.set_template_html(node_handle(template), "<p>x</p>").unwrap_err().contains("template replacement"));
    let arena = &bridge.state.inert_documents;
    assert!(arena.entries.is_empty());
    assert!(!arena.template_contents.contains_key(&node_handle(template)));
    let paid = backing(arena);
    assert!(paid > 0);
    assert_eq!(bridge.state.retained, before + paid);
    let Value::Host(fragment) = bridge.template_content(node_handle(template)).unwrap() else { panic!("real template content") };
    assert!(bridge.inert_children(fragment).unwrap().is_empty());
    assert_eq!(backing(&bridge.state.inert_documents), paid);
    assert_eq!(bridge.state.retained, before + paid);
}

#[test]
fn browser_inert_storage_paid_spare_slots_need_no_new_retained_allowance() {
    let (mut doc, mut forms, mut page) = page("<html><body></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    bridge.create_dom_parser().unwrap();
    let before = bridge.state.retained;
    assert!(bridge.state.inert_documents.entries.capacity() > 1);
    bridge.state.retained_limit = before;
    bridge.create_dom_parser().unwrap();
    assert_eq!(bridge.state.retained, before);
    assert_eq!(bridge.state.inert_documents.entries.len(), 2);
}

#[test]
fn browser_inert_storage_document_fixed_names_and_links_admit_exact_owned_payload() {
    let (mut doc, mut forms, mut page) = page("<html><body></body></html>");
    let before = page.state.retained;
    // Six records, the actual about:blank/html/head/body text, and two pairs
    // of child handles. There is no unrelated fixed 64-byte payload surcharge.
    let bundle = 6 * size_of::<Entry>() + 11 + 4 + 4 + 4 + 4 * size_of::<u32>();
    page.state.retained_limit = before + bundle - 1;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.create_inert_document(&[]).is_err());
    assert!(bridge.state.inert_documents.entries.is_empty());
    assert_eq!(backing(&bridge.state.inert_documents), 0);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    let Value::Host(owner) = bridge.create_inert_document(&[]).unwrap() else { panic!("real document") };
    assert_eq!(bridge.state.retained, before + bundle);
    assert_eq!(bridge.state.inert_documents.entries.len(), 6);
    assert_eq!(bridge.state.inert_documents.node(owner).unwrap().owner, owner);
}

fn template_links_fixture() -> (Document, Forms, Page, u32, Vec<u32>) {
    let (mut doc, mut forms, mut page) = page("<html><body><template></template></body></html>");
    let template = doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "template")).unwrap();
    // Select the supported inert-handle path instead of parking main-arena
    // placeholders, so this witness isolates child/source/entry backing.
    page.state.node_limit = doc.nodes.len();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    bridge.set_template_html(node_handle(template), &"<b></b>".repeat(32)).unwrap();
    let Value::Host(fragment) = bridge.template_content(node_handle(template)).unwrap() else { panic!("template content") };
    let children = bridge.inert_children(fragment).unwrap().to_vec();
    (doc, forms, page, fragment, children)
}

#[test]
fn browser_template_links_empty_replacement_refunds_only_root_backing_and_keeps_saved_children() {
    let (mut doc, mut forms, mut page, fragment, children) = template_links_fixture();
    let before = page.state.retained;
    let released = page.state.inert_documents.node(fragment).unwrap().children.capacity() * size_of::<u32>();
    let counts = (page.state.inert_documents.entries.len(), page.state.inert_documents.generated.len());
    page.state.retained_limit = before - released - 1;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let parsed = || crate::html::parse_fragment("", "template", true).unwrap();
    assert!(bridge.replace_template_children(fragment, parsed()).is_err());
    assert_eq!(bridge.inert_children(fragment).unwrap(), children);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    bridge.replace_template_children(fragment, parsed()).unwrap();
    assert_eq!(bridge.state.retained, before - released);
    for handle in children {
        assert_eq!(bridge.state.inert_documents.node(handle).unwrap().parent, None);
        assert!(matches!(bridge.inert_import_source(handle), Some(ImportSource::Parsed { node: Node { kind: NodeKind::Element { tag, .. }, .. }, .. }) if tag == "b"));
    }
    for _ in 0..100 { bridge.replace_template_children(fragment, parsed()).unwrap(); }
    assert_eq!(bridge.state.retained, before - released);
    assert_eq!((bridge.state.inert_documents.entries.len(), bridge.state.inert_documents.generated.len()), counts);
}

#[test]
fn browser_template_links_released_backing_funds_joint_exact_entry_and_source_growth() {
    let (mut doc, mut forms, mut page, fragment, children) = template_links_fixture();
    let arena = &page.state.inert_documents;
    assert_eq!(arena.entries.len(), arena.entries.capacity());
    assert_eq!(arena.generated.len(), arena.generated.capacity());
    let released = arena.node(fragment).unwrap().children.capacity() * size_of::<u32>();
    let before = page.state.retained;
    // New Rc<Node>, tag text, one root link, and one slot in each registry.
    let bundle = size_of::<Node>() + 2 * size_of::<usize>() + 1 + size_of::<u32>()
        + size_of::<Entry>() + size_of::<GeneratedNode>();
    page.state.retained_limit = before - released + bundle - 1;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let parsed = || crate::html::parse_fragment("<i></i>", "template", true).unwrap();
    assert!(bridge.replace_template_children(fragment, parsed()).is_err());
    assert_eq!(bridge.inert_children(fragment).unwrap(), children);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    bridge.replace_template_children(fragment, parsed()).unwrap();
    assert_eq!(bridge.state.retained, before - released + bundle);
    let fresh = bridge.inert_children(fragment).unwrap();
    assert_eq!(fresh.len(), 1);
    assert!(matches!(bridge.inert_import_source(fresh[0]), Some(ImportSource::Parsed { node: Node { kind: NodeKind::Element { tag, .. }, .. }, .. }) if tag == "i"));
    for child in children { assert_eq!(bridge.state.inert_documents.node(child).unwrap().parent, None); }
}

#[test]
fn browser_template_links_detach_work_and_copy_refusals_leave_both_old_owners_unchanged() {
    let (mut doc, mut forms, mut page, fragment, children) = template_links_fixture();
    let retained = page.state.retained;
    let storage = backing(&page.state.inert_documents);
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let parsed = || crate::html::parse_fragment("<i></i>", "template", true).unwrap();
    for copy in [false, true] {
        bridge.state.host_work = if copy { bridge.state.limits.host_work } else { children.len() - 1 };
        bridge.state.copy_work = if copy { 0 } else { bridge.state.limits.copy_bytes };
        assert!(bridge.replace_template_children(fragment, parsed()).is_err());
        assert_eq!(bridge.inert_children(fragment).unwrap(), children);
        for &child in &children { assert_eq!(bridge.state.inert_documents.node(child).unwrap().parent, Some(fragment)); }
        assert_eq!(bridge.state.retained, retained);
        assert_eq!(backing(&bridge.state.inert_documents), storage);
    }
    bridge.state.host_work = bridge.state.limits.host_work;
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    bridge.replace_template_children(fragment, parsed()).unwrap();
    assert_eq!(bridge.inert_children(fragment).unwrap().len(), 1);
}
