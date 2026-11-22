//! Original main child-buffer ownership, exact admission and recovery cases.
use super::*;
fn start(mut doc: Document) -> (Document, Forms, Page) {
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::new();
    assert_eq!(page.start(&mut doc, &mut forms, &[]).errors, 0);
    (doc, forms, page)
}
fn parsed(markup: &str) -> Document {
    crate::html::parse_loaded_document(markup, &|| Ok(())).unwrap()
}
fn element(doc: &Document, name: &str) -> usize {
    doc.nodes.iter().position(|node| matches!(&node.kind, NodeKind::Element { tag, .. } if tag == name)).unwrap()
}
fn wide_children(doc: &mut Document, node: usize, slots: usize) {
    let extra = slots - doc.nodes[node].children.len();
    doc.nodes[node].children.try_reserve_exact(extra).unwrap();
}
fn call(bridge: &mut Bridge<'_>, node: usize, method: &str, args: &[Value]) -> Result<Value, String> {
    bridge.dom_call(node_handle(node), method, args)?.ok_or("Missing original DOM operation.".into())
}
fn native(value: Value) -> u32 { let Value::Host(handle) = value else { panic!("native Node") }; handle }
fn fresh(bridge: &mut Bridge<'_>, name: &str) -> usize {
    node_index(native(bridge.create_dom_node(NodeKind::Element { tag: name.into(), id: None, href: None, attrs: Vec::new() }).unwrap())).unwrap()
}

#[test]
fn main_child_storage_startup_and_clear_retain_backing_while_replacement_refunds_it() {
    let mut doc = parsed("<html><body><p>x</p></body></html>");
    let parent = element(&doc, "p");
    let text = doc.nodes[parent].children[0];
    wide_children(&mut doc, parent, 64);
    let baseline = document_bytes(&doc);
    let width = child_link_bytes(&doc.nodes[parent].children);
    let (mut doc, mut forms, mut page) = start(doc);
    let expected=baseline+page.state.tree_storage_bytes();
    assert_eq!(page.state.retained, expected);
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    bridge.set_text_content(parent, String::new().into()).unwrap();
    assert_eq!(bridge.state.retained, expected);
    assert_eq!(child_link_bytes(&bridge.doc.nodes[parent].children), width);
    let old_order=bridge.state.order_bytes();
    bridge.state.rebuild(bridge.doc).unwrap();
    assert_eq!(bridge.state.retained,expected-old_order+bridge.state.order_bytes());
    let after_clear=bridge.state.retained;
    bridge.state.retained_limit = after_clear - width;
    bridge.set_inner_html(parent, "").unwrap();
    assert_eq!(bridge.state.retained, after_clear - width);
    assert_eq!(bridge.doc.nodes[parent].children.capacity(), 0);
    assert_eq!(bridge.doc.nodes[text].kind.character_data(), Some("x"));
    assert!(bridge.state.detached_nodes.contains(&text));
}

#[test]
fn main_child_storage_append_exact_and_copy_refusal_keep_parent_and_reuse_paid_slots() {
    let (mut doc, mut forms, mut page) = start(parsed("<html><body><p></p></body></html>"));
    let parent = element(&doc, "p");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let child = fresh(&mut bridge, "b");
    // Isolate child-link admission from later state-array compaction refunds.
    bridge.state.dirty=true;bridge.state.rebuild(bridge.doc).unwrap();
    let limit=bridge.state.retained_limit;
    let before = bridge.state.retained;
    bridge.state.retained_limit = before + size_of::<usize>() - 1;
    assert!(call(&mut bridge, parent, "appendChild", &[Value::Host(node_handle(child))]).is_err());
    bridge.state.retained_limit += 1;
    bridge.state.copy_work = 0;
    assert!(call(&mut bridge, parent, "appendChild", &[Value::Host(node_handle(child))]).is_err());
    assert_eq!(bridge.doc.nodes[parent].children.capacity(), 0);
    assert_eq!(bridge.state.tree.parents[child], None);
    assert_eq!(bridge.state.retained, before);
    bridge.state.copy_work = size_of::<usize>();
    call(&mut bridge, parent, "appendChild", &[Value::Host(node_handle(child))]).unwrap();
    assert_eq!(bridge.state.retained, before + size_of::<usize>());
    assert_eq!(bridge.state.copy_work, 0);
    bridge.state.copy_work = bridge.state.limits.copy_bytes;
    // A connected order index may grow before the later removal; this part
    // isolates reuse of the paid child-link slot from that separate store.
    bridge.state.retained_limit=limit;
    call(&mut bridge, parent, "removeChild", &[Value::Host(node_handle(child))]).unwrap();
    bridge.state.retained_limit = bridge.state.retained;
    call(&mut bridge, parent, "appendChild", &[Value::Host(node_handle(child))]).unwrap();
    assert_eq!(bridge.state.retained, before + size_of::<usize>());
    assert_eq!(bridge.doc.nodes[parent].children, [child]);
}

