//! Windows events, file picking, clipboard, and pixel presentation.
//! Windows draws native frames/dialogs; client-area layout and text remain ours.
use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::ptr::{null, null_mut};

use crate::app::{App, PlatformAction};

#[path = "platform_inspector.rs"]
mod inspector_window;
#[path = "platform_keyboard.rs"]
mod keyboard_input;
#[path = "platform_license.rs"]
mod license_window;

pub fn license_prompt(
    script: Option<&std::path::Path>,
    report: Option<&std::path::Path>,
) -> Result<Option<crate::license_acceptance::Scope>, String> {
    license_window::run(script, report)
}

type Handle = *mut c_void;
type WndProc = unsafe extern "system" fn(Handle, u32, usize, isize) -> isize;
type FileDialogHook = unsafe extern "system" fn(Handle, u32, usize, isize) -> usize;

const MAX_CLIPBOARD_BYTES: usize = 2 * 1024 * 1024;
const CF_UNICODETEXT: u32 = 13;
const SCRIPT_TIMER_ID: usize = 0x4f42;
const HISTORY_TIMER_ID: usize = 0x4f43;
const DOWNLOAD_TIMER_ID: usize = 0x4f44;
const APP_ICON_ID: usize = 101;
std::thread_local! {
    // The shell owns one window. Remember its armed deadline so mouse/paint
    // activity cannot keep resetting and postponing the same timer.
    static SCRIPT_TIMER_WAKE: Cell<Option<(usize, std::time::Instant)>> = const { Cell::new(None) };
    static HISTORY_TIMER_WAKE: Cell<Option<(usize, std::time::Instant)>> = const { Cell::new(None) };
    static DOWNLOAD_TIMER_OWNER: Cell<Option<(usize, u32)>> = const { Cell::new(None) };
    static FRAME_COLORS: RefCell<Vec<(usize, u32, u32)>> = const { RefCell::new(Vec::new()) };
}

#[repr(C)]
struct Point {
    x: i32,
    y: i32,
}
#[repr(C)]
struct MouseTracking {
    size: u32,
    flags: u32,
    window: Handle,
    hover_time: u32,
}
#[repr(C)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}
#[repr(C)]
struct WindowClass {
    style: u32,
    procedure: Option<WndProc>,
    class_extra: i32,
    window_extra: i32,
    instance: Handle,
    icon: Handle,
    cursor: Handle,
    background: Handle,
    menu_name: *const u16,
    class_name: *const u16,
}
#[repr(C)]
struct Message {
    window: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}
#[repr(C)]
struct PaintStruct {
    dc: Handle,
    erase: i32,
    rect: Rect,
    restore: i32,
    incremental: i32,
    reserved: [u8; 32],
}
#[repr(C)]
struct BitmapHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bits: u16,
    compression: u32,
    image_size: u32,
    x_pixels_per_meter: i32,
    y_pixels_per_meter: i32,
    colors_used: u32,
    important_colors: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapHeader,
    colors: [u32; 1],
}
#[repr(C)]
struct CreateStruct {
    parameters: *mut c_void,
    instance: Handle,
    menu: Handle,
    parent: Handle,
    height: i32,
    width: i32,
    y: i32,
    x: i32,
    style: i32,
    name: *const u16,
    class: *const u16,
    extended_style: u32,
}
#[repr(C)]
struct MinMaxInfo {
    reserved: Point,
    max_size: Point,
    max_position: Point,
    min_track_size: Point,
    max_track_size: Point,
}
#[repr(C)]
struct OpenFileName {
    size: u32,
    owner: Handle,
    instance: Handle,
    filter: *const u16,
    custom_filter: *mut u16,
    max_custom_filter: u32,
    filter_index: u32,
    file: *mut u16,
    max_file: u32,
    file_title: *mut u16,
    max_file_title: u32,
    initial_directory: *const u16,
    title: *const u16,
    flags: u32,
    file_offset: u16,
    file_extension: u16,
    default_extension: *const u16,
    custom_data: isize,
    hook: Option<FileDialogHook>,
    template_name: *const u16,
    reserved: *mut c_void,
    reserved_value: u32,
    extended_flags: u32,
}

// Exact Windows x64 C layouts; build errors catch accidental ABI changes.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(size_of::<WindowClass>() == 72);
    assert!(size_of::<Message>() == 48);
    assert!(size_of::<PaintStruct>() == 72);
    assert!(size_of::<BitmapHeader>() == 40);
    assert!(size_of::<BitmapInfo>() == 44);
    assert!(size_of::<CreateStruct>() == 80);
    assert!(size_of::<MinMaxInfo>() == 40);
    assert!(size_of::<OpenFileName>() == 152);
    assert!(size_of::<MouseTracking>() == 24);
};

