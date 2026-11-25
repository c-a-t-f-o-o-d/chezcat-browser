//! Finite font allowance refusal must not expose a complete stale snapshot.
use super::*;

const PROPERTIES: [&str; 4] = ["font", "font-family", "font-style", "font-weight"];

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap()
}

fn source(property: &str, heavy: bool) -> String {
    let repeated = if heavy {
        format!("font-family:'{}';", "F".repeat(250)).repeat(520)
    } else { String::new() };
    let source = format!(
        "#parent{{font:italic 500 20px/1.75 'Parent Face',serif;--visible:yes}}\
         #target{{font:800 27px/2.25 'Child Face',monospace;{repeated}{property}:var(--missing)}}"
    );
    let usage = StylesheetLimits::default().accept(&source, StylesheetUsage::default()).unwrap();
    assert_eq!((usage.bytes, usage.rules), (source.len(), 2), "the complete fixture must be admitted");
    source
}

fn document() -> Document {
    crate::html::parse("<!doctype html><html><head><link id=sheet rel=stylesheet href=fonts.css></head><body><div id=parent><div id=target></div></div></body></html>")
}

fn budgeted(doc: &Document, external: &HashMap<usize, String>, transform: bool) -> Result<CascadeOutput, String> {
    let (mut work, mut copies) = (usize::MAX, usize::MAX);
    let mut budget = CssBudget::new(&mut work, &mut copies);
    if transform {
        compute_with_environment_and_properties_transform_budgeted(doc, external, None, None, &mut budget)
    } else {
        compute_with_environment_and_properties_budgeted(doc, external, None, None, &mut budget)
    }
}

fn recovered(doc: &Document, result: &CascadeOutput, property: &str) {
    let target = id(doc, "target");
    let style = &result.styles[target];
    let whole = property == "font";
    assert_eq!(style.font_families, if whole || property == "font-family" {
        vec!["Parent Face", "serif"]
    } else { vec!["Child Face", "monospace"] });
    let weight = if whole || property == "font-weight" { 500 } else { 800 };
    let size = if whole { 20.0 } else { 27.0 };
    assert_eq!((style.font_weight, style.bold), (weight, weight >= 600));
    assert_eq!(style.italic, whole || property == "font-style");
    assert_eq!(style.font_size, size);
    assert!(matches!(style.line_height, LineHeight::Factor(value)
        if value == if whole { 1.75 } else { 2.25 }));
    assert_eq!(result.computed_properties.read_font_size(target), Ok(Some(size)));
    assert_eq!(result.computed_properties.read(target, "--visible"), Ok(Some("yes")));
}

#[test]
fn font_finite_allowance_refuses_both_budgeted_routes_and_fresh_sources_recover() {
    for property in PROPERTIES {
        let doc = document();
        let before = format!("{:?}", doc.nodes);
        let sheet = id(&doc, "sheet");
        let mut external = HashMap::from([(sheet, source(property, true))]);
        for transform in [false, true] {
            assert!(budgeted(&doc, &external, transform).is_err(),
                "{property}: an ample caller budget cannot hide the finite font allowance refusal, route {transform}");
            assert_eq!(format!("{:?}", doc.nodes), before);
        }
        external.insert(sheet, source(property, false));
        for transform in [false, true] {
            recovered(&doc, &budgeted(&doc, &external, transform).unwrap(), property);
            assert_eq!(format!("{:?}", doc.nodes), before);
        }
    }
}

#[test]
fn font_finite_allowance_keeps_unmetered_readback_incomplete_until_fresh_recovery() {
    for property in PROPERTIES {
        let doc = document();
        let before = format!("{:?}", doc.nodes);
        let sheet = id(&doc, "sheet");
        let target = id(&doc, "target");
        let mut external = HashMap::from([(sheet, source(property, true))]);
        let refused = compute_with_environment_and_properties(&doc, &external, None, None);
        assert_eq!(refused.computed_properties.read_font_size(target), Err(ComputedStyleUnavailable), "{property}");
        assert_eq!(refused.computed_properties.read(target, "--visible"), Err(ComputedStyleUnavailable), "{property}");
        assert_eq!(format!("{:?}", doc.nodes), before);
        external.insert(sheet, source(property, false));
        recovered(&doc, &compute_with_environment_and_properties(&doc, &external, None, None), property);
        assert_eq!(format!("{:?}", doc.nodes), before);
    }
}
