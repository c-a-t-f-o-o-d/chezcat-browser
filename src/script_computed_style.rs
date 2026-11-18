//! Finite live, read-only computed custom-property declarations.
//! CSSOM getComputedStyle is NewObject: handles retain only the genuine element
//! and owner, never inline text, a stale cascade, or a second style resolver.
use super::*;

const BASE: u32 = 0x8400_0000;
const END: u32 = 0x8500_0000;
pub(super) fn index(handle: u32) -> Option<usize> {
    (BASE..END)
        .contains(&handle)
        .then(|| (handle - BASE) as usize)
}

pub(super) struct Declaration {
    node: usize,
    owner: u32,
}

impl Bridge<'_> {
    pub(super) fn computed_style_owner(&self, handle: u32) -> Option<u32> {
        index(handle)
            .and_then(|index| self.state.computed_styles.get(index))
            .map(|style| style.owner)
    }
    pub(super) fn computed_style_receiver(&self, handle: u32) -> bool {
        self.computed_style_owner(handle).is_some()
    }

    pub(super) fn validate_computed_target(
        &self,
        value: Option<&Value>,
    ) -> Result<usize, (&'static str, String)> {
        let Some(Value::Host(handle)) = value else {
            return Err((
                "TypeError",
                "getComputedStyle requires a genuine Element.".into(),
            ));
        };
        if self.is_frame_handle(*handle) {
            if let Some(owner) = self.frame_instance_window(*handle) {
                self.frame_window_access(Some(WINDOW), owner)
                    .map_err(|message| ("SecurityError", message))?;
            }
            return if self.frame_element_sibling_receiver(*handle) {
                Err((
                    "NotSupportedError",
                    "Computed style for child Elements has no renderer binding.".into(),
                ))
            } else {
                Err((
                    "TypeError",
                    "getComputedStyle requires a genuine Element.".into(),
                ))
            };
        }
        if !self.attr_element_receiver(*handle) {
            return Err((
                "TypeError",
                "getComputedStyle requires a genuine accessible Element.".into(),
            ));
        }
        Ok(node_index(*handle).unwrap())
    }

    pub(super) fn validate_computed_call(
        &self,
        object: u32,
        method: &str,
        args: &[Value],
    ) -> Result<(), (&'static str, String)> {
        if object == WINDOW && method == "getComputedStyle" {
            self.validate_computed_target(args.first())?;
            if args.get(1).is_some_and(|value| match value {
                Value::Null | Value::Undefined => false,
                Value::String(text) => !text.is_empty(),
                _ => true,
            }) {
                return Err((
                    "NotSupportedError",
                    "Computed pseudo-element style is not implemented.".into(),
                ));
            }
        } else if index(object).is_some() {
            if !self.computed_style_receiver(object) {
                return Err((
                    "TypeError",
                    "Computed declaration receiver is unavailable.".into(),
                ));
            }
            if matches!(method, "setProperty" | "removeProperty") {
                return Err((
                    "NoModificationAllowedError",
                    "Computed style declarations are read-only.".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn create_computed_style(&mut self, args: &[Value]) -> Result<Value, String> {
        self.validate_computed_call(WINDOW, "getComputedStyle", args)
            .map_err(|(_, message)| message)?;
        let node = self
            .validate_computed_target(args.first())
            .map_err(|(_, message)| message)?;
        // Recheck physical connection after observable pseudo argument conversion.
        self.state.rebuild(self.doc)?;
        let length = self.state.computed_styles.len();
        let handle = u32::try_from(length)
            .ok()
            .and_then(|index| BASE.checked_add(index))
            .filter(|&handle| handle < END)
            .ok_or("Computed declaration identity space exhausted.")?;
        self.state
            .computed_styles
            .try_reserve(1)
            .map_err(|_| "Computed declaration storage reservation failed.")?;
        self.state.work(1)?;
        self.state
            .replace_bytes(0, std::mem::size_of::<Declaration>())?;
        self.state.computed_styles.push(Declaration {
            node,
            owner: WINDOW,
        });
        Ok(Value::Host(handle))
    }

    pub(super) fn computed_property_value(
        &mut self,
        object: u32,
        args: &[Value],
    ) -> Result<Value, String> {
        let style = index(object)
            .and_then(|index| self.state.computed_styles.get(index))
            .ok_or("Invalid computed declaration receiver.")?;
        let (node, owner) = (style.node, style.owner);
        if owner != WINDOW {
            return Err("Computed declaration owner is unavailable.".into());
        }
        let Some(Value::String(name)) = args.first() else {
            return Err("getPropertyValue requires runtime DOMString conversion.".into());
        };
        if name.is_empty() {
            return Ok(Value::String(String::new()));
        }
        if !crate::css::computed_custom_property_name(name) {
            return Err(
                "Computed readback supports only the admitted custom-property name grammar.".into(),
            );
        }
        self.state.work(
            name.len()
                .checked_add(1)
                .ok_or("Computed name work overflow.")?,
        )?;
        self.state.rebuild(self.doc)?;
        if !self.attr_element_receiver(node_handle(node)) {
            return Err("Computed declaration Element is inert or unavailable.".into());
        }
        // CSSOM's declaration list is empty for a disconnected Element. The
        // retained handle remains live, so a later reconnection reads anew.
        if !self.state.dom.connected.get(node).copied().unwrap_or(false) {
            return Ok(Value::String(String::new()));
        }
        let geometry = self
            .state
            .geometry
            .as_mut()
            .ok_or("Computed style requires the actual document renderer.")?;
        geometry
            .computed_custom_property(
                self.doc,
                self.forms,
                node,
                name,
                &mut self.state.host_work,
                &mut self.state.copy_work,
            )
            .map(|value| Value::String(value.unwrap_or_default()))
    }
}
