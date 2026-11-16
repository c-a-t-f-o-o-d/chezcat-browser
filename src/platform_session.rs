//! UI-thread native shell. Windows retain checked numeric registry keys only.
//! No Session/App/registry guard spans a native call, including modal dialogs.
use super::*;
use crate::app::session::window_ui::{Outcome, RoutedRequest, SessionUi};
use crate::app::session::{Limits, Session, TabId, WindowId};
use crate::app::{PlatformCompletion, PlatformRequest};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[path = "platform_session_routes.rs"]
mod routes;
pub(super) use routes::{Binding, InputOwner, Owner};
use routes::{InputRoutes, Registry};
const CLASS: &str = "ChezCatWindow";
const TIMER: usize = 0x4f45;
struct Browser {
    session: Session,
    ui: SessionUi,
    instance: usize,
    large: usize,
    small: usize,
    finalizing: bool,
    final_error: Option<String>,
    show_auxiliary: bool,
    scheduler_failed: bool,
}
std::thread_local! {
    static BROWSER:RefCell<Option<Browser>>=const{RefCell::new(None)};
    static NATIVE:RefCell<Option<Registry>>=const{RefCell::new(None)};
    static INPUT:RefCell<InputRoutes>=RefCell::new(InputRoutes::new());
    static CLOSE_PROMPT:Cell<bool>=const{Cell::new(false)};
    #[cfg(test)] static TEST_HISTORY_CHOICE:Cell<Option<i32>>=const{Cell::new(None)};
    #[cfg(test)] static TEST_REENTRY_BUSY:Cell<bool>=const{Cell::new(false)};
    #[cfg(test)] static TEST_MODAL_ERRORS:RefCell<Option<Vec<String>>>=const{RefCell::new(None)};
}
fn edit<T>(action: impl FnOnce(&mut Browser) -> Result<T, String>) -> Result<T, String> {
    BROWSER.with(|slot| {
        let mut guard = slot
            .try_borrow_mut()
            .map_err(|_| "The browser dispatcher is busy.")?;
        action(guard.as_mut().ok_or("The browser dispatcher has closed.")?)
    })
}
fn registry<T>(action: impl FnOnce(&mut Registry) -> T) -> Option<T> {
    NATIVE.with(|slot| slot.try_borrow_mut().ok()?.as_mut().map(action))
}
pub(super) fn all_routes() -> Vec<Binding> {
    records()
}
fn records() -> Vec<Binding> {
    registry(|r| r.records()).unwrap_or_default()
}
pub(super) unsafe fn binding(hwnd: Handle) -> Option<Binding> {
    let key = GetWindowLongPtrW(hwnd, -21) as usize;
    registry(|r| r.lookup(key, hwnd as usize)).flatten()
}
pub(super) unsafe fn native_matches(route: Binding) -> bool {
    GetWindowLongPtrW(route.hwnd as Handle, -21) as usize == route.key
        && registry(|r| r.get(route.key).is_some_and(|r| r.hwnd == route.hwnd)).unwrap_or(false)
}
pub(super) unsafe fn destroy_route(route: Binding) {
    if route.hwnd != 0 && native_matches(route) {
        DestroyWindow(route.hwnd as Handle);
    }
}
pub(super) unsafe fn retire_route(route: Binding) {
    registry(|r| r.close_key(route.key));
    destroy_route(route);
}
pub(super) fn attached(route: Binding) -> bool {
    registry(|r| r.lookup(route.key, route.hwnd).is_some()).unwrap_or(false)
}
fn alive(session: &Session, owner: InputOwner) -> bool {
    session
        .window_tabs(owner.window)
        .is_ok_and(|tabs| tabs.contains(&owner.tab))
        && (owner.inspector || session.active(owner.window).ok().flatten() == Some(owner.tab))
}
fn same_active(route: Binding, origin: Option<InputOwner>) -> bool {
    attached(route) && origin.is_some_and(|owner| observe_tab(owner, |_| ()).is_ok())
}
pub(super) fn with_tab<T>(
    owner: InputOwner,
    action: impl FnOnce(&mut App) -> T,
) -> Result<T, String> {
    edit(|browser| {
        if !alive(&browser.session, owner) {
            return Err("The originating browser owner is no longer active.".into());
        }
        browser.session.with_tab(owner.tab, action)
    })
}
pub(super) fn observe_tab<T>(
    owner: InputOwner,
    action: impl FnOnce(&App) -> T,
) -> Result<T, String> {
    edit(|browser| {
        if !alive(&browser.session, owner) {
            return Err("The originating browser owner has closed.".into());
        }
        browser.session.observe_tab(owner.tab, action)
    })
}
pub(super) fn owner_for(route: Binding) -> Option<InputOwner> {
    if !attached(route) {
        return None;
    }
    edit(|browser| match route.owner {
        Owner::Main(window) => Ok(InputOwner {
            window,
            tab: browser
                .session
                .active(window)?
                .ok_or("This browser window has no active page.")?,
            inspector: false,
        }),
        Owner::Inspector(window, tab) => {
            let owner = InputOwner {
                window,
                tab,
                inspector: true,
            };
            if alive(&browser.session, owner) {
                Ok(owner)
            } else {
                Err("The inspected tab has closed.".into())
            }
        }
    })
    .ok()
}
pub(super) fn main_route(window: WindowId) -> Option<Binding> {
    records()
        .into_iter()
        .find(|r| r.owner == Owner::Main(window) && !r.closing)
}
pub(super) fn inspector_routes(window: WindowId) -> Vec<Binding> {
    records()
        .into_iter()
        .filter(|r| matches!(r.owner,Owner::Inspector(w,_) if w==window))
        .collect()
}
pub(super) fn tab_ids(window: WindowId) -> Vec<TabId> {
    edit(|b| Ok(b.session.window_tabs(window)?.to_vec())).unwrap_or_default()
}
pub(super) fn reserve_inspector(window: WindowId, tab: TabId) -> Result<usize, String> {
    registry(|r| r.reserve(Owner::Inspector(window, tab)))
        .ok_or("Native routes are unavailable.")?
}
pub(super) unsafe fn retire_reserved(key: usize) {
    if let Some(route) = registry(|r| r.close_key(key)).flatten() {
        destroy_route(route);
    }
}
pub(super) fn forget_key(key: usize) {
    registry(|r| r.remove(key));
}
pub(super) fn attach_key(key: usize, hwnd: Handle) -> bool {
    registry(|r| r.attach(key, hwnd as usize)).unwrap_or(false)
}
pub(super) unsafe fn destroyed(hwnd: Handle) {
    #[cfg(test)]
    {
        let busy = BROWSER.with(|s| s.try_borrow_mut().is_err())
            || NATIVE.with(|s| s.try_borrow_mut().is_err());
        if busy {
            TEST_REENTRY_BUSY.with(|s| s.set(true));
        }
    }
    let key = GetWindowLongPtrW(hwnd, -21) as usize;
    registry(|r| r.remove(key));
    INPUT.with(|r| r.borrow_mut().forget(hwnd as usize));
    KillTimer(hwnd, TIMER);
    forget_frame_colors(hwnd);
    SetWindowLongPtrW(hwnd, -21, 0);
}
pub(super) unsafe fn schedule() {
    arm_scheduler();
}
pub(super) fn show_auxiliary() -> bool {
    edit(|b| Ok(b.show_auxiliary)).unwrap_or(false)
}
unsafe fn discard_text(hwnd: Handle) {
    INPUT.with(|r| r.borrow_mut().forget(hwnd as usize));
    // TranslateMessage posts its character messages synchronously. Remove old
    // translations after a logical owner switch, before dispatch can target the
    // replacement. Do not remove later KEYDOWNs; those acquire the new owner.
    let mut message: Message = zeroed();
    for (first, last) in [(0x0102, 0x0103), (0x0106, 0x0107), (0x0109, 0x0109)] {
        while PeekMessageW(&mut message, hwnd, first, last, 1) != 0 {
            if message.message == 0x0012 {
                PostQuitMessage(message.wparam as i32);
                return;
            }
        }
    }
}
pub(super) fn activate_window(window: WindowId) {
    let _ = edit(|b| b.ui.activate_window(&mut b.session, window));
}
pub(super) fn forget_input(hwnd: Handle) {
    INPUT.with(|r| r.borrow_mut().forget(hwnd as usize));
}
pub(super) fn text_owner(hwnd: Handle, lparam: isize) -> Option<InputOwner> {
    INPUT.with(|r| r.borrow().text(hwnd as usize, lparam))
}
pub(super) fn key_owner(hwnd: Handle, key: u32, down: bool, route: Binding) -> Option<InputOwner> {
    if down {
        let owner = owner_for(route)?;
        INPUT.with(|r| r.borrow_mut().key_down(hwnd as usize, key, owner));
        Some(owner)
    } else {
        INPUT.with(|r| r.borrow_mut().key_up(hwnd as usize, key))
    }
}
// Inspector routes are inaccessible before any pure operation capable of
// consuming an App. Reconcile them before invoking even one native function.
unsafe fn mutate<T>(
    operation: impl FnOnce(&mut Browser) -> Result<T, String>,
) -> Result<T, String> {
    let before = edit(|b| {
        Ok(b.session
            .window_ids()
            .map(|w| (w, b.session.active(w).ok().flatten()))
            .collect::<Vec<_>>())
    })
    .unwrap_or_default();
    registry(|r| r.suspend_inspectors());
    let result = edit(operation);
    let live = edit(|b| {
        Ok(b.session
            .window_ids()
            .map(|w| (w, b.session.window_tabs(w).unwrap_or(&[]).to_vec()))
            .collect::<Vec<_>>())
    })
    .unwrap_or_default();
    let retired = registry(|r| {
        r.reconcile(|owner| match owner {
            Owner::Main(_) => true,
            Owner::Inspector(w, t) => live
                .iter()
                .any(|(window, tabs)| *window == w && tabs.contains(&t)),
        })
    })
    .unwrap_or_default();
    for route in retired {
        destroy_route(route);
    }
    let after = edit(|b| {
        Ok(b.session
            .window_ids()
            .map(|w| (w, b.session.active(w).ok().flatten()))
            .collect::<Vec<_>>())
    })
    .unwrap_or_default();
    for (window, active) in before {
        if after.iter().find(|(w, _)| *w == window).map(|(_, a)| *a) != Some(active) {
            if let Some(route) = main_route(window) {
                discard_text(route.hwnd as Handle);
            }
        }
    }
    result
}
unsafe fn report(route: Binding, error: &str) {
    if attached(route) {
        #[cfg(test)]
        if TEST_MODAL_ERRORS.with(|slot| {
            let mut slot = slot.borrow_mut();
            if let Some(errors) = slot.as_mut() {
                errors.push(error.to_owned());
                true
            } else {
                false
            }
        }) {
            return;
        }
        let text = wide(error);
        let title = wide("ChezCat");
        MessageBoxW(route.hwnd as Handle, text.as_ptr(), title.as_ptr(), 0x10);
    }
}

