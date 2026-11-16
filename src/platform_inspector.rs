//! Each auxiliary window is pinned to one complete Session/Window/Tab identity.
//! Userdata contains a checked numeric native key, never an App or binding pointer.
use super::session_window::{self as host, Binding, InputOwner, Owner};
use super::*;
use crate::app::session::WindowId;
const CLASS: &str = "ChezCatInspectorWindow";

pub(super) unsafe fn register(instance: Handle, icon: Handle) -> Result<(), String> {
    let name = wide(CLASS);
    let class = WindowClass {
        style: 3,
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
    for route in host::all_routes() {
        if matches!(route.owner, Owner::Inspector(..)) {
            host::retire_route(route);
        }
    }
    let name = wide(CLASS);
    UnregisterClassW(name.as_ptr(), instance);
}
// Bounded by the browser's tab limit. Multiple detached tools remain pinned to
// their original pages when the selected tab changes, including inactive pages.
pub(super) unsafe fn sync(window: WindowId) {
    let Some(main) = host::main_route(window) else {
        return;
    };
    for tab in host::tab_ids(window) {
        if !host::attached(main) {
            return;
        }
        let origin = InputOwner {
            window,
            tab,
            inspector: true,
        };
        let Ok((detached, colors)) = observe_owner(main, origin, |app| {
            (app.inspector_detached(), app.title_bar_colors())
        }) else {
            continue;
        };
        let existing = host::inspector_routes(window)
            .into_iter()
            .find(|r| r.owner == Owner::Inspector(window, tab) && !r.closing);
        if !detached {
            if let Some(route) = existing {
                host::retire_route(route);
            }
            continue;
        }
        if let Some(route) = existing {
            if host::attached(route) {
                sync_frame_colors(route.hwnd as Handle, colors);
                if host::attached(route) {
                    InvalidateRect(route.hwnd as Handle, null(), 0);
                }
            }
            continue;
        }
        let key = match host::reserve_inspector(window, tab) {
            Ok(key) => key,
            Err(error) => {
                let _ = with_owner(main, origin, |app| app.inspector_detach_failed(error));
                continue;
            }
        };
        let name = wide(CLASS);
        let title = wide("Developer tools - ChezCat");
        let hwnd = CreateWindowExW(
            0,
            name.as_ptr(),
            title.as_ptr(),
            0x00cf0000,
            i32::MIN,
            i32::MIN,
            1040,
            680,
            main.hwnd as Handle,
            null_mut(),
            GetModuleHandleW(null()),
            key as *mut c_void,
        );
        if hwnd.is_null() {
            host::forget_key(key);
            let error = std::io::Error::last_os_error().to_string();
            let _ = with_owner(main, origin, |app| app.inspector_detach_failed(error));
            continue;
        }
        let Some(route) = host::binding(hwnd)
            .filter(|route| route.key == key && route.owner == Owner::Inspector(window, tab))
        else {
            host::retire_reserved(key);
            continue;
        };
        if !host::attached(main)
            || observe_owner(main, origin, |a| a.inspector_detached()).ok() != Some(true)
        {
            host::retire_reserved(key);
            continue;
        }
        sync_frame_colors(hwnd, colors);
        if host::show_auxiliary() && host::attached(route) {
            ShowWindow(hwnd, 5);
            if host::attached(route) {
                UpdateWindow(hwnd);
            }
        }
    }
}
fn with_owner<T>(
    route: Binding,
    origin: InputOwner,
    action: impl FnOnce(&mut App) -> T,
) -> Result<T, String> {
    if !host::attached(route) {
        return Err("The inspector native owner has retired.".into());
    }
    host::with_tab(origin, action)
}
fn observe_owner<T>(
    route: Binding,
    origin: InputOwner,
    action: impl FnOnce(&App) -> T,
) -> Result<T, String> {
    if !host::attached(route) {
        return Err("The inspector native owner has retired.".into());
    }
    host::observe_tab(origin, action)
}
unsafe fn repaint(route: Binding, origin: InputOwner) {
    if !host::attached(route) {
        return;
    }
    host::refresh_window(origin.window);
    if host::attached(route) {
        InvalidateRect(route.hwnd as Handle, null(), 0);
    }
    host::schedule();
}
unsafe fn cursor(route: Binding, origin: InputOwner, x: i32, y: i32) {
    if !host::attached(route) {
        return;
    }
    let hand = observe_owner(route, origin, |a| a.inspector_wants_hand(x, y)).unwrap_or(false);
    let cursor = LoadCursorW(
        null_mut(),
        (if hand { 32649usize } else { 32512usize }) as *const u16,
    );
    if host::attached(route) {
        SetCursor(cursor);
    }
}
unsafe extern "system" fn procedure(
    hwnd: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        0x0081 => {
            let create = &*(lparam as *const CreateStruct);
            let key = create.parameters as usize;
            if !host::attach_key(key, hwnd) {
                return 0;
            }
            SetLastError(0);
            let previous = SetWindowLongPtrW(hwnd, -21, key as isize);
            if previous == 0 && GetLastError() != 0 {
                host::forget_key(key);
                return 0;
            }
            return 1;
        }
        0x0082 => {
            host::destroyed(hwnd);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        0x0002 => return 0,
        0x0014 => return 1,
        0x0024 => {
            (&mut *(lparam as *mut MinMaxInfo)).min_track_size = Point { x: 480, y: 260 };
            return 0;
        }
        _ => {}
    }
    let Some(route) = host::binding(hwnd) else {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    let Some(origin) = host::owner_for(route) else {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    match message {
        0x031a | 0x0320 => {
            forget_frame_colors(hwnd);
            if let Ok(colors) = observe_owner(route, origin, |a| a.title_bar_colors()) {
                sync_frame_colors(hwnd, colors);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        0x0010 => {
            let _ = with_owner(route, origin, |a| a.close_inspector());
            host::refresh_window(origin.window);
            0
        }
        0x0007 => {
            host::activate_window(origin.window);
            let _ = with_owner(route, origin, |a| a.inspector_focus());
            InvalidateRect(hwnd, null(), 0);
            0
        }
        0x0008 | 0x001f | 0x0215 => {
            let _ = with_owner(route, origin, |a| a.inspector_cancel_pointer());
            host::forget_input(hwnd);
            if message != 0x0215 && GetCapture() == hwnd {
                ReleaseCapture();
            }
            if host::attached(route) {
                InvalidateRect(hwnd, null(), 0);
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        0x0005 => {
            let mut rect: Rect = zeroed();
            GetClientRect(hwnd, &mut rect);
            if rect.right > 0 && rect.bottom > 0 {
                let _ = with_owner(route, origin, |a| {
                    a.inspector_window_size(rect.right, rect.bottom)
                });
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        0x000f => {
            let mut paint: PaintStruct = zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut rect: Rect = zeroed();
            GetClientRect(hwnd, &mut rect);
            if !dc.is_null() && rect.right > 0 && rect.bottom > 0 && host::attached(route) {
                let canvas = with_owner(route, origin, |a| {
                    a.inspector_window_size(rect.right, rect.bottom);
                    a.draw_inspector_window()
                });
                if let Ok(canvas) = canvas {
                    present_canvas(dc, &rect, &canvas);
                }
            }
            EndPaint(hwnd, &paint);
            0
        }
        0x020a => {
            let (x, y) = mouse_coordinates(lparam);
            let mut point = Point { x, y };
            ScreenToClient(hwnd, &mut point);
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
            let ctrl = key_is_down(0x11);
            let shift = key_is_down(0x10);
            let _ = with_owner(route, origin, |a| {
                a.inspector_wheel(point.x, point.y, delta, ctrl, shift)
            });
            repaint(route, origin);
            0
        }
        0x020b if matches!((wparam >> 16) & 0xffff, 1 | 2) => 1,
        0x020c if matches!((wparam >> 16) & 0xffff, 1 | 2) => {
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            let _ = with_owner(route, origin, |a| {
                a.mouse_navigation((wparam >> 16) & 0xffff == 2)
            });
            repaint(route, origin);
            1
        }
        0x0319 if matches!((lparam as usize >> 16) & 0x0fff, 1 | 2) => {
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            let _ = with_owner(route, origin, |a| {
                a.mouse_navigation((lparam as usize >> 16) & 0x0fff == 2)
            });
            repaint(route, origin);
            1
        }
        0x0100 | 0x0104 => {
            let key = wparam as u32;
            let Some(origin) = host::key_owner(hwnd, key, true, route) else {
                return 0;
            };
            if matches!(key, 0xa6 | 0xa7) {
                let _ = with_owner(route, origin, |a| a.mouse_navigation(key == 0xa7));
                keyboard_input::cancel(hwnd, message, wparam, lparam);
                repaint(route, origin);
                return 0;
            }
            let ctrl = key_is_down(0x11);
            let shift = key_is_down(0x10);
            let alt = key_is_down(0x12);
            if ctrl && alt {
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            if ctrl || alt {
                keyboard_input::cancel(hwnd, message, wparam, lparam);
            }
            host::inspector_request(route, origin, |a| a.inspector_key(key, ctrl, shift, alt));
            repaint(route, origin);
            0
        }
        0x0101 | 0x0105 => {
            let _ = host::key_owner(hwnd, wparam as u32, false, route);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        0x0102 => {
            let ctrl = key_is_down(0x11);
            let alt = key_is_down(0x12);
            if wparam <= u16::MAX as usize && wparam >= 0x20 && wparam != 0x7f && (!ctrl || alt) {
                if let Some(origin) = host::text_owner(hwnd, lparam) {
                    let _ = with_owner(route, origin, |a| a.inspector_utf16_input(wparam as u16));
                    repaint(route, origin);
                }
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
                let _ = with_owner(route, origin, |a| {
                    a.inspector_paste(ch.encode_utf8(&mut buffer))
                });
                repaint(route, origin);
            }
            0
        }
        0x0201 => {
            let (x, y) = mouse_coordinates(lparam);
            SetFocus(hwnd);
            if !host::attached(route) {
                return 0;
            }
            SetCapture(hwnd);
            if host::attached(route) {
                let _ = with_owner(route, origin, |a| a.inspector_pointer_down(x, y));
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        0x0200 => {
            let (x, y) = mouse_coordinates(lparam);
            let dragging = wparam & 1 != 0 && GetCapture() == hwnd;
            if with_owner(route, origin, |a| a.inspector_pointer_move(x, y, dragging))
                .unwrap_or(false)
            {
                InvalidateRect(hwnd, null(), 0);
            }
            cursor(route, origin, x, y);
            0
        }
        0x0202 => {
            let (x, y) = mouse_coordinates(lparam);
            let request = with_owner(route, origin, |a| {
                let action = a.inspector_pointer_up(x, y);
                a.inspector_platform_request(action)
            });
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            if let Ok(request) = request {
                host::perform_request(route, origin, request);
            }
            repaint(route, origin);
            0
        }
        0x0020 if lparam & 0xffff == 1 => {
            let mut point = Point { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 && ScreenToClient(hwnd, &mut point) != 0 {
                cursor(route, origin, point.x, point.y);
                1
            } else {
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