#[link(name = "user32")]
extern "system" {
    fn RegisterClassW(class: *const WindowClass) -> u16;
    fn UnregisterClassW(class: *const u16, instance: Handle) -> i32;
    fn CreateWindowExW(
        ex_style: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: Handle,
        menu: Handle,
        instance: Handle,
        parameters: *mut c_void,
    ) -> Handle;
    fn DefWindowProcW(window: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
    fn ShowWindow(window: Handle, command: i32) -> i32;
    fn UpdateWindow(window: Handle) -> i32;
    fn DestroyWindow(window: Handle) -> i32;
    fn GetMessageW(message: *mut Message, window: Handle, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Message) -> i32;
    fn DispatchMessageW(message: *const Message) -> isize;
    fn PostQuitMessage(code: i32);
    fn PostMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> i32;
    fn SendMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
    fn SetTimer(
        window: Handle,
        id: usize,
        milliseconds: u32,
        callback: Option<unsafe extern "system" fn(Handle, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(window: Handle, id: usize) -> i32;
    fn BeginPaint(window: Handle, paint: *mut PaintStruct) -> Handle;
    fn EndPaint(window: Handle, paint: *const PaintStruct) -> i32;
    fn GetClientRect(window: Handle, rect: *mut Rect) -> i32;
    fn InvalidateRect(window: Handle, rect: *const Rect, erase: i32) -> i32;
    fn GetWindowLongPtrW(window: Handle, index: i32) -> isize;
    fn SetWindowLongPtrW(window: Handle, index: i32, value: isize) -> isize;
    fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    fn LoadImageW(
        instance: Handle,
        name: *const u16,
        image_type: u32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> Handle;
    fn GetSystemMetrics(index: i32) -> i32;
    fn DestroyIcon(icon: Handle) -> i32;
    fn SetCursor(cursor: Handle) -> Handle;
    fn GetCursorPos(point: *mut Point) -> i32;
    fn TrackMouseEvent(tracking: *mut MouseTracking) -> i32;
    fn ScreenToClient(window: Handle, point: *mut Point) -> i32;
    fn GetKeyState(key: i32) -> i16;
    fn GetKeyboardState(state: *mut u8) -> i32;
    fn GetKeyboardLayout(thread: u32) -> Handle;
    fn ToUnicodeEx(
        key: u32,
        scan: u32,
        state: *const u8,
        buffer: *mut u16,
        count: i32,
        flags: u32,
        layout: Handle,
    ) -> i32;
    fn SetFocus(window: Handle) -> Handle;
    fn SetCapture(window: Handle) -> Handle;
    fn GetCapture() -> Handle;
    fn ReleaseCapture() -> i32;
    fn OpenClipboard(owner: Handle) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardData(format: u32) -> Handle;
    fn SetClipboardData(format: u32, memory: Handle) -> Handle;
    fn SetWindowTextW(window: Handle, text: *const u16) -> i32;
    fn SetProcessDPIAware() -> i32;
    fn MessageBoxW(window: Handle, text: *const u16, caption: *const u16, kind: u32) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn SetLastError(error: u32);
    fn GetLastError() -> u32;
    fn GlobalAlloc(flags: u32, bytes: usize) -> Handle;
    fn GlobalFree(memory: Handle) -> Handle;
    fn GlobalLock(memory: Handle) -> *mut c_void;
    fn GlobalUnlock(memory: Handle) -> i32;
    fn GlobalSize(memory: Handle) -> usize;
}
#[link(name = "gdi32")]
extern "system" {
    fn StretchDIBits(
        dc: Handle,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        src_x: i32,
        src_y: i32,
        src_width: i32,
        src_height: i32,
        pixels: *const c_void,
        info: *const BitmapInfo,
        usage: u32,
        operation: u32,
    ) -> i32;
}
#[link(name = "shell32")]
extern "system" {
    fn DragAcceptFiles(window: Handle, accept: i32);
    fn DragQueryFileW(drop: Handle, index: u32, file: *mut u16, count: u32) -> u32;
    fn DragFinish(drop: Handle);
    fn ShellExecuteW(
        owner: Handle,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show: i32,
    ) -> Handle;
}
#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
    fn CoUninitialize();
}
#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(
        window: Handle,
        attribute: u32,
        value: *const c_void,
        size: u32,
    ) -> i32;
}
#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(dialog: *mut OpenFileName) -> i32;
    fn CommDlgExtendedError() -> u32;
}

fn wide(value: &str) -> Vec<u16> {
    value
        .encode_utf16()
        .filter(|c| *c != 0)
        .chain(Some(0))
        .collect()
}

// Own each requested size separately: LR_SHARED can reuse a cached handle for
// the same resource name regardless of the new requested dimensions. Keep these
// alive until the window is destroyed and its class has been unregistered.
struct WindowIcons {
    large: Handle,
    small: Handle,
}

