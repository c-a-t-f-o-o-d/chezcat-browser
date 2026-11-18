//! Genuine HTMLCanvasElement dimension accessors over raw content attributes.
use super::*;

pub(super) fn accessor(operation: &str) -> Option<(&'static str, bool)> {
    match operation {
        "\0HTMLCanvasElement.get.width" => Some(("width", false)),
        "\0HTMLCanvasElement.set.width" => Some(("width", true)),
        "\0HTMLCanvasElement.get.height" => Some(("height", false)),
        "\0HTMLCanvasElement.set.height" => Some(("height", true)),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn canvas_receiver(&self, object: u32) -> bool {
        node_index(object).is_some_and(|node| {
            self.class_list_element_receiver(object) && crate::html_canvas::element(self.doc, node)
        })
    }

    pub(super) fn canvas_member(&self, object: u32, name: &str) -> bool {
        matches!(name, "width" | "height") && self.canvas_receiver(object)
    }

    pub(super) fn validate_canvas_accessor(&self, object: u32) -> Result<(), (&'static str, String)> {
        if self.canvas_receiver(object) { Ok(()) }
        else { Err(("TypeError", "Canvas dimensions require a genuine supported HTMLCanvasElement.".into())) }
    }

    pub(super) fn canvas_dimension_get(&mut self, object: u32, name: &str) -> Result<Value, String> {
        self.validate_canvas_accessor(object).map_err(|(_, reason)| reason)?;
        let node = self.node(object)?;
        let NodeKind::Element { attrs, .. } = &self.doc.nodes[node].kind else { unreachable!() };
        self.state.work(attrs.len())?;
        let raw = self.doc.attr(node, name);
        self.state.work(raw.map_or(1, |value| value.len().saturating_add(1)))?;
        Ok(Value::Number(f64::from(crate::html_canvas::reflected_dimension(raw, name))))
    }

    pub(super) fn canvas_dimension_set(&mut self, object: u32, name: &str, value: &Value) -> Result<(), String> {
        self.validate_canvas_accessor(object).map_err(|(_, reason)| reason)?;
        let node = self.node(object)?;
        // Native number conversion already ran once, with receiver validation
        // before author callbacks. Web IDL unsigned long uses modulo 2^32.
        let number = timer_integer(value)? as u32;
        let number = if number <= i32::MAX as u32 { number }
            else { crate::html_canvas::default_dimension(name) };
        self.state.copies(10)?;
        self.attribute(node, name, Some(number.to_string()))
    }
}
