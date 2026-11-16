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
#[path = "platform_session.rs"]
mod session_window;
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
const APP_ICON_ID: usize = 101;
std::thread_local! {
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
    fn PeekMessageW(message: *mut Message, window: Handle, min: u32, max: u32, remove: u32) -> i32;
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
    fn CreatePopupMenu() -> Handle;
    fn AppendMenuW(menu: Handle, flags: u32, item: usize, text: *const u16) -> i32;
    fn TrackPopupMenu(menu: Handle, flags: u32, x: i32, y: i32,
        reserved: i32, owner: Handle, exclude: *const Rect) -> i32;
    fn DestroyMenu(menu: Handle) -> i32;
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
    session_window::run(app)
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
    let Some(route) = session_window::binding(window) else { return; };
    let changed = FRAME_COLORS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(previous) = cache.iter_mut().find(|entry| entry.0 == window as usize) {
            if previous.1 == colors.0 && previous.2 == colors.1 {
                return false;
            }
            *previous = (window as usize, colors.0, colors.1);
        } else {
            // Bounded main/inspector native routes share this small frame cache.
            if cache.len() >= 40 {
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
    if !session_window::native_matches(route) { return; }
    let _ = DwmSetWindowAttribute(
        window,
        20,
        (&dark as *const i32).cast(),
        size_of::<i32>() as u32,
    );
    if !session_window::native_matches(route) { return; }
    let _ = DwmSetWindowAttribute(
        window,
        35,
        (&caption as *const u32).cast(),
        size_of::<u32>() as u32,
    );
    if !session_window::native_matches(route) { return; }
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