pub(super) unsafe fn perform_request(route: Binding, origin: InputOwner, request: PlatformRequest) {
    if !attached(route) || observe_tab(origin, |_| ()).is_err() {
        return;
    }
    let hwnd = route.hwnd as Handle;
    let completion: Result<Option<PlatformCompletion>, String> = match request.action() {
        PlatformAction::None => Ok(None),
        PlatformAction::OpenFile if origin.inspector => Ok(None),
        PlatformAction::OpenFile => {
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            if !attached(route) {
                return;
            }
            choose_file(hwnd).map(|path| path.map(PlatformCompletion::OpenFile))
        }
        PlatformAction::OpenFolder(path) => open_folder(hwnd, path).map(|_| None),
        PlatformAction::CopyBuildInformation(_) if origin.inspector => {
            Err("Build-information copy belongs to the browser About panel.".into())
        }
        PlatformAction::Copy(text) | PlatformAction::CopyBuildInformation(text) => {
            write_clipboard(hwnd, text).map(|_| None)
        }
        PlatformAction::Cut(text) => {
            write_clipboard(hwnd, text).map(|_| Some(PlatformCompletion::CutWritten))
        }
        PlatformAction::Paste => {
            read_clipboard(hwnd).map(|text| text.map(PlatformCompletion::Paste))
        }
    };
    match completion {
        Ok(Some(result)) if attached(route) => {
            let _ = with_tab(origin, |app| app.complete_platform_request(request, result));
        }
        Err(error) => report(route, &error),
        _ => {}
    }
}
pub(super) unsafe fn inspector_request(
    route: Binding,
    origin: InputOwner,
    action: impl FnOnce(&mut App) -> PlatformAction,
) {
    if !attached(route) {
        return;
    }
    if let Ok(request) = with_tab(origin, |app| {
        let action = action(app);
        app.inspector_platform_request(action)
    }) {
        perform_request(route, origin, request);
    }
}
unsafe fn outcome(route: Binding, result: Result<Outcome, String>) {
    match result {
        Ok(outcome) => {
            if let Some(window) = outcome.closed {
                close_native(window, true);
            }
            if let Some((window, _)) = outcome.opened {
                if let Err(error) = create_main(window, true) {
                    let _ = mutate(|b| {
                        b.session.close_window(window)?;
                        b.ui.forget(window);
                        Ok(())
                    });
                    report(route, &error);
                }
            }
            if let Some(RoutedRequest {
                window,
                tab,
                request,
            }) = outcome.request
            {
                perform_request(
                    route,
                    InputOwner {
                        window,
                        tab,
                        inspector: false,
                    },
                    request,
                );
            }
            if outcome.repaint && attached(route) {
                refresh_window(route.owner.window());
            }
        }
        Err(error) => report(route, &error),
    }
    arm_scheduler();
}
pub(super) unsafe fn refresh_window(window: WindowId) {
    let Some(route) = main_route(window) else {
        return;
    };
    let data = edit(|b| {
        let tab = b
            .session
            .active(window)?
            .ok_or("This window has no active tab.")?;
        let title = b.ui.title(&b.session, window)?;
        let colors = b.session.observe_tab(tab, |app| app.title_bar_colors())?;
        let prompt = b
            .session
            .with_tab(tab, |app| app.take_resubmission_prompt())?;
        Ok((tab, title, colors, prompt))
    });
    let Ok((tab, title, colors, prompt)) = data else {
        return;
    };
    if let Some((token, address)) = prompt {
        let hwnd = route.hwnd as Handle;
        if GetCapture() == hwnd {
            ReleaseCapture();
        }
        if !attached(route) {
            return;
        }
        let text=wide(&format!("This page was opened by submitting a form. To show it again, ChezCat must resend the form data to:\n\n{address}\n\nResend the form?"));
        let caption = wide("Confirm form resubmission");
        let accepted = MessageBoxW(hwnd, text.as_ptr(), caption.as_ptr(), 0x24) == 6;
        let origin = InputOwner {
            window,
            tab,
            inspector: false,
        };
        if attached(route) {
            let _ = with_tab(origin, |app| app.confirm_resubmission(token, accepted));
        }
        if !attached(route) {
            return;
        }
        // Confirmation may navigate; obtain the owned chrome again.
        refresh_window_without_prompt(window);
        return;
    }
    if !attached(route) {
        return;
    }
    let title = wide(&title);
    SetWindowTextW(route.hwnd as Handle, title.as_ptr());
    if !attached(route) {
        return;
    }
    sync_frame_colors(route.hwnd as Handle, colors);
    if !attached(route) {
        return;
    }
    inspector_window::sync(window);
    if attached(route) {
        InvalidateRect(route.hwnd as Handle, null(), 0);
    }
}
unsafe fn refresh_window_without_prompt(window: WindowId) {
    let Some(route) = main_route(window) else {
        return;
    };
    if let Ok((title, colors)) = edit(|b| {
        Ok((
            b.ui.title(&b.session, window)?,
            b.session.observe_active(window, |a| a.title_bar_colors())?,
        ))
    }) {
        let title = wide(&title);
        SetWindowTextW(route.hwnd as Handle, title.as_ptr());
        if attached(route) {
            sync_frame_colors(route.hwnd as Handle, colors);
        }
        if attached(route) {
            inspector_window::sync(window);
        }
        if attached(route) {
            InvalidateRect(route.hwnd as Handle, null(), 0);
        }
    }
}
unsafe fn cursor(route: Binding, x: i32, y: i32) {
    let Owner::Main(window) = route.owner else {
        return;
    };
    let resource = edit(|b| {
        Ok(if b.ui.resize_cursor(&b.session, window, x, y)? {
            32645
        } else if b.ui.wants_hand(&b.session, window, x, y)? {
            32649
        } else {
            32512
        })
    })
    .unwrap_or(32512);
    let cursor = LoadCursorW(null_mut(), resource as *const u16);
    if !cursor.is_null() && attached(route) {
        SetCursor(cursor);
    }
}
fn due_millis(due: Instant) -> u32 {
    let remaining = due.saturating_duration_since(Instant::now());
    remaining
        .as_millis()
        .saturating_add(u128::from(remaining.subsec_nanos() % 1_000_000 != 0))
        .clamp(1, 0x7fff_ffff) as u32
}
unsafe fn arm_scheduler() {
    let due = edit(|b| {
        Ok(if b.scheduler_failed {
            None
        } else {
            b.session.next_deadline()
        })
    })
    .ok()
    .flatten();
    let hosts = records()
        .into_iter()
        .filter(|r| matches!(r.owner, Owner::Main(_)) && !r.closing && r.hwnd != 0)
        .collect::<Vec<_>>();
    let chosen = hosts.first().map(|r| r.key);
    for route in hosts {
        let desired = if Some(route.key) == chosen { due } else { None };
        if route.timer == desired
            || route
                .timer
                .zip(desired)
                .is_some_and(|(armed, wanted)| armed <= wanted)
        {
            continue;
        }
        registry(|r| r.set_timer(route.key, None));
        KillTimer(route.hwnd as Handle, TIMER);
        if let Some(deadline) = desired {
            if attached(route) {
                if SetTimer(route.hwnd as Handle, TIMER, due_millis(deadline), None) != 0 {
                    registry(|r| r.set_timer(route.key, Some(deadline)));
                } else {
                    let first = edit(|b| {
                        let first = !b.scheduler_failed;
                        b.scheduler_failed = true;
                        Ok(first)
                    })
                    .unwrap_or(false);
                    if first {
                        report(route,"Windows could not schedule browser work. Close and reopen the browser to retry.");
                    }
                }
            }
        }
    }
}
unsafe fn poll(route: Binding) {
    KillTimer(route.hwnd as Handle, TIMER);
    registry(|r| r.set_timer(route.key, None));
    let result = edit(|b| {
        let report = b.session.poll(4)?;
        let windows = b
            .session
            .window_ids()
            .filter(|w| {
                report.downloads_changed
                    || b.session
                        .window_tabs(*w)
                        .is_ok_and(|tabs| tabs.iter().any(|t| report.changed.contains(t)))
            })
            .collect::<Vec<_>>();
        Ok(windows)
    });
    match result {
        Ok(windows) => {
            for window in windows {
                refresh_window(window);
            }
        }
        Err(error) => report(route, &error),
    }
    arm_scheduler();
}
pub(super) unsafe fn create_main(window: WindowId, show: bool) -> Result<Handle, String> {
    let (instance, large, small, title) = edit(|b| {
        Ok((
            b.instance,
            b.large,
            b.small,
            b.ui.title(&b.session, window)?,
        ))
    })?;
    let key = registry(|r| r.reserve(Owner::Main(window)))
        .ok_or("Native browser routes are unavailable.")??;
    let name = wide(CLASS);
    let title = wide(&title);
    let hwnd = CreateWindowExW(
        0,
        name.as_ptr(),
        title.as_ptr(),
        0x00cf0000,
        i32::MIN,
        i32::MIN,
        1120,
        840,
        null_mut(),
        null_mut(),
        instance as Handle,
        key as *mut c_void,
    );
    if hwnd.is_null() {
        forget_key(key);
        return Err(std::io::Error::last_os_error().to_string());
    }
    let Some(route) =
        binding(hwnd).filter(|route| route.key == key && route.owner == Owner::Main(window))
    else {
        retire_reserved(key);
        return Err("The native browser route did not attach.".into());
    };
    let opened = (|| {
        // Hidden windows need not receive WM_SIZE during CreateWindowExW.
        // Bind the complete client area before any input, paint or refresh.
        let mut rect: Rect = zeroed();
        if GetClientRect(hwnd, &mut rect) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if !attached(route) {
            return Err("The new browser window closed while opening.".into());
        }
        let (width, height) = (rect.right - rect.left, rect.bottom - rect.top);
        if width <= 0 || height <= 0 {
            return Err("The new browser window has no client area.".into());
        }
        edit(|b| b.ui.resize(&mut b.session, window, width, height))?;
        SendMessageW(hwnd, 0x0080, 1, large as isize);
        if !attached(route) {
            return Err("The new browser window closed while opening.".into());
        }
        SendMessageW(hwnd, 0x0080, 0, small as isize);
        if !attached(route) {
            return Err("The new browser window closed while opening.".into());
        }
        DragAcceptFiles(hwnd, 1);
        if !attached(route) {
            return Err("The new browser window closed while opening.".into());
        }
        refresh_window(window);
        if !attached(route) {
            return Err("The new browser window closed while opening.".into());
        }
        if show {
            ShowWindow(hwnd, 5);
            if !attached(route) {
                return Err("The new browser window closed while opening.".into());
            }
            UpdateWindow(hwnd);
        }
        arm_scheduler();
        if !attached(route) {
            return Err("The new browser window closed while opening.".into());
        }
        Ok(hwnd)
    })();
    if opened.is_err() {
        retire_reserved(key);
        let _ = edit(|b| {
            b.ui.forget(window);
            Ok(())
        });
    }
    opened
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
// Returns a retryable result without UI. Tests use this exact production seam.
fn shutdown_attempt() -> Result<(), String> {
    edit(|b| {
        b.finalizing = true;
        match b.session.shutdown(now()) {
            Ok(_) => {
                b.final_error = None;
                Ok(())
            }
            Err(error) => {
                b.final_error = Some(error.clone());
                Err(error)
            }
        }
    })
}
unsafe fn retire_native(routes: Vec<Binding>) {
    for route in routes {
        if route.hwnd != 0 && native_matches(route) {
            KillTimer(route.hwnd as Handle, TIMER);
        }
    }
    for route in records() {
        if matches!(route.owner, Owner::Inspector(..)) && route.closing {
            destroy_route(route);
        }
    }
}
unsafe fn history_choice(host: Handle, text: *const u16, title: *const u16) -> i32 {
    #[cfg(test)]
    if let Some(answer) = TEST_HISTORY_CHOICE.with(|s| s.take()) {
        return answer;
    }
    #[cfg(test)]
    if TEST_MODAL_ERRORS.with(|slot| {
        let mut slot = slot.borrow_mut();
        if let Some(errors) = slot.as_mut() {
            errors.push("Unexpected history cleanup prompt.".into());
            true
        } else {
            false
        }
    }) {
        return 2;
    }
    MessageBoxW(host, text, title, 0x15)
}
unsafe fn finalize(host: Binding) {
    if CLOSE_PROMPT.with(|s| s.replace(true)) {
        return;
    }
    let routes = registry(|r| r.close_all()).unwrap_or_default();
    retire_native(routes);
    loop {
        match shutdown_attempt() {
            Ok(()) => {
                for route in records() {
                    destroy_route(route);
                }
                PostQuitMessage(0);
                break;
            }
            Err(error) => {
                let text=wide(&format!("Browsing has stopped. History cleanup failed:\n\n{error}\n\nRetry cleanup? Cancel keeps this closing window available for another retry."));
                let title = wide("ChezCat - history cleanup");
                if history_choice(host.hwnd as Handle, text.as_ptr(), title.as_ptr()) != 4 {
                    let title =
                        wide("ChezCat - closing; close this window to retry history cleanup");
                    if native_matches(host) {
                        SetWindowTextW(host.hwnd as Handle, title.as_ptr());
                    }
                    if native_matches(host) {
                        InvalidateRect(host.hwnd as Handle, null(), 0);
                    }
                    break;
                }
            }
        }
    }
    CLOSE_PROMPT.with(|s| s.set(false));
}
unsafe fn close_native(window: WindowId, already_closed: bool) {
    let routes = registry(|r| r.close_window(window)).unwrap_or_default();
    let host = routes
        .iter()
        .find(|r| matches!(r.owner, Owner::Main(_)))
        .copied()
        .or_else(|| {
            records()
                .into_iter()
                .find(|r| r.owner == Owner::Main(window))
        });
    let Some(host) = host else {
        return;
    };
    // Disable inspector/main routes before the core consumes any Box<App>.
    let result = if already_closed {
        edit(|b| {
            b.ui.forget(window);
            Ok(())
        })
    } else {
        edit(|b| {
            b.session.close_window(window)?;
            b.ui.forget(window);
            Ok(())
        })
    };
    if let Err(error) = result {
        registry(|r| r.restore_window(window));
        report(host, &error);
        return;
    }
    if edit(|b| Ok(b.session.window_count() == 0)).unwrap_or(true) {
        finalize(host);
        return;
    }
    retire_native(routes.clone());
    for route in routes {
        destroy_route(route);
    }
    arm_scheduler();
}

unsafe fn dispatch_native(message: &Message) {
    if matches!(message.message, 0x0100 | 0x0104) && binding(message.window).is_some() {
        let captured = binding(message.window);
        let origin = captured.and_then(owner_for);
        let prepared = keyboard_input::prepare(message);
        DispatchMessageW(message);
        let canceled = if prepared {
            keyboard_input::complete(message)
        } else {
            true
        };
        if prepared && !canceled {
            if let Some(origin) = origin {
                if observe_tab(origin, |_| ()).is_ok() && captured.is_some_and(attached) {
                    INPUT.with(|r| {
                        r.borrow_mut()
                            .translated(message.window as usize, message.lparam, origin)
                    });
                    TranslateMessage(message);
                }
            }
        }
    } else {
        TranslateMessage(message);
        DispatchMessageW(message);
    }
}

pub(super) fn run(app: App) -> Result<(), String> {
    unsafe {
        SetProcessDPIAware();
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err(std::io::Error::last_os_error().to_string());
        }
        let icons = WindowIcons::load(instance)?;
        let session =
            Session::adopt(Box::new(app), Limits::default()).map_err(|(_, error)| error)?;
        let initial = session
            .window_ids()
            .next()
            .ok_or("The browser has no initial window.")?;
        let native_registry = Registry::new(40)?;
        let name = wide(CLASS);
        let class = WindowClass {
            style: 3,
            procedure: Some(procedure),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: icons.large,
            cursor: LoadCursorW(null_mut(), 32512usize as *const u16),
            background: null_mut(),
            menu_name: null(),
            class_name: name.as_ptr(),
        };
        if RegisterClassW(&class) == 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        if let Err(error) = inspector_window::register(instance, icons.large) {
            UnregisterClassW(name.as_ptr(), instance);
            return Err(error);
        }
        NATIVE.with(|slot| *slot.borrow_mut() = Some(native_registry));
        BROWSER.with(|slot| {
            *slot.borrow_mut() = Some(Browser {
                session,
                ui: SessionUi::new(),
                instance: instance as usize,
                large: icons.large as usize,
                small: icons.small as usize,
                finalizing: false,
                final_error: None,
                show_auxiliary: true,
                scheduler_failed: false,
            })
        });
        let result = (|| {
            create_main(initial, true)?;
            let mut message: Message = zeroed();
            loop {
                arm_scheduler();
                let code = GetMessageW(&mut message, null_mut(), 0, 0);
                if code == -1 {
                    return Err(std::io::Error::last_os_error().to_string());
                }
                if code == 0 {
                    // An unrelated WM_QUIT cannot silently skip final history I/O.
                    if !edit(|b| Ok(b.finalizing && b.final_error.is_none())).unwrap_or(false) {
                        if let Some(host) = records()
                            .into_iter()
                            .find(|r| matches!(r.owner, Owner::Main(_)))
                        {
                            finalize(host);
                            if edit(|b| Ok(b.final_error.is_some())).unwrap_or(false) {
                                continue;
                            }
                        }
                    }
                    return Ok(());
                }
                dispatch_native(&message);
            }
        })();
        // Infrastructure failure retires routes before dropping the session. No Drop
        // performs hidden history I/O; propagate explicit cleanup errors to main.
        let closing = registry(|r| r.close_all()).unwrap_or_default();
        for route in closing {
            destroy_route(route);
        }
        inspector_window::unregister(instance);
        UnregisterClassW(name.as_ptr(), instance);
        let cleanup = shutdown_attempt();
        BROWSER.with(|s| s.borrow_mut().take());
        NATIVE.with(|s| s.borrow_mut().take());
        match (result, cleanup) {
            (Err(e), Err(c)) => Err(format!("{e}\n\n{c}")),
            (Err(e), _) | (_, Err(e)) => Err(e),
            _ => Ok(()),
        }
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
            if !attach_key(key, hwnd) {
                return 0;
            }
            SetLastError(0);
            let previous = SetWindowLongPtrW(hwnd, -21, key as isize);
            if previous == 0 && GetLastError() != 0 {
                forget_key(key);
                return 0;
            }
            return 1;
        }
        0x0082 => {
            destroyed(hwnd);
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
        0x0002 => return 0, // Only successful finalization posts WM_QUIT.
        0x0014 => return 1,
        0x0024 => {
            (&mut *(lparam as *mut MinMaxInfo)).min_track_size = Point { x: 480, y: 320 };
            return 0;
        }
        _ => {}
    }
    // A failed final-history transaction retains only a closing retry host.
    let raw_key = GetWindowLongPtrW(hwnd, -21) as usize;
    if let Some(raw) = registry(|r| r.get(raw_key)).flatten() {
        if raw.closing {
            if message == 0x0010 {
                finalize(raw);
                return 0;
            }
            if message == 0x000f {
                let mut paint: PaintStruct = zeroed();
                let dc = BeginPaint(hwnd, &mut paint);
                let mut rect: Rect = zeroed();
                GetClientRect(hwnd, &mut rect);
                if !dc.is_null() && rect.right > 0 && rect.bottom > 0 {
                    let mut canvas =
                        crate::paint::Canvas::new(rect.right as usize, rect.bottom as usize);
                    canvas.fill_rect(0, 0, rect.right, rect.bottom, 0xf4f4f4);
                    canvas.text(
                        24,
                        32,
                        "Browsing stopped. Close this window to retry history cleanup.",
                        2,
                        0x202020,
                    );
                    present_canvas(dc, &rect, &canvas);
                }
                EndPaint(hwnd, &paint);
                return 0;
            }
            if message == 0x0233 {
                DragFinish(wparam as Handle);
                return 0;
            }
            return DefWindowProcW(hwnd, message, wparam, lparam);
        }
    }
    let Some(route) = binding(hwnd) else {
        if message == 0x0233 {
            DragFinish(wparam as Handle);
            return 0;
        }
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    let Owner::Main(window) = route.owner else {
        return DefWindowProcW(hwnd, message, wparam, lparam);
    };
    let initial = owner_for(route);
    match message {
        0x0010 => {
            close_native(window, false);
            0
        }
        0x0113 if wparam == TIMER => {
            poll(route);
            0
        }
        0x031a | 0x0320 => {
            forget_frame_colors(hwnd);
            refresh_window_without_prompt(window);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        0x000f => {
            let mut paint: PaintStruct = zeroed();
            let dc = BeginPaint(hwnd, &mut paint);
            let mut rect: Rect = zeroed();
            GetClientRect(hwnd, &mut rect);
            if !dc.is_null() && rect.right > 0 && rect.bottom > 0 && attached(route) {
                let frame = edit(|b| {
                    b.ui.resize(&mut b.session, window, rect.right, rect.bottom)?;
                    b.ui.frame(&b.session, window)
                });
                if let Ok(canvas) = frame {
                    present_canvas(dc, &rect, &canvas);
                }
            }
            EndPaint(hwnd, &paint);
            0
        }
        0x0005 => {
            let mut rect: Rect = zeroed();
            GetClientRect(hwnd, &mut rect);
            if rect.right > 0 && rect.bottom > 0 && attached(route) {
                if let Err(error) =
                    edit(|b| b.ui.resize(&mut b.session, window, rect.right, rect.bottom))
                {
                    report(route, &error);
                }
                inspector_window::sync(window);
                InvalidateRect(hwnd, null(), 0);
                arm_scheduler();
            }
            0
        }
        0x020a => {
            let (x, y) = mouse_coordinates(lparam);
            let mut point = Point { x, y };
            ScreenToClient(hwnd, &mut point);
            let delta = ((wparam >> 16) & 0xffff) as u16 as i16 as i32;
            let ctrl = key_is_down(0x11);
            let shift = key_is_down(0x10);
            if same_active(route, initial) {
                let result = mutate(|b| {
                    b.ui.wheel_at(&mut b.session, window, point.x, point.y, delta, ctrl, shift)
                });
                if let Err(error) = result {
                    report(route, &error);
                }
                refresh_window(window);
                arm_scheduler();
            }
            0
        }
        0x020b if matches!((wparam >> 16) & 0xffff, 1 | 2) => 1,
        0x020c if matches!((wparam >> 16) & 0xffff, 1 | 2) => {
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            if same_active(route, initial) {
                let _ = mutate(|b| {
                    b.ui.mouse_navigation(&mut b.session, window, (wparam >> 16) & 0xffff == 2)
                });
                refresh_window(window);
                arm_scheduler();
            }
            1
        }
        0x0319 if matches!((lparam as usize >> 16) & 0x0fff, 1 | 2) => {
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            if same_active(route, initial) {
                let _ = mutate(|b| {
                    b.ui.mouse_navigation(
                        &mut b.session,
                        window,
                        (lparam as usize >> 16) & 0x0fff == 2,
                    )
                });
                refresh_window(window);
                arm_scheduler();
            }
            1
        }
        0x0007 => {
            let _ = mutate(|b| {
                b.ui.activate_window(&mut b.session, window)?;
                b.session.with_active(window, |a| a.main_window_focus())
            });
            0
        }
        0x0100 | 0x0104 => {
            let key = wparam as u32;
            let _ = key_owner(hwnd, key, true, route);
            if matches!(key, 0xa6 | 0xa7) {
                let _ = mutate(|b| b.ui.mouse_navigation(&mut b.session, window, key == 0xa7));
                refresh_window(window);
                arm_scheduler();
                return 0;
            }
            let input = keyboard_input::event(key, lparam, true, message == 0x0104);
            let (ctrl, alt) = (input.ctrl, input.alt);
            if input.meta
                || message == 0x0104
                    && !(ctrl && alt)
                    && (!alt || !matches!(key, 0x25 | 0x27 | 0x24 | 0x44 | 0x46))
            {
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            let result = mutate(|b| b.ui.key_event(&mut b.session, window, input));
            if result.as_ref().map_or(true, |o| o.suppress_text) {
                keyboard_input::cancel(hwnd, message, wparam, lparam);
            }
            outcome(route, result);
            0
        }
        0x0101 | 0x0105 => {
            let input = keyboard_input::event(wparam as u32, lparam, false, message == 0x0105);
            if let Some(origin) = key_owner(hwnd, wparam as u32, false, route) {
                if observe_tab(origin, |_| ()).is_ok() {
                    let result = mutate(|b| b.ui.key_event(&mut b.session, window, input));
                    outcome(route, result);
                }
            }
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        0x0102 | 0x0106 => {
            let ctrl = key_is_down(0x11);
            let alt = key_is_down(0x12);
            if message == 0x0106 && !(ctrl && alt) {
                return if matches!(wparam as u32, 0x44 | 0x64 | 0x46 | 0x66) {
                    0
                } else {
                    DefWindowProcW(hwnd, message, wparam, lparam)
                };
            }
            if wparam <= u16::MAX as usize && wparam >= 0x20 && wparam != 0x7f && (!ctrl || alt) {
                if let Some(origin) = text_owner(hwnd, lparam) {
                    if with_tab(origin, |a| a.utf16_input(wparam as u16)).is_ok() {
                        refresh_window(window);
                        arm_scheduler();
                    }
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
                if let Some(origin) = owner_for(route) {
                    let _ = with_tab(origin, |a| a.text_input(ch));
                    refresh_window(window);
                    arm_scheduler();
                }
            }
            0
        }
        0x0201 => {
            let (x, y) = mouse_coordinates(lparam);
            let shift = key_is_down(0x10);
            SetFocus(hwnd);
            if !same_active(route, initial) {
                return 0;
            }
            if key_is_down(0x12)
                && !key_is_down(0x11)
                && !shift
                && mutate(|b| b.ui.download_at(&mut b.session, window, x, y)).unwrap_or(false)
            {
                refresh_window(window);
                arm_scheduler();
                return 0;
            }
            SetCapture(hwnd);
            if same_active(route, initial) {
                let result = mutate(|b| b.ui.pointer_down(&mut b.session, window, x, y, shift));
                outcome(route, result);
            }
            0
        }
        0x0200 => {
            let mut tracking = MouseTracking {
                size: size_of::<MouseTracking>() as u32,
                flags: 2,
                window: hwnd,
                hover_time: 0,
            };
            TrackMouseEvent(&mut tracking);
            let (x, y) = mouse_coordinates(lparam);
            let dragging = wparam & 1 != 0 && GetCapture() == hwnd;
            if same_active(route, initial) {
                let changed = edit(|b| b.ui.pointer_move(&mut b.session, window, x, y, dragging))
                    .unwrap_or(false);
                cursor(route, x, y);
                if changed {
                    InvalidateRect(hwnd, null(), 0);
                }
                arm_scheduler();
            }
            0
        }
        0x02a3 => {
            if edit(|b| b.ui.pointer_leave(&mut b.session, window)).unwrap_or(false) {
                InvalidateRect(hwnd, null(), 0);
            }
            0
        }
        0x0202 => {
            let (x, y) = mouse_coordinates(lparam);
            let result = mutate(|b| b.ui.pointer_up(&mut b.session, window, x, y));
            if GetCapture() == hwnd {
                ReleaseCapture();
            }
            if attached(route) {
                let _ = edit(|b| b.ui.pointer_move(&mut b.session, window, x, y, false));
            }
            outcome(route, result);
            0
        }
        0x0215 | 0x0008 | 0x001f => {
            let _ = edit(|b| b.ui.pointer_cancel(&mut b.session, window));
            INPUT.with(|r| r.borrow_mut().forget(hwnd as usize));
            if message != 0x0215 && GetCapture() == hwnd {
                ReleaseCapture();
            }
            InvalidateRect(hwnd, null(), 0);
            DefWindowProcW(hwnd, message, wparam, lparam)
        }
        0x0020 if lparam & 0xffff == 1 => {
            let mut point = Point { x: 0, y: 0 };
            if GetCursorPos(&mut point) != 0 && ScreenToClient(hwnd, &mut point) != 0 {
                cursor(route, point.x, point.y);
                1
            } else {
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
        }
        0x0233 => {
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
                if let Some(origin) = initial.filter(|_| same_active(route, initial)) {
                    let _ = with_tab(origin, |a| a.open(&path));
                }
            }
            refresh_window(window);
            arm_scheduler();
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

#[cfg(test)]
#[path = "platform_session_tests.rs"]
mod tests;
