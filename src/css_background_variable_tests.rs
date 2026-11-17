//! Original finite background variable, computed-value and resource controls.
use super::*;

fn compute_styles(source:&str,body:&str)->(Document,Vec<Style>) {
    let doc=crate::html::parse(&format!("<html><head><style id=sheet>{source}</style></head><body>{body}</body></html>"));
    let result=compute(&doc,&HashMap::new());(doc,result)
}
fn node(doc:&Document,name:&str)->usize {
    (0..doc.nodes.len()).find(|&node|doc.attr(node,"id")==Some(name)).unwrap()
}
fn at<'a>(doc:&Document,styles:&'a[Style],name:&str)->&'a Style {&styles[node(doc,name)]}

#[test]
fn background_variable_cascade_case_fallback_and_inherited_aliases_stay_real() {
    let(doc,styles)=compute_styles(
        "#p{--Fill:#0d6efd;--fill:#b02070;--alias:var(--Fill);--Priority:#111111!important}#a{background-color:var(--Fill)}#b{background:var(--fill)}#c{background-color:var(--later);--later:rgb(1, 2, 3)}#d{--Fill:#112233;background-color:var(--alias)}#e{--Priority:#445566;background-color:var(--Priority)}#f{background-color:var(--FILL,#010203)}",
        "<div id=p><div id=a></div><div id=b></div><div id=c></div><div id=d></div><div id=e></div><div id=f></div></div>");
    for(name,color)in[("a",0x0d6efd),("b",0xb02070),("c",0x010203),("d",0x0d6efd),("e",0x445566),("f",0x010203)] {
        assert_eq!(at(&doc,&styles,name).background,Some(color),"{name}");
    }
}

#[test]
fn background_variable_currentcolor_uses_final_color_and_inherits_the_keyword() {
    let(doc,styles)=compute_styles(
        "#p{color:#102030;--Paint:currentColor;background-color:var(--Paint);--Pad:5px}#a{background-color:var(--Paint);color:#406080;padding:var(--Pad);border:2px solid currentColor}#b{color:#406080;background:var(--Paint);border:2px solid rgba(0,0,0,.2)}#c{color:#abcdef;background-color:var(--missing,inherit)}#d{color:#abcdef;background:var(--missing,inherit)}#e{color:#abcdef;background-color:currentColor}#f{background:currentColor;color:#010203}#g{color:#abcdef;background-color:inherit}#h{color:#abcdef;background:inherit}#literal-parent{color:#102030;background-color:#102030}#literal-child{color:#abcdef;background:var(--missing,inherit)}#override{background-color:var(--Paint);background-color:#102030;color:#abcdef}#reset{background-color:var(--Paint);background-color:var(--missing)}",
        "<div id=p><div id=a></div><div id=b></div><div id=c></div><div id=d></div><div id=e></div><div id=f></div><div id=g></div><div id=h></div><div id=override></div><div id=reset></div></div><div id=literal-parent><div id=literal-child></div></div>");
    for(name,color)in[("p",0x102030),("a",0x406080),("b",0x406080),("c",0xabcdef),("d",0xabcdef),("e",0xabcdef),("f",0x010203),("g",0xabcdef),("h",0xabcdef),("literal-child",0x102030),("override",0x102030)] {
        assert_eq!(at(&doc,&styles,name).background,Some(color),"{name}");
    }
    for name in ["p","a","b","c","d","e","f","g","h"] {assert!(at(&doc,&styles,name).background_current_color,"{name}");}
    for name in ["literal-parent","literal-child","override","reset"] {assert!(!at(&doc,&styles,name).background_current_color,"{name}");}
    assert_eq!(at(&doc,&styles,"reset").background,None);
    let a=at(&doc,&styles,"a");assert_eq!(a.padding,[Length::Px(5.0);4]);assert_eq!(a.border_color,[None;4]);assert_eq!(a.border_alpha,[255;4]);
    let b=at(&doc,&styles,"b");assert_eq!(b.border_color,[Some(0);4]);assert_eq!(b.border_alpha,[51;4]);
    let(doc,styles)=compute_styles(
        "#p{color:#123456;--paint:currentColor}#plain-a,#a{color:red;color:currentColor}#plain-b,#b{color:red;color:inherit}#plain-c,#c{color:red;color:unset}#plain-d,#d{color:red;color:initial}#a,#b,#c,#d{background-color:var(--paint)}",
        "<div id=p><div id=plain-a></div><div id=a></div><div id=plain-b></div><div id=b></div><div id=plain-c></div><div id=c></div><div id=plain-d></div><div id=d></div></div>");
    for(reference,target)in[("plain-a","a"),("plain-b","b"),("plain-c","c"),("plain-d","d")] {
        let expected=at(&doc,&styles,reference).color;let actual=at(&doc,&styles,target);
        assert_eq!(actual.color,expected,"Existing foreground policy stays unchanged: {target}");
        assert_eq!(actual.background,Some(expected),"{target}");
    }
}

