//! Original finite opaque foreground cascade and inherited computed-value controls.
use super::*;

fn compute_styles(source:&str,body:&str)->(Document,Vec<Style>) {
    let doc=crate::html::parse(&format!("<html><head><style id=sheet>{source}</style></head><body>{body}</body></html>"));
    let result=compute(&doc,&HashMap::new());(doc,result)
}
fn node(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}
fn at<'a>(doc:&Document,styles:&'a[Style],name:&str)->&'a Style {&styles[node(doc,name)]}

#[test]
fn color_variable_cascade_case_forward_refs_and_computed_alias_inheritance() {
    let(doc,styles)=compute_styles(
        "#p{--Paint:#ffffff;--paint:#b02070;--alias:var(--Paint);--Priority:#111111!important}#a{color:var(--Paint)}#b{color:var(--paint)}#c{color:var(--later);--later:rgb(1, 2, 3)}#d{--Paint:#112233;color:var(--alias)}#e{--Priority:#445566;color:var(--Priority)}#f{color:var(--PAINT,#010203)}#g{--Paint:#223344!important;--Paint:#556677;color:var(--Paint)}#h{color:var(--Paint)!important;color:red}",
        "<div id=p><div id=a></div><div id=b></div><div id=c></div><div id=d></div><div id=e></div><div id=f></div><div id=g style='--Paint:#778899'></div><div id=h style='color:blue'></div><div id=inherited style='color:var(--Paint)'><span id=descendant></span></div></div>");
    for(name,color)in[("a",0xffffff),("b",0xb02070),("c",0x010203),("d",0xffffff),("e",0x445566),("f",0x010203),("g",0x223344),("h",0xffffff),("inherited",0xffffff),("descendant",0xffffff)] {
        assert_eq!(at(&doc,&styles,name).color,color,"{name}");
    }
}

#[test]
fn color_variable_computed_invalid_unset_initial_and_currentcolor_use_inherited_policy() {
    let(doc,styles)=compute_styles(
        "#p{color:#112233;--Self:currentColor}#missing{color:red;color:var(--missing)}#late{color:var(--missing);color:#445566}#important{color:var(--missing)!important;color:red}#plain-current{color:red;color:currentColor}#var-current{color:red;color:var(--Self)}#plain-unset{color:red;color:unset}#var-unset{color:red;color:var(--missing,unset)}#plain-inherit{color:red;color:inherit}#var-inherit{color:red;color:var(--missing,inherit)}#plain-initial{color:red;color:initial}#var-initial{color:red;color:var(--missing,initial)}#anchor-unset{color:unset}#anchor-missing{color:var(--missing)}#empty{--Blank:;color:red;color:var(--Blank,white)}",
        "<div id=p><div id=missing></div><div id=late></div><div id=important></div><div id=plain-current></div><div id=var-current></div><div id=plain-unset></div><div id=var-unset></div><div id=plain-inherit></div><div id=var-inherit></div><div id=plain-initial></div><div id=var-initial></div><a id=anchor-unset href=#target>Unset</a><a id=anchor-missing href=#target>Missing</a><a id=anchor-default href=#target>Default</a><div id=empty></div></div>");
    for name in ["missing","important","plain-current","var-current","plain-unset","var-unset","plain-inherit","var-inherit","anchor-unset","anchor-missing","empty"] {
        assert_eq!(at(&doc,&styles,name).color,0x112233,"Inherited foreground, not earlier author red or the UA link color: {name}");
    }
    assert_eq!(at(&doc,&styles,"late").color,0x445566);
    for name in ["plain-initial","var-initial"] {assert_eq!(at(&doc,&styles,name).color,Style::default().color,"{name}");}
    assert_eq!(at(&doc,&styles,"anchor-default").color,0x1569b6,"Unspecified UA link color stays unchanged");
    assert_eq!(Style::default().color,0x26364a,"The existing finite initial palette stays unchanged");
}

