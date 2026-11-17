//! Original finite font-size variable cascade and computed-unit controls.
use super::*;

fn compute_styles(source:&str,body:&str)->(Document,Vec<Style>) {
    let doc=crate::html::parse(&format!("<html><head><style id=sheet>{source}</style></head><body>{body}</body></html>"));
    let result=compute(&doc,&HashMap::new());(doc,result)
}
fn node(doc:&Document,name:&str)->usize {(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()}
fn at<'a>(doc:&Document,styles:&'a[Style],name:&str)->&'a Style {&styles[node(doc,name)]}
fn root(doc:&Document)->usize {doc.nodes.iter().position(|entry|matches!(&entry.kind,NodeKind::Element{tag,..}if tag=="html")).unwrap()}

#[test]
fn font_size_variable_cascade_case_forward_refs_and_computed_aliases_remain_ordered() {
    let(doc,styles)=compute_styles(
        "html{font-size:20px}#p{font-size:10px;--Size:24px;--size:6px;--Alias:var(--Size);--Priority:11px!important}#a{font-size:var(--Size)}#b{font-size:var(--size)}#c{--Size:31px;font-size:var(--Alias)}#d{font-size:var(--later);--later:1.5em}#e{--Priority:13px;font-size:var(--Priority)}#f{--Size:18px!important;--Size:8px;font-size:var(--Size)}#g{font-size:var(--SIZE,50%)}#h{font-size:var(--Size)!important;font-size:8px}#inherit{font-size:var(--Size)}",
        "<div id=p><div id=a></div><div id=b></div><div id=c></div><div id=d></div><div id=e></div><div id=f style='--Size:9px'></div><div id=g></div><div id=h style='font-size:9px'></div><div id=inherit><span id=child></span></div></div>");
    for(name,size)in[("a",24.0),("b",6.0),("c",24.0),("d",15.0),("e",13.0),("f",18.0),("g",5.0),("h",24.0),("inherit",24.0),("child",24.0)] {assert_eq!(at(&doc,&styles,name).font_size,size,"{name}");}
}

#[test]
fn font_size_variable_iacvt_initial_unset_and_inherit_do_not_revive_author_or_ua_sizes() {
    let(doc,styles)=compute_styles(
        "#p{font-size:20px}#missing{font-size:28px;font-size:var(--missing)}#late{font-size:var(--missing);font-size:24px}#important{font-size:var(--missing)!important;font-size:28px}#empty{--Empty:;font-size:28px;font-size:var(--Empty,24px)}#literal-inherit{font-size:28px;font-size:inherit}#var-inherit{font-size:28px;font-size:var(--missing,inherit)}#literal-unset{font-size:28px;font-size:unset}#var-unset{font-size:28px;font-size:var(--missing,unset)}#literal-initial{font-size:28px;font-size:initial}#var-initial{font-size:28px;font-size:var(--missing,initial)}#heading-invalid{font-size:var(--missing)}",
        "<div id=p><div id=missing></div><div id=late></div><div id=important></div><div id=empty></div><div id=literal-inherit></div><div id=var-inherit></div><div id=literal-unset></div><div id=var-unset></div><div id=literal-initial></div><div id=var-initial></div><h1 id=heading-invalid>Invalid</h1><h1 id=heading-default>Default</h1></div>");
    for name in ["missing","important","empty","literal-inherit","var-inherit","literal-unset","var-unset","heading-invalid"] {assert_eq!(at(&doc,&styles,name).font_size,20.0,"{name}");}
    for name in ["literal-initial","var-initial"] {assert_eq!(at(&doc,&styles,name).font_size,INITIAL_FONT_SIZE,"{name}");}
    assert_eq!(at(&doc,&styles,"late").font_size,24.0);assert_eq!(at(&doc,&styles,"heading-default").font_size,32.0,"Unspecified UA heading size stays unchanged");
}

#[test]
fn font_size_variable_parent_units_and_root_rem_publish_computed_sizes_at_the_right_time() {
    let(doc,styles)=compute_styles(
        "html{--Root:2rem;font-size:var(--Root)}body{font-size:1rem}#p{font-size:20px;--Percent:150%;--Em:2em;--Alias:var(--Em);--Rem:1.5rem}#percent,#percent-child{font-size:var(--Percent)}#em{font-size:var(--Em)}#em-child{font-size:var(--Alias)}#rem{font-size:var(--Rem)}#root-token{font-size:var(--Root)}",
        "<div id=p><div id=percent><span id=percent-child></span></div><div id=em><span id=em-child></span></div><div id=rem></div><div id=root-token></div></div>");
    assert_eq!(styles[root(&doc)].font_size,32.0);assert_eq!(styles[root(&doc)].root_font_size,32.0);
    for(name,size)in[("p",20.0),("percent",30.0),("percent-child",45.0),("em",40.0),("em-child",80.0),("rem",48.0),("root-token",64.0)] {let style=at(&doc,&styles,name);assert_eq!(style.font_size,size,"{name}");assert_eq!(style.root_font_size,32.0,"{name}");}
    for root_value in ["var(--missing)","var(--missing,initial)"] {
        let(doc,styles)=compute_styles(&format!("html{{font-size:80px;font-size:{root_value}}}#rem{{font-size:2rem}}"),"<div id=rem></div>");
        assert_eq!(styles[root(&doc)].font_size,INITIAL_FONT_SIZE);assert_eq!(styles[root(&doc)].root_font_size,INITIAL_FONT_SIZE);assert_eq!(at(&doc,&styles,"rem").font_size,32.0);
    }
}

