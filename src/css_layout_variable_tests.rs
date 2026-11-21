//! Shared existing layout grammars must consume real substituted values.
use super::*;
fn styles(css: &str, body: &str) -> (Document, Vec<Style>) {
    let d = crate::html::parse(&format!(
        "<html><head><style>{css}</style></head><body>{body}</body></html>"
    ));
    let s = compute(&d, &HashMap::new());
    (d, s)
}
fn at<'a>(d: &Document, s: &'a [Style], id: &str) -> &'a Style {
    &s[(0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some(id))
        .unwrap()]
}
#[test]
fn layout_variable_sizing_display_flex_and_gaps_share_supported_literal_grammars() {
    let(d,s)=styles("#theme{--display:flex;--width:260px;--min:30px;--max:400px;--minh:20px;--maxh:90px;--basis:25%;--grow:2;--shrink:3;--gap:8px 12px;--box:border-box;--flow:row wrap}#row{display:var(--display);width:var(--width);min-width:var(--min);max-width:var(--max);min-height:var(--minh);max-height:var(--maxh);box-sizing:var(--box);gap:var(--gap);flex-flow:var(--flow)}#child{flex-grow:var(--grow);flex-shrink:var(--shrink);flex-basis:var(--basis)}#short{--flex:2 3 60px;flex:var(--flex)}","<div id=theme><div id=row><div id=child></div><div id=short></div></div></div>");
    let row = at(&d, &s, "row");
    assert_eq!(row.display, Display::Flex);
    assert_eq!(row.width, Length::Px(260.0));
    assert_eq!(row.min_width, Length::Px(30.0));
    assert_eq!(row.max_width, Some(Length::Px(400.0)));
    assert_eq!(row.min_height, Length::Px(20.0));
    assert_eq!(row.max_height, Some(Length::Px(90.0)));
    assert!(row.border_box);
    assert_eq!(row.gap, [Length::Px(8.0), Length::Px(12.0)]);
    assert!(row.flex_wrap);
    let child = at(&d, &s, "child");
    assert_eq!(child.flex_grow, 2.0);
    assert_eq!(child.flex_shrink, 3.0);
    assert_eq!(child.flex_basis, Length::Percent(25.0));
    let short = at(&d, &s, "short");
    assert_eq!(
        (short.flex_grow, short.flex_shrink, short.flex_basis),
        (2.0, 3.0, Length::Px(60.0))
    );
}
#[test]
fn layout_variable_invalid_computed_values_reset_without_reviving_lower_cascade_or_joining_tokens()
{
    let(d,s)=styles("#parent{width:200px;min-width:25px;max-width:300px;--number:12;--bad:-2;--cycle:var(--cycle)}#bad{display:none;display:var(--missing);width:30px;width:var(--number)px;max-width:50px;max-width:var(--cycle);flex:2 3 40px;flex:var(--missing);gap:10px;gap:var(--bad)}#inherit{width:var(--missing,inherit);min-width:var(--missing,inherit);max-width:var(--missing,inherit)}#none{max-width:var(--missing,none);max-height:var(--missing,none)}#literal{width:33px;width:12 px}","<div id=parent><div id=bad></div><div id=inherit></div><div id=none></div><div id=literal></div></div>");
    let bad = at(&d, &s, "bad");
    assert_eq!(bad.display, Display::Inline);
    assert_eq!(bad.width, Length::Auto);
    assert_eq!(bad.max_width, None);
    assert_eq!(
        (bad.flex_grow, bad.flex_shrink, bad.flex_basis),
        (0.0, 1.0, Length::Auto)
    );
    assert_eq!(bad.gap, [Length::Px(0.0); 2]);
    let inherited = at(&d, &s, "inherit");
    assert_eq!(inherited.width, Length::Px(200.0));
    assert_eq!(inherited.min_width, Length::Px(25.0));
    assert_eq!(inherited.max_width, Some(Length::Px(300.0)));
    assert_eq!(at(&d, &s, "none").max_height, None);
    assert_eq!(at(&d, &s, "literal").width, Length::Px(33.0));
}

#[test]
fn layout_variable_inherited_relative_sizing_uses_parent_computed_metrics() {
    let(d,s)=styles("html{font-size:10px}#parent{font-size:20px;width:2em;min-width:1em;max-width:3em;height:4rem;gap:1em;flex-basis:3em}#child{font-size:30px;width:var(--missing,inherit);min-width:inherit;max-width:inherit;height:inherit;gap:var(--missing,inherit);flex-basis:inherit}","<div id=parent><div id=child></div></div>");
    let child = at(&d, &s, "child");
    assert_eq!(child.width, Length::Px(40.0));
    assert_eq!(child.min_width, Length::Px(20.0));
    assert_eq!(child.max_width, Some(Length::Px(60.0)));
    assert_eq!(child.height, Length::Px(40.0));
    assert_eq!(child.gap, [Length::Px(20.0); 2]);
    assert_eq!(child.flex_basis, Length::Px(60.0));
}
