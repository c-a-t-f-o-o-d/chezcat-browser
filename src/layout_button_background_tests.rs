//! Original background-source witnesses for genuine native button controls.
use super::*;

const BASE: &str = "html,body{margin:0;padding:0}button,input{margin:0;padding:0;width:48px;height:32px}";
fn document(markup: &str, rules: &str) -> Document {
    crate::html::parse(&format!("<!doctype html><html><head><style id=sheet>{BASE}{rules}</style></head><body>{markup}</body></html>"))
}
fn node(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some(name)).expect(name)
}
fn page(doc: &Document, cache: Option<&css::StylesheetCache>) -> Layout {
    build_viewport_with_fonts_cached(doc,800,480,1.0,&HashMap::new(),&HashMap::new(),None,&Book::empty(),cache)
}
fn sources(page: &Layout) -> HashMap<usize,(&str,Option<usize>)> {
    let list:Vec<_>=page.background_sources().map(|(node,image)|(node,(image.url.as_str(),image.stylesheet))).collect();
    let map:HashMap<_,_>=list.iter().copied().collect();
    assert_eq!(map.len(),list.len(),"exactly one selected background source per owner");map
}
fn set_attr(doc: &mut Document, node: usize, name: &str, value: &str) {
    let NodeKind::Element{attrs,..}=&mut doc.nodes[node].kind else {panic!("element")};
    if let Some(attr)=attrs.iter_mut().find(|attr|attr.name.html_is(name)){attr.value=value.into();}
    else{attrs.push(crate::html::Attribute::unqualified(name.to_owned(),value.to_owned()));}
}

#[test]
fn button_and_input_button_families_select_their_real_css_background_once() {
    let doc=document("<button id=a type=button>A</button><button id=b type=submit>B</button><button id=c type=reset>C</button><input id=d type=button value=D><input id=e type=submit value=E><input id=f type=reset value=F>","button,input{background-image:url(Tile.ppm)}");
    let page=page(&doc,None);let selected=sources(&page);assert_eq!(selected.len(),6);let sheet=node(&doc,"sheet");
    for id in ["a","b","c","d","e","f"] {
        let node=node(&doc,id);assert_eq!(selected[&node],("Tile.ppm",Some(sheet)));
        assert_eq!(page.items.iter().filter(|item|matches!(item,PaintItem::Control(i)if page.controls[*i].node==node)).count(),1);
        assert!(!page.items.iter().any(|item|matches!(item,PaintItem::Background{node:owner,..}if *owner==node)),"native painter owns the one image layer");
    }
}

#[test]
fn ordinary_controls_and_image_buttons_do_not_join_the_new_button_slice() {
    let doc=document("<input id=text><input id=check type=checkbox><input id=radio type=radio><input id=image type=image src=content.ppm><textarea id=area>Text</textarea><select id=select><option>Choice</option></select><button id=button type=button>Action</button><div id=block style='width:20px;height:20px'></div>","input,textarea,select,button,div{background-image:url(Shared.ppm)}");
    let page=page(&doc,None);let selected=sources(&page);
    assert_eq!(selected.len(),2);assert!(selected.contains_key(&node(&doc,"button")));assert!(selected.contains_key(&node(&doc,"block")));
    for id in ["text","check","radio","image","area","select"] {assert!(!selected.contains_key(&node(&doc,id)));}
}

#[test]
fn absent_controls_do_not_select_images_but_visibility_and_opacity_keep_sources() {
    let doc=document("<button id=normal>N</button><button id=hidden hidden>H</button><button id=none style='display:none'>D</button><input id=hidden-input type=hidden><button id=invisible style='visibility:hidden'>V</button><button id=transparent style='opacity:0'>O</button>","button,input{background-image:url(State.ppm)}");
    let page=page(&doc,None);let selected=sources(&page);
    assert_eq!(selected.len(),3);
    for id in ["normal","invisible","transparent"] {assert!(selected.contains_key(&node(&doc,id)),"{id}");}
    for id in ["hidden","none","hidden-input"] {assert!(!selected.contains_key(&node(&doc,id)),"{id}");}
    assert!(!page.controls.iter().any(|control|control.node==node(&doc,"invisible")));
    assert!(page.controls.iter().any(|control|control.node==node(&doc,"transparent")));
}

#[test]
fn inline_overrides_preserve_url_case_and_stylesheet_owner_identity() {
    let doc=document("<button id=sheet-owned>A</button><button id=inline-owned style='background-image:url(Own/Case.PPM?Token=Keep)'>B</button>","button{background-image:url(Sheet/Case.PPM?Token=Keep)}");
    let page=page(&doc,None);let selected=sources(&page);
    assert_eq!(selected[&node(&doc,"sheet-owned")],("Sheet/Case.PPM?Token=Keep",Some(node(&doc,"sheet"))));
    assert_eq!(selected[&node(&doc,"inline-owned")],("Own/Case.PPM?Token=Keep",None));
}