#[test]
fn color_variable_cycles_empty_and_token_boundaries_cannot_revive_lower_color() {
    let(doc,styles)=compute_styles(
        "#p{color:#112233}#self{--x:var(--x);color:var(--x,#ffffff)}#mutual{--a:var(--b);--b:var(--a);color:var(--a,#445566)}#unused{--ready:green;--x:var(--ready,var(--x));color:var(--x,#778899)}#repaired{--bad:var(--missing);--good:var(--bad,#aabbcc);color:var(--good)}#joined{--part:#fff;color:red;color:var(--part)fff}#quoted{--paint:'white';color:red;color:var(--paint)}#list{--paint:red blue;color:red;color:var(--paint)}#blank{color:red;color:var(--missing,)}#nested{color:var(--missing,var(--other,rgb(7,8,9)))}#initial-custom{--paint:initial;color:var(--paint,#abcdef)}#malformed{color:red;color:var(not-custom)}",
        "<div id=p><div id=self></div><div id=mutual></div><div id=unused></div><div id=repaired></div><div id=joined></div><div id=quoted></div><div id=list></div><div id=blank></div><div id=nested></div><div id=initial-custom></div><div id=malformed></div></div>");
    for(name,color)in[("self",0xffffff),("mutual",0x445566),("unused",0x778899),("repaired",0xaabbcc),("nested",0x070809),("initial-custom",0xabcdef)] {assert_eq!(at(&doc,&styles,name).color,color,"{name}");}
    for name in ["joined","quoted","list","blank"] {assert_eq!(at(&doc,&styles,name).color,0x112233,"{name}");}
    assert_eq!(at(&doc,&styles,"malformed").color,0xff0000,"Parse-invalid var syntax stays ignored rather than entering computed-value substitution");
}

#[test]
fn color_variable_final_foreground_drives_background_marker_and_preserves_box_resources() {
    let(doc,styles)=compute_styles(
        "#p{color:#112233;--Foreground:#abcdef;--Paint:currentColor;--Pad:6px}#a{background:currentColor;color:var(--Foreground)}#b{color:var(--Foreground);background:currentColor}#c{background-color:var(--Paint);color:red;color:var(--missing)}#d{--Foreground:#445566;background-color:inherit;color:var(--Foreground)}#literal-parent{background:#112233}#literal-child{background:inherit;color:var(--Foreground)}#keep{color:var(--Foreground);font-family:monospace;font-size:19px;font-weight:bold;background-color:#0d6efd;background-image:url(KeptCase.ppm);background-repeat:no-repeat;background-position:right bottom;background-size:contain;padding:var(--Pad);border:2px solid rgba(0,0,0,.2)}",
        "<div id=p><div id=a><div id=d></div></div><div id=b></div><div id=c></div><div id=keep></div><div id=literal-parent><div id=literal-child></div></div></div>");
    for(name,color)in[("a",0xabcdef),("b",0xabcdef),("d",0x445566)] {let style=at(&doc,&styles,name);assert_eq!(style.color,color);assert_eq!(style.background,Some(color));assert!(style.background_current_color,"{name}");}
    assert_eq!(at(&doc,&styles,"c").color,0x112233);assert_eq!(at(&doc,&styles,"c").background,Some(0x112233));
    let literal=at(&doc,&styles,"literal-child");assert_eq!(literal.color,0xabcdef);assert_eq!(literal.background,Some(0x112233));assert!(!literal.background_current_color);
    let keep=at(&doc,&styles,"keep");assert_eq!(keep.color,0xabcdef);assert_eq!(keep.background,Some(0x0d6efd));assert!(!keep.background_current_color);
    assert_eq!(keep.background_image,Some(BackgroundImage{url:"KeptCase.ppm".into(),stylesheet:Some(node(&doc,"sheet"))}));assert_eq!(keep.background_repeat,[false;2]);assert_eq!(keep.background_size,BackgroundSize::Contain);
    assert_eq!(keep.padding,[Length::Px(6.0);4]);assert_eq!(keep.border_width,[Length::Px(2.0);4]);assert_eq!(keep.border_color,[Some(0);4]);assert_eq!(keep.border_alpha,[51;4]);
    assert_eq!(keep.font_families,vec!["monospace".to_string()]);assert_eq!(keep.font_size,19.0);assert!(keep.bold);
}

#[test]
fn color_variable_does_not_admit_alpha_system_font_or_resource_color_grammars() {
    for unsupported in ["rgba(0,0,0,.2)","transparent","hsl(0,0%,0%)","#1234","CanvasText","url(UnrequestedCase.ppm)","red blue","'white'","18px monospace"] {
        let(doc,styles)=compute_styles(&format!("#p{{color:#112233}}#literal{{color:red;color:{unsupported}}}#variable{{--Paint:{unsupported};color:red;color:var(--Paint)}}"),
            "<div id=p><div id=literal></div><div id=variable></div></div>");
        assert_eq!(at(&doc,&styles,"literal").color,0xff0000,"Unsupported literal stays parse-ignored: {unsupported}");
        let variable=at(&doc,&styles,"variable");assert_eq!(variable.color,0x112233,"Var-bearing unsupported grammar computes to inherited foreground: {unsupported}");
        assert_eq!(variable.background_image,None);assert_eq!(variable.font_families,Style::default().font_families);assert_eq!(variable.font_size,INITIAL_FONT_SIZE);
    }
}
