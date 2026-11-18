use super::*;

fn style(css: &str, inline: &str) -> Style {
    let doc = crate::html::parse(&format!("<html><head><style>{css}</style></head><body id=parent><div id=target style='{inline}'></div></body></html>"));
    let styles = compute(&doc, &HashMap::new());
    let node = (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some("target"))
        .unwrap();
    styles[node].clone()
}

#[test]
fn background_clip_css_keywords_cascade_and_noninheritance() {
    use BackgroundClip::*;
    assert_eq!(
        style("#parent{background-clip:content-box}", "").background_clip,
        Some(BorderBox)
    );
    for (value, expected) in [
        ("BORDER-BOX", BorderBox),
        ("padding-box", PaddingBox),
        ("content-box", ContentBox),
        ("inherit", ContentBox),
        ("initial", BorderBox),
        ("unset", BorderBox),
    ] {
        assert_eq!(
            style(
                "#parent{background-clip:content-box}",
                &format!("background-clip:{value}")
            )
            .background_clip,
            Some(expected),
            "{value}"
        );
    }
    assert_eq!(
        style(
            "div{background-clip:content-box}#target{background-clip:padding-box!important}",
            "background-clip:border-box"
        )
        .background_clip,
        Some(PaddingBox)
    );
    assert_eq!(
        style(
            "#target{background-clip:padding-box!important}",
            "background-clip:content-box!important"
        )
        .background_clip,
        Some(ContentBox)
    );
    assert_eq!(style("", "background-clip:padding-box;background-clip:bogus;background-clip:padding-box content-box").background_clip, Some(PaddingBox));
    assert_eq!(
        style("", "background-clip:padding-box;background-clip:text").background_clip,
        None
    );
    assert_eq!(
        style(
            "",
            "background-clip:padding-box;background-clip:border-box,content-box"
        )
        .background_clip,
        None
    );
}

#[test]
fn background_clip_css_shorthand_resets_only_accepted_background() {
    use BackgroundClip::*;
    assert_eq!(
        style("", "background-clip:padding-box;background:red").background_clip,
        Some(BorderBox)
    );
    assert_eq!(
        style("", "background:red;background-clip:padding-box").background_clip,
        Some(PaddingBox)
    );
    assert_eq!(
        style("", "background-clip:content-box!important;background:red").background_clip,
        Some(ContentBox)
    );
    assert_eq!(
        style("", "background-clip:content-box;background:red!important").background_clip,
        Some(BorderBox)
    );
    for later in [
        "background-color:red",
        "background-image:url(Picture.PPM)",
        "background:url(Picture.PPM)",
        "background:bogus",
    ] {
        assert_eq!(
            style("", &format!("background-clip:padding-box;{later}")).background_clip,
            Some(PaddingBox),
            "{later}"
        );
    }
    assert_eq!(
        style("#parent{background-clip:content-box}", "background:inherit").background_clip,
        Some(ContentBox)
    );
    for later in [
        "background:initial",
        "background:unset",
        "background:none",
        "background:transparent",
    ] {
        assert_eq!(
            style("", &format!("background-clip:content-box;{later}")).background_clip,
            Some(BorderBox),
            "{later}"
        );
    }
}

#[test]
fn background_clip_css_variables_invalid_computed_values_and_rollback() {
    use BackgroundClip::*;
    assert_eq!(
        style("#parent{--Clip:content-box}", "background-clip:var(--Clip)").background_clip,
        Some(ContentBox)
    );
    for value in ["var(--missing)", "var(--missing,bogus)", "var(--cycle)"] {
        assert_eq!(
            style(
                "#target{--cycle:var(--cycle)}",
                &format!("background-clip:padding-box;background-clip:{value}")
            )
            .background_clip,
            Some(BorderBox),
            "{value}"
        );
    }
    for value in ["var(--missing)", "var(--Bad)", "var(--Url)", "var(--cycle)"] {
        assert_eq!(
            style(
                "#target{--Bad:bogus;--Url:url(Picture.PPM);--cycle:var(--cycle)}",
                &format!("background-clip:content-box;background:{value}")
            )
            .background_clip,
            Some(BorderBox),
            "{value}"
        );
    }
    assert_eq!(
        style(
            "",
            "background-clip:content-box;background-color:var(--missing)"
        )
        .background_clip,
        Some(ContentBox)
    );
    assert_eq!(
        style(
            "#parent{background-clip:content-box;--v:var(--missing,inherit)}",
            "background:var(--v)"
        )
        .background_clip,
        Some(ContentBox)
    );
    assert_eq!(
        style(
            "#target{background-clip:padding-box}",
            "background:blue;background-clip:revert-layer"
        )
        .background_clip,
        Some(PaddingBox)
    );
    assert_eq!(
        style(
            "#target{background-clip:content-box!important}",
            "background-clip:padding-box;background-clip:revert-layer!important"
        )
        .background_clip,
        Some(ContentBox)
    );
    assert_eq!(
        style(
            "#target{background-clip:content-box!important;background:red!important}",
            "background-clip:padding-box;background-clip:revert-layer!important"
        )
        .background_clip,
        Some(BorderBox)
    );
    assert_eq!(
        style(
            "#target{background-clip:padding-box;background:url(Picture.PPM)}",
            "background:red;background-clip:revert-layer"
        )
        .background_clip,
        Some(PaddingBox)
    );
    assert_eq!(
        style(
            "#target{background-clip:padding-box;background-clip:revert-layer}",
            ""
        )
        .background_clip,
        Some(BorderBox)
    );
    assert_eq!(
        style(
            "#target{background-clip:padding-box}",
            "background-clip:revert"
        )
        .background_clip,
        Some(BorderBox)
    );
}

#[test]
fn background_clip_css_substitution_refusal_preserves_unknown_winner() {
    let mut style = Style::default();
    style.background_clip = Some(BackgroundClip::PaddingBox);
    let parent = Style::default();
    let mut sheet = style.background_clip;
    let mut budget = custom_properties::Budget::default().operation(0, 0);
    let declaration = Declaration {
        name: "background-clip".into(),
        value: "var(--missing,padding-box)".into(),
        important: false,
        stylesheet: None,
    };
    assert!(apply(
        &mut style,
        &parent,
        &declaration,
        false,
        &mut sheet,
        &custom_properties::Variables::default(),
        &mut budget
    ));
    assert_eq!(style.background_clip, None);
    assert_eq!(sheet, None);
    let declaration = Declaration {
        name: "background".into(),
        value: "var(--missing,inherit)".into(),
        important: false,
        stylesheet: None,
    };
    let mut style = Style::default();
    let mut parent = Style::default();
    parent.background_clip = Some(BackgroundClip::ContentBox);
    let mut budget = custom_properties::Budget::default().operation(0, 0);
    assert!(custom_properties::apply_box(
        &mut style,
        &parent,
        &declaration,
        &custom_properties::Variables::default(),
        &mut budget
    ));
    assert_eq!(
        style.background_clip, None,
        "A refused shorthand cannot prove an initial clip instead of inheritance"
    );
}