#[test]
fn main_child_storage_cross_parent_replacement_refunds_both_original_buffers() {
    let mut doc = parsed("<html><body><p><b></b></p><div><i></i></div></body></html>");
    let parent = element(&doc, "p");
    let source = element(&doc, "div");
    let old = element(&doc, "b");
    let incoming = element(&doc, "i");
    wide_children(&mut doc, parent, 64);
    wide_children(&mut doc, source, 32);
    let released = child_link_bytes(&doc.nodes[parent].children) + child_link_bytes(&doc.nodes[source].children);
    let (mut doc, mut forms, mut page) = start(doc);
    let before = page.state.retained;
    let final_bytes = before - released + size_of::<usize>();
    page.state.retained_limit = final_bytes - 1;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let args = [Value::Host(node_handle(incoming)), Value::Host(node_handle(old))];
    assert!(call(&mut bridge, parent, "replaceChild", &args).is_err());
    assert_eq!(bridge.doc.nodes[parent].children, [old]);
    assert_eq!(bridge.doc.nodes[source].children, [incoming]);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    call(&mut bridge, parent, "replaceChild", &args).unwrap();
    assert_eq!(bridge.state.retained, final_bytes);
    assert_eq!(bridge.doc.nodes[parent].children, [incoming]);
    assert_eq!(bridge.doc.nodes[source].children.capacity(), 0);
    assert_eq!(bridge.state.tree.parents[old], None);
    assert_eq!(bridge.state.tree.parents[incoming], Some(parent));
}

#[test]
fn main_child_storage_fragment_admits_destination_and_keeps_reusable_donor_backing() {
    let (mut doc, mut forms, mut page) = start(parsed("<html><body><p></p></body></html>"));
    let parent = element(&doc, "p");
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let donor = node_index(native(bridge.create_dom_node(NodeKind::DocumentFragment).unwrap())).unwrap();
    let a = fresh(&mut bridge, "b");
    let b = fresh(&mut bridge, "i");
    for child in [a, b] { call(&mut bridge, donor, "appendChild", &[Value::Host(node_handle(child))]).unwrap(); }
    let donor_bytes = child_link_bytes(&bridge.doc.nodes[donor].children);
    let before = bridge.state.retained;
    bridge.state.retained_limit = before + 2 * size_of::<usize>() - 1;
    assert!(call(&mut bridge, parent, "appendChild", &[Value::Host(node_handle(donor))]).is_err());
    assert_eq!(bridge.doc.nodes[donor].children, [a, b]);
    assert_eq!(bridge.state.retained, before);
    bridge.state.retained_limit += 1;
    call(&mut bridge, parent, "appendChild", &[Value::Host(node_handle(donor))]).unwrap();
    assert_eq!(bridge.state.retained, before + 2 * size_of::<usize>());
    assert_eq!(child_link_bytes(&bridge.doc.nodes[donor].children), donor_bytes);
    assert!(bridge.doc.nodes[donor].children.is_empty());
    assert_eq!(bridge.doc.nodes[parent].children, [a, b]);
}

