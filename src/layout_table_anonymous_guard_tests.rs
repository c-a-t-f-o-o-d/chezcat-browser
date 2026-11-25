//! Supplementary fallback and capacity controls for the finite row repair.
use super::*;

fn render(body: &str) -> (Document, Layout) {
    let doc = crate::html::parse(&format!("<!doctype html><style>html,body{{margin:0;padding:0}}.sheet{{display:table;border-spacing:8px 0}}.group{{display:table-row-group}}.cell{{display:table-cell}}img{{display:block;width:20px;height:20px}}</style><body>{body}</body>"));
    let page = build_viewport(&doc, 800, 480, 1.0, &HashMap::new(), &HashMap::new());
    (doc, page)
}
fn text(page: &Layout) -> String { page.runs.iter().map(|run|run.text.as_str()).collect() }

#[test]
fn outer_visible_text_keeps_fallback_when_child_group_could_gain_anonymous_row() {
    let (_, page) = render("<div class=sheet>OUTER<div class=group><a class=cell href=detail><img alt=''></a></div></div>");
    assert!(text(&page).contains("OUTER"));
    assert_eq!(page.items.iter().filter(|item|matches!(item,PaintItem::Image{..})).count(),1);
}

#[test]
fn visible_content_in_one_group_keeps_fallback_for_the_whole_table() {
    let (_, page) = render("<div class=sheet><div class=group>FIRST<a class=cell><img alt=''></a></div><div class=group><a class=cell><img alt=''></a></div></div>");
    assert!(text(&page).contains("FIRST"));
    assert_eq!(page.items.iter().filter(|item|matches!(item,PaintItem::Image{..})).count(),2);
}

#[test]
fn visible_ordinary_box_beside_direct_cells_is_not_discarded() {
    let (doc, page) = render("<div class=sheet><div id=ordinary style='width:12px;height:7px;background:red'></div><a class=cell><img alt=''></a></div>");
    let node = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("ordinary")).unwrap();
    let rect = page.border_rects[node].expect("fallback ordinary box");
    assert_eq!((rect.width,rect.height),(12,7));
    assert!(page.items.iter().any(|item|matches!(item,PaintItem::Rect{color:0xff0000,..})));
}

#[test]
fn direct_cell_collection_uses_the_existing_per_row_column_limit() {
    let cells = (0..160).map(|i|format!("<div class=cell><img id=i{i} alt=''></div>")).collect::<String>();
    let (doc, page) = render(&format!("<div class=sheet>{cells}</div>"));
    assert_eq!(page.items.iter().filter(|item|matches!(item,PaintItem::Image{..})).count(),128);
    let last = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("i127")).unwrap();
    let omitted = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("i128")).unwrap();
    assert!(page.border_rects[last].is_some());assert!(page.border_rects[omitted].is_none());
}

#[test]
fn permanently_unrendered_containers_do_not_generate_rows_or_columns() {
    for tag in ["template", "object", "datalist"] {
        for display in ["table-row-group", "table-row"] {
            let (doc, page) = render(&format!("<div id=sheet class=sheet><{tag} style='display:{display}'><div class=cell><img id=hidden alt=''></div></{tag}><div class=cell><img id=shown alt=''></div></div>"));
            assert_eq!(page.items.iter().filter(|item|matches!(item,PaintItem::Image{..})).count(),1,"{tag} {display}");
            let shown = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("shown")).unwrap();
            let sheet = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("sheet")).unwrap();
            let rect = page.border_rects[shown].unwrap();
            assert_eq!((rect.x,rect.width),(8,20),"{tag} {display}");
            assert_eq!(page.border_rects[sheet].unwrap().width,36,"{tag} {display}");
        }
    }
}

#[test]
fn hidden_controls_are_absent_but_hidden_or_transparent_cells_keep_columns() {
    let (doc, page) = render("<div id=sheet class=sheet><div class=cell style='visibility:hidden'><img alt=''></div><input class=cell type=hidden><div class=cell style='opacity:0'><img alt=''></div><div class=cell><img id=shown alt=''></div></div>");
    let shown = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("shown")).unwrap();
    let sheet = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("sheet")).unwrap();
    assert_eq!(page.border_rects[shown].unwrap().x,64);
    assert_eq!(page.border_rects[sheet].unwrap().width,92);
}

#[test]
fn nested_row_groups_keep_the_existing_fallback_without_partial_repair() {
    let (doc, page) = render("<div id=sheet class=sheet><div class=group><div class=group><div class=cell><img alt=''></div></div></div><div class=cell><img alt=''></div></div>");
    let sheet = (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("sheet")).unwrap();
    assert_eq!(page.border_rects[sheet].unwrap().width,8);
    assert_eq!(page.items.iter().filter(|item|matches!(item,PaintItem::Image{..})).count(),2);
}