#[test]
fn font_size_variable_cycles_token_boundaries_and_unsupported_grammars_inherit() {
    let(doc,styles)=compute_styles(
        "#p{font-size:20px}#self{--Size:var(--Size);font-size:var(--Size,24px)}#mutual{--a:var(--b);--b:var(--a);font-size:var(--a,32px)}#unused{--ok:12px;--Size:var(--ok,var(--Size));font-size:var(--Size,26px)}#repair{--bad:var(--missing);--good:var(--bad,14px);font-size:var(--good)}#joined{--n:24;font-size:28px;font-size:var(--n)px}#quoted{--Size:'24px';font-size:var(--Size)}#blank{font-size:var(--missing,)}#malformed{font-size:28px;font-size:var(not-custom)}",
        "<div id=p><div id=self></div><div id=mutual></div><div id=unused></div><div id=repair></div><div id=joined></div><div id=quoted></div><div id=blank></div><div id=malformed></div></div>");
    for(name,size)in[("self",24.0),("mutual",32.0),("unused",26.0),("repair",14.0),("joined",20.0),("quoted",20.0),("blank",20.0),("malformed",28.0)] {assert_eq!(at(&doc,&styles,name).font_size,size,"{name}");}
    for unsupported in ["-1px","24 px","24\u{a0}px","24\u{2003}px","24px 12px","24px/2","24px Geometry","normal","currentColor","calc(1px + 1px)","url(Unrequested.ttf)","1vh"] {
        let(doc,styles)=compute_styles(&format!("#p{{font-size:20px}}#literal{{font-size:28px;font-size:{unsupported}}}#variable{{--Size:{unsupported};font-size:28px;font-size:var(--Size)}}"),"<div id=p><div id=literal></div><div id=variable></div></div>");
        assert_eq!(at(&doc,&styles,"literal").font_size,28.0,"{unsupported}");assert_eq!(at(&doc,&styles,"variable").font_size,20.0,"{unsupported}");assert!(at(&doc,&styles,"variable").background_image.is_none());
    }
}

#[test]
fn font_size_variable_existing_finite_literals_shorthand_order_and_other_components_stay_real() {
    for(value,expected)in[("0",1.0),("999999px",256.0),("xx-small",9.0),("medium",16.0),("large",18.0),("larger",24.0),("smaller",16.0),("1rem",16.0),("2em",40.0),("150%",30.0),("12pt",16.0),("2pc",32.0)] {
        let(doc,styles)=compute_styles(&format!("#p{{font-size:20px}}#literal{{font-size:{value}}}#variable{{--Size:{value};font-size:var(--Size)}}"),"<div id=p><div id=literal></div><div id=variable></div></div>");
        assert_eq!(at(&doc,&styles,"literal").font_size,expected,"{value}");assert_eq!(at(&doc,&styles,"variable").font_size,expected,"{value}");
    }
    let(doc,styles)=compute_styles(
        "#p{--Size:24px;--Pad:2px 3px}#a{font-size:var(--Size);font:italic 700 12px/2 Geometry}#b{font:italic 700 12px/2 Geometry;font-size:var(--Size)}#keep{font:400 19px/2 Geometry;font-size:var(--Size);color:#123456;background:currentColor;background-image:url(KeptCase.ppm);padding:var(--Pad);border:2px solid rgba(0,0,0,.2)}#finite{--All:24px Geometry;--Leading:3;font:400 19px/2 Geometry;font:var(--All);font-size:var(--Size);line-height:var(--Leading)}",
        "<div id=p><div id=a></div><div id=b></div><div id=keep></div><div id=finite></div></div>");
    for(name,size)in[("a",12.0),("b",24.0)] {let style=at(&doc,&styles,name);assert_eq!(style.font_size,size);assert!(style.italic&&style.bold);assert!(matches!(style.line_height,LineHeight::Factor(value)if value==2.0));assert_eq!(style.font_families,vec!["Geometry".to_string()]);}
    let keep=at(&doc,&styles,"keep");assert_eq!(keep.font_size,24.0);assert_eq!(keep.font_families,vec!["Geometry".to_string()]);assert!(matches!(keep.line_height,LineHeight::Factor(value)if value==2.0));assert_eq!(keep.color,0x123456);assert_eq!(keep.background,Some(0x123456));assert!(keep.background_current_color);
    assert_eq!(keep.background_image,Some(BackgroundImage{url:"KeptCase.ppm".into(),stylesheet:Some(node(&doc,"sheet"))}));assert_eq!(keep.padding,[Length::Px(2.0),Length::Px(3.0),Length::Px(2.0),Length::Px(3.0)]);assert_eq!(keep.border_alpha,[51;4]);
    let finite=at(&doc,&styles,"finite");assert_eq!(finite.font_size,24.0);assert!(matches!(finite.line_height,LineHeight::Factor(value)if value==2.0));assert_eq!(finite.font_families,vec!["Geometry".to_string()]);assert_eq!(finite.font_weight,400);
}