#[test]
fn main_child_storage_clone_and_import_admit_compact_child_buffers_before_nodes() {
    for import in [false, true] {
        let (mut doc, mut forms, mut page) = start(parsed("<html><body><p><b></b><i></i></p></body></html>"));
        let source = element(&doc, "p");
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        let handle = if import {
            let owner = native(bridge.parse_inert_document(&[Value::String("<p><b></b><i></i></p>".into())]).unwrap());
            let body = native(bridge.inert_get(owner, "body").unwrap());
            bridge.inert_children(body).unwrap()[0]
        } else { node_handle(source) };
        let before = bridge.state.retained;
        let nodes = bridge.doc.nodes.len();
        let bundle = 3 + 2 * size_of::<usize>() + bridge.main_node_minimum(3).unwrap();
        bridge.state.retained_limit = before + bundle - 1;
        let copy = |bridge: &mut Bridge<'_>| if import {
            bridge.import_node(&[Value::Host(handle), Value::Number(1.0)], None)
        } else { bridge.clone_node(handle, true) };
        assert!(copy(&mut bridge).is_err());
        assert_eq!(bridge.doc.nodes.len(), nodes);
        assert_eq!(bridge.state.retained, before);
        bridge.state.retained_limit += 1;
        let copied = node_index(native(copy(&mut bridge).unwrap())).unwrap();
        assert_eq!(bridge.state.retained, before + bundle);
        assert_eq!(bridge.doc.nodes[copied].children.len(), 2);
        assert_eq!(child_link_bytes(&bridge.doc.nodes[copied].children), 2 * size_of::<usize>());
    }
}

#[test]
fn main_child_storage_inner_html_moves_nested_buffers_and_refunds_wide_root() {
    let markup = "<div><b>x</b></div>";
    let fragment = crate::html::parse_fragment(markup, "p", true).unwrap();
    let mut owned = fragment.nodes.iter().skip(1).map(|node| {
        node_bytes(node) + child_link_bytes(&node.children) + match &node.kind {
            NodeKind::Element { tag, href, .. } => tag.len() + href.as_ref().map_or(0, String::len),
            _ => 0,
        }
    }).sum::<usize>() + size_of::<usize>();
    let mut doc = parsed("<html><body><p></p></body></html>");
    let parent = element(&doc, "p");
    wide_children(&mut doc, parent, 64);
    let released = child_link_bytes(&doc.nodes[parent].children);
    let (mut doc, mut forms, mut page) = start(doc);
    let before = page.state.retained;
    owned += Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.main_node_minimum(fragment.nodes.len()-1).unwrap();
    page.state.retained_limit = before - released + owned - 1;
    let nodes = doc.nodes.len();
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    assert!(bridge.set_inner_html(parent, markup).is_err());
    assert_eq!(bridge.doc.nodes.len(), nodes);
    assert_eq!(bridge.state.retained, before);
    assert_eq!(child_link_bytes(&bridge.doc.nodes[parent].children), released);
    bridge.state.retained_limit += 1;
    bridge.set_inner_html(parent, markup).unwrap();
    assert_eq!(bridge.state.retained, before - released + owned);
    assert_eq!(bridge.doc.nodes[parent].children.len(), 1);
}

#[test]
fn main_child_storage_normalize_refunds_parent_capacity_and_keeps_removed_text() {
    let mut doc = parsed("<html><body><p>x</p></body></html>");
    let parent = element(&doc, "p");
    let first = doc.nodes[parent].children[0];
    let second = doc.nodes.len();
    doc.nodes.push(Node { kind: NodeKind::Text("y".into()), children: Vec::new(), namespace: crate::html::Namespace::None });
    doc.nodes[parent].children.push(second);
    wide_children(&mut doc, parent, 64);
    let old_links = child_link_bytes(&doc.nodes[parent].children);
    let old_text = doc.nodes[first].kind.character_data_capacity().unwrap();
    let (mut doc, mut forms, mut page) = start(doc);
    let before = page.state.retained;
    page.state.retained_limit = before;
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    bridge.normalize_node(node_handle(parent)).unwrap();
    let new_links = child_link_bytes(&bridge.doc.nodes[parent].children);
    let new_text = bridge.doc.nodes[first].kind.character_data_capacity().unwrap();
    assert_eq!(bridge.state.retained, before - old_links - old_text + new_links + new_text);
    assert!(new_links < old_links);
    assert_eq!(bridge.doc.nodes[first].kind.character_data(), Some("xy"));
    assert_eq!(bridge.doc.nodes[second].kind.character_data(), Some("y"));
    assert!(bridge.state.detached_nodes.contains(&second));
}

