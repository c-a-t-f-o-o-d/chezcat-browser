//! Page keyboard dispatch. Browser shortcuts and browser-owned editors stay
//! outside the page realm; supported form controls keep cancelable defaults.
use super::*;
use crate::scripting::KeyboardInput;

impl App {
    /// Test entry point with a US logical key fallback. The native shell always
    /// supplies its actual layout-derived key and physical scan code.
    #[cfg(test)]
    pub fn key(&mut self, key: u32, ctrl: bool, shift: bool, alt: bool) -> PlatformAction {
        let logical = named_key(key)
            .map(str::to_owned)
            .unwrap_or_else(|| match key {
                0x41..=0x5a => char::from_u32(if shift { key } else { key + 32 })
                    .unwrap()
                    .to_string(),
                0x30..=0x39 => char::from_u32(key).unwrap().to_string(),
                0x20 => " ".into(),
                _ => "Unidentified".into(),
            });
        self.key_event(KeyboardInput {
            down: true,
            key: logical,
            code: fallback_code(key).into(),
            ctrl,
            shift,
            alt,
            key_code: key,
            ..KeyboardInput::default()
        })
        .0
    }

    /// Returns whether native text translation must be suppressed for this
    /// particular keydown. Keyup is independent and is still delivered.
    pub fn key_event(&mut self, mut input: KeyboardInput) -> (PlatformAction, bool) {
        let down = input.down;
        let (key, ctrl, shift, alt, meta) = (
            input.key_code,
            input.ctrl,
            input.shift,
            input.alt,
            input.meta,
        );
        let mut prevented = false;
        if self.page_keyboard_allowed(key, ctrl, shift, alt, meta) {
            input.target = match self.focus {
                Focus::Control(node) => Some(node),
                _ => None,
            };
            self.script_begin_job();
            self.update_script_geometry();
            if let Some(page) = &mut self.scripting {
                let outcome = page.keyboard(&mut self.document, &mut self.forms, input);
                prevented = outcome.prevented;
                if outcome.layout_changed {
                    self.reflow();
                }
                self.validate_control_focus();
                self.sync_editors();
                if outcome.errors > 0 {
                    self.show_script_errors();
                }
            }
        }
        if !down {
            return (PlatformAction::None, false);
        }
        if prevented {
            return (PlatformAction::None, true);
        }
        if meta {
            return (PlatformAction::None, false);
        }
        (self.key_default(key, ctrl, shift, alt), false)
    }

    fn page_keyboard_allowed(
        &self,
        key: u32,
        ctrl: bool,
        shift: bool,
        alt: bool,
        meta: bool,
    ) -> bool {
        if !self.scripts_enabled
            || self.scripting.is_none()
            || self.view != View::Page
            || self.browser_panel.is_some()
            || self.menu
            || self.chrome.focus.is_some()
            || self.inspector.focused
            || !matches!(self.focus, Focus::Page | Focus::Control(_))
        {
            return false;
        }
        // Native OS shortcuts and browser navigation commands remain available
        // even if a page installs a handler that prevents every key.
        if meta || alt && !ctrl {
            return false;
        }
        if matches!(key, 0x72 | 0x74 | 0x75 | 0x7b | 0xa6 | 0xa7) {
            return false;
        }
        if key == 0x1b && (self.is_loading() || self.find_open) {
            return false;
        }
        if ctrl
            && !alt
            && (matches!(
                key,
                0x44 | 0x48
                    | 0xbc
                    | 0x4c
                    | 0x4f
                    | 0x52
                    | 0x46
                    | 0x55
                    | 0x49
                    | 0x4a
                    | 0xbb
                    | 0x6b
                    | 0xbd
                    | 0x6d
                    | 0x30
                    | 0x60
            ) || key == 0x42 && shift)
        {
            return false;
        }
        true
    }
}

pub(crate) fn named_key(key: u32) -> Option<&'static str> {
    Some(match key {
        0x03 => "Cancel",
        0x08 => "Backspace",
        0x09 => "Tab",
        0x0c => "Clear",
        0x0d => "Enter",
        0x10 | 0xa0 | 0xa1 => "Shift",
        0x11 | 0xa2 | 0xa3 => "Control",
        0x12 | 0xa4 | 0xa5 => "Alt",
        0x13 => "Pause",
        0x14 => "CapsLock",
        0x1b => "Escape",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0x23 => "End",
        0x24 => "Home",
        0x25 => "ArrowLeft",
        0x26 => "ArrowUp",
        0x27 => "ArrowRight",
        0x28 => "ArrowDown",
        0x2c => "PrintScreen",
        0x2d => "Insert",
        0x2e => "Delete",
        0x5b | 0x5c => "Meta",
        0x5d => "ContextMenu",
        0x70 => "F1",
        0x71 => "F2",
        0x72 => "F3",
        0x73 => "F4",
        0x74 => "F5",
        0x75 => "F6",
        0x76 => "F7",
        0x77 => "F8",
        0x78 => "F9",
        0x79 => "F10",
        0x7a => "F11",
        0x7b => "F12",
        0x7c => "F13",
        0x7d => "F14",
        0x7e => "F15",
        0x7f => "F16",
        0x80 => "F17",
        0x81 => "F18",
        0x82 => "F19",
        0x83 => "F20",
        0x84 => "F21",
        0x85 => "F22",
        0x86 => "F23",
        0x87 => "F24",
        0x90 => "NumLock",
        0x91 => "ScrollLock",
        0xa6 => "BrowserBack",
        0xa7 => "BrowserForward",
        0xe5 => "Process",
        _ => return None,
    })
}

