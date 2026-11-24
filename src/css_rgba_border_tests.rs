//! Finite border-only comma RGBA grammar, clamping and computed cascade gates.
use super::*;
fn styles(source:&str)->(Document,Vec<Style>) {
    let doc=crate::html::parse(&format!("<html><head><style>{source}</style></head><body><div id=parent><div id=a></div><div id=b></div><div id=c></div></div></body></html>"));
    let result=compute(&doc,&HashMap::new());(doc,result)
}
fn at<'a>(doc:&Document,styles:&'a[Style],name:&str)->&'a Style {&styles[(0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()]}
#[test]
fn rgba_border_channels_numeric_alpha_clipping_and_opaque_endpoints_are_finite() {
    for(value,expected)in[
        ("rgba(0, 0, 0, 0.2)",(Some(0),51)),
        ("rgba(300,-20,12.4,2)",(Some(0xff000c),255)),
        ("rgba(100%,0%,50%,-1)",(Some(0xff0080),0)),
        ("rgba(0,255,0,0)",(Some(0x00ff00),0)),
        ("rgba(0,255,0,1)",(Some(0x00ff00),255)),
        ("rgba(0,0,255,2e-1)",(Some(0x0000ff),51)),
        ("transparent",(Some(0),0)),("currentcolor",(None,255)),("#123456",(Some(0x123456),255)),
    ] {assert_eq!(border_color_value(value),Some(expected),"{value}");}
    assert_eq!(color("rgba(0,0,0,.2)"),None,"This slice does not silently change other CSS color consumers");
    assert_eq!(color("transparent"),None);
}
#[test]
fn rgba_border_numbers_are_complete_ascii_css_tokens_before_float_conversion() {
    for (text,value) in [(".2",0.2),("+2e-1",0.2),("-.2",-0.2),("12.5",12.5),("1E+2",100.0),("0",0.0)] {
        assert_eq!(rgba_number(text),Some(value),"{text}");
    }
    for text in ["1.","1.e2",".","+","-.","1e","1e+","1e-","--1","NaN","inf","1e999","0x1","1_0","1 2","1\t2","١","\u{a0}1"] {
        assert_eq!(rgba_number(text),None,"{text}");
        assert_eq!(border_color_value(&format!("rgba({text},0,0,.2)")),None,"channel {text}");
        assert_eq!(border_color_value(&format!("rgba(0,0,0,{text})")),None,"alpha {text}");
    }
    assert_eq!(border_color_value("rgba(0,0,0,.2)"),Some((Some(0),51)));
    assert_eq!(border_color_value("rgba(0,0,0,+2e-1)"),Some((Some(0),51)));
    assert_eq!(border_color_value("rgba(1.%,0%,0%,.2)"),None);
    let(doc,styles)=styles("#a{border:4px solid red;border-color:rgba(1.,0,0,.2)}#b{--bad:rgba(1.e2,0,0,.2);border:4px solid red;border-color:var(--bad)}");
    assert_eq!(at(&doc,&styles,"a").border_color,[Some(0xff0000);4]);assert_eq!(at(&doc,&styles,"a").border_alpha,[255;4]);
    assert_eq!(at(&doc,&styles,"b").border_color,[None;4]);assert_eq!(at(&doc,&styles,"b").border_alpha,[255;4]);
}
#[test]
fn rgba_border_functional_tokens_shorthands_and_longhands_publish_alpha_together() {
    assert_eq!(border_parts("4px solid rgba(0, 0, 0, .2)").unwrap(),["4px","solid","rgba(0, 0, 0, .2)"]);
    let(doc,styles)=styles("#a{border:4px solid RGBA(0, 0, 0, .2)}#b{border:2px solid red;border-color:rgba(255,0,0,.2) rgba(0,255,0,.4) transparent currentColor;border-left-color:rgba(0,0,255,.6)}#c{color:#123456;border:3px solid currentColor}");
    let a=at(&doc,&styles,"a");assert_eq!(a.border_width,[Length::Px(4.0);4]);assert_eq!(a.border_solid,[true;4]);assert_eq!(a.border_color,[Some(0);4]);assert_eq!(a.border_alpha,[51;4]);
    let b=at(&doc,&styles,"b");assert_eq!(b.border_color,[Some(0xff0000),Some(0x00ff00),Some(0),Some(0x0000ff)]);assert_eq!(b.border_alpha,[51,102,0,153]);
    let c=at(&doc,&styles,"c");assert_eq!(c.border_color,[None;4]);assert_eq!(c.border_alpha,[255;4]);assert_eq!(c.color,0x123456);
}
#[test]
fn rgba_border_invalid_literal_grammar_never_partially_changes_prior_components() {
    for value in ["rgba(0,0,0)","rgba(0,0,0,)","rgba(0,0,0,.2,1)","rgba(0%,0,0,.2)","rgba(0,0,0,20%)","rgba(nan,0,0,.2)","rgba(0,0,0,inf)","rgba(0,0,0,1e999)","rgba(0 0 0 / .2)","rgba(0,0,0,calc(.2))"] {assert_eq!(border_color_value(value),None,"{value}");}
    for value in ["4px solid rgba(0,0,0,.2", "4px solid rgba(0,0,0,.2))", "2px 3px solid red", "solid solid red", "red blue"] {assert!(parse_border(value).is_none(),"{value}");}
    assert!(border_parts(&"x".repeat(2049)).is_none());
    let(doc,styles)=styles("#a{border:6px solid red;border:4px solid rgba(0,bad,0,.2)}#b{border:6px solid red;border-color:rgba(0,0,0,.2) bad-color}#c{border:6px solid red;border-left-color:rgba(0,0,0,bad)}");
    for name in ["a","b","c"] {let style=at(&doc,&styles,name);assert_eq!(style.border_width,[Length::Px(6.0);4]);assert_eq!(style.border_solid,[true;4]);assert_eq!(style.border_color,[Some(0xff0000);4]);assert_eq!(style.border_alpha,[255;4]);}
}
#[test]
fn rgba_border_variable_substitution_and_computed_invalid_reset_keep_alpha_atomic() {
    let(doc,styles)=styles("#a{--edge:RGBA(100%,0%,0%,.8);border:4px solid var(--edge);border-left:var(--missing)}#b{--bad:rgba(no);border:6px solid red;border-color:var(--bad)}#c{border:6px solid red;border:var(--missing)}");
    let a=at(&doc,&styles,"a");assert_eq!(a.border_alpha,[204,204,204,255]);assert_eq!(a.border_color,[Some(0xff0000),Some(0xff0000),Some(0xff0000),None]);assert_eq!(a.border_solid,[true,true,true,false]);
    let b=at(&doc,&styles,"b");assert_eq!(b.border_width,[Length::Px(6.0);4]);assert_eq!(b.border_solid,[true;4]);assert_eq!(b.border_color,[None;4]);assert_eq!(b.border_alpha,[255;4]);
    let c=at(&doc,&styles,"c");assert_eq!(c.border_width,[Length::Px(3.0);4]);assert_eq!(c.border_solid,[false;4]);assert_eq!(c.border_color,[None;4]);assert_eq!(c.border_alpha,[255;4]);
}
#[test]
fn rgba_border_transparent_and_explicit_inherit_do_not_change_other_color_consumers() {
    let(doc,styles)=styles("#parent{border:2px solid rgba(1,2,3,.4)}#a{border:4px solid transparent}#b{border-color:var(--missing,inherit)}#c{color:red;color:rgba(0,0,0,.2);background:green;background:rgba(0,0,0,.2);border:2px solid currentColor}");
    let a=at(&doc,&styles,"a");assert_eq!(a.border_width,[Length::Px(4.0);4]);assert_eq!(a.border_solid,[true;4]);assert_eq!(a.border_color,[Some(0);4]);assert_eq!(a.border_alpha,[0;4]);
    let b=at(&doc,&styles,"b");assert_eq!(b.border_color,[Some(0x010203);4]);assert_eq!(b.border_alpha,[102;4]);
    let c=at(&doc,&styles,"c");assert_eq!(c.color,0xff0000);assert_eq!(c.background,Some(0x008000));assert_eq!(c.border_color,[None;4]);assert_eq!(c.border_alpha,[255;4]);
}
#[test]
fn rgba_border_direct_color_inherit_and_side_inherit_carry_computed_rgb_and_alpha() {
    let(doc,styles)=styles("#parent{border:2px solid rgba(1,2,3,.4)}#a{border-color:inherit;border-left-color:currentColor}#b{border-left-color:inherit}#c{border-color:rgba(7,8,9,.2);border-color:initial;border-right-color:inherit}");
    let a=at(&doc,&styles,"a");assert_eq!(a.border_color,[Some(0x010203),Some(0x010203),Some(0x010203),None]);assert_eq!(a.border_alpha,[102,102,102,255]);
    let b=at(&doc,&styles,"b");assert_eq!(b.border_color,[None,None,None,Some(0x010203)]);assert_eq!(b.border_alpha,[255,255,255,102]);
    let c=at(&doc,&styles,"c");assert_eq!(c.border_color,[None,Some(0x010203),None,None]);assert_eq!(c.border_alpha,[255,102,255,255]);
    let(doc,styles)=self::styles("#parent{color:#123456;border-color:currentColor}#a{color:blue;border-color:inherit}#b{border-color:transparent;border-color:unset}");
    assert_eq!(at(&doc,&styles,"a").border_color,[None;4]);assert_eq!(at(&doc,&styles,"a").border_color.map(|rgb|rgb.unwrap_or(at(&doc,&styles,"a").color)),[0x0000ff;4]);assert_eq!(at(&doc,&styles,"a").border_alpha,[255;4]);
    assert_eq!(at(&doc,&styles,"b").border_color,[None;4]);assert_eq!(at(&doc,&styles,"b").border_alpha,[255;4]);
}