#[test]
fn main_child_storage_six_thousand_cloned_children_pay_actual_compact_backing() {
    let markup = format!("<html><body><div>{}</div></body></html>", "<b></b>".repeat(6000));
    let (mut doc, mut forms, mut page) = start(parsed(&markup));
    let source = element(&doc, "div");
    let before = page.state.retained;
    let bundle = 3 + 6000 + 6000 * size_of::<usize>()
        + Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state}.main_node_minimum(6001).unwrap();
    page.state.retained_limit = before + bundle;
    let copied = {
        let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
        node_index(native(bridge.clone_node(node_handle(source), true).unwrap())).unwrap()
    };
    assert_eq!(page.state.retained, before + bundle);
    assert_eq!(doc.nodes[copied].children.len(), 6000);
    assert_eq!(child_link_bytes(&doc.nodes[copied].children), 6000 * size_of::<usize>());
}

#[test]
fn main_child_storage_script_text_reservation_recovers_without_unpublished_nodes_or_links() {
    let (mut doc, mut forms, mut page) = start(parsed("<html><body></body></html>"));
    let mut bridge = Bridge { doc: &mut doc, forms: &mut forms, state: &mut page.state };
    let script = fresh(&mut bridge, "script");
    let nodes = bridge.doc.nodes.len();
    let before = bridge.state.retained;
    let bundle = "payload".len() + size_of::<usize>() + bridge.main_node_minimum(1).unwrap();
    bridge.state.retained_limit = before + bundle - 1;
    assert!(bridge.set(node_handle(script), "text", Value::String("payload".into())).is_err());
    assert_eq!(bridge.state.retained, before);
    assert_eq!(bridge.doc.nodes.len(), nodes);
    bridge.state.retained_limit += 1;
    let node_limit = bridge.state.node_limit;
    bridge.state.node_limit = nodes;
    assert!(bridge.set(node_handle(script), "text", Value::String("payload".into())).is_err());
    assert_eq!(bridge.state.retained, before);
    assert_eq!(bridge.doc.nodes.len(), nodes);
    assert_eq!(bridge.doc.nodes[script].children.capacity(), 0);
    bridge.state.node_limit = node_limit;
    bridge.set(node_handle(script), "text", Value::String("payload".into())).unwrap();
    assert_eq!(bridge.state.retained, before + bundle);
    assert_eq!(bridge.doc.nodes.len(), nodes + 1);
    assert_eq!(bridge.doc.nodes[bridge.doc.nodes[script].children[0]].kind.character_data(), Some("payload"));
}

#[test]
fn main_child_storage_normalize_exact_one_record_and_empty_output_need_no_geometric_spare() {
    for empty in [false, true] {
        let mut doc = parsed("<html><body><p></p></body></html>");
        let parent = element(&doc, "p");
        let first = doc.nodes.len();
        doc.nodes.push(Node { kind: NodeKind::Text(if empty {String::new().into()}else{"x".into()}), children: Vec::new(), namespace: crate::html::Namespace::None });
        doc.nodes[parent].children.try_reserve_exact(if empty {1}else{2}).unwrap();
        doc.nodes[parent].children.push(first);
        if !empty {
            let second=doc.nodes.len();
            doc.nodes.push(Node { kind: NodeKind::Text("y".into()), children: Vec::new(), namespace: crate::html::Namespace::None });
            doc.nodes[parent].children.push(second);
        }
        let old_links=child_link_bytes(&doc.nodes[parent].children);
        let (mut doc,mut forms,mut page)=start(doc);
        let before=page.state.retained;
        let final_bytes=before-old_links+if empty {0}else{size_of::<usize>()+1};
        page.state.retained_limit=final_bytes-1;
        let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
        assert!(bridge.normalize_node(node_handle(parent)).is_err());
        assert_eq!(bridge.state.retained,before);
        bridge.state.retained_limit+=1;
        bridge.normalize_node(node_handle(parent)).unwrap();
        assert_eq!(bridge.state.retained,final_bytes);
        assert_eq!(bridge.doc.nodes[parent].children.len(),usize::from(!empty));
        assert_eq!(child_link_bytes(&bridge.doc.nodes[parent].children),if empty {0}else{size_of::<usize>()});
    }
}
