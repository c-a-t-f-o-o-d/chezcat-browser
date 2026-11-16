//! Original finite responsive CSS controls; no upstream stylesheet or renderer.
use super::*;

fn env(width:f64,height:f64)->MediaEnvironment {MediaEnvironment {width,height,hover:Hover::Hover}}
fn id(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}

#[test]
fn responsive_media_em_is_initial_ua_font_not_author_root_body_or_physical_zoom() {
    let mut doc=crate::html::parse("<html><head><style>html{font-size:32px}body{font-size:18px}#target{width:13px}@media print,screen and (min-width:64em){#target{width:91px}}</style></head><body><div id=target>Original</div></body></html>");
    let target=id(&doc,"target");
    for (width,expected) in [(1023.2,13.0),(1023.999,13.0),(1024.0,91.0),(1280.0,91.0)] {
        doc.media_environment=Some(env(width,760.0));let styles=compute(&doc,&HashMap::new());
        assert_eq!(styles[target].width,Length::Px(expected),"CSS width {width}");
        let html=doc.nodes.iter().position(|n|matches!(&n.kind,NodeKind::Element {tag,..} if tag=="html")).unwrap();
        assert_eq!(styles[html].font_size,32.0,"Author font change must be real, but not the media basis");
    }
    assert_eq!(Style::default().font_size,INITIAL_FONT_SIZE);
    assert!(media::matches("(min-width:6.4e1EM)",Some(env(1024.0,760.0))));
    let zoomed=MediaEnvironment::desktop_viewport(1280,760,1.25);
    assert_eq!(zoomed.width,1024.0);assert!(media::matches("(min-width:64em)",Some(zoomed)));
    assert!(!media::matches("(min-width:64em)",Some(MediaEnvironment::desktop_viewport(1279,760,1.25))));
}

#[test]
fn responsive_media_em_keeps_unknown_syntax_overflow_negation_and_axis_boundaries() {
    let context=Some(env(640.25,480.0));
    assert!(media::matches("(width:40.015625em) and (height:30em)",context));
    assert!(media::matches("(max-height:30em)",context));
    assert!(media::matches("not screen and (width:64em)",context),"Known false feature can be negated");
    for source in ["not screen and (width:64rem)","not screen and (width:1e308em)","not screen and (width:64 em)","not screen and (width:64emx)","screen and(width:40em)","(width:NaNem)","(width:Infinityem)","(width:40em) trailing"] {
        assert!(!media::matches(source,context),"Unknown/malformed query must not broaden: {source}");
    }
    assert!(!media::matches("(min-width:1em)",None));
    assert!(!media::matches("not screen and (min-width:1em)",None));
    let small=StylesheetLimits {rules:2,..StylesheetLimits::default()};
    assert!(small.accept("@media (min-width:64em){a{color:red}b{color:blue}}",StylesheetUsage::default()).is_err(),"Inactive descendants still consume whole-sheet block admission");
}

#[test]
fn responsive_style_pseudo_targets_keep_atomic_lists_without_ordinary_matches_or_specificity() {
    let doc=crate::html::parse("<html><head><style>.target{width:13px;background:#008844}.target,::before,::after{background:#aaccee}#absent::before,.target{color:#112233}.target.strong{color:#445566}.target::before,.target::after{width:9000px;background:red;font-family:NeverSelect}.target,::original-unsupported{background:red}</style></head><body><div id=target class='target strong'>Original</div><div id=other>Other</div></body></html>");
    let styles=compute(&doc,&HashMap::new());let target=id(&doc,"target");let other=id(&doc,"other");
    assert_eq!(styles[target].background,Some(0xaaccee));assert_eq!(styles[target].width,Length::Px(13.0));
    assert_eq!(styles[target].color,0x445566,"A nonmatching pseudo alternative cannot raise another alternative's specificity");
    assert_eq!(styles[target].font_families,vec!["serif".to_string()]);assert_eq!(styles[other].background,None);
    let selectors=parse_selector_list("*,::before,::AFTER",true).unwrap();
    assert_eq!(selectors.iter().map(|s|s.target).collect::<Vec<_>>(),[SelectorTarget::Element,SelectorTarget::Before,SelectorTarget::After]);
    assert!(!valid_query_selector("div,::before"),"DOM query pseudo-target boundary remains explicitly unsupported");
    assert!(valid_query_selector("[data-value='::before']"),"Quoted attribute data is not a target token");
}

#[test]
fn responsive_pseudo_selector_suffixes_unknown_lists_and_existing_caps_remain_atomic() {
    for source in [".target,::unknown",".target::before > b",".target::before.more",".target::after::before",".target::before[data-x]",".target::before()",".target,","::before,"] {
        assert!(parse_selector_list(source,true).is_none(),"No prefix success for {source}");
    }
    let sixteen=std::iter::once("*").chain(std::iter::repeat_n("::before",15)).collect::<Vec<_>>().join(",");
    assert_eq!(parse_selector_list(&sixteen,true).unwrap().len(),16);
    assert!(parse_selector_list(&(sixteen+",::after"),true).is_none());
    assert!(parse_selector(&format!("{}::before","a".repeat(MAX_SELECTOR_BYTES))).is_none());
    assert!(parse_selector_list("div:before",true).is_none(),"Legacy single-colon aliases are outside this double-colon slice");
}

#[test]
fn responsive_box_sizing_explicit_inherit_initial_unset_preserve_noninherited_default() {
    let doc=crate::html::parse("<html><head><style>#parent{box-sizing:border-box}#inherited,#grand{box-sizing:inherit}#initial{box-sizing:initial}#unset{box-sizing:unset}#invalid{box-sizing:border-box;box-sizing:inherit wrong}#important{box-sizing:content-box;box-sizing:inherit!important}#after{box-sizing:inherit;box-sizing:content-box}</style></head><body><div id=parent><div id=normal></div><div id=inherited><div id=grand></div><div id=plain-grand></div></div><div id=initial></div><div id=unset></div><div id=invalid></div><div id=important></div><div id=after></div></div></body></html>");
    let styles=compute(&doc,&HashMap::new());
    for (name,expected) in [("parent",true),("normal",false),("inherited",true),("grand",true),("plain-grand",false),("initial",false),("unset",false),("invalid",true),("important",true),("after",false)] {
        assert_eq!(styles[id(&doc,name)].border_box,expected,"{name}");
    }
}
