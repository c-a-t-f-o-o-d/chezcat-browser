//! Original template index/backing admission and stable-owner witnesses.
use super::*;

fn page(markup: &str) -> (Document, Forms, Page, Vec<u32>) {
    let mut doc = crate::html::parse_loaded_document(markup, &|| Ok(())).unwrap();
    doc.scripting_enabled = true;
    let templates = doc.nodes.iter().enumerate().filter_map(|(index, node)|
        matches!(&node.kind, NodeKind::Element { tag, .. } if tag == "template").then_some(node_handle(index))).collect();
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page, templates)
}

#[test]
fn browser_template_indexes_joint_exact_maps_and_records_refuse_before_any_publication() {
    let (mut doc, mut forms, mut page, templates) = page("<html><body><template></template></body></html>");
    let before = page.state.retained;
    let bundle = 3 * size_of::<Entry>() + 2 * size_of::<(u32, u32)>();
    page.state.retained_limit = before + bundle - 1;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.template_content(templates[0]).is_err());
    assert_eq!(bridge.state.inert_documents.backing_bytes(), 0);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    bridge.state.copy_work = 0;
    assert!(bridge.template_content(templates[0]).is_err());
    assert_eq!(bridge.state.inert_documents.backing_bytes(), 0);
    assert!(bridge.state.inert_documents.template_contents.is_empty());
    bridge.state.copy_work = bundle;
    let Value::Host(fragment) = bridge.template_content(templates[0]).unwrap() else { panic!("real content") };
    assert_eq!(bridge.state.retained, before + bundle);
    assert_eq!(bridge.state.inert_documents.backing_bytes(), bundle);
    assert_eq!(bridge.state.copy_work, 0);
    assert!(bridge.inert_children(fragment).unwrap().is_empty());
    let Value::Host(saved) = bridge.template_content(templates[0]).unwrap() else { panic!("saved content") };
    assert_eq!(saved, fragment);
    assert_eq!(bridge.state.retained, before + bundle);
}

#[test]
fn browser_template_indexes_seventy_thousand_real_contents_keep_same_objects_and_owner() {
    let markup = format!("<html><body>{}</body></html>", "<template></template>".repeat(70_000));
    let (mut doc, mut forms, mut page, templates) = page(&markup);
    let before = page.state.retained;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let mut first = None;
    let mut last = 0;
    for &template in &templates {
        let Value::Host(content) = bridge.template_content(template).unwrap() else { panic!("real content") };
        first.get_or_insert(content);
        last = content;
    }
    let arena = &bridge.state.inert_documents;
    assert_eq!(arena.template_documents.len(), 1);
    assert_eq!(arena.template_contents.len(), 70_000);
    assert_eq!(arena.entries.len(), 70_002);
    assert_eq!(bridge.state.retained - before, arena.backing_bytes());
    assert_eq!(arena.node(first.unwrap()).unwrap().owner, arena.node(last).unwrap().owner);
    for (template, saved) in [(templates[0], first.unwrap()), (templates[69_999], last)] {
        let Value::Host(content) = bridge.template_content(template).unwrap() else { panic!("saved content") };
        assert_eq!(content, saved);
    }
    assert!(bridge.state.retained <= bridge.state.retained_limit);
}

#[test]
fn browser_template_indexes_deep_clone_publishes_ordered_nested_content_batch() {
    let markup = format!("<html><body><template>{}</template></body></html>", "<template></template>".repeat(6000));
    let (mut doc, mut forms, mut page, templates) = page(&markup);
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let Value::Host(source) = bridge.template_content(templates[0]).unwrap() else { panic!("source") };
    let children = bridge.inert_children(source).unwrap().to_vec();
    assert_eq!(children.len(), 6000);
    for &child in &children { bridge.template_content(child).unwrap(); }
    let Value::Host(clone) = bridge.clone_inert_tree(templates[0], true).unwrap() else { panic!("clone") };
    let Value::Host(content) = bridge.template_content(clone).unwrap() else { panic!("cloned content") };
    let copied = bridge.inert_children(content).unwrap().to_vec();
    assert_eq!(copied.len(), 6000);
    for index in [0, 5999] {
        assert_ne!(copied[index], children[index]);
        let Value::Host(original) = bridge.template_content(children[index]).unwrap() else { panic!("original nested content") };
        let Value::Host(nested) = bridge.template_content(copied[index]).unwrap() else { panic!("cloned nested content") };
        assert_ne!(nested, original);
        assert_eq!(bridge.state.inert_documents.node(nested).unwrap().owner,
            bridge.state.inert_documents.node(original).unwrap().owner);
        assert!(bridge.inert_children(nested).unwrap().is_empty());
    }
    assert_eq!(bridge.state.inert_documents.template_contents.len(), 12_002);
    assert!(bridge.state.retained <= bridge.state.retained_limit);
}

#[test]
fn browser_template_indexes_append_charges_real_link_backing_and_clear_refunds_it() {
    let (mut doc, mut forms, mut page, templates) = page("<html><body><template></template></body></html>");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let Value::Host(fragment) = bridge.template_content(templates[0]).unwrap() else { panic!("content") };
    let element = || NodeKind::Element { tag: "b".into(), id: None, href: None, attrs: Vec::new() };
    let Value::Host(child) = bridge.create_dom_node(element()).unwrap() else { panic!("author-created element") };
    let before = bridge.state.retained;
    bridge.append_template_child(fragment, &[Value::Host(child)]).unwrap();
    let allocated = bridge.state.inert_documents.node(fragment).unwrap().children.capacity() * size_of::<u32>();
    assert!(allocated > size_of::<u32>());
    assert_eq!(bridge.state.retained, before + size_of::<(u32, InertNode)>() + allocated);
    bridge.set_template_html(templates[0], "").unwrap();
    assert_eq!(bridge.state.retained, before + size_of::<(u32, InertNode)>());
    assert_eq!(bridge.state.inert_documents.node(child).unwrap().parent, None);
    let Value::Host(next) = bridge.create_dom_node(element()).unwrap() else { panic!("next author element") };
    let before = bridge.state.retained;
    bridge.state.retained_limit = before + size_of::<(u32, InertNode)>() + size_of::<u32>() - 1;
    assert!(bridge.append_template_child(fragment, &[Value::Host(next)]).is_err());
    assert!(!bridge.state.inert_documents.adopted.contains_key(&next));
    assert_eq!(bridge.state.inert_documents.node(fragment).unwrap().children.capacity(), 0);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    bridge.append_template_child(fragment, &[Value::Host(next)]).unwrap();
    assert_eq!(bridge.state.retained, bridge.state.retained_limit);
    assert_eq!(bridge.inert_children(fragment).unwrap(), [next]);
}
