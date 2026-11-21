//! Native shared-cascade geometry, independent of SVG source serialization.
use super::*;
fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&n| doc.attr(n, "id") == Some(name))
        .unwrap()
}
fn value(
    output: &CascadeOutput,
    doc: &Document,
    name: &str,
    key: &str,
    reference: f64,
) -> Option<f64> {
    output
        .computed_properties
        .read_svg_geometry(id(doc, name))
        .unwrap()
        .unwrap()
        .resolve(key, reference)
        .unwrap()
}
#[test]
fn svg_shared_geometry_external_selectors_priority_and_attribute_hints() {
    let doc=crate::html::parse("<html><head><link id=sheet rel=stylesheet><style>rect{width:8px}#icon .shape{width:12px!important}</style></head><body><svg id=icon viewBox='0 0 40 20'><rect id=shape class=shape width=30 height=20 style='width:16px;height:50%'></rect></svg></body></html>");
    let mut external = HashMap::new();
    external.insert(
        id(&doc, "sheet"),
        "#icon > #shape{width:20px!important;x:5px}".into(),
    );
    let out = compute_with_environment_and_properties(&doc, &external, None, None);
    assert_eq!(value(&out, &doc, "shape", "width", 40.), Some(20.));
    assert_eq!(value(&out, &doc, "shape", "height", 20.), Some(10.));
    assert_eq!(value(&out, &doc, "shape", "x", 40.), Some(5.));
    assert_eq!(doc.attr(id(&doc, "shape"), "width"), Some("30"));
    assert_eq!(
        doc.attr(id(&doc, "shape"), "style"),
        Some("width:16px;height:50%")
    );
}
#[test]
fn svg_shared_geometry_variables_font_context_noninheritance_and_explicit_inherit() {
    let doc=crate::html::parse("<html><head><style>body{font-size:10px;--size:2em}#group{width:30px;font-size:12px}#a{width:var(--size);x:25%;height:1rem}#b{width:inherit}#c{width:8px;width:var(--missing)}#d{width:8px;width:garbage}</style></head><body><svg id=icon><g id=group><rect id=a></rect><rect id=b></rect><rect id=c></rect><rect id=d></rect><rect id=e></rect></g></svg></body></html>");
    let out = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    assert_eq!(value(&out, &doc, "a", "width", 40.), Some(24.));
    assert_eq!(value(&out, &doc, "a", "height", 20.), Some(16.));
    assert_eq!(value(&out, &doc, "a", "x", 40.), Some(10.));
    assert_eq!(value(&out, &doc, "b", "width", 40.), Some(30.));
    assert_eq!(value(&out, &doc, "c", "width", 40.), None);
    assert_eq!(value(&out, &doc, "d", "width", 40.), Some(8.));
    assert_eq!(value(&out, &doc, "e", "width", 40.), None);
}
#[test]
fn svg_shared_geometry_unsupported_winners_and_coordinate_guard_stay_unavailable() {
    let doc=crate::html::parse("<style>#a{width:calc(2px + 3%)}#b{width:revert}#c{r:100000000px}#d{r:-3px}</style><svg><rect id=a></rect><rect id=b></rect><circle id=c></circle><circle id=d></circle></svg><div id=html></div>");
    let out = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    for (name, key) in [("a", "width"), ("b", "width"), ("c", "r")] {
        assert!(
            out.computed_properties
                .read_svg_geometry(id(&doc, name))
                .unwrap()
                .unwrap()
                .resolve(key, 40.)
                .is_err(),
            "{name}"
        );
    }
    assert_eq!(value(&out, &doc, "d", "r", 40.), Some(0.));
    assert!(out
        .computed_properties
        .read_svg_geometry(id(&doc, "html"))
        .unwrap()
        .is_none());
}
#[test]
fn svg_shared_geometry_unknown_values_are_per_property_and_explicit_inheritance_recovers() {
    let doc=crate::html::parse("<style>#parent{width:calc(2px + 3%);height:12px}#a{width:inherit;height:inherit;width:garbage}#b{width:inherit;width:7px}path{width:calc(3px + 2%);x:4px}</style><svg><g id=parent><rect id=a></rect><rect id=b></rect><path id=p d='M0 0L10 0L0 10Z'></path></g></svg>");
    let out = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    assert!(out
        .computed_properties
        .read_svg_geometry(id(&doc, "a"))
        .unwrap()
        .unwrap()
        .resolve("width", 40.)
        .is_err());
    assert_eq!(value(&out, &doc, "a", "height", 40.), Some(12.));
    assert_eq!(value(&out, &doc, "b", "width", 40.), Some(7.));
    assert_eq!(value(&out, &doc, "p", "x", 40.), Some(4.));
}
#[test]
fn svg_shared_geometry_media_changes_and_budget_refusal_recover_without_stale_snapshot() {
    let doc=crate::html::parse("<style>rect{width:10px}@media(min-width:400px){rect{width:20px}}</style><svg><rect id=a></rect></svg>");
    let env = |width| {
        Some(MediaEnvironment {
            width,
            height: 300.,
            hover: Hover::None,
        })
    };
    let small = compute_with_environment_and_properties(&doc, &HashMap::new(), None, env(300.));
    let wide = compute_with_environment_and_properties(&doc, &HashMap::new(), None, env(500.));
    assert_eq!(value(&small, &doc, "a", "width", 40.), Some(10.));
    assert_eq!(value(&wide, &doc, "a", "width", 40.), Some(20.));
    let mut work = 1_000_000;
    let mut copies = 0;
    let mut budget = CssBudget::new(&mut work, &mut copies);
    assert!(compute_with_environment_and_properties_budgeted(
        &doc,
        &HashMap::new(),
        None,
        env(500.),
        &mut budget
    )
    .is_err());
    let recovered = compute_with_environment_and_properties(&doc, &HashMap::new(), None, env(300.));
    assert_eq!(value(&recovered, &doc, "a", "width", 40.), Some(10.));
}

#[test]
fn svg_shared_geometry_font_presentation_hint_and_sparse_lookup_preserve_uncertainty() {
    let doc=crate::html::parse("<html><body><svg font-size='20'><g><rect id=a width='3' style='width:2em;height:1rem'></rect><rect id=b font-size='30' style='font-size:12px;width:2em'></rect></g></svg></body></html>");
    let out = compute_with_environment_and_properties(&doc, &HashMap::new(), None, None);
    assert_eq!(value(&out, &doc, "a", "width", 40.), Some(40.));
    assert_eq!(value(&out, &doc, "a", "height", 40.), Some(16.));
    assert_eq!(value(&out, &doc, "b", "width", 40.), Some(24.));
    assert!(out
        .computed_properties
        .read_svg_geometry(doc.nodes.len())
        .is_err());
    let node = id(&doc, "a");
    let vars = custom_properties::Variables::default();
    let mut denied = custom_properties::Budget::default().operation(0, 0);
    assert!(svg_geometry::compute(
        &doc,
        node,
        "rect",
        false,
        None,
        false,
        16.,
        16.,
        true,
        std::iter::empty(),
        &vars,
        &mut denied
    )
    .is_none());
    let mut recovered = custom_properties::Budget::default();
    assert!(svg_geometry::compute(
        &doc,
        node,
        "rect",
        false,
        None,
        false,
        16.,
        16.,
        true,
        std::iter::empty(),
        &vars,
        &mut recovered
    )
    .is_some());
}
