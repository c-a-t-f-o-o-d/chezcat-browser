//! Original typed sizing arithmetic through the real cascade and resolution.
use super::*;
fn style(rule: &str) -> Style {
    let d=crate::html::parse(&format!("<html><head><style>html{{font-size:16px}}#target{{font-size:20px;{rule}}}</style></head><body><div id=target></div></body></html>"));
    let s = compute(&d, &HashMap::new());
    s[(0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some("target"))
        .unwrap()]
    .clone()
}
#[test]
fn sizing_math_typed_sums_products_division_and_variables_resolve_against_real_context() {
    for (value, expected) in [
        ("calc(50% - 20px)", 80.0),
        ("calc((1em + 2rem) * 2)", 104.0),
        ("calc(120px / 3 + 10%)", 60.0),
        ("calc(2 * calc(10px + 5%))", 40.0),
        ("calc(-20px)", 0.0),
    ] {
        let s = style(&format!("width:{value}"));
        assert_eq!(
            s.width.resolve(200.0, s.font_size, s.root_font_size),
            Some(expected),
            "{value}"
        );
    }
    let s=style("--guide:24px;width:calc(100% - var(--guide));height:calc(2rem + 1em);min-width:calc(10% + 4px);max-width:calc(100% - 2px);gap:calc(2px + 1em)");
    assert_eq!(s.width.resolve(200.0, 20.0, 16.0), Some(176.0));
    assert_eq!(s.height.resolve(200.0, 20.0, 16.0), Some(52.0));
    assert_eq!(s.min_width.resolve(200.0, 20.0, 16.0), Some(24.0));
    assert_eq!(s.max_width.unwrap().resolve(200.0, 20.0, 16.0), Some(198.0));
    assert_eq!(s.gap[0].resolve(200.0, 20.0, 16.0), Some(22.0));
}
#[test]
fn sizing_math_dimensional_and_token_refusals_preserve_literal_cascade() {
    for value in [
        "calc(0 + 2px)",
        "calc(2px * 3px)",
        "calc(20px / 0)",
        "calc(1px+2px)",
        "calc(1px +2px)",
        "calc(2 * 3 px)",
        "calc(1e999 * 2px)",
        "calc(2px + auto)",
    ] {
        let s = style(&format!("width:30px;width:{value}"));
        assert_eq!(s.width, Length::Px(30.0), "{value}");
    }
    let s = style("--number:2;width:30px;width:calc(var(--number)px + 1px)");
    assert_eq!(
        s.width,
        Length::Auto,
        "substitution must not join tokens or revive a lower winner"
    );
}

#[test]
fn sizing_math_shorthands_preserve_grouped_components_and_percentage_context() {
    let s=style("flex:1 0 calc(50% - 10px);gap:calc(1em + 2px) calc(10% + 1px);padding:calc(2px + 1em);margin:calc(5% - 8px)");
    assert_eq!(s.flex_basis.resolve(200.0, 20.0, 16.0), Some(90.0));
    assert_eq!(s.gap[0].resolve(200.0, 20.0, 16.0), Some(22.0));
    assert_eq!(s.gap[1].resolve(200.0, 20.0, 16.0), Some(21.0));
    assert_eq!(s.padding[0].resolve(200.0, 20.0, 16.0), Some(22.0));
    assert_eq!(s.margin[0].resolve(100.0, 20.0, 16.0), Some(-3.0));
    assert!(
        style("height:calc(100% - 100% + 1px)")
            .height
            .has_percentage(),
        "zero percentage terms retain their containing-block context"
    );
}
