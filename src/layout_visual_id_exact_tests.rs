//! Native ID consumers preserve exact attribute identity.
use super::*;
fn node(doc:&Document,id:&str)->usize{(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some(id)).unwrap()}
fn record(doc:&mut Document,n:usize,name:&str,units:&[u16]){let NodeKind::Element{attrs,..}=&mut doc.nodes[n].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is(name)).unwrap().value=crate::dom_string::DomString::from_units(units,4096).unwrap();}
#[test]
fn visual_id_exact_all_anchor_paths_use_native_scalar_values_and_skip_raw_aliases(){
    for content in ["<span id='A�z'>target</span>","<div id='A�z'>target</div>","<span id='A�z' style='display:inline-block'>target</span>","<br id='A�z'>","<img id='A�z' width=20 height=20>","<input id='A�z' value=target>","<table><tr id='A�z'><td>target</td></tr></table>"]{
        let mut doc=crate::html::parse(&format!("<!doctype html><html><body>{content}</body></html>"));let n=node(&doc,"A�z");
        assert!(layout(&doc,640).anchors.iter().any(|(id,_)|id=="A�z"),"scalar control {content}");
        record(&mut doc,n,"id",&[65,0xd800,122]);assert!(!layout(&doc,640).anchors.iter().any(|(id,_)|id=="A�z"),"raw alias {content}");
        record(&mut doc,n,"id",&"actual😺".encode_utf16().collect::<Vec<_>>());let page=layout(&doc,640);assert!(page.anchors.iter().any(|(id,_)|id=="actual😺"),"authoritative scalar {content}");assert!(!page.anchors.iter().any(|(id,_)|id=="A�z"));
    }
}
#[test]
fn visual_id_exact_raw_first_anchor_does_not_mask_later_literal_or_fixed_state(){
    let mut doc=crate::html::parse("<!doctype html><style>body{margin:0}.raw{position:fixed;top:0}.literal{margin-top:800px}</style><body><div class=raw id=first>raw</div><div class=literal id=second>literal</div></body>");let first=node(&doc,"first");let second=node(&doc,"second");record(&mut doc,first,"id",&[65,0xd800,122]);record(&mut doc,second,"id",&"A�z".encode_utf16().collect::<Vec<_>>());
    let page=layout(&doc,640);let anchors:Vec<_>=page.anchors.iter().filter(|(id,_)|id=="A�z").collect();assert_eq!(anchors.len(),1);assert!(anchors[0].1>=800);assert!(!page.anchor_is_fixed("A�z"));
}
#[test]
fn visual_id_exact_table_cell_alignment_keeps_anchor_span_and_following_geometry(){
    let mut doc=crate::html::parse("<!doctype html><style>td{vertical-align:bottom}</style><body><table><tr><td id='A�z'><span id=inner>short</span></td><td style='height:100px'>tall</td></tr></table><p id=after>after</p></body>");let cell=node(&doc,"A�z");let scalar=layout(&doc,640);let before_inner=scalar.anchors.iter().find(|(id,_)|id=="inner").unwrap().1;let before_after=scalar.anchors.iter().find(|(id,_)|id=="after").unwrap().1;record(&mut doc,cell,"id",&[65,0xd800,122]);
    let page=layout(&doc,640);assert!(!page.anchors.iter().any(|(id,_)|id=="A�z"));let inner=page.anchors.iter().find(|(id,_)|id=="inner").unwrap().1;let after=page.anchors.iter().find(|(id,_)|id=="after").unwrap().1;assert_eq!(inner,before_inner);assert_eq!(after,before_after);assert!(after>inner);
}