impl WindowIcons {
    unsafe fn load(instance: Handle) -> Result<Self, String> {
        let mut icons = Self {
            large: null_mut(),
            small: null_mut(),
        };
        icons.large = LoadImageW(
            instance,
            APP_ICON_ID as *const u16,
            1,                    // IMAGE_ICON
            GetSystemMetrics(11), // SM_CXICON
            GetSystemMetrics(12), // SM_CYICON
            0,
        );
        if icons.large.is_null() {
            return Err(format!(
                "Cannot load the embedded ChezCat icon: {}",
                std::io::Error::last_os_error()
            ));
        }
        icons.small = LoadImageW(
            instance,
            APP_ICON_ID as *const u16,
            1,                    // IMAGE_ICON
            GetSystemMetrics(49), // SM_CXSMICON
            GetSystemMetrics(50), // SM_CYSMICON
            0,
        );
        if icons.small.is_null() {
            return Err(format!(
                "Cannot load the embedded ChezCat small icon: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(icons)
    }
}

impl Drop for WindowIcons {
    fn drop(&mut self) {
        unsafe {
            if !self.small.is_null() {
                DestroyIcon(self.small);
            }
            if !self.large.is_null() {
                DestroyIcon(self.large);
            }
        }
    }
}

pub fn show_error(message: &str) {
    show_message(message, 0x10);
}

pub fn show_info(message: &str) {
    show_message(message, 0x40);
}

fn show_message(message: &str, kind: u32) {
    let text = wide(message);
    let title = wide("ChezCat");
    unsafe {
        MessageBoxW(null_mut(), text.as_ptr(), title.as_ptr(), kind);
    }
}

pub fn run(app: App) -> Result<(), String> {
    // Single UI thread. This Box outlives the window and the entire message loop.
    let mut state = Box::new(app);
    let class_name = wide("ChezCatWindow");
    let title = wide(&state.title());
    unsafe {
        SetProcessDPIAware();
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let icons = WindowIcons::load(instance)?;
        let class = WindowClass {
            style: 0x0003, // CS_HREDRAW | CS_VREDRAW
            procedure: Some(window_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: icons.large,
            cursor: LoadCursorW(null_mut(), 32512usize as *const u16),
            background: null_mut(),
            menu_name: null(),
            class_name: class_name.as_ptr(),
        };
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if let Err(error) = inspector_window::register(instance, icons.large) {
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err(error);
        }
        let state_ptr: *mut App = &mut *state;
        let window = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            0x00cf0000,
            i32::MIN,
            i32::MIN,
            1120,
            840,
            null_mut(),
            null_mut(),
            instance,
            state_ptr.cast(),
        );
        if window.is_null() {
            let error = std::io::Error::last_os_error().to_string();
            inspector_window::unregister(instance);
            UnregisterClassW(class_name.as_ptr(), instance);
            return Err(error);
        }
        SendMessageW(window, 0x0080, 1, icons.large as isize); // WM_SETICON / ICON_BIG
        SendMessageW(window, 0x0080, 0, icons.small as isize); // WM_SETICON / ICON_SMALL
        let colors = state.title_bar_colors();
        sync_frame_colors(window, colors);
        DragAcceptFiles(window, 1);
        let wake_window = window as usize;
        state.set_load_wake(std::sync::Arc::new(move || {
            // This callback posts only a signal; it never touches App across threads.
            PostMessageW(wake_window as Handle, 0x8001, 0, 0);
        }));
        if state.poll_load() {
            refresh(window, state_ptr);
        }
        sync_script_timer(window, state_ptr);
        sync_history_timer(window, state_ptr);
        sync_download_timer(window, state_ptr);
        ShowWindow(window, 5);
        UpdateWindow(window);
        let mut message: Message = zeroed();
        let result = loop {
            let code = GetMessageW(&mut message, null_mut(), 0, 0);
            if code == 0 {
                // Also handles a WM_QUIT posted independently of WM_DESTROY.
                DestroyWindow(window);
                break Ok(());
            }
            if code == -1 {
                let error = std::io::Error::last_os_error().to_string();
                DestroyWindow(window);
                break Err(error);
            }
            if message.window == window && matches!(message.message, 0x0100 | 0x0104) {
                // Run page keydown before producing its WM_CHAR. A canceled
                // key must not insert text later, and the decision must belong
                // to this exact message rather than an unrelated next key.
                keyboard_input::prepare(&message);
                DispatchMessageW(&message);
                let canceled = keyboard_input::complete(&message);
                if !canceled && state_is_attached(window, state_ptr) {
                    TranslateMessage(&message);
                }
            } else {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        };
        inspector_window::unregister(instance);
        UnregisterClassW(class_name.as_ptr(), instance);
        let cleanup = state.finish_browsing_session();
        match (result, cleanup) {
            (Err(error), Err(cleanup)) => Err(format!("{error}\n\n{cleanup}")),
            (Err(error), _) | (_, Err(error)) => Err(error),
            _ => Ok(()),
        }
    }
}

unsafe extern "system" fn window_proc(
    window: Handle,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        0x0081 => {
            // WM_NCCREATE: attach the stable Box pointer before later messages.
            let create = &*(lparam as *const CreateStruct);
            SetLastError(0);
            let previous = SetWindowLongPtrW(window, -21, create.parameters as isize);
            if previous == 0 && GetLastError() != 0 {
                return 0;
            }
            return 1;
        }
        0x0082 => {
            // WM_NCDESTROY: no later callback may access application state.
            KillTimer(window, SCRIPT_TIMER_ID);
            KillTimer(window, HISTORY_TIMER_ID);
            KillTimer(window, DOWNLOAD_TIMER_ID);
            DOWNLOAD_TIMER_OWNER.with(|owner| {
                if owner
                    .get()
                    .is_some_and(|(handle, _)| handle == window as usize)
                {
                    owner.set(None);
                }
            });
            forget_frame_colors(window);
            SCRIPT_TIMER_WAKE.with(|wake| {
                if wake
                    .get()
                    .is_some_and(|(owner, _)| owner == window as usize)
                {
                    wake.set(None);
                }
            });
            HISTORY_TIMER_WAKE.with(|wake| {
                if wake
                    .get()
                    .is_some_and(|(owner, _)| owner == window as usize)
                {
                    wake.set(None);
                }
            });
            SetWindowLongPtrW(window, -21, 0);
            return DefWindowProcW(window, message, wparam, lparam);
        }
        0x0002 => {
            PostQuitMessage(0);
            return 0;
        } // WM_DESTROY
        0x0014 => return 1, // WM_ERASEBKGND: our complete pixel buffer clears the surface.
        0x0024 => {
            // WM_GETMINMAXINFO
            let sizes = &mut *(lparam as *mut MinMaxInfo);
            sizes.min_track_size = Point { x: 420, y: 300 };
            return 0;
        }
        _ => {}
    }
    let state = GetWindowLongPtrW(window, -21) as *mut App;
    if state.is_null() {
        return DefWindowProcW(window, message, wparam, lparam);
    }

    // Do not retain &mut App across Win32 calls: some APIs synchronously re-enter
    // this procedure. App methods themselves do not invoke the windowing APIs.
    match message {
        0x0113 if wparam == DOWNLOAD_TIMER_ID => {
            // Visible request bars advance every 100 ms; a downloads-only popup
            // needs just one elapsed-time update per second. These are measured
            // durations, never fabricated byte percentages.
            (&mut *state).poll_load();
            refresh(window, state);
            0
        }
        0x0113 if wparam == HISTORY_TIMER_ID => {
            KillTimer(window, HISTORY_TIMER_ID);
            HISTORY_TIMER_WAKE.with(|wake| wake.set(None));
            if (&mut *state).poll_history_maintenance() {
                refresh(window, state);
            } else {
                sync_history_timer(window, state);
            }
            0
        }
        0x0113 if wparam == SCRIPT_TIMER_ID => {
            // Windows repeats timers; use it as a one-shot signal and arm only
            // the next deadline after running one callback on this UI thread.
            KillTimer(window, SCRIPT_TIMER_ID);
            SCRIPT_TIMER_WAKE.with(|wake| wake.set(None));
            if (&mut *state).poll_page_timer() {
                refresh(window, state);
            } else {
                sync_script_timer(window, state);
            }
            0
        }
        0x8001 => {
            if (&mut *state).poll_load() {
                refresh(window, state);
            }
            0
        }
        0x031a | 0x0320 => {
            // WM_THEMECHANGED / WM_DWMCOLORIZATIONCOLORCHANGED.
            forget_frame_colors(window);
            let colors = (&*state).title_bar_colors();
            sync_frame_colors(window, colors);
            DefWindowProcW(window, message, wparam, lparam)
        }
        0x000f => {
            // WM_PAINT
            let mut paint: PaintStruct = zeroed();
            let dc = BeginPaint(window, &mut paint);
            let mut rect: Rect = zeroed();
            GetClientRect(window, &mut rect);
            if !dc.is_null() && rect.right > 0 && rect.bottom > 0 {
                let canvas = {
                    let app = &mut *state;
                    app.resize(rect.right, rect.bottom);
                    app.draw()
                };
                present_canvas(dc, &rect, &canvas);
            }
            EndPaint(window, &paint);
            0
        }
        0x0005 => {
            // WM_SIZE
            let mut rect: Rect = zeroed();
            GetClientRect(window, &mut rect);
            if rect.right > 0 && rect.bottom > 0 {
                {
                    (&mut *state).resize(rect.right, rect.bottom);
                }
                inspector_window::sync(window, state);
                sync_script_timer(window, state);
                InvalidateRect(window, null(), 0);
            }
            0
        }
        0x020a => {
            // WM_MOUSEWHEEL uses screen coordinates even when another window
            // currently owns focus. Route the wheel to the pane under it.
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
            let ctrl = key_is_down(0x11);
            let shift = key_is_down(0x10);
            let (x, y) = mouse_coordinates(lparam);
            let mut point = Point { x, y };
            ScreenToClient(window, &mut point);
            {
                (&mut *state).wheel_at(point.x, point.y, delta, ctrl, shift);
            }
            refresh(window, state);
            0
        }
        // Consume both halves of X-button delivery, but navigate only on up.
        // DefWindowProc would otherwise generate a second WM_APPCOMMAND.
        0x020b if matches!((wparam >> 16) & 0xffff, 1 | 2) => 1,
        0x020c if matches!((wparam >> 16) & 0xffff, 1 | 2) => {
            if GetCapture() == window {
                ReleaseCapture();
            }
            (&mut *state).mouse_navigation((wparam >> 16) & 0xffff == 2);
            refresh(window, state);
            1
        }
        0x0319 if matches!((lparam as usize >> 16) & 0x0fff, 1 | 2) => {
            if GetCapture() == window {
                ReleaseCapture();
            }
            (&mut *state).mouse_navigation((lparam as usize >> 16) & 0x0fff == 2);
            refresh(window, state);
            1
        }
        0x0007 => {
            (&mut *state).main_window_focus();
            0
        }
        0x0100 | 0x0104 => {
            // WM_KEYDOWN / WM_SYSKEYDOWN. Leave native system shortcuts alone.
            let key = wparam as u32;
            if matches!(key, 0xa6 | 0xa7) {
                (&mut *state).mouse_navigation(key == 0xa7);
                refresh(window, state);
                return 0;
            }
            let input = keyboard_input::event(key, lparam, true, message == 0x0104);
            let (ctrl, alt) = (input.ctrl, input.alt);
            // AltGr is reported as Ctrl+Alt on Windows. It belongs to text entry,
            // not Ctrl/Alt browser shortcuts; TranslateMessage supplies its text.
            if input.meta
                || message == 0x0104
                    && !(ctrl && alt)
                    && (!alt || !matches!(key, 0x25 | 0x27 | 0x24 | 0x44 | 0x46))
            {
                return DefWindowProcW(window, message, wparam, lparam);
            }
            let (action, prevented) = { (&mut *state).key_event(input) };
            if prevented {
                keyboard_input::cancel(window, message, wparam, lparam);
            }
            perform_action(window, state, action);
            refresh(window, state);
            0
        }
        0x0101 | 0x0105 => {
            // Keyup still reaches the live page if keydown prevented scrolling
            // or text. Browser-owned focus/shortcuts are filtered inside App.
            let input = keyboard_input::event(wparam as u32, lparam, false, message == 0x0105);
            (&mut *state).key_event(input);
            refresh(window, state);
            DefWindowProcW(window, message, wparam, lparam)
        }
        0x0102 | 0x0106 => {
            // WM_CHAR / WM_SYSCHAR supply UTF-16 code units, including surrogate pairs.
            // Commands already handled by WM_KEYDOWN must not insert control text.
            // AltGr appears as Ctrl+Alt and may legitimately produce printable text.
            let ctrl = key_is_down(0x11);
            let alt = key_is_down(0x12);
            if message == 0x0106 && !(ctrl && alt) {
                // Alt+D/Alt+F were handled as key commands; other native system text
                // keeps its default handling. AltGr-generated text is allowed.
                return if matches!(wparam as u32, 0x44 | 0x64 | 0x46 | 0x66) {
                    0
                } else {
                    DefWindowProcW(window, message, wparam, lparam)
                };
            }
            if wparam <= u16::MAX as usize && wparam >= 0x20 && wparam != 0x7f && (!ctrl || alt) {
                {
                    (&mut *state).utf16_input(wparam as u16);
                }
                refresh(window, state);
            }
            0
        }
        0x0109 => {
            // WM_UNICHAR: acknowledge the Unicode support probe, or accept a scalar.
            if wparam == 0xffff {
                return 1;
            }
            if let Some(character) = u32::try_from(wparam).ok().and_then(char::from_u32) {
                if !character.is_control() {
                    {
                        (&mut *state).text_input(character);
                    }
                    refresh(window, state);
                }
            }
            0
        }
        0x0201 => {
            // WM_LBUTTONDOWN; client coordinates are signed 16-bit values.
            let (x, y) = mouse_coordinates(lparam);
            let shift = key_is_down(0x10);
            SetFocus(window);
            if key_is_down(0x12) && !key_is_down(0x11) && !shift && (&mut *state).download_at(x, y)
            {
                refresh(window, state);
                return 0;
            }
            SetCapture(window);
            let action = { (&mut *state).pointer_down(x, y, shift) };
            perform_action(window, state, action);
            refresh(window, state);
            0
        }
        0x0200 => {
            // WM_MOUSEMOVE continues a selection outside the client area while captured.
            let mut tracking = MouseTracking {
                size: size_of::<MouseTracking>() as u32,
                flags: 0x00000002, // TME_LEAVE, no hover timer.
                window,
                hover_time: 0,
            };
            TrackMouseEvent(&mut tracking);
            let (x, y) = mouse_coordinates(lparam);
            let dragging = wparam & 1 != 0 && GetCapture() == window;
            let changed = { (&mut *state).pointer_move(x, y, dragging) };
            set_client_cursor(state, x, y);
            // Hover can change the footer without a drag; unchanged motion is idle.
            if changed {
                InvalidateRect(window, null(), 0);
            }
            0
        }
        0x02a3 => {
            // WM_MOUSELEAVE clears hover without terminating a captured selection.
            if (&mut *state).pointer_leave() {
                InvalidateRect(window, null(), 0);
            }
            0
        }
        0x0202 => {
            // WM_LBUTTONUP completes selection before releasing capture. ReleaseCapture
            // can re-enter through WM_CAPTURECHANGED, whose cancellation is idempotent.
            let (x, y) = mouse_coordinates(lparam);
            let action = { (&mut *state).pointer_up(x, y) };
            if GetCapture() == window {
                ReleaseCapture();
            }
            // Restore hover after normal capture release. Actual capture loss
            // still cancels a press without triggering its action.
            (&mut *state).pointer_move(x, y, false);
            perform_action(window, state, action);
            refresh(window, state);
            0
        }
        0x0215 => {
            // WM_CAPTURECHANGED must cancel dragging without following a pending link.
            {
                (&mut *state).pointer_cancel();
            }
            InvalidateRect(window, null(), 0);
            0
        }
        0x0008 | 0x001f => {
            // WM_KILLFOCUS / WM_CANCELMODE: do not retain a drag across a dialog or Alt+Tab.
            {
                (&mut *state).pointer_cancel();
            }
            if GetCapture() == window {
                ReleaseCapture();
            }
            InvalidateRect(window, null(), 0);
            DefWindowProcW(window, message, wparam, lparam)
        }
        0x0020 if lparam & 0xffff == 1 => {
            // WM_SETCURSOR / HTCLIENT. Nonclient resize cursors remain Windows-owned.
            let mut point = Point { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 && ScreenToClient(window, &mut point) != 0 {
                set_client_cursor(state, point.x, point.y);
                1
            } else {
                DefWindowProcW(window, message, wparam, lparam)
            }
        }
        0x0233 => {
            // WM_DROPFILES; load only the first explicitly dropped file.
            let drop = wparam as Handle;
            let length = DragQueryFileW(drop, 0, null_mut(), 0);
            let path = if length > 0 && length < 32768 {
                let mut buffer = vec![0u16; length as usize + 1];
                let copied = DragQueryFileW(drop, 0, buffer.as_mut_ptr(), buffer.len() as u32);
                Some(PathBuf::from(std::ffi::OsString::from_wide(
                    &buffer[..copied as usize],
                )))
            } else {
                None
            };
            DragFinish(drop);
            if let Some(path) = path {
                (&mut *state).open(&path);
            }
            refresh(window, state);
            0
        }
        _ => DefWindowProcW(window, message, wparam, lparam),
    }
}

unsafe fn refresh(window: Handle, state: *mut App) {
    if !state_is_attached(window, state) {
        return;
    }
    let confirmation = { (&mut *state).take_resubmission_prompt() };
    if let Some((token, address)) = confirmation {
        if GetCapture() == window {
            ReleaseCapture();
        }
        let text = wide(&format!("This page was loaded by sending form data. Sending it again can repeat the action.\n\nDestination: {address}\n\nResend the form data?"));
        let caption = wide("Resend form data? - ChezCat");
        // Yes/No, default No. The dialog dispatches messages; keep no borrow of
        // App across it and recheck window ownership and the request token.
        let accepted = MessageBoxW(window, text.as_ptr(), caption.as_ptr(), 0x124) == 6;
        if !state_is_attached(window, state) {
            return;
        }
        (&mut *state).confirm_resubmission(token, accepted);
    }
    let title = { wide(&(*state).title()) };
    SetWindowTextW(window, title.as_ptr());
    let colors = (&*state).title_bar_colors();
    sync_frame_colors(window, colors);
    sync_script_timer(window, state);
    sync_history_timer(window, state);
    sync_download_timer(window, state);
    inspector_window::sync(window, state);
    InvalidateRect(window, null(), 0);
}

unsafe fn present_canvas(dc: Handle, rect: &Rect, canvas: &crate::paint::Canvas) {
    if canvas.pixels.is_empty() {
        return;
    }
    let info = BitmapInfo {
        header: BitmapHeader {
            size: size_of::<BitmapHeader>() as u32,
            width: canvas.width as i32,
            height: -(canvas.height as i32),
            planes: 1,
            bits: 32,
            compression: 0,
            image_size: 0,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: 0,
            important_colors: 0,
        },
        colors: [0],
    };
    StretchDIBits(
        dc,
        0,
        0,
        rect.right,
        rect.bottom,
        0,
        0,
        canvas.width as i32,
        canvas.height as i32,
        canvas.pixels.as_ptr().cast(),
        &info,
        0,
        0x00cc0020,
    );
}

unsafe fn sync_history_timer(window: Handle, state: *mut App) {
    if !state_is_attached(window, state) {
        return;
    }
    let desired = (&*state)
        .next_history_maintenance()
        .map(|due| (window as usize, due));
    if HISTORY_TIMER_WAKE.with(|wake| wake.get() == desired) {
        return;
    }
    KillTimer(window, HISTORY_TIMER_ID);
    HISTORY_TIMER_WAKE.with(|wake| wake.set(None));
    let Some((_, due)) = desired else {
        return;
    };
    let remaining = due.saturating_duration_since(std::time::Instant::now());
    let milliseconds = remaining
        .as_millis()
        .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
        .clamp(1, 0x7fff_ffff) as u32;
    if SetTimer(window, HISTORY_TIMER_ID, milliseconds, None) == 0 {
        (&mut *state).history_timer_unavailable();
    } else {
        HISTORY_TIMER_WAKE.with(|wake| wake.set(desired));
    }
}

unsafe fn sync_script_timer(window: Handle, state: *mut App) {
    if !state_is_attached(window, state) {
        return;
    }
    let desired = (&*state)
        .next_page_timer()
        .map(|due| (window as usize, due));
    if SCRIPT_TIMER_WAKE.with(|wake| wake.get() == desired) {
        return;
    }
    KillTimer(window, SCRIPT_TIMER_ID);
    SCRIPT_TIMER_WAKE.with(|wake| wake.set(None));
    let Some((_, due)) = desired else {
        return;
    };
    let remaining = due.saturating_duration_since(std::time::Instant::now());
    // SetTimer has a platform minimum granularity. Round up; never run early.
    let milliseconds = remaining
        .as_millis()
        .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
        .clamp(1, 0x7fff_ffff) as u32;
    if SetTimer(window, SCRIPT_TIMER_ID, milliseconds, None) == 0 {
        (&mut *state).script_timer_unavailable();
    } else {
        SCRIPT_TIMER_WAKE.with(|wake| wake.set(desired));
    }
}

unsafe fn sync_download_timer(window: Handle, state: *mut App) {
    if !state_is_attached(window, state) {
        return;
    }
    let desired = if (&*state).network_needs_tick() {
        Some((window as usize, 100))
    } else if (&*state).downloads_open() && (&*state).downloads_active() {
        Some((window as usize, 1000))
    } else {
        None
    };
    if DOWNLOAD_TIMER_OWNER.with(|owner| owner.get() == desired) {
        return;
    }
    KillTimer(window, DOWNLOAD_TIMER_ID);
    DOWNLOAD_TIMER_OWNER.with(|owner| owner.set(None));
    if let Some((_, milliseconds)) = desired {
        if SetTimer(window, DOWNLOAD_TIMER_ID, milliseconds, None) != 0 {
            DOWNLOAD_TIMER_OWNER.with(|owner| owner.set(desired));
        }
    }
}

fn colorref(rgb: u32) -> u32 {
    ((rgb & 0xff) << 16) | (rgb & 0xff00) | ((rgb >> 16) & 0xff)
}

fn forget_frame_colors(window: Handle) {
    FRAME_COLORS.with(|colors| {
        colors
            .borrow_mut()
            .retain(|entry| entry.0 != window as usize)
    });
}

unsafe fn sync_frame_colors(window: Handle, colors: (u32, u32)) {
    let changed = FRAME_COLORS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(previous) = cache.iter_mut().find(|entry| entry.0 == window as usize) {
            if previous.1 == colors.0 && previous.2 == colors.1 {
                return false;
            }
            *previous = (window as usize, colors.0, colors.1);
        } else {
            // Only the main window and its one detached inspector use this.
            if cache.len() >= 4 {
                cache.clear();
            }
            cache.push((window as usize, colors.0, colors.1));
        }
        true
    });
    if !changed {
        return;
    }
    // Record the values before OS calls because a frame update may re-enter the
    // window procedure. Unsupported attributes return an HRESULT on older
    // Windows; the ordinary native frame remains usable in that case.
    let caption = colorref(colors.0);
    let text = colorref(colors.1);
    let luminance =
        ((colors.0 >> 16) & 255) * 299 + ((colors.0 >> 8) & 255) * 587 + (colors.0 & 255) * 114;
    let dark: i32 = i32::from(luminance < 128_000);
    let _ = DwmSetWindowAttribute(
        window,
        20,
        (&dark as *const i32).cast(),
        size_of::<i32>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        window,
        35,
        (&caption as *const u32).cast(),
        size_of::<u32>() as u32,
    );
    let _ = DwmSetWindowAttribute(
        window,
        36,
        (&text as *const u32).cast(),
        size_of::<u32>() as u32,
    );
}

fn mouse_coordinates(lparam: isize) -> (i32, i32) {
    (
        (lparam & 0xffff) as u16 as i16 as i32,
        ((lparam >> 16) & 0xffff) as u16 as i16 as i32,
    )
}

unsafe fn key_is_down(key: i32) -> bool {
    GetKeyState(key) < 0
}

unsafe fn state_is_attached(window: Handle, state: *const App) -> bool {
    GetWindowLongPtrW(window, -21) as *const App == state
}

unsafe fn set_client_cursor(state: *const App, x: i32, y: i32) {
    let hand = { (&*state).wants_hand(x, y) };
    let resize = { (&*state).inspector_resize_cursor(x, y) };
    let resource = if resize {
        32645usize
    } else if hand {
        32649usize
    } else {
        32512usize
    };
    let cursor = LoadCursorW(null_mut(), resource as *const u16);
    if !cursor.is_null() {
        SetCursor(cursor);
    }
}

unsafe fn perform_action(window: Handle, state: *mut App, action: PlatformAction) {
    // The action owns its data. Clipboard functions and the modal file picker may
    // dispatch messages, so no application reference is held while they run.
    let result = match action {
        PlatformAction::None => Ok(()),
        PlatformAction::OpenFile => {
            if GetCapture() == window {
                ReleaseCapture();
            }
            match choose_file(window) {
                Ok(Some(path)) => {
                    if state_is_attached(window, state) {
                        (&mut *state).open(&path);
                    }
                    Ok(())
                }
                Ok(None) => Ok(()),
                Err(error) => Err(error),
            }
        }
        PlatformAction::OpenFolder(path) => open_folder(window, &path),
        PlatformAction::Copy(text) => write_clipboard(window, &text),
        PlatformAction::Cut(text) => match write_clipboard(window, &text) {
            Ok(()) => {
                if state_is_attached(window, state) {
                    (&mut *state).cut_copied(&text);
                }
                Ok(())
            }
            Err(error) => Err(error),
        },
        PlatformAction::Paste => match read_clipboard(window) {
            Ok(Some(text)) => {
                if state_is_attached(window, state) {
                    (&mut *state).paste(&text);
                }
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(error) => Err(error),
        },
    };
    if let Err(error) = result {
        if state_is_attached(window, state) {
            let text = wide(&error);
            let caption = wide("ChezCat");
            MessageBoxW(window, text.as_ptr(), caption.as_ptr(), 0x10);
        }
    }
}

/// This action accepts only a real local directory, then asks Explorer to
/// explore it. No command line is constructed and downloaded files are never
/// handed to a file association or launched by this browser.
unsafe fn open_folder(window: Handle, path: &std::path::Path) -> Result<(), String> {
    let path = crate::downloads::prepare_folder(path)?;
    if !std::fs::symlink_metadata(&path)
        .map_err(|e| format!("Cannot inspect the folder: {e}"))?
        .is_dir()
    {
        return Err("Only a download folder can be opened with this action.".into());
    }
    let mut filename: Vec<u16> = path.as_os_str().encode_wide().collect();
    // The shell expects a normal DOS path rather than canonicalize()'s extended
    // path prefix. prepare_folder admits local drive paths, never UNC/devices.
    if filename.starts_with(&[92, 92, 63, 92]) {
        filename.drain(..4);
    }
    if filename.contains(&0) {
        return Err("The folder path contains a NUL character.".into());
    }
    filename.push(0);
    let operation = wide("explore");
    let initialized = CoInitializeEx(null_mut(), 0x2 | 0x4); // STA, disable OLE1 DDE.
    if initialized < 0 {
        return Err(format!(
            "Cannot initialize Windows folder browsing (HRESULT {initialized:#010x})."
        ));
    }
    // No App reference is held across this call; Explorer may dispatch messages.
    let result = ShellExecuteW(
        window,
        operation.as_ptr(),
        filename.as_ptr(),
        null(),
        null(),
        1,
    ) as isize;
    CoUninitialize();
    if result <= 32 {
        Err(format!(
            "Windows could not open the download folder (shell error {result})."
        ))
    } else {
        Ok(())
    }
}

unsafe fn choose_file(window: Handle) -> Result<Option<PathBuf>, String> {
    let mut path = vec![0u16; 32768];
    // Unlike a window title, the filter is deliberately double-NUL terminated.
    let filter: Vec<u16> = concat!(
        "Pages, text and supported images\0*.html;*.htm;*.txt;*.png;*.apng;*.jpg;*.jpeg;*.jpe;*.jfif;*.gif;*.bmp;*.dib;*.ico;*.cur;*.svg;*.webp;*.tif;*.tiff;*.qoi;*.pbm;*.pgm;*.ppm;*.pnm;*.pam;*.tga;*.pcx\0",
        "HTML and text (*.html;*.htm;*.txt)\0*.html;*.htm;*.txt\0",
        "Supported images\0*.png;*.apng;*.jpg;*.jpeg;*.jpe;*.jfif;*.gif;*.bmp;*.dib;*.ico;*.cur;*.svg;*.webp;*.tif;*.tiff;*.qoi;*.pbm;*.pgm;*.ppm;*.pnm;*.pam;*.tga;*.pcx\0",
        "All files (*.*)\0*.*\0\0"
    )
        .encode_utf16()
        .collect();
    let title = wide("Open local page or image");
    let mut dialog: OpenFileName = zeroed();
    dialog.size = size_of::<OpenFileName>() as u32;
    dialog.owner = window;
    dialog.filter = filter.as_ptr();
    dialog.filter_index = 1;
    dialog.file = path.as_mut_ptr();
    dialog.max_file = path.len() as u32;
    dialog.title = title.as_ptr();
    dialog.flags = 0x0000_0004 // OFN_HIDEREADONLY
        | 0x0000_0008 // OFN_NOCHANGEDIR
        | 0x0000_0800 // OFN_PATHMUSTEXIST
        | 0x0000_1000 // OFN_FILEMUSTEXIST
        | 0x0002_0000 // OFN_NONETWORKBUTTON
        | 0x0008_0000 // OFN_EXPLORER
        | 0x0010_0000 // OFN_NODEREFERENCELINKS
        | 0x0080_0000 // OFN_ENABLESIZING
        | 0x0200_0000; // OFN_DONTADDTORECENT
    if GetOpenFileNameW(&mut dialog) == 0 {
        let error = CommDlgExtendedError();
        return if error == 0 {
            Ok(None)
        } else {
            Err(format!(
                "The file picker could not open (Windows error 0x{error:04x})."
            ))
        };
    }
    let length = path
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| "The selected file path was too long.".to_owned())?;
    if length == 0 {
        return Ok(None);
    }
    Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
        &path[..length],
    ))))
}