#[cfg(test)]
fn fallback_code(key: u32) -> &'static str {
    match key {
        0x41 => "KeyA",
        0x42 => "KeyB",
        0x43 => "KeyC",
        0x44 => "KeyD",
        0x45 => "KeyE",
        0x46 => "KeyF",
        0x47 => "KeyG",
        0x48 => "KeyH",
        0x49 => "KeyI",
        0x4a => "KeyJ",
        0x4b => "KeyK",
        0x4c => "KeyL",
        0x4d => "KeyM",
        0x4e => "KeyN",
        0x4f => "KeyO",
        0x50 => "KeyP",
        0x51 => "KeyQ",
        0x52 => "KeyR",
        0x53 => "KeyS",
        0x54 => "KeyT",
        0x55 => "KeyU",
        0x56 => "KeyV",
        0x57 => "KeyW",
        0x58 => "KeyX",
        0x59 => "KeyY",
        0x5a => "KeyZ",
        0x20 => "Space",
        _ => named_key(key).unwrap_or("Unidentified"),
    }
}

/// Windows scan-code set 1 -> UI Events code. This intentionally uses physical
/// position, so a layout-derived "a" can still have code "KeyQ" on AZERTY.
pub(crate) fn physical_code(key: u32, scan: u32, extended: bool) -> &'static str {
    if key == 0x13 {
        return "Pause";
    }
    if key == 0x2c {
        return "PrintScreen";
    }
    if extended {
        return match scan {
            0x1c => "NumpadEnter",
            0x1d => "ControlRight",
            0x35 => "NumpadDivide",
            0x38 => "AltRight",
            0x47 => "Home",
            0x48 => "ArrowUp",
            0x49 => "PageUp",
            0x4b => "ArrowLeft",
            0x4d => "ArrowRight",
            0x4f => "End",
            0x50 => "ArrowDown",
            0x51 => "PageDown",
            0x52 => "Insert",
            0x53 => "Delete",
            0x5b => "MetaLeft",
            0x5c => "MetaRight",
            0x5d => "ContextMenu",
            _ => "Unidentified",
        };
    }
    match scan {
        0x01 => "Escape",
        0x02 => "Digit1",
        0x03 => "Digit2",
        0x04 => "Digit3",
        0x05 => "Digit4",
        0x06 => "Digit5",
        0x07 => "Digit6",
        0x08 => "Digit7",
        0x09 => "Digit8",
        0x0a => "Digit9",
        0x0b => "Digit0",
        0x0c => "Minus",
        0x0d => "Equal",
        0x0e => "Backspace",
        0x0f => "Tab",
        0x10 => "KeyQ",
        0x11 => "KeyW",
        0x12 => "KeyE",
        0x13 => "KeyR",
        0x14 => "KeyT",
        0x15 => "KeyY",
        0x16 => "KeyU",
        0x17 => "KeyI",
        0x18 => "KeyO",
        0x19 => "KeyP",
        0x1a => "BracketLeft",
        0x1b => "BracketRight",
        0x1c => "Enter",
        0x1d => "ControlLeft",
        0x1e => "KeyA",
        0x1f => "KeyS",
        0x20 => "KeyD",
        0x21 => "KeyF",
        0x22 => "KeyG",
        0x23 => "KeyH",
        0x24 => "KeyJ",
        0x25 => "KeyK",
        0x26 => "KeyL",
        0x27 => "Semicolon",
        0x28 => "Quote",
        0x29 => "Backquote",
        0x2a => "ShiftLeft",
        0x2b => "Backslash",
        0x2c => "KeyZ",
        0x2d => "KeyX",
        0x2e => "KeyC",
        0x2f => "KeyV",
        0x30 => "KeyB",
        0x31 => "KeyN",
        0x32 => "KeyM",
        0x33 => "Comma",
        0x34 => "Period",
        0x35 => "Slash",
        0x36 => "ShiftRight",
        0x37 => "NumpadMultiply",
        0x38 => "AltLeft",
        0x39 => "Space",
        0x3a => "CapsLock",
        0x3b => "F1",
        0x3c => "F2",
        0x3d => "F3",
        0x3e => "F4",
        0x3f => "F5",
        0x40 => "F6",
        0x41 => "F7",
        0x42 => "F8",
        0x43 => "F9",
        0x44 => "F10",
        0x45 => "NumLock",
        0x46 => "ScrollLock",
        0x47 => "Numpad7",
        0x48 => "Numpad8",
        0x49 => "Numpad9",
        0x4a => "NumpadSubtract",
        0x4b => "Numpad4",
        0x4c => "Numpad5",
        0x4d => "Numpad6",
        0x4e => "NumpadAdd",
        0x4f => "Numpad1",
        0x50 => "Numpad2",
        0x51 => "Numpad3",
        0x52 => "Numpad0",
        0x53 => "NumpadDecimal",
        0x56 => "IntlBackslash",
        0x57 => "F11",
        0x58 => "F12",
        _ if (0x70..=0x87).contains(&key) => named_key(key).unwrap_or("Unidentified"),
        _ => "Unidentified",
    }
}

pub(crate) fn key_location(code: &str) -> u8 {
    if code.starts_with("Numpad") {
        3
    } else if matches!(code, "ShiftLeft" | "ControlLeft" | "AltLeft" | "MetaLeft") {
        1
    } else if matches!(
        code,
        "ShiftRight" | "ControlRight" | "AltRight" | "MetaRight"
    ) {
        2
    } else {
        0
    }
}

#[cfg(test)]
#[path = "app_keyboard_tests.rs"]
mod tests;
