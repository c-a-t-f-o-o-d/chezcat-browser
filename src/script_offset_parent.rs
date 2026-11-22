//! HTMLElement offsets from the owning document's real layout tree.
//! https://drafts.csswg.org/cssom-view/#extensions-to-the-htmlelement-interface
use super::*;
use crate::script_geometry::Axis;

#[derive(Clone, Copy)]
pub(super) enum Offset {
    Parent,
    Coordinate(Axis),
}

impl Offset {
    pub(super) fn getter(operation: &str) -> Option<Self> {
        match operation {
            "\0HTMLElement.get.offsetParent" => Some(Self::Parent),
            "\0HTMLElement.get.offsetLeft" => Some(Self::Coordinate(Axis::X)),
            "\0HTMLElement.get.offsetTop" => Some(Self::Coordinate(Axis::Y)),
            _ => None,
        }
    }

    fn empty(self) -> Value {
        match self {
            Self::Parent => Value::Null,
            Self::Coordinate(_) => Value::Number(0.0),
        }
    }
}

impl Bridge<'_> {
    pub(super) fn html_offset_member(&self, object: u32, name: &str) -> bool {
        self.html_offset_attribute(object, name) && !self.form_named_control(object, name)
    }

    pub(super) fn html_offset_attribute(&self, object: u32, name: &str) -> bool {
        matches!(name, "offsetParent" | "offsetLeft" | "offsetTop")
            && self.html_element_receiver(object) && self.html_reflection_receiver(object)
    }

    pub(super) fn validate_html_offset(
        &self,
        object: u32,
        offset: Offset,
    ) -> Result<(), (&'static str, String)> {
        if !self.html_element_receiver(object) || !self.html_reflection_receiver(object) {
            return Err((
                "TypeError",
                "Offset geometry requires an HTMLElement.".into(),
            ));
        }
        if self.is_frame_handle(object) {
            match (self.frame_concrete_html_interface(object), offset) {
                (Some("HTMLBodyElement"), _) | (Some("HTMLHtmlElement"), Offset::Parent) => {}
                _ => {
                    return Err((
                        "NotSupportedError",
                        "Child element geometry is unavailable.".into(),
                    ))
                }
            }
        }
        Ok(())
    }

    pub(super) fn html_offset(&mut self, object: u32, offset: Offset) -> Result<Value, String> {
        // The VM checks both access owners before its brand preflight. At
        // execution the current caller is installed, including for a saved
        // getter invoked from the represented child document itself.
        if self.is_frame_handle(object) {
            self.frame_light_dom_element(object)?;
        }
        self.validate_html_offset(object, offset)
            .map_err(|(_, message)| message)?;
        self.state.work(1)?;
        if self.is_frame_handle(object) {
            return Ok(offset.empty());
        }
        let node = self.dom_node(object)?;
        if !self.state.dom.connected[node] {
            return Ok(offset.empty());
        }
        if let Some(root) = self.state.tree.document_element {
            if node == root && matches!(offset, Offset::Parent) {
                return Ok(Value::Null);
            }
            if self.state.tree.parents[node] == Some(root)
                && matches!(tag(self.doc, node), "body" | "frameset")
            {
                self.state.work(self.doc.nodes[root].children.len())?;
                if self.doc.body_element(root) == Some(node) {
                    return Ok(offset.empty());
                }
            }
        }
        let Some(geometry) = self.state.geometry.as_mut() else {
            return Err(match offset {
                Offset::Parent => "offsetParent requires the document renderer.".into(),
                Offset::Coordinate(Axis::X) => self.unavailable_offset(node, "offsetLeft"),
                Offset::Coordinate(Axis::Y) => self.unavailable_offset(node, "offsetTop"),
            });
        };
        let work = &mut self.state.host_work;
        let copies = &mut self.state.copy_work;
        match offset {
            Offset::Parent => Ok(geometry
                .offset_parent(self.doc, self.forms, node, work, copies)?
                .map_or(Value::Null, |parent| Value::Host(node_handle(parent)))),
            Offset::Coordinate(axis) => geometry
                .offset_coordinate(self.doc, self.forms, node, false, axis, work, copies)
                .map(Value::Number),
        }
    }
}
