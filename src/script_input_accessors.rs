//! Canonical HTMLInputElement attributes over the existing finite native modes.
//! https://html.spec.whatwg.org/multipage/input.html#htmlinputelement
use super::*;

pub(super) fn accessor(operation: &str) -> Option<(&'static str, bool)> {
    match operation {
        "\0HTMLInputElement.get.value" => Some(("value", false)),
        "\0HTMLInputElement.set.value" => Some(("value", true)),
        "\0HTMLInputElement.get.checked" => Some(("checked", false)),
        "\0HTMLInputElement.set.checked" => Some(("checked", true)),
        _ => None,
    }
}

impl Bridge<'_> {
    pub(super) fn input_accessor_member(&self, object: u32, name: &str) -> bool {
        matches!(name, "value" | "checked") && self.input_receiver(object)
    }

    pub(super) fn validate_input_accessor(
        &self,
        object: u32,
    ) -> Result<(), (&'static str, String)> {
        // Supported child blank documents have no native INPUT representation.
        // Never infer that brand from an author prototype or spelling alone.
        if !self.input_receiver(object) {
            return Err((
                "TypeError",
                "Input attributes require a genuine supported HTMLInputElement.".into(),
            ));
        }
        Ok(())
    }
}
