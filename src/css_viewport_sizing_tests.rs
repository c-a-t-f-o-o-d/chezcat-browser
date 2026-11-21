//! Viewport lengths use the actual CSS media environment, never a default size.
use super::*;
fn computed(css: &str, width: f32, height: f32) -> (Document, Vec<Style>) {
    let mut d = crate::html::parse(&format!(
        "<html><head><style>{css}</style></head><body><div id=target></div></body></html>"
    ));
    d.media_environment = Some(MediaEnvironment {
        width: width.into(),
        height: height.into(),
        hover: Hover::Hover,
    });
    let s = compute(&d, &HashMap::new());
    (d, s)
}
fn at<'a>(d: &Document, s: &'a [Style]) -> &'a Style {
    &s[(0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some("target"))
        .unwrap()]
}
#[test]
fn viewport_sizing_literals_and_math_use_real_width_height_with_variable_and_font_context() {
    for (w, h) in [(800.0, 600.0), (1200.0, 900.0)] {
        let(d,s)=computed("#target{font-size:20px;--gutter:2vw;width:calc(50vw - var(--gutter) + 10%);height:25vh;min-width:10vw;max-height:calc(50vh - 1em);padding:1vw 2vh;gap:calc(1vw + 2px);flex-basis:20vw}",w,h);
        let s = at(&d, &s);
        assert_eq!(
            s.width.resolve(200.0, s.font_size, s.root_font_size),
            Some(w * 0.48 + 20.0)
        );
        assert_eq!(
            s.height.resolve(200.0, s.font_size, s.root_font_size),
            Some(h * 0.25)
        );
        assert_eq!(s.min_width.resolve(200.0, 20.0, 16.0), Some(w * 0.1));
        assert_eq!(
            s.max_height.unwrap().resolve(200.0, 20.0, 16.0),
            Some(h * 0.5 - 20.0)
        );
        assert_eq!(s.padding[0].resolve(200.0, 20.0, 16.0), Some(w * 0.01));
        assert_eq!(s.padding[1].resolve(200.0, 20.0, 16.0), Some(h * 0.02));
        assert_eq!(s.gap[0].resolve(200.0, 20.0, 16.0), Some(w * 0.01 + 2.0));
        assert_eq!(s.flex_basis.resolve(200.0, 20.0, 16.0), Some(w * 0.2));
    }
}
#[test]
fn viewport_sizing_without_environment_does_not_fabricate_a_viewport_and_bad_tokens_stay_invalid() {
    for invalid in ["1.vw", "NaNvh", "2 vw", "1e+vh", "2\u{a0}vw"] {
        assert_eq!(
            sizing_math::viewport_literal(invalid, true),
            None,
            "{invalid}"
        );
    }
    let d = crate::html::parse(
        "<style>#target{width:50vw;height:calc(25vh + 2px)}</style><div id=target></div>",
    );
    let s = compute(&d, &HashMap::new());
    let s = at(&d, &s);
    assert_eq!(s.width.resolve(200.0, 16.0, 16.0), None);
    assert_eq!(s.height.resolve(200.0, 16.0, 16.0), None);
    let(d,s)=computed("#target{width:33px;width:2 vw;height:20px;height:-1vh;--number:2;min-width:30px;min-width:var(--number)vw}",800.0,600.0);
    let s = at(&d, &s);
    assert_eq!(s.width, Length::Px(33.0));
    assert_eq!(s.height, Length::Px(20.0));
    assert_eq!(s.min_width, Length::Auto);
}
