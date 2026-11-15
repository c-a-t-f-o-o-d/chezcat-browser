//! Native keyboard values and per-message text cancellation. ToUnicodeEx's
//! no-state-change flag preserves dead-key/AltGr composition for TranslateMessage
//! (Windows 10 version 1607 or later). No keyboard layout details reach pages.
use super::*;
use crate::app::keyboard::{key_location, named_key, physical_code};
use crate::scripting::KeyboardInput;

type KeyMessage = (usize, u32, usize, isize);
std::thread_local! {
    static TRANSLATION: Cell<Option<(KeyMessage, bool)>> = const { Cell::new(None) };
}

fn identity(message: &Message) -> KeyMessage {
    (
        message.window as usize,
        message.message,
        message.wparam,
        message.lparam,
    )
}

pub(super) fn prepare(message: &Message) {
    TRANSLATION.with(|slot| slot.set(Some((identity(message), false))));
}

pub(super) fn complete(message: &Message) -> bool {
    TRANSLATION.with(|slot| {
        slot.replace(None)
            .is_some_and(|(key, canceled)| key == identity(message) && canceled)
    })
}

pub(super) fn cancel(window: Handle, message: u32, wparam: usize, lparam: isize) {
    TRANSLATION.with(|slot| {
        if let Some((key, _)) = slot.get() {
            if key == (window as usize, message, wparam, lparam) {
                slot.set(Some((key, true)));
            }
        }
    });
}

pub(super) unsafe fn event(key: u32, lparam: isize, down: bool, system: bool) -> KeyboardInput {
    let ctrl = key_is_down(0x11);
    let shift = key_is_down(0x10);
    let alt = key_is_down(0x12) || system && lparam & (1 << 29) != 0;
    let meta = key_is_down(0x5b) || key_is_down(0x5c);
    let scan = (lparam as usize >> 16) as u32 & 0xff;
    let extended = lparam & (1 << 24) != 0;
    let code = physical_code(key, scan, extended);
    let logical = if let Some(name) = named_key(key) {
        name.to_owned()
    } else {
        let mut state = [0u8; 256];
        if GetKeyboardState(state.as_mut_ptr()) == 0 {
            "Unidentified".into()
        } else {
            // Ctrl+A emits a control character to WM_CHAR, but event.key is the
            // printable layout key. Preserve Ctrl+Alt because it can be AltGr.
            if ctrl && !alt {
                for index in [0x11, 0xa2, 0xa3] {
                    state[index] = 0;
                }
            }
            let mut buffer = [0u16; 16];
            let count = ToUnicodeEx(
                key,
                scan,
                state.as_ptr(),
                buffer.as_mut_ptr(),
                buffer.len() as i32,
                4,
                GetKeyboardLayout(0),
            );
            if count < 0 {
                "Dead".into()
            } else if count == 0 {
                "Unidentified".into()
            } else {
                let text = String::from_utf16_lossy(&buffer[..(count as usize).min(buffer.len())]);
                if text.chars().any(char::is_control) {
                    "Unidentified".into()
                } else {
                    text
                }
            }
        }
    };
    KeyboardInput {
        down,
        target: None,
        key: logical,
        code: code.into(),
        ctrl,
        shift,
        alt,
        meta,
        repeat: down && lparam & (1 << 30) != 0,
        location: key_location(code),
        key_code: key,
    }
}
