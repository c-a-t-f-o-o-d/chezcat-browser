//! Original computed-style and admission controls; App pixels/network are separate.
use super::*;

fn environment(width: u32, height: u32, hover: Hover) -> MediaEnvironment {
    MediaEnvironment { width:f64::from(width), height:f64::from(height), hover }
}

fn node(doc: &Document, id: &str) -> usize {
    doc.nodes.iter().enumerate()
        .find_map(|(index, _)| (doc.attr(index, "id") == Some(id)).then_some(index))
        .unwrap()
}

fn query_applies(query: &str, environment: MediaEnvironment) -> bool {
    let mut doc = crate::html::parse(&format!(
        "<style>#target{{width:13px}}@media {query}{{#target{{width:91px}}}}</style><div id=target>Original</div>"
    ));
    doc.media_environment = Some(environment);
    let styles = compute(&doc, &HashMap::new());
    styles[node(&doc, "target")].width == Length::Px(91.0)
}

#[test]
fn css_media_types_alternatives_and_physical_pixel_boundaries() {
    let env = environment(640, 480, Hover::Hover);
    for (query, expected) in [
        ("screen", true), ("all", true), ("print", false),
        ("only screen and (width:640px)", true),
        ("not print", true), ("not screen", false),
        ("print, screen and (width:640px)", true),
        ("print, screen and (min-width:641px)", false),
        ("screen and (min-width:640px) and (max-width:640px) and (height:480px)", true),
        ("(min-width:641px)", false), ("(max-width:639px)", false),
        ("(min-height:480px) and (max-height:480px)", true),
        ("(height:481px)", false),
        ("ScReEn AnD (MIN-WIDTH:640PX)", true),
    ] {
        assert_eq!(query_applies(query, env), expected, "{query}");
    }
    let fractional = MediaEnvironment { width:1023.6, height:600.25, hover:Hover::Hover };
    assert!(!query_applies("(min-width:1024px)", fractional));
    assert!(query_applies("(max-width:1023.75px)", fractional));
    assert!(!query_applies("(min-height:600.5px)", fractional));
    assert!(query_applies("(min-width:1024px)", MediaEnvironment { width:1024.0, ..fractional }));
    assert!(!query_applies("(min-width:0px)", MediaEnvironment { width:f64::NAN, ..fractional }));
}

