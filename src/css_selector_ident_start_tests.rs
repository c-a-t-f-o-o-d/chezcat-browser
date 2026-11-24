//! Original shared selector identifier-start checks.
use super::*;
#[test]
fn selector_ident_start_non_ascii_supported_positions_keep_exact_spelling(){
    for name in ["猫","😺","É","é","\u{80}","\u{301}","�","-猫","--ready"] {
        for selector in [name.to_owned(),format!(".{name}"),format!("#{name}"),format!("[{name}]"),format!("[data-x={name}]")] {
            assert!(parse_selector(&selector).is_some(),"{selector:?}");
        }
    }
    let parsed=parse_selector("猫.😺#É[名字=é]").unwrap();
    assert_eq!(parsed.parts[0].tag.as_deref(),Some("猫"));assert_eq!(parsed.parts[0].classes,["😺"]);assert_eq!(parsed.parts[0].ids,["É"]);
}
#[test]
fn selector_ident_start_native_dom_exact_case_normalization_and_cached_cascade(){
    let mut doc=crate::html::parse("<!doctype html><style>.😺{width:44px}.猫{color:#654321}#É{background:#123456}</style><p id=É class='😺 猫 é é' 名字=é></p>");
    let node=(0..doc.nodes.len()).find(|&n|doc.attr(n,"id")==Some("É")).unwrap();
    for selector in [".😺.猫#É[名字=é]",".é",".é",":not(.😸).猫"] {let mut work=100_000;assert_eq!(query_first(&doc,selector,&[node],None,&mut work).unwrap(),Some(node),"{selector}");}
    for selector in [".É","#é",".😸","[名字=É]"] {let mut work=100_000;assert_eq!(query_first(&doc,selector,&[node],None,&mut work).unwrap(),None,"{selector}");}
    let sources=HashMap::new();let first=compute_with_environment_and_properties_cached(&doc,&sources,None,None,None);
    assert_eq!(first.styles[node].width,Length::Px(44.0));assert_eq!(first.styles[node].color,0x654321);assert_eq!(first.styles[node].background,Some(0x123456));
    let NodeKind::Element{attrs,..}=&mut doc.nodes[node].kind else{unreachable!()};attrs.iter_mut().find(|a|a.name.html_is("class")).unwrap().value="😸".into();
    let next=compute_with_environment_and_properties_cached(&doc,&sources,None,None,first.stylesheet_cache.as_ref());assert_eq!(next.styles[node].width,Length::Auto);assert_ne!(next.styles[node].color,0x654321);assert_eq!(next.styles[node].background,Some(0x123456));
}
#[test]
fn selector_ident_start_invalid_prefixes_escapes_and_whole_lists_remain_refused(){
    for selector in [".1猫","#1猫","-1猫",".-","[1猫]","[x=1猫]",".猫\\31",".\\732b",".猫, .1bad",".猫 >",".猫:unknown"] {assert!(parse_selector_list(selector,false).is_none(),"{selector}");}
    assert!(parse_selector(".-猫").is_some());assert!(parse_selector(".--猫").is_some());
}
#[test]
fn selector_ident_start_admission_refusal_preserves_dom_and_recovers(){
    let doc=crate::html::parse("<!doctype html><p class=😺></p>");let before=format!("{:?}",doc.nodes);let roots:Vec<_>=(0..doc.nodes.len()).collect();
    let mut work=0;assert!(query_first(&doc,".😺",&roots,None,&mut work).is_err());assert_eq!(work,0);assert_eq!(format!("{:?}",doc.nodes),before);
    let mut work=100_000;assert!(query_first(&doc,".😺",&roots,None,&mut work).unwrap().is_some());assert!(work<100_000);
    assert!(parse_selector(&format!(".{}","猫".repeat(MAX_SELECTOR_BYTES))).is_none());
    let (mut work,mut copies)=(100_000,100_000);
    let error=query_all_with_storage(&doc,".😺",&roots,None,&mut work,0,None,&mut copies).unwrap_err();
    assert!(error.contains("retained selector result allowance"));assert!(work<100_000);assert_eq!(format!("{:?}",doc.nodes),before);
    let (mut work,mut copies)=(100_000,100_000);
    assert_eq!(query_all_with_storage(&doc,".😺",&roots,None,&mut work,1024,None,&mut copies).unwrap().len(),1);
}