#[test]
fn backgrounds_do_not_inherit_without_an_explicit_inherit_declaration() {
    let doc=document("<div id=parent><button id=plain>A</button><button id=inherited style='background-image:inherit'>B</button></div>","#parent{background-image:url(Parent.ppm)}");
    let page=page(&doc,None);let selected=sources(&page);assert_eq!(selected.len(),2);
    assert!(!selected.contains_key(&node(&doc,"plain")));
    assert_eq!(selected[&node(&doc,"inherited")],("Parent.ppm",Some(node(&doc,"sheet"))));
}

#[test]
fn cached_cascade_replaces_and_retires_sources_after_class_and_inline_changes() {
    let mut doc=document("<button id=target class=first>Action</button>",".first{background-image:url(First.ppm)}.second{background-image:url(Second.ppm)}");
    let target=node(&doc,"target");let first=page(&doc,None);assert_eq!(sources(&first)[&target].0,"First.ppm");
    set_attr(&mut doc,target,"class","second");let second=page(&doc,first.stylesheet_cache.as_ref());assert_eq!(sources(&second)[&target].0,"Second.ppm");
    set_attr(&mut doc,target,"style","background-image:none");let removed=page(&doc,second.stylesheet_cache.as_ref());assert!(!sources(&removed).contains_key(&target));
    set_attr(&mut doc,target,"style","background-image:url(Inline.ppm)");let replaced=page(&doc,removed.stylesheet_cache.as_ref());assert_eq!(sources(&replaced)[&target],("Inline.ppm",None));
    set_attr(&mut doc,target,"hidden","");let absent=page(&doc,replaced.stylesheet_cache.as_ref());assert!(!sources(&absent).contains_key(&target));
}

#[test]
fn invalid_image_declarations_keep_the_previous_valid_source() {
    for invalid in ["url(One.ppm),url(Two.ppm)","linear-gradient(red,blue)","url()"] {
        let doc=document("<button id=target>Action</button>",&format!("button{{background-image:url(Valid.ppm);background-image:{invalid}}}"));
        let page=page(&doc,None);assert_eq!(sources(&page)[&node(&doc,"target")].0,"Valid.ppm");
    }
}

#[test]
fn background_source_size_never_changes_control_geometry_or_native_hits() {
    let markup="<div style='display:flex;width:200px;height:50px'><button id=a style='flex:1;align-self:stretch'>First</button><button id=b style='flex:1'>Second</button></div>";
    let plain=document(markup,"");let decorated=document(markup,"button{background-image:url(HugeUndecoded.ppm)}");
    let a=page(&plain,None);let b=page(&decorated,None);
    assert_eq!(a.controls.len(),b.controls.len());assert_eq!(sources(&b).len(),2);
    for (left,right) in a.controls.iter().zip(&b.controls) {
        assert_eq!((left.x,left.y,left.width,left.height),(right.x,right.y,right.width,right.height));
        assert_eq!(b.control_at(right.x+right.width/2,right.y+right.height/2),Some(right.node));
    }
    assert!(!b.items.iter().any(|item|matches!(item,PaintItem::Image{..})));
}

#[test]
fn moved_transformed_and_fixed_buttons_keep_a_single_source_owner() {
    let doc=document("<div style='transform:translate(10px,12px)'><button id=moved style='position:relative;left:3px;top:4px'>Moved</button></div><button id=fixed style='position:fixed;left:100px;top:20px'>Fixed</button>","button{background-image:url(Position.ppm)}");
    let mut page=page(&doc,None);assert_eq!(sources(&page).len(),2);
    let fixed=node(&doc,"fixed");let before=page.controls.iter().find(|control|control.node==fixed).unwrap().y;
    page.set_scroll(75);assert_eq!(sources(&page).len(),2);assert_eq!(page.controls.iter().find(|control|control.node==fixed).unwrap().y,before+75);
    page.set_scroll(0);assert_eq!(page.controls.iter().find(|control|control.node==fixed).unwrap().y,before);
}

#[test]
fn source_selection_obeys_the_existing_native_control_count_limit() {
    let markup=(0..forms::MAX_CONTROLS+8).map(|i|format!("<button id=b{i}>X</button>")).collect::<String>();
    let doc=document(&markup,"button{background-image:url(Bounded.ppm)}");let page=page(&doc,None);
    assert_eq!(page.controls.len(),forms::MAX_CONTROLS);assert_eq!(sources(&page).len(),forms::MAX_CONTROLS);
    assert!(!sources(&page).contains_key(&node(&doc,&format!("b{}",forms::MAX_CONTROLS))));
}
