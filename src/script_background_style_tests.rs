//! Original Page/DOM/CSS bridge controls, without App IO or fabricated images.
use super::*;

fn page(markup: &str, source: &str) -> (Document, Forms, Page) {
    let mut doc = crate::html::parse(markup);
    doc.scripting_enabled = true;
    let mut forms = Forms::new(&doc);
    let mut page = Page::for_test();
    let location = crate::navigation::Location {
        url: Some(
            crate::url::parse("https://example.test/folder/page")
                .unwrap()
                .0,
        ),
        path: None,
        root: None,
        fragment: String::new(),
    };
    page.set_location(&location);
    page.set_resource_base(&location);
    let outcome = page.start(
        &mut doc,
        &mut forms,
        &[Script {
            node: 0,
            label: "Original background-style bridge control".into(),
            source: source.into(),
        }],
    );
    assert_eq!(outcome.errors, 0, "{}", page.report());
    (doc, forms, page)
}
fn target(doc: &Document) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some("target"))
        .unwrap()
}
fn evaluate(page: &mut Page, doc: &mut Document, forms: &mut Forms, source: &str) {
    let outcome = page.console_eval(doc, forms, source);
    assert_eq!(outcome.errors, 0, "{}", page.report());
}

#[test]
fn background_style_bridge_keeps_initial_url_delimiters_and_real_cascade_across_property_updates() {
    let (mut doc, mut forms, mut page) = page(
        r#"<html><head><style>#target{background-image:url(SheetCase.PPM)!important}</style></head><body><div id=target style='background-image:url("Initial/Case.PPM?A=1;B=2!Bang");color:red'></div></body></html>"#,
        r#"
var p=document.getElementById('target'),styleAttr=p.getAttributeNode('style');
if(p.style.backgroundImage!=='url("Initial/Case.PPM?A=1;B=2!Bang")')throw Error('initial URL getter truncated');
p.style.color='blue';
if(p.style.backgroundImage!=='url("Initial/Case.PPM?A=1;B=2!Bang")')throw Error('unrelated setter split URL');
p.style.backgroundImage='URL("Pixels/MiXeD.PPM?Token=Keep;Part=Two!Bang")';
p.style.backgroundRepeat='no-repeat';p.style.backgroundPosition='right bottom';p.style.backgroundSize='cover';
if(p.style.backgroundImage!=='URL("Pixels/MiXeD.PPM?Token=Keep;Part=Two!Bang")'||p.style.getPropertyValue('background-image')!==p.style.backgroundImage)throw Error('raw URL identity');
if(p.style.backgroundRepeat!=='no-repeat'||p.style.backgroundPosition!=='right bottom'||p.style.backgroundSize!=='cover')throw Error('implemented longhand reflection');
if(p.getAttributeNode('style')!==styleAttr||!('backgroundImage' in p.style)||!('background-size' in p.style)||'madeUpProperty' in p.style||!('cssText' in p.style))throw Error('identity/property scope');
"#,
    );
    let node = target(&doc);
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert_eq!(
        styles[node].background_image.as_ref().unwrap().url,
        "SheetCase.PPM",
        "Stylesheet important must still win."
    );
    assert_eq!(styles[node].background_repeat, [false, false]);
    assert_eq!(
        styles[node].background_size,
        crate::css::BackgroundSize::Cover
    );
    evaluate(
        &mut page,
        &mut doc,
        &mut forms,
        r#"
p.style.setProperty('background-image','url("Inline/Chosen.PPM?Value=A;B!C")','important');
if(p.style.backgroundImage!=='url("Inline/Chosen.PPM?Value=A;B!C")'||p.getAttributeNode('style')!==styleAttr)throw Error('priority getter/Attr identity');
"#,
    );
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert_eq!(
        styles[node].background_image.as_ref().unwrap().url,
        "Inline/Chosen.PPM?Value=A;B!C"
    );
    assert_eq!(
        styles[node].background_image.as_ref().unwrap().stylesheet,
        None
    );
    evaluate(
        &mut page,
        &mut doc,
        &mut forms,
        r#"
if(p.style.removeProperty('background-image')!=='url("Inline/Chosen.PPM?Value=A;B!C")'||p.style.backgroundImage!==''||p.getAttributeNode('style')!==styleAttr)throw Error('remove actual declaration');
"#,
    );
    assert_eq!(
        crate::css::compute(&doc, &HashMap::new())[node]
            .background_image
            .as_ref()
            .unwrap()
            .url,
        "SheetCase.PPM"
    );
}

