//! Owned, modeless developer-tools window. The browser's Box<App> remains the
//! sole owner. Userdata stores the owner HWND, never another owning App pointer.
//! No App borrow crosses a Win32 call that can dispatch messages.
use super::*;

const CLASS: &str = "ChezCatInspectorWindow";
std::thread_local! {
    static WINDOW: Cell<Option<(usize, usize)>> = const { Cell::new(None) };
}

pub(super) unsafe fn register(instance: Handle, icon: Handle) -> Result<(), String> {
    let name = wide(CLASS);
    let class = WindowClass {
        style: 0x0003,
        procedure: Some(procedure),
        class_extra: 0,
        window_extra: 0,
        instance,
        icon,
        cursor: LoadCursorW(null_mut(), 32512usize as *const u16),
        background: null_mut(),
        menu_name: null(),
        class_name: name.as_ptr(),
    };
    if RegisterClassW(&class) == 0 {
        Err(format!(
            "Inspector window class: {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

pub(super) unsafe fn unregister(instance: Handle) {
    if let Some((_, window)) = WINDOW.with(|slot| slot.replace(None)) {
        DestroyWindow(window as Handle);
    }
    let name = wide(CLASS);
    UnregisterClassW(name.as_ptr(), instance);
}

pub(super) unsafe fn sync(owner: Handle, state: *mut App) {
    if !state_is_attached(owner, state) {
        return;
    }
    let detached = (&*state).inspector_detached();
    let existing = WINDOW.with(|slot| slot.get());
    if detached {
        if let Some((parent, window)) = existing {
            if parent == owner as usize {
                let colors = (&*state).title_bar_colors();
                sync_frame_colors(window as Handle, colors);
                InvalidateRect(window as Handle, null(), 0);
            }
            return;
        }
        let name = wide(CLASS);
        let title = wide("Developer tools - ChezCat");
        let window = CreateWindowExW(
            0,
            name.as_ptr(),
            title.as_ptr(),
            0x00cf0000,
            i32::MIN,
            i32::MIN,
            1040,
            680,
            owner,
            null_mut(),
            GetModuleHandleW(null()),
            owner,
        );
        if window.is_null() {
            let error = std::io::Error::last_os_error().to_string();
            if state_is_attached(owner, state) {
                (&mut *state).inspector_detach_failed(error);
            }
            return;
        }
        if !state_is_attached(owner, state) {
            DestroyWindow(window);
            return;
        }
        WINDOW.with(|slot| slot.set(Some((owner as usize, window as usize))));
        let colors = (&*state).title_bar_colors();
        sync_frame_colors(window, colors);
        ShowWindow(window, 5);
        UpdateWindow(window);
    } else if let Some((parent, window)) = existing {
        if parent == owner as usize {
            WINDOW.with(|slot| slot.set(None));
            DestroyWindow(window as Handle);
        }
    }
}

unsafe fn app(window: Handle) -> Option<(Handle, *mut App)> {
    let owner = GetWindowLongPtrW(window, -21) as Handle;
    if owner.is_null() {
        return None;
    }
    let state = GetWindowLongPtrW(owner, -21) as *mut App;
    (!state.is_null()).then_some((owner, state))
}

unsafe fn action(window: Handle, owner: Handle, state: *mut App, action: PlatformAction) {
    let result = match action {
        PlatformAction::None | PlatformAction::OpenFile => Ok(()),
        PlatformAction::OpenFolder(path) => open_folder(window, &path),
        PlatformAction::Copy(text) => write_clipboard(window, &text),
        PlatformAction::CopyBuildInformation(_) => Err("Build-information copy belongs to the browser About panel.".into()),
        PlatformAction::Cut(text) => match write_clipboard(window, &text) {
            Ok(()) => {
                if state_is_attached(owner, state) {
                    (&mut *state).inspector_cut_copied(&text);
                }
                Ok(())
            }
            Err(error) => Err(error),
        },
        PlatformAction::Paste => match read_clipboard(window) {
            Ok(Some(text)) => {
                if state_is_attached(owner, state) {
                    (&mut *state).inspector_paste(&text);
                }
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(error) => Err(error),
        },
    };
    if let Err(error) = result {
        if state_is_attached(owner, state) {
            let text = wide(&error);
            let title = wide("ChezCat inspector");
            MessageBoxW(window, text.as_ptr(), title.as_ptr(), 0x10);
        }
    }
}

unsafe extern "system" fn procedure(
    window: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        0x0081 => {
            let create = &*(lparam as *const CreateStruct);
            SetLastError(0);
            let previous = SetWindowLongPtrW(window, -21, create.parameters as isize);
            if previous == 0 && GetLastError() != 0 {
                return 0;
            }
            return 1;
        }
        0x0082 => {
            forget_frame_colors(window);
            WINDOW.with(|slot| {
                if slot
                    .get()
                    .is_some_and(|(_, handle)| handle == window as usize)
                {
                    slot.set(None);
                }
            });
            SetWindowLongPtrW(window, -21, 0);
            return DefWindowProcW(window, message, wparam, lparam);
        }
        // Closing a developer window must never terminate the browser's loop.
        0x0002 => return 0,
        0x0014 => return 1,
        0x0024 => {
            (&mut *(lparam as *mut MinMaxInfo)).min_track_size = Point { x: 480, y: 260 };
            return 0;
        }
        _ => {}
    }
    let Some((owner, state)) = app(window) else {
        return DefWindowProcW(window, message, wparam, lparam);
    };
    match message {
        0x031a | 0x0320 => {
            forget_frame_colors(window);
            let colors = (&*state).title_bar_colors();
            sync_frame_colors(window, colors);
            DefWindowProcW(window, message, wparam, lparam)
        }
        0x0010 => {
            (&mut *state).close_inspector();
            refresh(owner, state);
            0
        }
        0x0007 => {
            (&mut *state).inspector_focus();
            InvalidateRect(window, null(), 0);
            0
        }
        0x0008 | 0x001f | 0x0215 => {
            (&mut *state).inspector_cancel_pointer();
            if message != 0x0215 && GetCapture() == window {
                ReleaseCapture();
            }
            InvalidateRect(window, null(), 0);
            DefWindowProcW(window, message, wparam, lparam)
        }
        0x0005 => {
            let mut rect: Rect = zeroed();
            GetClientRect(window, &mut rect);
            if rect.right > 0 && rect.bottom > 0 {
                (&mut *state).inspector_window_size(rect.right, rect.bottom);
            }
            InvalidateRect(window, null(), 0);
            0
        }
        0x000f => {
            let mut paint: PaintStruct = zeroed();
            let dc = BeginPaint(window, &mut paint);
            let mut rect: Rect = zeroed();
            GetClientRect(window, &mut rect);
            if !dc.is_null() && rect.right > 0 && rect.bottom > 0 {
                let canvas = {
                    let app = &mut *state;
                    app.inspector_window_size(rect.right, rect.bottom);
                    app.draw_inspector_window()
                };
                present_canvas(dc, &rect, &canvas);
            }
            EndPaint(window, &paint);
            0
        }
        0x020a => {
            let (x, y) = mouse_coordinates(lparam);
            let mut point = Point { x, y };
            ScreenToClient(window, &mut point);
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
            let ctrl = key_is_down(0x11);
            let shift = key_is_down(0x10);
            (&mut *state).inspector_wheel(point.x, point.y, delta, ctrl, shift);
            InvalidateRect(window, null(), 0);
            0
        }
        0x020b if matches!((wparam >> 16) & 0xffff, 1 | 2) => 1,
        0x020c if matches!((wparam >> 16) & 0xffff, 1 | 2) => {
            if GetCapture() == window {
                ReleaseCapture();
            }
            (&mut *state).mouse_navigation((wparam >> 16) & 0xffff == 2);
            refresh(owner, state);
            1
        }
        0x0319 if matches!((lparam as usize >> 16) & 0x0fff, 1 | 2) => {
            if GetCapture() == window {
                ReleaseCapture();
            }
            (&mut *state).mouse_navigation((lparam as usize >> 16) & 0x0fff == 2);
            refresh(owner, state);
            1
        }
        0x0100 => {
            let key = wparam as u32;
            if matches!(key, 0xa6 | 0xa7) {
                (&mut *state).mouse_navigation(key == 0xa7);
                refresh(owner, state);
                return 0;
            }
            let ctrl = key_is_down(0x11);
            let shift = key_is_down(0x10);
            let alt = key_is_down(0x12);
            if ctrl && alt {
                return DefWindowProcW(window, message, wparam, lparam);
            }
            let next = (&mut *state).inspector_key(key, ctrl, shift, alt);
            action(window, owner, state, next);
            refresh(owner, state);
            0
        }
        0x0102 => {
            let ctrl = key_is_down(0x11);
            let alt = key_is_down(0x12);
            if wparam <= u16::MAX as usize && wparam >= 0x20 && wparam != 0x7f && (!ctrl || alt) {
                (&mut *state).inspector_utf16_input(wparam as u16);
                InvalidateRect(window, null(), 0);
            }
            0
        }
        0x0109 => {
            if wparam == 0xffff {
                return 1;
            }
            if let Some(ch) = u32::try_from(wparam)
                .ok()
                .and_then(char::from_u32)
                .filter(|ch| !ch.is_control())
            {
                let mut buffer = [0u8; 4];
                (&mut *state).inspector_paste(ch.encode_utf8(&mut buffer));
                InvalidateRect(window, null(), 0);
            }
            0
        }
        0x0201 => {
            let (x, y) = mouse_coordinates(lparam);
            SetFocus(window);
            SetCapture(window);
            (&mut *state).inspector_pointer_down(x, y);
            InvalidateRect(window, null(), 0);
            0
        }
        0x0200 => {
            let (x, y) = mouse_coordinates(lparam);
            let dragging = wparam & 1 != 0 && GetCapture() == window;
            if (&mut *state).inspector_pointer_move(x, y, dragging) {
                InvalidateRect(window, null(), 0);
            }
            let hand = (&*state).inspector_wants_hand(x, y);
            SetCursor(LoadCursorW(
                null_mut(),
                (if hand { 32649usize } else { 32512usize }) as *const u16,
            ));
            0
        }
        0x0202 => {
            let (x, y) = mouse_coordinates(lparam);
            let next = (&mut *state).inspector_pointer_up(x, y);
            if GetCapture() == window {
                ReleaseCapture();
            }
            action(window, owner, state, next);
            refresh(owner, state);
            0
        }
        0x0020 if lparam & 0xffff == 1 => {
            let mut point = Point { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 && ScreenToClient(window, &mut point) != 0 {
                let hand = (&*state).inspector_wants_hand(point.x, point.y);
                SetCursor(LoadCursorW(
                    null_mut(),
                    (if hand { 32649usize } else { 32512usize }) as *const u16,
                ));
                1
            } else {
                DefWindowProcW(window, message, wparam, lparam)
            }
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}
