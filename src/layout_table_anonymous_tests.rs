//! Original table-cell runs that require an anonymous formatting row.
use super::*;

const BASE: &str = "html,body,div,a,span,img{margin:0;padding:0;border:0}body{font-size:16px;line-height:20px}.sheet{display:table;border-spacing:8px 0}.row{display:table-row}.group{display:table-row-group}.cell{display:table-cell;overflow:hidden}img{display:block;height:20px;width:auto;max-width:none;margin:0 auto}";
const PAIR: &str = "<div id=sheet class=sheet><a id=a class=cell href=a><div><img id=ia src=a.ppm alt=''></div></a><a id=b class=cell href=b><div><img id=ib src=b.ppm alt=''></div></a></div>";

fn node(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&n| doc.attr(n, "id") == Some(name)).expect(name)
}
fn doc(markup: &str, rules: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style>{BASE}{rules}</style></head><body>{markup}</body></html>"))
}
fn sizes(doc: &Document, entries: &[(&str, usize, usize)]) -> HashMap<usize, (usize, usize)> {
    entries.iter().map(|&(id,w,h)| (node(doc,id),(w,h))).collect()
}
fn render(doc: &Document, entries: &[(&str, usize, usize)]) -> Layout {
    build_viewport(doc, 800, 480, 1.0, &HashMap::new(), &sizes(doc,entries))
}
fn rect(doc: &Document, page: &Layout, name: &str) -> [i32; 4] {
    let r=page.border_rects[node(doc,name)].expect(name);
    [r.x,r.y,r.width,r.height]
}
fn image_rect(doc: &Document, page: &Layout, name: &str) -> (usize,[i32;4]) {
    let target=node(doc,name);
    page.items.iter().enumerate().find_map(|(i,item)| match item {
        PaintItem::Image {node,x,y,width,height,..} if *node==target => Some((i,[*x,*y,*width,*height])),
        _=>None,
    }).expect("image paint item")
}

#[test]
fn direct_css_cells_share_anonymous_row_and_natural_image_columns() {
    let doc=doc(PAIR,"");let page=render(&doc,&[("ia",4,2),("ib",6,2)]);
    assert_eq!(rect(&doc,&page,"sheet"),[0,0,124,20]);
    for (cell,image,x,width) in [("a","ia",8,40),("b","ib",56,60)] {
        assert_eq!(rect(&doc,&page,cell),[x,0,width,20]);
        assert_eq!(image_rect(&doc,&page,image).1,[x,0,width,20]);
        assert_eq!(page.link_at(x+width-1,19),Some(cell));
        assert_eq!(page.link_at(x+width,19),None);
    }
}

#[test]
fn anonymous_row_matches_explicit_css_row_for_any_cell_tag() {
    let cells="<span id=a class=cell><img id=ia></span><div id=b class=cell><img id=ib></div>";
    let implicit=doc(&format!("<div id=sheet class=sheet>{cells}</div>"),"");
    let explicit=doc(&format!("<div id=sheet class=sheet><div class=row>{cells}</div></div>"),"");
    let a=render(&implicit,&[("ia",4,2),("ib",6,2)]);let b=render(&explicit,&[("ia",4,2),("ib",6,2)]);
    for id in ["sheet","a","b","ia","ib"] {assert_eq!(rect(&implicit,&a,id),rect(&explicit,&b,id),"{id}");}
    assert_eq!(rect(&implicit,&a,"sheet"),[0,0,124,20]);
}

#[test]
fn whitespace_comments_and_hidden_elements_do_not_split_cell_run() {
    let doc=doc("<div id=sheet class=sheet> \n<a id=a class=cell><img id=ia></a> \n<!-- between --><div style='display:none'>ignored</div>\t<a id=b class=cell><img id=ib></a> \n</div>","");
    let page=render(&doc,&[("ia",4,2),("ib",6,2)]);
    assert_eq!(rect(&doc,&page,"sheet"),[0,0,124,20]);
    assert_eq!(rect(&doc,&page,"a"),[8,0,40,20]);
    assert_eq!(rect(&doc,&page,"b"),[56,0,60,20]);
    assert!(page.runs.is_empty());
}

