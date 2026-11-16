//! Native keyboard values and per-message text cancellation. ToUnicodeEx's
//! no-state-change flag preserves dead-key/AltGr composition for TranslateMessage
//! (Windows 10 version 1607 or later). No keyboard layout details reach pages.
use super::*;
use crate::app::keyboard::{key_location, named_key, physical_code};
use crate::scripting::KeyboardInput;

type KeyMessage = (usize, u32, usize, isize);
std::thread_local! {
    // Modal/native nested dispatch must not overwrite an outer key's decision.
    static TRANSLATION: RefCell<Vec<(KeyMessage, bool)>> = const { RefCell::new(Vec::new()) };
}
fn identity(message: &Message) -> KeyMessage {
    (message.window as usize,message.message,message.wparam,message.lparam)
}
pub(super) fn prepare(message: &Message) -> bool {
    TRANSLATION.with(|slot|{let Ok(mut stack)=slot.try_borrow_mut() else{return false;};
        if stack.len()==32{return false;}stack.push((identity(message),false));true})
}
pub(super) fn complete(message: &Message) -> bool {
    TRANSLATION.with(|slot|{let Ok(mut stack)=slot.try_borrow_mut() else{return true;};
        let Some(index)=stack.iter().rposition(|(key,_)|*key==identity(message)) else{return true;};stack.remove(index).1})
}
pub(super) fn cancel(window: Handle,message:u32,wparam:usize,lparam:isize) {
    TRANSLATION.with(|slot|{if let Ok(mut stack)=slot.try_borrow_mut(){if let Some((_,canceled))=stack.iter_mut().rev().find(|(key,_)|*key==(window as usize,message,wparam,lparam)){*canceled=true;}}});
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

#[cfg(test)]
mod tests {
    use super::*;
    fn message(key:usize)->Message{Message{window:7usize as Handle,message:0x0100,wparam:key,lparam:1,time:0,point:Point{x:0,y:0},private:0}}
    #[test] fn nested_native_translation_keeps_outer_cancel_decision(){
        let outer=message(65);let inner=message(66);assert!(prepare(&outer));cancel(outer.window,outer.message,outer.wparam,outer.lparam);
        assert!(prepare(&inner));assert!(!complete(&inner));assert!(complete(&outer));assert!(complete(&outer));
    }
}