#[test]
fn css_media_real_document_environment_changes_width_height_and_hover() {
    let mut doc = crate::html::parse(
        "<style>#target{width:20px;height:17px;color:#111111;background:#222222}
        @media screen and (min-width:700px){#target{width:113px}}
        @media (min-height:600px){#target{height:47px}}
        @media (hover:hover){#target{color:#13579b}}
        @media (hover:none){#target{background:#2468ac}}
        </style><div id=target>Original</div>"
    );
    let target = node(&doc, "target");
    doc.media_environment = Some(environment(800, 640, Hover::Hover));
    let wide = compute(&doc, &HashMap::new());
    assert_eq!(wide[target].width, Length::Px(113.0));
    assert_eq!(wide[target].height, Length::Px(47.0));
    assert_eq!(wide[target].color, 0x13579b);
    assert_eq!(wide[target].background, Some(0x222222));
    doc.media_environment = Some(environment(500, 400, Hover::None));
    let narrow = compute(&doc, &HashMap::new());
    assert_eq!(narrow[target].width, Length::Px(20.0));
    assert_eq!(narrow[target].height, Length::Px(17.0));
    assert_eq!(narrow[target].color, 0x111111);
    assert_eq!(narrow[target].background, Some(0x2468ac));
    doc.media_environment = Some(environment(800, 640, Hover::Hover));
    assert_eq!(compute(&doc, &HashMap::new())[target].width, wide[target].width);
}

#[test]
fn css_media_unknown_features_stay_nonmatching_under_negation_unlike_media_types() {
    let env = environment(640, 480, Hover::Hover);
    assert!(!query_applies("screen and (original-unknown-feature: yes)", env));
    assert!(!query_applies("not screen and (original-unknown-feature: yes)", env));
    assert!(!query_applies("screen and (hover:unknown)", env));
    assert!(!query_applies("not screen and (hover:unknown)", env));
    assert!(!query_applies("print, (original-unknown-feature: yes)", env));
    assert!(query_applies("not original-unknown-media-type", env));
}

#[test]
fn css_media_malformed_conditions_do_not_apply_a_prefix_or_invert_to_true() {
    let env = environment(640, 480, Hover::Hover);
    for query in [
        "screen and (min-width:640px) garbage",
        "screen and (width:640px) and",
        "not screen and (width:)",
        "not screen and (width:640em)",
        "not screen and (width:12px:extra)",
        "only", "not", "screen and ((width:640px))",
        "not screen and (width:éa)",
        "screen and (width:640\u{a0}px)",
    ] {
        assert!(!query_applies(query, env), "Malformed query granted a rule: {query}");
    }
}

#[test]
fn css_media_nested_rules_keep_source_order_and_external_background_owner() {
    let mut doc = crate::html::parse(
        "<link id=sheet rel=stylesheet href=theme.css><style>
        @media screen and (min-width:700px){#target{color:#abcdef}}
        </style><div id=target>Original</div>"
    );
    let sheet = node(&doc, "sheet");
    let target = node(&doc, "target");
    let external = HashMap::from([(sheet, String::from(
        "#target{width:19px;height:11px;color:#111111;background-image:url('Base/Keep.PNG')}
        @media screen and (min-width:700px){
          #target{width:131px;color:#222222;background-image:url('Media/Active.PNG')}
          @media (max-height:700px){#target{height:53px;width:145px;color:#333333}}
          #target{height:61px;color:#444444}
          @media print{#target{width:999px}}
        }
        #target{color:#555555}"
    ))]);
    doc.media_environment = Some(environment(800, 600, Hover::Hover));
    let active = compute(&doc, &external);
    assert_eq!(active[target].width, Length::Px(145.0), "nested later rule wins over its earlier outer rule");
    assert_eq!(active[target].height, Length::Px(61.0), "outer rule after nested block retains its order");
    assert_eq!(active[target].color, 0xabcdef, "later document sheet retains priority");
    let image = active[target].background_image.as_ref().unwrap();
    assert_eq!(image.url, "Media/Active.PNG");
    assert_eq!(image.stylesheet, Some(sheet));
    doc.media_environment = Some(environment(500, 600, Hover::Hover));
    let inactive = compute(&doc, &external);
    assert_eq!(inactive[target].width, Length::Px(19.0));
    assert_eq!(inactive[target].height, Length::Px(11.0));
    assert_eq!(inactive[target].color, 0x555555);
    assert_eq!(inactive[target].background_image.as_ref().unwrap().url, "Base/Keep.PNG");
    assert_eq!(inactive[target].background_image.as_ref().unwrap().stylesheet, Some(sheet));
}

#[test]
fn css_media_inactive_descendants_are_counted_and_refusal_has_no_applied_prefix() {
    let source = "#target{color:red}@media print{#target{color:green}@media screen{#target{width:99px}}}#target{height:31px}";
    let limits = StylesheetLimits { sheet_bytes:8192, document_bytes:16384, rules:6 };
    let usage = limits.accept(source, StylesheetUsage::default()).unwrap();
    assert_eq!(usage.rules, 6, "containers and inactive descendants all consume the allowance");
    assert_eq!(usage.bytes, source.len());
    let smaller = StylesheetLimits { rules:5, ..limits };
    assert!(smaller.accept(source, StylesheetUsage::default()).is_err());
    let mut doc = crate::html::parse(&format!(
        "<style>{source}</style><style>#target{{color:blue;width:37px}}</style><div id=target>Original</div>"
    ));
    doc.stylesheet_limits = smaller;
    doc.media_environment = Some(environment(800, 600, Hover::Hover));
    let target = node(&doc, "target");
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[target].color, 0x0000ff);
    assert_eq!(styles[target].width, Length::Px(37.0));
    assert_eq!(styles[target].height, Length::Auto, "refused sheet's tail must not be applied");
    assert_eq!(stylesheet_usage(&doc, &HashMap::new()).rules, 1,
        "whole refused source must not consume a later valid sheet's budget");
    assert_eq!(stylesheet_diagnostics(&doc, &HashMap::new()).len(), 1);
    assert_eq!(limits.accept(source, StylesheetUsage::default()).unwrap(), usage,
        "a fresh sufficient policy remains usable after refusal");
}

#[test]
fn css_media_conditional_font_faces_remain_deferred_without_flattening() {
    let source = "@font-face{font-family:Top;src:url('Fonts/Keep.TTF')}
        @media screen{@font-face{font-family:Conditional;src:url('Fonts/Never.TTF')}#target{width:83px}}
        @media print{@font-face{font-family:Print;src:url('Fonts/Print.TTF')}}";
    let faces = font_faces(source);
    assert_eq!(faces.len(), 1, "conditional font-face discovery is explicitly deferred");
    assert_eq!(faces[0].family, "Top");
    match &faces[0].sources[0] {
        FontSource::Url { url, .. } => assert_eq!(url, "Fonts/Keep.TTF"),
        _ => panic!("original top-level URL was not retained"),
    }
    let mut doc = crate::html::parse(&format!(
        "<style>{source}</style><div id=target style='font-family:Conditional'>Original</div>"
    ));
    doc.media_environment = Some(environment(800, 600, Hover::Hover));
    let style = compute(&doc, &HashMap::new());
    assert_eq!(style[node(&doc, "target")].width, Length::Px(83.0),
        "deferred nested face must not swallow adjacent active ordinary rules");
    // This tests discovery only. Actual absence of font HTTP requests is an App/loader assertion.
}