#[test]
fn background_variable_invalid_computed_values_do_not_revive_lower_declarations() {
    let(doc,styles)=compute_styles(
        "#p{background-color:green}#a{background-color:red;background-color:var(--missing)}#b{background-color:var(--missing);background-color:#445566}#c{background-color:var(--missing)!important;background-color:red}#d{--bad:rgba(0,0,0,.2);background-color:blue;background-color:var(--bad)}#e{background-color:var(--missing,initial)}#f{background-color:var(--missing,unset)}#g{--empty:;background-color:red;background-color:var(--empty,blue)}#h{--pair:#112233 #445566;background-color:red;background-color:var(--pair)}",
        "<div id=p><div id=a></div><div id=b></div><div id=c></div><div id=d></div><div id=e></div><div id=f></div><div id=g></div><div id=h></div></div>");
    for name in ["a","c","d","e","f","g","h"] {assert_eq!(at(&doc,&styles,name).background,None,"{name}");}
    assert_eq!(at(&doc,&styles,"b").background,Some(0x445566));
}

#[test]
fn background_variable_cycles_and_unused_fallback_dependencies_keep_existing_rules() {
    let(doc,styles)=compute_styles(
        "#a{--x:var(--x);background:var(--x,#112233)}#b{--a:var(--b);--b:var(--a);background-color:var(--a,#445566)}#c{--ready:green;--x:var(--ready,var(--x));background-color:var(--x,#778899)}#d{--bad:var(--missing);--good:var(--bad,#aabbcc);background-color:var(--good)}",
        "<div id=a></div><div id=b></div><div id=c></div><div id=d></div>");
    for(name,color)in[("a",0x112233),("b",0x445566),("c",0x778899),("d",0xaabbcc)] {assert_eq!(at(&doc,&styles,name).background,Some(color),"{name}");}
}

#[test]
fn background_variable_color_and_shorthand_preserve_the_single_layer_resource_boundary() {
    let(doc,styles)=compute_styles(
        "#p{--Paint:#0d6efd;--Url:url(UnrequestedCase.ppm);background-color:red;background-image:url(ParentCase.ppm);background-repeat:repeat-x;background-position:right bottom;background-size:cover}.layer{background-color:red;background-image:url(OwnCase.ppm);background-repeat:no-repeat;background-position:center;background-size:contain}#color{background-color:var(--Paint)}#short{background:var(--Paint)}#bad{background:var(--Url)}#badcolor{background-color:var(--Url)}#inherit{background:var(--missing,inherit)}#image{background-image:var(--Url)}",
        "<div id=p><div id=color class=layer></div><div id=short class=layer></div><div id=bad class=layer></div><div id=badcolor class=layer></div><div id=inherit></div><div id=image class=layer></div></div>");
    let color=at(&doc,&styles,"color");assert_eq!(color.background,Some(0x0d6efd));
    let own=BackgroundImage {url:"OwnCase.ppm".into(),stylesheet:Some(node(&doc,"sheet"))};
    assert_eq!(color.background_image,Some(own.clone()));assert_eq!(color.background_repeat,[false;2]);assert_eq!(color.background_position,[ObjectPosition::CENTER;2]);assert_eq!(color.background_size,BackgroundSize::Contain);
    for name in ["short","bad"] {
        let style=at(&doc,&styles,name);assert_eq!(style.background_image,None,"{name}");assert_eq!(style.background_repeat,[true;2]);assert_eq!(style.background_position,[ObjectPosition::START;2]);assert_eq!(style.background_size,BackgroundSize::Auto);
    }
    assert_eq!(at(&doc,&styles,"short").background,Some(0x0d6efd));assert_eq!(at(&doc,&styles,"bad").background,None);
    assert_eq!(at(&doc,&styles,"badcolor").background,None);assert_eq!(at(&doc,&styles,"badcolor").background_image,Some(own.clone()));
    let parent=at(&doc,&styles,"p");let inherited=at(&doc,&styles,"inherit");
    assert_eq!(inherited.background,parent.background);assert_eq!(inherited.background_image,parent.background_image);assert_eq!(inherited.background_repeat,parent.background_repeat);assert_eq!(inherited.background_position,parent.background_position);assert_eq!(inherited.background_size,parent.background_size);
    assert_eq!(at(&doc,&styles,"image").background_image,Some(own),"background-image var syntax is not admitted by this color-only slice");
}
