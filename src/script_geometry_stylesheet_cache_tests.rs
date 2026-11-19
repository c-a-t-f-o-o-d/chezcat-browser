use super::*;

fn id(doc: &Document, name: &str) -> usize {
    (0..doc.nodes.len())
        .find(|&node| doc.attr(node, "id") == Some(name))
        .unwrap()
}
fn inline(doc: &mut Document, node: usize, value: &str) {
    let crate::html::NodeKind::Element { attrs, .. } = &mut doc.nodes[node].kind else {
        panic!("Element");
    };
    attrs.retain(|attr| !attr.name.html_is("style"));
    attrs.push(crate::html::Attribute::unqualified(
        "style".into(),
        value.into(),
    ));
}

#[test]
fn stylesheet_cache_geometry_sync_keeps_parse_reuse_across_live_invalidations_without_refilling() {
    let mut doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=box></div>");
    let sheet = id(&doc, "sheet");
    let node = id(&doc, "box");
    let source = format!(
        "/*{}*/#box{{opacity:.5;width:42px;height:20px}}",
        "original finite source ".repeat(3000)
    );
    let styles = HashMap::from([(sheet, source.clone())]);
    let forms = Forms::new(&doc);
    let layout = layout::build_viewport(&doc, 640, 480, 1.0, &styles, &HashMap::new());
    let mut geometry = Geometry::new(640, 480, 1.0, &styles, HashMap::new(), &Book::empty());
    geometry.sync(640, 480, 1.0, HashMap::new(), &layout);
    let source_cache = geometry.stylesheet_cache.clone().unwrap();
    let (mut work, mut copies) = (100_000, 8_000_000);
    for value in ["1", "0.5", "1", "0.5"] {
        inline(&mut doc, node, &format!("opacity:{value}"));
        geometry.invalidate();
        assert!(geometry.boxes.is_none() && geometry.computed_properties.is_none());
        let before = (work, copies);
        let result = geometry
            .computed_snapshot(&doc, &forms, &mut work, &mut copies)
            .unwrap();
        assert_eq!(
            result.read_opacity(node).unwrap(),
            Some(value.parse().unwrap())
        );
        assert!(
            geometry.boxes.is_none(),
            "style-only queries do not create fabricated geometry"
        );
        assert!(source_cache.shares_rules(geometry.stylesheet_cache.as_ref().unwrap()));
        assert!(work < before.0 && copies < before.1);
    }
    assert_eq!(
        geometry
            .read(
                &doc,
                &forms,
                node,
                false,
                "clientWidth",
                &mut work,
                &mut copies
            )
            .unwrap(),
        42.0
    );
    geometry.set_stylesheet(sheet, Some(&source.replace("opacity:.5", "opacity:.9")));
    let failed = geometry.computed_snapshot(&doc, &forms, &mut work, &mut copies);
    assert_eq!(
        failed.err().as_deref(),
        Some("CSS computation work limit reached.")
    );
    assert_eq!(work, 0);
    assert!(geometry.computed_properties.is_none());
    assert!(source_cache.shares_rules(geometry.stylesheet_cache.as_ref().unwrap()));
    // An explicit later job can restore the same source. The failed job's
    // exhausted counter remains zero, and its old computed answer is absent.
    geometry.set_stylesheet(sheet, Some(&source));
    geometry.begin_job();
    let (mut next_work, mut next_copies) = (100_000, 8_000_000);
    assert_eq!(
        geometry
            .computed_snapshot(&doc, &forms, &mut next_work, &mut next_copies)
            .unwrap()
            .read_opacity(node)
            .unwrap(),
        Some(0.5)
    );
    assert_eq!(work, 0);
}

#[test]
fn stylesheet_cache_geometry_expired_job_leaves_the_cold_cascade_unpublished() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=box></div>");
    let node = id(&doc, "box");
    let forms = Forms::new(&doc);
    let sheets = HashMap::from([(id(&doc, "sheet"), "#box{opacity:.625}".into())]);
    let mut geometry = Geometry::new(640, 480, 1.0, &sheets, HashMap::new(), &Book::empty());
    let (mut work, mut copies) = (100_000, 8_000_000);
    geometry.set_execution_deadline(Instant::now());
    let result = geometry.computed_snapshot(&doc, &forms, &mut work, &mut copies);
    assert_eq!(result.err().as_deref(), Some("CSS computation time limit reached."));
    assert!(geometry.computed_properties.is_none());
    assert!(geometry.stylesheet_cache.is_none());

    geometry.begin_job();
    geometry.set_execution_deadline(Instant::now() + std::time::Duration::from_secs(10));
    let result = geometry.computed_snapshot(&doc, &forms, &mut work, &mut copies).unwrap();
    assert_eq!(result.read_opacity(node).unwrap(), Some(0.625));
}

#[test]
fn stylesheet_cache_geometry_viewport_change_reparses_current_media_and_keeps_live_values() {
    let doc = crate::html::parse("<link id=sheet rel=stylesheet><div id=box></div>");
    let sheet = id(&doc, "sheet");
    let node = id(&doc, "box");
    let forms = Forms::new(&doc);
    let styles = HashMap::from([(
        sheet,
        "#box{opacity:.25}@media(min-width:700px){#box{opacity:.75}}".into(),
    )]);
    let layout = layout::build_viewport(&doc, 640, 480, 1.0, &styles, &HashMap::new());
    let mut geometry = Geometry::new(640, 480, 1.0, &styles, HashMap::new(), &Book::empty());
    geometry.sync(640, 480, 1.0, HashMap::new(), &layout);
    let cache = geometry.stylesheet_cache.clone().unwrap();
    let (mut work, mut copies) = (1_000_000, 8_000_000);
    assert_eq!(
        geometry
            .computed_snapshot(&doc, &forms, &mut work, &mut copies)
            .unwrap()
            .read_opacity(node)
            .unwrap(),
        Some(0.25)
    );
    geometry.update(900, 480, 1.0, HashMap::new());
    assert_eq!(
        geometry
            .computed_snapshot(&doc, &forms, &mut work, &mut copies)
            .unwrap()
            .read_opacity(node)
            .unwrap(),
        Some(0.75)
    );
    assert!(!cache.shares_rules(geometry.stylesheet_cache.as_ref().unwrap()));
}
