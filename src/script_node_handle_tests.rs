//! Original typed node-handle boundary and genuine owner acceptance.
use super::*;

#[test]
fn node_handle_domains_round_trip_every_boundary_without_wrapping_wrappers() {
    for index in [0, 499_999, 500_000, 999_999, NODE_HANDLES as usize - 1] {
        let node = node_handle(index);
        assert_eq!(node_index(node), Some(index));
        let tokens = script_class_list::handle(node);
        let attrs = script_named_map::handle(node);
        assert_eq!(script_class_list::element(tokens), Some(node));
        assert_eq!(script_named_map::element(attrs), Some(node));
        assert!((0x8000_0000..0x8100_0000).contains(&tokens));
        assert!((0xc000_0000..0xc100_0000).contains(&attrs));
        for foreign in [node, STYLE_BASE + index as u32, DATASET_BASE + index as u32, EVENT_BASE,
            script_attr::ATTR_BASE, NODE_LIST_BASE] {
            assert!(script_class_list::element(foreign).is_none());
            assert!(script_named_map::element(foreign).is_none());
        }
    }
    for end in [0x8100_0000, 0xc100_0000] {
        assert!(script_class_list::element(end).is_none());
        assert!(script_named_map::element(end).is_none());
    }
    assert!(node_index(NODE_BASE - 1).is_none());
    assert!(node_index(STYLE_BASE).is_none());
}

#[test]
fn node_handle_large_real_owner_preserves_style_dataset_and_element_wrapper_identity() {
    let mut doc = crate::html::parse_loaded_document("<html><body><p id='kept' class='one' data-value='real' style='color:red'>text</p></body></html>", &||Ok(())).unwrap();
    let old = (0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some("kept")).unwrap();
    let body = doc.nodes.iter().position(|node|matches!(&node.kind,NodeKind::Element{tag,..} if tag=="body")).unwrap();
    // Real storage and provenance at the old decoder boundary. This fixture
    // does not claim that the loaded parser or production node-count policy grew.
    let index = 500_001;
    doc.nodes.try_reserve_exact(index + 1 - doc.nodes.len()).unwrap();
    while doc.nodes.len() <= index {
        doc.nodes.push(Node {kind:NodeKind::DocumentFragment,children:Vec::new(),namespace:crate::html::Namespace::Html});
    }
    doc.nodes.swap(old,index);
    for child in &mut doc.nodes[body].children { if *child==old {*child=index;} }
    doc.scripting_enabled=true;
    let mut forms=Forms::new(&doc);let mut page=Page::new();
    assert_eq!(page.start(&mut doc,&mut forms,&[]).errors,0);
    let mut bridge=Bridge{doc:&mut doc,forms:&mut forms,state:&mut page.state};
    let owner=node_handle(index);
    assert_eq!(bridge.node(owner).unwrap(),index);
    let tokens=script_class_list::handle(owner);let attrs=script_named_map::handle(owner);
    assert!(bridge.class_list_receiver(tokens));assert!(bridge.named_map_receiver(attrs));
    assert_eq!(bridge.style_node(STYLE_BASE+index as u32).unwrap(),index);
    for object in [owner,tokens,attrs,STYLE_BASE+index as u32,DATASET_BASE+index as u32] {
        assert_eq!(bridge.instance_ancestry(object).unwrap().window,WINDOW);
    }
    assert!(matches!(bridge.get(DATASET_BASE+index as u32,"value").unwrap(),Value::String(value) if value=="real"));
    assert!(matches!(bridge.class_list_get(tokens,"value").unwrap(),Value::String(value) if value=="one"));
    assert!(matches!(bridge.named_map_get(attrs,"length").unwrap(),Value::Number(value) if value==4.0));
    // In-range identities still require a real, accessible Element owner.
    let missing=bridge.doc.nodes.len()+1;
    for object in [STYLE_BASE+missing as u32,DATASET_BASE+missing as u32,
        script_class_list::handle(node_handle(missing)),script_named_map::handle(node_handle(missing))] {
        assert!(bridge.instance_ancestry(object).is_none());
        assert!(bridge.get(object,"length").is_err());
    }
    assert!(!bridge.class_list_receiver(script_class_list::handle(node_handle(old))));
    assert!(!bridge.named_map_receiver(script_named_map::handle(node_handle(old))));
}