#[test]
fn direct_cells_in_row_groups_keep_group_extents_and_shared_columns() {
    let doc=doc("<div id=sheet class=sheet><div id=g1 class=group><a id=a class=cell><img id=ia></a><a id=b class=cell><img id=ib></a></div><div id=g2 class=group><a id=c class=cell><img id=ic></a><a id=d class=cell><img id=id></a></div></div>",".sheet{border-spacing:8px 3px}");
    let page=render(&doc,&[("ia",4,2),("ib",6,2),("ic",4,2),("id",6,2)]);
    assert_eq!(rect(&doc,&page,"sheet"),[0,0,124,49]);
    for (id,x,y,w) in [("a",8,3,40),("b",56,3,60),("c",8,26,40),("d",56,26,60)] {assert_eq!(rect(&doc,&page,id),[x,y,w,20]);}
    for id in ["g1","g2"] {let b=page.client_boxes[node(&doc,id)].unwrap();assert_eq!((b.width,b.height),(108,20),"{id}");}
}

#[test]
fn explicit_rows_separate_anonymous_runs_without_losing_cells() {
    let doc=doc("<div id=sheet class=sheet><a id=a class=cell><img id=ia></a><a id=b class=cell><img id=ib></a><div id=middle class=row><a id=c class=cell><img id=ic></a><a id=d class=cell><img id=id></a></div><a id=e class=cell><img id=ie></a><a id=f class=cell><img id=if></a></div>",".sheet{border-spacing:8px 3px}");
    let page=render(&doc,&[("ia",4,2),("ib",6,2),("ic",4,2),("id",6,2),("ie",4,2),("if",6,2)]);
    assert_eq!(rect(&doc,&page,"sheet"),[0,0,124,72]);
    for (id,x,y,w) in [("a",8,3,40),("b",56,3,60),("c",8,26,40),("d",56,26,60),("e",8,49,40),("f",56,49,60)] {assert_eq!(rect(&doc,&page,id),[x,y,w,20]);}
    assert_eq!(page.items.iter().filter(|item|matches!(item,PaintItem::Image{..})).count(),6);
    let row=page.client_boxes[node(&doc,"middle")].unwrap();assert_eq!((row.width,row.height),(108,20));
}

#[test]
fn nested_anonymous_tables_do_not_escape_their_cell_or_parent_columns() {
    let doc=doc("<div id=sheet class=sheet><div id=outer class=cell><div id=inner class=sheet><a id=a class=cell><img id=ia></a><a id=b class=cell><img id=ib></a></div></div><a id=c class=cell><img id=ic></a></div>","");
    let page=render(&doc,&[("ia",4,2),("ib",6,2),("ic",2,2)]);
    assert_eq!(rect(&doc,&page,"sheet"),[0,0,168,20]);
    assert_eq!(rect(&doc,&page,"outer"),[8,0,124,20]);
    assert_eq!(rect(&doc,&page,"inner"),[8,0,124,20]);
    assert_eq!(rect(&doc,&page,"a"),[16,0,40,20]);
    assert_eq!(rect(&doc,&page,"b"),[64,0,60,20]);
    assert_eq!(rect(&doc,&page,"c"),[140,0,20,20]);
}

#[test]
fn six_clipped_image_cells_keep_ratios_spacing_and_full_hit_widths() {
    let markup=format!("<div id=sheet class=sheet>{}</div>",(0..6).map(|i|format!("<a id=c{i} class=cell href=c{i}><div><img id=i{i} alt=''></div></a>")).collect::<String>());
    let doc=doc(&markup,"img{height:40px;max-height:40px}");
    let entries=[("i0",3,4),("i1",5,4),("i2",2,4),("i3",3,4),("i4",4,4),("i5",6,4)];
    let page=render(&doc,&entries);
    assert_eq!(rect(&doc,&page,"sheet"),[0,0,286,40]);
    for (i,(x,w)) in [(8,30),(46,50),(104,20),(132,30),(170,40),(218,60)].into_iter().enumerate() {
        let cell=format!("c{i}");let image=format!("i{i}");
        assert_eq!(rect(&doc,&page,&cell),[x,0,w,40]);
        let (item,r)=image_rect(&doc,&page,&image);assert_eq!(r,[x,0,w,40]);
        let clip=page.paint_clip(item).expect("cell overflow clip");
        assert_eq!((clip.left,clip.top,clip.right,clip.bottom),(x,0,x+w,40));
        assert_eq!(page.link_at(x+w-1,39),Some(cell.as_str()));
        assert_eq!(page.link_at(x+w,39),None);
    }
}