fn clipboard_units(text: &str) -> Result<Vec<u16>, String> {
    if text.len() > MAX_CLIPBOARD_BYTES {
        return Err("The selected text is too large for this build's clipboard limit.".to_owned());
    }
    // Windows text clipboard lines use CRLF. Embedded NULs cannot be represented.
    let text = text
        .replace('\0', "")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\n', "\r\n");
    let count = text.encode_utf16().count();
    if count >= MAX_CLIPBOARD_BYTES / size_of::<u16>() {
        return Err("The selected text is too large for this build's clipboard limit.".to_owned());
    }
    Ok(text.encode_utf16().chain(Some(0)).collect())
}

unsafe fn write_clipboard(window: Handle, text: &str) -> Result<(), String> {
    let units = clipboard_units(text)?;
    let bytes = units
        .len()
        .checked_mul(size_of::<u16>())
        .ok_or_else(|| "The selected text is too large to copy.".to_owned())?;
    let memory = GlobalAlloc(0x0042, bytes); // GMEM_MOVEABLE | GMEM_ZEROINIT
    if memory.is_null() {
        return Err("Windows could not allocate clipboard memory.".to_owned());
    }
    let pointer = GlobalLock(memory).cast::<u16>();
    if pointer.is_null() {
        GlobalFree(memory);
        return Err("Windows could not access clipboard memory.".to_owned());
    }
    std::ptr::copy_nonoverlapping(units.as_ptr(), pointer, units.len());
    GlobalUnlock(memory);

    if OpenClipboard(window) == 0 {
        GlobalFree(memory);
        return Err("The clipboard is busy. Try copying again.".to_owned());
    }
    let result = if EmptyClipboard() == 0 {
        Err("Windows could not clear the clipboard for copying.".to_owned())
    } else if SetClipboardData(CF_UNICODETEXT, memory).is_null() {
        Err("Windows could not place the text on the clipboard.".to_owned())
    } else {
        Ok(())
    };
    CloseClipboard();
    if result.is_err() {
        GlobalFree(memory);
    }
    // On success Windows owns the allocation; this process must not free it.
    result
}

