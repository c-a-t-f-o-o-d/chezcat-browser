//! Bounded CSSOM scroll offsets from the actual viewport scrolling state.
//!
//! The viewport has a downward vertical offset and a fixed horizontal origin.
//! Ordinary non-scrollable CSS axes and genuine no-box elements stay at zero.
//! Nested scroll containers and native control internals have no measured
//! offset implementation here and remain explicitly unavailable. In
//! particular, an extent equal to the client size is not a completeness proof.
//! https://drafts.csswg.org/cssom-view/#dom-element-scrolltop
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Axis {
    X,
    Y,
}

fn native_scroll_internals(doc: &Document, node: usize) -> bool {
    crate::forms::kind(doc, node).is_some()
        || matches!(
            &doc.nodes[node].kind,
            crate::html::NodeKind::Element { tag, .. }
                if matches!(tag.as_str(), "img" | "video" | "audio" | "canvas" | "iframe"
                    | "object" | "embed" | "option" | "optgroup")
        )
}

impl Geometry {
    /// Read the current offset, or apply an already-converted CSS-pixel value
    /// and return the resulting offset. The caller supplies the validated real
    /// root identity; body is an ordinary element in this standards-only engine.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn scroll_offset(
        &mut self,
        doc: &Document,
        forms: &Forms,
        node: usize,
        is_root: bool,
        axis: Axis,
        request: Option<f64>,
        host_work: &mut usize,
        copy_work: &mut usize,
    ) -> Result<f64, String> {
        crate::css::CssBudget::new(host_work, copy_work).work(1)?;
        if !doc.nodes.get(node).is_some_and(|node| {
            node.namespace == crate::html::Namespace::Html
                && matches!(node.kind, crate::html::NodeKind::Element { .. })
        }) {
            return Err("Scroll offsets require a known HTML Element.".into());
        }
        self.ensure_layout(doc, forms, host_work, copy_work)?;
        let request = request.map(|value| if value.is_finite() { value } else { 0.0 });
        if is_root {
            return match axis {
                Axis::X => {
                    // There is no retained horizontal range or computed
                    // direction proof. Even a negative request cannot be
                    // silently clamped by assuming left-to-right CSS flow.
                    if request.is_some_and(|value| value != 0.0) {
                        return Err("Horizontal viewport scrolling is not implemented.".into());
                    }
                    Ok(0.0)
                }
                Axis::Y => {
                    if let Some(value) = request {
                        let pixels = (value * f64::from(self.zoom))
                            .round()
                            .clamp(0.0, f64::from(self.viewport_max_scroll.max(0)))
                            as i32;
                        if pixels != self.scroll {
                            self.scroll = pixels;
                            self.pending_scroll = Some(pixels);
                        }
                    }
                    Ok(f64::from(self.scroll) / f64::from(self.zoom))
                }
            };
        }

        let client = self
            .boxes
            .as_ref()
            .and_then(|boxes| boxes.get(node))
            .copied()
            .flatten();
        if client.is_none() && !self.offset_unavailable.get(node).copied().unwrap_or(true) {
            return Ok(0.0);
        }
        let overflow = self
            .computed_properties
            .as_ref()
            .ok_or("Scroll offsets require current computed style.")?
            .read_overflow(node)
            .map_err(|_| "Scroll offsets require a complete computed overflow value.".to_owned())?;
        let Some(overflow) = overflow else {
            // The shared cascade excludes detached and inert elements.
            return Ok(0.0);
        };
        if native_scroll_internals(doc, node) {
            return Err(
                "Native control or replaced-content scroll offsets are not measured.".into(),
            );
        }
        let value = match axis {
            Axis::X => overflow.x,
            Axis::Y => overflow.y,
        };
        if matches!(
            value,
            crate::css::OverflowValue::Visible | crate::css::OverflowValue::Clip
        ) {
            return Ok(0.0);
        }
        if client.is_none() {
            return Err("Scroll offsets require unmeasured layout fragments.".into());
        }
        Err("Nested scroll-container offsets are not implemented.".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(body: &str, zoom: f32) -> (Document, Forms, Geometry) {
        let doc = crate::html::parse(&format!(
            "<html id=root><head><style>body{{margin:0}}</style></head><body id=body>{body}</body></html>"
        ));
        let forms = Forms::new(&doc);
        let geometry = Geometry::new(
            320,
            80,
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

    fn access(
        geometry: &mut Geometry,
        doc: &Document,
        forms: &Forms,
        id: &str,
        axis: Axis,
        request: Option<f64>,
    ) -> Result<f64, String> {
        geometry.scroll_offset(
            doc,
            forms,
            node(doc, id),
            id == "root",
            axis,
            request,
            &mut 64_000_000,
            &mut 64_000_000,
        )
    }

    #[test]
    fn scroll_offset_geometry_root_y_moves_immediately_with_fractional_css_readback_and_clamps() {
        let (doc, forms, mut geometry) = fixture("<div style='height:600px'></div>", 1.5);
        assert_eq!(
            access(&mut geometry, &doc, &forms, "root", Axis::Y, None).unwrap(),
            0.0
        );
        assert!(geometry.viewport_max_scroll > 0);
        let one_pixel = 1.0 / f64::from(geometry.zoom);
        assert_eq!(
            access(
                &mut geometry,
                &doc,
                &forms,
                "root",
                Axis::Y,
                Some(one_pixel)
            )
            .unwrap(),
            one_pixel
        );
        assert_eq!(geometry.pending_scroll, Some(1));
        assert_eq!(
            access(&mut geometry, &doc, &forms, "root", Axis::Y, None).unwrap(),
            one_pixel
        );
        assert_eq!(geometry.pending_scroll, Some(1));

        let maximum = f64::from(geometry.viewport_max_scroll) / f64::from(geometry.zoom);
        assert_eq!(
            access(&mut geometry, &doc, &forms, "root", Axis::Y, Some(f64::MAX)).unwrap(),
            maximum
        );
        assert_eq!(geometry.pending_scroll, Some(geometry.viewport_max_scroll));
        assert_eq!(
            access(&mut geometry, &doc, &forms, "root", Axis::Y, Some(-10.0)).unwrap(),
            0.0
        );
        assert_eq!(geometry.pending_scroll, Some(0));
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            access(
                &mut geometry,
                &doc,
                &forms,
                "root",
                Axis::Y,
                Some(one_pixel),
            )
            .unwrap();
            assert_eq!(
                access(&mut geometry, &doc, &forms, "root", Axis::Y, Some(value)).unwrap(),
                0.0
            );
            assert_eq!(geometry.pending_scroll, Some(0));
        }
    }

    #[test]
    fn scroll_offset_geometry_root_x_origin_and_rejected_move_preserve_y_and_pending() {
        let (doc, forms, mut geometry) =
            fixture("<div style='height:600px;width:900px'></div>", 1.0);
        access(&mut geometry, &doc, &forms, "root", Axis::Y, Some(25.0)).unwrap();
        for request in [None, Some(0.0), Some(f64::NAN), Some(f64::INFINITY)] {
            assert_eq!(
                access(&mut geometry, &doc, &forms, "root", Axis::X, request).unwrap(),
                0.0
            );
        }
        assert!(access(&mut geometry, &doc, &forms, "root", Axis::X, Some(1.0)).is_err());
        assert!(access(&mut geometry, &doc, &forms, "root", Axis::X, Some(-4.0)).is_err());
        assert_eq!((geometry.scroll, geometry.pending_scroll), (25, Some(25)));
    }

    #[test]
    fn scroll_offset_geometry_non_scrollable_axes_and_no_boxes_are_zero_without_body_aliasing() {
        let (mut doc, forms, mut geometry) = fixture(
            "<div id=visible style='width:40px;height:20px'><div style='width:200px;height:200px'></div></div><div id=clip style='overflow:clip;width:40px;height:20px'><div style='width:200px;height:200px'></div></div><span id=inline>Inline</span><div id=hidden style='display:none;overflow:auto'></div><input id=hidden-input type=hidden><div id=detached style='overflow:auto'></div>", 1.0);
        let detached = node(&doc, "detached");
        for entry in &mut doc.nodes {
            entry.children.retain(|&child| child != detached);
        }
        access(&mut geometry, &doc, &forms, "root", Axis::Y, Some(30.0)).unwrap();
        for id in [
            "body",
            "visible",
            "clip",
            "inline",
            "hidden",
            "hidden-input",
            "detached",
        ] {
            for axis in [Axis::X, Axis::Y] {
                assert_eq!(
                    access(&mut geometry, &doc, &forms, id, axis, None).unwrap(),
                    0.0,
                    "{id} {axis:?}"
                );
                assert_eq!(
                    access(&mut geometry, &doc, &forms, id, axis, Some(55.5)).unwrap(),
                    0.0,
                    "{id} {axis:?}"
                );
            }
        }
        assert_eq!(geometry.scroll, 30);
    }

    #[test]
    fn scroll_offset_geometry_nested_scrollable_axes_and_native_internals_are_unavailable() {
        let (doc, forms, mut geometry) = fixture(
            "<div id=hidden style='overflow:hidden;width:40px;height:20px'><div style='width:200px;height:200px'></div></div><div id=fit style='overflow:auto;width:40px;height:20px'></div><div id=reverse style='overflow:auto;display:flex;flex-direction:column-reverse;height:20px'><div style='height:80px;flex:none'></div></div><input id=input value=editing><textarea id=textarea>Text</textarea><button id=button>Choose</button>", 1.0);
        for id in ["hidden", "fit", "reverse", "input", "textarea", "button"] {
            for axis in [Axis::X, Axis::Y] {
                assert!(
                    access(&mut geometry, &doc, &forms, id, axis, None).is_err(),
                    "{id} {axis:?}"
                );
                assert!(
                    access(&mut geometry, &doc, &forms, id, axis, Some(3.0)).is_err(),
                    "{id} {axis:?}"
                );
            }
        }
        assert_eq!((geometry.scroll, geometry.pending_scroll), (0, None));
    }

    #[test]
    fn scroll_offset_geometry_cache_hits_debit_one_lookup_and_zero_copy_work() {
        let (doc, forms, mut geometry) = fixture("<div id=box></div>", 1.0);
        access(&mut geometry, &doc, &forms, "box", Axis::Y, None).unwrap();
        let (mut work, mut copies) = (1, 0);
        assert_eq!(
            geometry
                .scroll_offset(
                    &doc,
                    &forms,
                    node(&doc, "box"),
                    false,
                    Axis::Y,
                    None,
                    &mut work,
                    &mut copies
                )
                .unwrap(),
            0.0
        );
        assert_eq!((work, copies), (0, 0));
        assert!(geometry
            .scroll_offset(
                &doc,
                &forms,
                node(&doc, "box"),
                false,
                Axis::Y,
                None,
                &mut work,
                &mut copies
            )
            .is_err());
        assert_eq!((geometry.scroll, geometry.pending_scroll), (0, None));
    }

    #[test]
    fn scroll_offset_geometry_uses_current_admitted_styles_and_viewport_after_invalidation() {
        let (doc, forms, mut geometry) = fixture(
            "<link id=sheet rel=stylesheet href=theme.css><div id=box style='width:40px;height:20px'></div>", 1.0);
        let sheet = node(&doc, "sheet");
        geometry.set_stylesheet(
            sheet,
            Some("#box{overflow:visible}@media(min-width:400px){#box{overflow:auto}}"),
        );
        assert_eq!(
            access(&mut geometry, &doc, &forms, "box", Axis::X, None).unwrap(),
            0.0
        );
        geometry.update(640, 80, 1.0, HashMap::new());
        assert!(access(&mut geometry, &doc, &forms, "box", Axis::X, None).is_err());
        geometry.set_stylesheet(sheet, Some("#box{overflow:clip}"));
        assert_eq!(
            access(&mut geometry, &doc, &forms, "box", Axis::X, None).unwrap(),
            0.0
        );
        assert_eq!((geometry.scroll, geometry.pending_scroll), (0, None));
    }

    #[test]
    fn scroll_offset_geometry_refused_overflow_cannot_become_a_zero_fallback() {
        let huge = "x".repeat(2048);
        let (doc, forms, mut geometry) = fixture(&format!(
            "<div style='height:600px'></div><div id=blocked style='--huge:{huge};overflow:var(--huge)'></div><div id=neighbor style='overflow:clip'></div>"
        ), 1.0);
        access(&mut geometry, &doc, &forms, "root", Axis::Y, Some(25.0)).unwrap();
        for axis in [Axis::X, Axis::Y] {
            assert!(access(&mut geometry, &doc, &forms, "blocked", axis, None).is_err());
            assert!(access(&mut geometry, &doc, &forms, "blocked", axis, Some(8.0)).is_err());
            assert_eq!(
                access(&mut geometry, &doc, &forms, "neighbor", axis, None).unwrap(),
                0.0
            );
        }
        assert_eq!((geometry.scroll, geometry.pending_scroll), (25, Some(25)));
    }
}
