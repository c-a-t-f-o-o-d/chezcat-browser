//! Hit ownership follows the actual transformed, clipped and fixed paint output.
use super::*;
fn id(d: &Document, name: &str) -> usize {
    (0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some(name))
        .unwrap()
}
#[test]
fn native_hover_hit_ownership_follows_transform_clip_and_fixed_scroll() {
    let d=crate::html::parse("<html><head><style>html,body{margin:0}#clip{position:absolute;left:20px;top:20px;width:80px;height:60px;overflow:hidden}#link{display:block;width:50px;height:30px;transform:translateX(40px)}#fixed{position:fixed;left:150px;top:20px;width:40px;height:30px;background:red}</style></head><body><div id=clip><a id=link href=x></a></div><div id=fixed></div></body></html>");
    let mut p = build_viewport(&d, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    assert_eq!(p.element_node_at(65, 30), Some(id(&d, "link")));
    assert_eq!(p.link_node_at(65, 30), Some(id(&d, "link")));
    assert_ne!(p.element_node_at(105, 30), Some(id(&d, "link")));
    assert_eq!(p.element_node_at(170, 30), Some(id(&d, "fixed")));
    p.set_scroll(100);
    assert_eq!(p.element_node_at(170, 130), Some(id(&d, "fixed")));
}
#[test]
fn native_hover_transparent_box_ownership_blocks_links_before_any_hover_paint() {
    let d=crate::html::parse("<style>html,body{margin:0}#under,#cover{position:absolute;left:20px;top:20px;width:80px;height:40px}#under{display:block;background:blue}</style><a id=under href=x></a><div id=cover></div>");
    let p = build_viewport(&d, 300, 200, 1.0, &HashMap::new(), &HashMap::new());
    assert_eq!(p.element_node_at(40, 30), Some(id(&d, "cover")));
    assert_eq!(p.link_node_at(40, 30), None);
}