unsafe fn read_clipboard(window: Handle) -> Result<Option<String>, String> {
    if OpenClipboard(window) == 0 {
        return Err("The clipboard is busy. Try pasting again.".to_owned());
    }
    let result = (|| {
        if IsClipboardFormatAvailable(CF_UNICODETEXT) == 0 {
            return Ok(None);
        }
        let memory = GetClipboardData(CF_UNICODETEXT);
        if memory.is_null() {
            return Err("Windows could not read text from the clipboard.".to_owned());
        }
        let bytes = GlobalSize(memory);
        if bytes < size_of::<u16>() || bytes > MAX_CLIPBOARD_BYTES {
            return Err("Clipboard text must fit within this build's 2 MiB limit.".to_owned());
        }
        let pointer = GlobalLock(memory).cast::<u16>();
        if pointer.is_null() {
            return Err("Windows could not access clipboard text.".to_owned());
        }
        let text = {
            let units = std::slice::from_raw_parts(pointer, bytes / size_of::<u16>());
            units
                .iter()
                .position(|unit| *unit == 0)
                .map(|length| Some(String::from_utf16_lossy(&units[..length])))
                .ok_or_else(|| "Clipboard text was not properly terminated.".to_owned())
        };
        GlobalUnlock(memory);
        text
    })();
    CloseClipboard();
    result
}