#[test]
fn natural_image_update_recomputes_anonymous_columns_with_cached_styles() {
    let doc=doc(PAIR,"");let initial=render(&doc,&[("ia",1,1),("ib",1,1)]);
    assert_eq!(rect(&doc,&initial,"sheet"),[0,0,64,20]);
    let changed=build_viewport_with_fonts_cached(&doc,800,480,1.0,&HashMap::new(),&sizes(&doc,&[("ia",4,2),("ib",6,2)]),None,&Book::empty(),initial.stylesheet_cache.as_ref());
    assert_eq!(rect(&doc,&changed,"sheet"),[0,0,124,20]);
    assert_eq!(rect(&doc,&changed,"b"),[56,0,60,20]);
}

#[test]
fn table_padding_negative_margin_and_scroll_wrapper_preserve_cell_geometry() {
    let doc=doc(&format!("<div id=wrapper>{PAIR}</div><div id=after style='height:7px'></div>"),"#wrapper{width:100px;overflow-x:auto;padding-left:22px}.sheet{margin:0 -8px;padding-right:14px}");
    let page=render(&doc,&[("ia",4,2),("ib",6,2)]);
    assert_eq!(rect(&doc,&page,"wrapper"),[0,0,122,20]);
    assert_eq!(rect(&doc,&page,"sheet"),[14,0,138,20]);
    assert_eq!(rect(&doc,&page,"a"),[22,0,40,20]);
    assert_eq!(rect(&doc,&page,"b"),[70,0,60,20]);
    assert_eq!(rect(&doc,&page,"after")[1],20);
    assert_eq!(page.link_at(121,19),Some("b"));
    assert_eq!(page.link_at(122,19),None);
}

#[test]
fn translated_ancestor_moves_cell_paint_and_hits_together() {
    let doc=doc(&format!("<div id=wrapper>{PAIR}</div>"),"#wrapper{transform:translate(10px,12px)}");
    let page=render(&doc,&[("ia",4,2),("ib",6,2)]);
    assert_eq!(image_rect(&doc,&page,"ia").1,[18,12,40,20]);
    assert_eq!(image_rect(&doc,&page,"ib").1,[66,12,60,20]);
    assert_eq!(page.link_at(125,31),Some("b"));
    assert_eq!(page.link_at(126,31),None);
}

#[test]
fn anonymous_rows_preserve_dom_identity_and_do_not_alias_parent_geometry() {
    let doc=doc(&format!("{PAIR}<div id=after style='height:7px'></div>"),"");
    let children:Vec<_>=doc.nodes.iter().map(|n|n.children.clone()).collect();let count=doc.nodes.len();
    let page=render(&doc,&[("ia",4,2),("ib",6,2)]);
    assert_eq!(doc.nodes.len(),count);assert_eq!(doc.nodes.iter().map(|n|n.children.clone()).collect::<Vec<_>>(),children);
    assert_eq!(page.client_boxes.len(),count);assert_eq!(page.border_rects.len(),count);
    let parent=page.client_boxes[node(&doc,"sheet")].unwrap();assert_eq!((parent.width,parent.height),(124,20));
    assert_eq!(rect(&doc,&page,"after"),[0,20,800,7]);
}

#[test]
fn explicit_rows_keep_legacy_height_alignment_and_spacing() {
    let doc=doc("<div id=sheet class=sheet><div id=row class=row><a id=a class=cell href=a><img id=ia></a><a id=b class=cell href=b valign=bottom><img id=ib></a></div></div>",".sheet{border-spacing:8px 3px}#row{height:40px}#a{height:40px}#ia{height:40px}");
    let page=render(&doc,&[("ia",2,2),("ib",3,2)]);
    assert_eq!(rect(&doc,&page,"sheet"),[0,0,94,46]);
    assert_eq!(image_rect(&doc,&page,"ia").1,[8,3,40,40]);
    assert_eq!(image_rect(&doc,&page,"ib").1,[56,23,30,20]);
    assert_eq!(rect(&doc,&page,"b"),[56,3,30,40]);
}
