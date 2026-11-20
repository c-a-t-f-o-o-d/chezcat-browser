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
    arena.entries.capacity() * size_of::<Entry>()
        + arena.generated.capacity() * size_of::<GeneratedNode>()
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
        for _ in 0..70_000 {
            let Value::Host(handle) = (if let Some(owner) = owner {
                bridge.inert_create_node(owner, "createTextNode", &[Value::String("x".into())])
            } else { bridge.create_dom_parser() }).unwrap() else { panic!("native handle") };
            first.get_or_insert(handle);
            last = handle;
        }
        assert_eq!(last - first.unwrap(), 69_999);
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
    // The first three content identities and map payload fit; the replacement
    // needs two further identities and must fail after content materialization.
    bridge.state.retained_limit = before + 4 * size_of::<Entry>() + 2 * size_of::<(u32, u32)>();
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
    assert!(bridge.state.retained > before + paid);
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
