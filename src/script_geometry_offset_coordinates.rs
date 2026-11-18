//! Integral CSSOM offsets from the renderer's retained first border boxes.
//! https://drafts.csswg.org/cssom-view/#dom-htmlelement-offsettop
//! Inline fragments and missing offset-parent geometry remain unavailable.
use super::*;

impl Geometry {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn offset_coordinate(
        &mut self,
        doc: &Document,
        forms: &Forms,
        node: usize,
        is_body: bool,
        axis: Axis,
        work: &mut usize,
        copies: &mut usize,
    ) -> Result<f64, String> {
        crate::css::CssBudget::new(work, copies).work(1)?;
        if !doc.nodes.get(node).is_some_and(|node| {
            node.namespace == crate::html::Namespace::Html
                && matches!(node.kind, crate::html::NodeKind::Element { .. })
        }) {
            return Err("Offset coordinates require a known HTML Element.".into());
        }
        // The caller has validated the actual document body identity. Its
        // offsets are zero independently of CSS boxes or position.
        if is_body {
            return Ok(0.0);
        }
        self.ensure_layout(doc, forms, work, copies)?;
        let target = self.rects.get(node).copied().flatten();
        let Some(target) = target else {
            if !self.offset_unavailable.get(node).copied().unwrap_or(true)
                || self
                    .computed_properties
                    .as_ref()
                    .is_some_and(|properties| properties.read_position(node) == Ok(None))
            {
                return Ok(0.0);
            }
            return Err("Offset coordinates require an unmeasured first layout box.".into());
        };
        let origin = match self.offset_parents.get(node).copied() {
            Some(layout::OffsetParent::None) => 0.0,
            Some(layout::OffsetParent::Element(parent)) => {
                if parent == node {
                    return Err("Offset coordinates require a genuine ancestor box.".into());
                }
                let parent_rect =
                    self.rects.get(parent).copied().flatten().ok_or(
                        "Offset coordinates require the offset parent's first border box.",
                    )?;
                let parent_client = self
                    .boxes
                    .as_ref()
                    .and_then(|boxes| boxes.get(parent))
                    .copied()
                    .flatten()
                    .ok_or("Offset coordinates require the offset parent's padding edge.")?;
                // Canonical fixed-family rectangles are viewport-relative;
                // ordinary rectangles are document-relative. Layout scrolling
                // changes paint/hits but never these retained measurements.
                // A genuine measured parent/child pair shares one family.
                if target.fixed != parent_rect.fixed {
                    return Err(
                        "Offset coordinates require matching measured coordinate spaces.".into(),
                    );
                }
                match axis {
                    Axis::X => f64::from(parent_rect.x) + f64::from(parent_client.left),
                    Axis::Y => f64::from(parent_rect.y) + f64::from(parent_client.top),
                }
            }
            _ => return Err("Offset coordinates require a measured offset parent.".into()),
        };
        let border = match axis {
            Axis::X => target.x,
            Axis::Y => target.y,
        };
        let css = (f64::from(border) - origin) / f64::from(self.zoom);
        // Integral CSSOM geometry rounds to nearest, with ties toward positive
        // infinity. This is not JS-to-IDL integer argument truncation, and
        // Rust's round() would choose the wrong direction for negative ties.
        Ok((css + 0.5).floor())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(body: &str, css: &str, zoom: f32) -> (Document, Forms, Geometry) {
        let doc = crate::html::parse(&format!(
            "<html id=root><head><style>body{{margin:0}}{css}</style></head><body id=body>{body}</body></html>"
        ));
        let forms = Forms::new(&doc);
        let geometry = Geometry::new(
            320,
            100,
            zoom,
            &HashMap::new(),
            HashMap::new(),
            &Book::empty(),
        );
        (doc, forms, geometry)
    }

    fn node(doc: &Document, id: &str) -> usize {
        (0..doc.nodes.len())
            .find(|&node| doc.attr(node, "id") == Some(id))
            .unwrap()
    }

    fn coordinate(
        geometry: &mut Geometry,
        doc: &Document,
        forms: &Forms,
        id: &str,
        axis: Axis,
    ) -> Result<f64, String> {
        geometry.offset_coordinate(
            doc,
            forms,
            node(doc, id),
            id == "body",
            axis,
            &mut 64_000_000,
            &mut 64_000_000,
        )
    }

    fn expect(
        geometry: &mut Geometry,
        doc: &Document,
        forms: &Forms,
        id: &str,
        expected: [f64; 2],
    ) {
        for (axis, expected) in [(Axis::X, expected[0]), (Axis::Y, expected[1])] {
            assert_eq!(
                coordinate(geometry, doc, forms, id, axis).unwrap(),
                expected,
                "{id} {axis:?}"
            );
        }
    }

    #[test]
    fn offset_coordinate_geometry_signed_border_padding_and_nearest_nested_parent() {
        let (doc, forms, mut geometry) = fixture(
            "<div id=outer style='position:relative;margin:11px 13px;border:3px solid;padding:5px 9px'><div id=flow style='margin-top:6px;margin-left:4px;height:10px'></div><div id=absolute style='position:absolute;left:-12px;top:-18px;margin-left:3px;margin-top:5px;border:2px solid;width:10px;height:10px'></div><div id=inner style='position:relative;border:5px solid;padding:7px'><div id=leaf style='height:10px'></div></div></div>",
            "", 1.0);
        expect(&mut geometry, &doc, &forms, "flow", [13.0, 11.0]);
        expect(&mut geometry, &doc, &forms, "absolute", [-9.0, -13.0]);
        expect(&mut geometry, &doc, &forms, "leaf", [7.0, 7.0]);
        assert_eq!(
            geometry.offset_parents[node(&doc, "leaf")],
            layout::OffsetParent::Element(node(&doc, "inner"))
        );
    }

    #[test]
    fn offset_coordinate_geometry_body_is_zero_and_null_parent_uses_real_root_fixed_coordinates() {
        let (doc, forms, mut geometry) = fixture(
            "<div id=fixed style='position:fixed;left:21px;top:-15px;width:10px;height:10px'></div>",
            "#root{margin:11px 17px}#body{margin:19px 13px;border:3px solid;padding:5px}", 1.0);
        expect(&mut geometry, &doc, &forms, "body", [0.0, 0.0]);
        expect(&mut geometry, &doc, &forms, "root", [17.0, 11.0]);
        expect(&mut geometry, &doc, &forms, "fixed", [21.0, -15.0]);
        assert_eq!(
            geometry.offset_parents[node(&doc, "root")],
            layout::OffsetParent::None
        );
        assert_eq!(
            geometry.offset_parents[node(&doc, "fixed")],
            layout::OffsetParent::None
        );
    }

    #[test]
    fn offset_coordinate_geometry_fixed_and_normal_families_ignore_viewport_scroll_at_browser_zoom()
    {
        for zoom in [0.5, 1.0, 1.5, 2.0] {
            let (doc, forms, mut geometry) = fixture(
                "<div id=normal style='position:relative;border:4px solid;padding:6px'><div id=normal-child style='margin-left:8px;margin-top:10px;height:20px'></div></div><div id=fixed style='position:fixed;left:20px;top:16px;border:4px solid;padding:6px'><div id=fixed-child style='margin-left:8px;margin-top:10px;height:20px'></div></div><div style='height:2000px'></div>", "", zoom);
            let mut page =
                layout::build_viewport(&doc, 320, 100, zoom, &HashMap::new(), &HashMap::new());
            for scroll in [0, 120, 0, 250] {
                page.set_scroll(scroll);
                geometry.sync(320, 100, zoom, HashMap::new(), &page);
                geometry.set_scroll(scroll);
                expect(&mut geometry, &doc, &forms, "normal-child", [14.0, 16.0]);
                expect(&mut geometry, &doc, &forms, "fixed-child", [14.0, 16.0]);
                expect(&mut geometry, &doc, &forms, "fixed", [20.0, 16.0]);
                assert!(geometry.rects[node(&doc, "fixed-child")].unwrap().fixed);
                assert!(!geometry.rects[node(&doc, "normal-child")].unwrap().fixed);
            }
        }
    }

    #[test]
    fn offset_coordinate_geometry_integral_rounding_keeps_negative_ties_toward_positive_infinity() {
        for (value, expected) in [
            (-2.5, -2.0),
            (-1.5, -1.0),
            (-0.5, 0.0),
            (0.5, 1.0),
            (1.5, 2.0),
        ] {
            let (doc, forms, mut geometry) = fixture(&format!(
                "<div id=target style='position:fixed;left:{value}px;top:{value}px;width:10px;height:10px'></div>"
            ), "", 2.0);
            expect(&mut geometry, &doc, &forms, "target", [expected, expected]);
        }
    }

    #[test]
    fn offset_coordinate_geometry_no_boxes_are_zero_and_unmeasured_fragments_parents_refuse() {
        let (mut doc, forms, mut geometry) = fixture(
            "<div id=hidden style='display:none'><div id=hidden-child></div></div><input id=hidden-input type=hidden><div id=detached></div><span id=inline>Actual inline</span><span style='position:relative'><button id=inline-parent>Choose</button></span><table><tbody><tr style='position:relative'><td><div id=row-parent style='position:relative'></div></td></tr></tbody></table>", "", 1.0);
        let detached = node(&doc, "detached");
        for entry in &mut doc.nodes {
            entry.children.retain(|&child| child != detached);
        }
        for id in ["hidden", "hidden-child", "hidden-input", "detached"] {
            expect(&mut geometry, &doc, &forms, id, [0.0, 0.0]);
        }
        for id in ["inline", "inline-parent", "row-parent"] {
            for axis in [Axis::X, Axis::Y] {
                assert!(
                    coordinate(&mut geometry, &doc, &forms, id, axis).is_err(),
                    "{id} {axis:?}"
                );
            }
        }
    }

    #[test]
    fn offset_coordinate_geometry_truncated_layout_does_not_invent_a_first_box() {
        let (doc, forms, mut geometry) = fixture(
            &format!(
                "<div id=before style='position:relative;left:12px'></div>{}<div id=after></div>",
                // Authored lengths are clamped to 10,000px. Use the same
                // admitted 1,000px steps as the real layout truncation gate.
                "<div style='height:1000px'></div>".repeat(1_002)
            ),
            "",
            1.0,
        );
        expect(&mut geometry, &doc, &forms, "before", [12.0, 0.0]);
        let after = node(&doc, "after");
        assert!(geometry.rects[after].is_none());
        assert!(geometry.boxes.as_ref().unwrap()[after].is_none());
        assert_eq!(
            geometry.offset_parents[after],
            layout::OffsetParent::Unavailable
        );
        assert!(coordinate(&mut geometry, &doc, &forms, "after", Axis::X).is_err());
        assert!(coordinate(&mut geometry, &doc, &forms, "after", Axis::Y).is_err());
    }

    #[test]
    fn offset_coordinate_geometry_cached_reads_charge_one_lookup_and_rebuild_limits_remain_real() {
        let (doc, forms, geometry) = fixture(
            "<div id=target style='position:fixed;left:18px;top:24px;width:10px;height:10px'></div>", "", 1.0);
        let mut geometry = geometry.with_limits(Limits { layouts_per_job: 1 });
        expect(&mut geometry, &doc, &forms, "target", [18.0, 24.0]);
        let (mut work, mut copies) = (1, 0);
        assert_eq!(
            geometry
                .offset_coordinate(
                    &doc,
                    &forms,
                    node(&doc, "target"),
                    false,
                    Axis::X,
                    &mut work,
                    &mut copies
                )
                .unwrap(),
            18.0
        );
        assert_eq!((work, copies), (0, 0));
        assert!(geometry
            .offset_coordinate(
                &doc,
                &forms,
                node(&doc, "target"),
                false,
                Axis::X,
                &mut work,
                &mut copies
            )
            .is_err());
        geometry.invalidate();
        assert!(coordinate(&mut geometry, &doc, &forms, "target", Axis::X).is_err());
        expect(&mut geometry, &doc, &forms, "body", [0.0, 0.0]);
        geometry.begin_job();
        expect(&mut geometry, &doc, &forms, "target", [18.0, 24.0]);
    }
}
