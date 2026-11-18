//! CSSOM View scroll offsets on genuine Elements of the owning document.
//! https://drafts.csswg.org/cssom-view/#dom-element-scrollleft
use super::*;
use crate::script_geometry::Axis;

pub(super) fn axis(name: &str) -> Option<Axis> {
    match name {
        "scrollLeft" => Some(Axis::X),
        "scrollTop" => Some(Axis::Y),
        _ => None,
    }
}

pub(super) fn accessor(operation: &str) -> Option<(&'static str, bool)> {
    match operation {
        "\0Element.get.scrollLeft" => Some(("scrollLeft", false)),
        "\0Element.get.scrollTop" => Some(("scrollTop", false)),
        "\0Element.set.scrollLeft" => Some(("scrollLeft", true)),
        "\0Element.set.scrollTop" => Some(("scrollTop", true)),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn scroll_offset_member(&self, object: u32, name: &str) -> bool {
        axis(name).is_some()
            && self.element_selector_receiver(object)
            && !self.form_named_control(object, name)
    }

    pub(super) fn validate_scroll_element(
        &self,
        object: u32,
    ) -> Result<(), (&'static str, String)> {
        if !self.element_selector_receiver(object) {
            return Err((
                "TypeError",
                "Scroll offsets require a genuine Element.".into(),
            ));
        }
        if self.is_frame_handle(object) {
            self.frame_light_dom_element(object)
                .map_err(|reason| ("SecurityError", reason))?;
            return Err((
                "NotSupportedError",
                "Child document scrolling geometry is unavailable.".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn element_scroll_offset(
        &mut self,
        object: u32,
        axis: Axis,
        request: Option<f64>,
    ) -> Result<Value, String> {
        self.validate_scroll_element(object)
            .map_err(|(_, reason)| reason)?;
        let node = self.dom_node(object)?;
        if !self.state.dom.connected[node] {
            self.state.work(1)?;
            return Ok(Value::Number(0.0));
        }
        let is_root = self.state.tree.document_element == Some(node);
        let geometry = self
            .state
            .geometry
            .as_mut()
            .ok_or("Scroll offsets require the document renderer.")?;
        geometry
            .scroll_offset(
                self.doc,
                self.forms,
                node,
                is_root,
                axis,
                request,
                &mut self.state.host_work,
                &mut self.state.copy_work,
            )
            .map(Value::Number)
    }
}