#[test]
fn background_style_bridge_set_attribute_and_live_attr_value_keep_case_and_real_typed_longhands() {
    let (mut doc, mut forms, mut page) = page(
        "<html><body><div id=target style='color:red'></div></body></html>",
        r#"
var p=document.getElementById('target'),styleAttr=p.getAttributeNode('style');
p.setAttribute('style','background-image:url("Case/Original.PPM?Token=A;B!C");background-repeat:repeat-x;background-position:right bottom;background-size:contain');
if(p.getAttributeNode('style')!==styleAttr||styleAttr.value!==p.getAttribute('style')||p.style.backgroundImage!=='url("Case/Original.PPM?Token=A;B!C")')throw Error('setAttribute value/identity');
"#,
    );
    let node = target(&doc);
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert_eq!(
        styles[node].background_image.as_ref().unwrap().url,
        "Case/Original.PPM?Token=A;B!C"
    );
    assert_eq!(styles[node].background_repeat, [true, false]);
    assert_eq!(
        styles[node].background_size,
        crate::css::BackgroundSize::Contain
    );
    evaluate(
        &mut page,
        &mut doc,
        &mut forms,
        r#"
styleAttr.value='background-image:none;background-repeat:repeat-y;background-position:center;background-size:auto';
if(p.getAttributeNode('style')!==styleAttr||p.style.backgroundImage!=='none'||p.style.backgroundRepeat!=='repeat-y')throw Error('live Attr mutation route');
"#,
    );
    let styles = crate::css::compute(&doc, &HashMap::new());
    assert!(styles[node].background_image.is_none());
    assert_eq!(styles[node].background_repeat, [false, true]);
    assert_eq!(
        styles[node].background_position,
        [crate::css::ObjectPosition {
            fraction: 0.5,
            offset: crate::css::Length::Px(0.0)
        }; 2]
    );
    assert_eq!(
        styles[node].background_size,
        crate::css::BackgroundSize::Auto
    );
}

#[test]
fn background_style_bridge_refuses_malformed_urls_unknown_names_and_css_text_atomically_then_recovers(
) {
    let (mut doc, mut forms, mut page) = page(
        "<html><body><div id=target style='color:red'></div></body></html>",
        r#"
var p=document.getElementById('target');p.style.backgroundImage='url("Kept/Original.PPM?A;B!C")';
"#,
    );
    let node = target(&doc);
    let before = doc.attr(node, "style").unwrap().to_owned();
    let commits = page.state.dom_commits;
    for source in [
        r#"p.style.backgroundImage='url("Unclosed.PPM)';"#,
        r#"p.setAttribute('style','background-image:url("Unclosed.PPM)');"#,
        r#"p.style.backgroundImage='url(First.PPM),url(Second.PPM)';"#,
        r#"p.style.backgroundImage='url(First.PPM);color:blue';"#,
        r#"p.style.backgroundImage='url("")';"#,
        r#"p.style.backgroundImage='linear-gradient(red,blue)';"#,
        r#"p.style.backgroundImage='url("Escaped\\Name.PPM")';"#,
        r#"p.setAttribute('style','background-image:url(Valid.PPM);not-supported-property:1');"#,
        r#"p.style.setProperty('made-up-property','red');"#,
        r#"p.style.cssText='background-image:url(First.PPM),url(Second.PPM)';"#,
    ] {
        let outcome = page.console_eval(&mut doc, &mut forms, source);
        assert_eq!(
            outcome.errors,
            1,
            "Expected a reported unsupported mutation for {source}: {}",
            page.report()
        );
        assert_eq!(
            doc.attr(node, "style"),
            Some(before.as_str()),
            "Partial mutation for {source}"
        );
        assert_eq!(page.state.dom_commits, commits);
    }
    let normal = page.state.limits.host.string_bytes;
    page.state.limits.host.string_bytes = 32;
    let outcome = page.console_eval(&mut doc, &mut forms, "p.style.backgroundImage='url(ThisOwnedOriginalNameIsLongerThanTheSmallStringProfile.PPM)';");
    assert_eq!(outcome.errors, 1);
    assert_eq!(doc.attr(node, "style"), Some(before.as_str()));
    assert_eq!(page.state.dom_commits, commits);
    page.state.limits.host.string_bytes = normal;
    evaluate(
        &mut page,
        &mut doc,
        &mut forms,
        "p.style.backgroundImage='url(RecoveredCase.PPM)';",
    );
    assert_eq!(
        crate::css::compute(&doc, &HashMap::new())[node]
            .background_image
            .as_ref()
            .unwrap()
            .url,
        "RecoveredCase.PPM"
    );
    let (changed, removed, requests) = page.take_image_changes();
    assert!(
        changed.is_empty() && removed.is_empty() && requests.is_empty(),
        "CSS background mutations must not fabricate IMG requests/events in Page."
    );
}
