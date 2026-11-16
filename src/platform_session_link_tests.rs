//! Original task-created hidden-HWND link lifetime controls. ALL UNRUN.
//! Message payloads are deterministic dispatcher controls, not physical OS input
//! acceptance. No user-owned HWND, clipboard, popup interaction or live site.
use super::*;
use std::path::PathBuf;
std::thread_local! { static RECAPTURE_PATH:RefCell<Option<PathBuf>>=const{RefCell::new(None)}; }

#[link(name = "user32")]
extern "system" { fn ClientToScreen(window: Handle, point: *mut Point) -> i32; }

struct LinkFiles(PathBuf);
impl LinkFiles {
    fn new(script: &str) -> Self {
        let mut bytes = [0u8; 8];
        crate::random::fill(&mut bytes).unwrap();
        let suffix: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
        let root = std::env::temp_dir().join(format!("chezcat-native-link-{suffix}"));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("target.html"), "<!doctype html><title>Native destination</title><p>Offline target</p>").unwrap();
        fs::write(root.join("source.html"), format!("<!doctype html><title>Native source</title><style>body{{margin:0}}a{{display:block;width:180px;height:32px}}</style><body><a id=go href=target.html>Original anchor</a><script>var go=document.getElementById('go'),calls=0;{script}</script></body>")).unwrap();
        Self(root)
    }
    fn app(&self) -> App {
        let mut app = App::new();
        app.open(&self.0.join("source.html"));
        app.wait_for_load();
        assert!(app.title().contains("Native source"), "Offline fixture did not commit: {}", app.activity_report());
        app
    }
}
impl Drop for LinkFiles { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
fn client_point(origin: InputOwner) -> (i32, i32) {
    observe_tab(origin, |app| {
        let (x, y) = (80, app.content_top() + 16);
        assert!(app.anchor_hit(x, y, 0).is_some(), "Fixture point is not a rendered original anchor");
        (x, y + crate::app::session::window_ui::STRIP_HEIGHT)
    }).unwrap()
}
fn packed(x: i32, y: i32) -> isize {
    (usize::from(x as u16) | usize::from(y as u16) << 16) as isize
}
fn queued_up(hwnd: Handle, button: u8, x: i32, y: i32, modifiers: usize, screen: (i32, i32)) -> Message {
    Message { window: hwnd, message: if button == 0 { 0x0202 } else { 0x0208 },
        wparam: modifiers, lparam: packed(x, y), time: 31,
        point: Point { x: screen.0, y: screen.1 }, private: 0 }
}
unsafe fn screen_point(hwnd: Handle, point: (i32, i32)) -> Point {
    let mut measured = Point { x: point.0, y: point.1 };
    assert_ne!(ClientToScreen(hwnd, &mut measured), 0);
    measured
}
unsafe fn leased_down(route: Binding, origin: InputOwner, point: (i32, i32), button: u8) -> PointerCapture {
    let lease = begin_pointer_capture(route, origin, button).unwrap();
    SetCapture(route.hwnd as Handle);
    assert_eq!(GetCapture(), route.hwnd as Handle);
    mutate(|b| b.ui.link_pointer_down(&mut b.session, origin.window, point.0, point.1, button, false)).unwrap();
    finish_pointer_capture(lease);
    assert!(current_capture(lease));
    lease
}
unsafe fn recapture_replacement(route: Binding) {
    no_native_borrows();
    let previous = owner_for(route).unwrap();
    mutate(|b| b.session.new_tab(previous.window)).unwrap();
    let replacement = owner_for(route).unwrap();
    let path = RECAPTURE_PATH.with(|slot| slot.borrow().as_ref().unwrap().clone());
    with_tab(replacement, |app| app.open(&path)).unwrap();
    leased_down(route, replacement, client_point(replacement), 0);
    no_native_borrows();
}
fn no_native_borrows() {
    assert!(BROWSER.with(|slot| slot.try_borrow_mut().is_ok()), "Browser guard crossed a native popup boundary");
    assert!(NATIVE.with(|slot| slot.try_borrow_mut().is_ok()), "Registry guard crossed a native popup boundary");
}
unsafe fn background_cancel_reentry(route: Binding) -> usize {
    no_native_borrows();
    SendMessageW(route.hwnd as Handle, 0x001f, 0, 0);
    no_native_borrows();
    LINK_BACKGROUND
}
unsafe fn background_after_input(route: Binding) -> usize {
    no_native_borrows();
    SendMessageW(route.hwnd as Handle, 0x0200, 0, packed(80, 160));
    LINK_BACKGROUND
}
unsafe fn background_after_move(route: Binding) -> usize {
    no_native_borrows();
    SendMessageW(route.hwnd as Handle, 0x0003, 0, packed(41, 57));
    LINK_BACKGROUND
}
unsafe fn background_after_owner_aba(route: Binding) -> usize {
    no_native_borrows();
    let original = owner_for(route).unwrap();
    mutate(|b| {
        let _ = b.session.new_tab(original.window)?;
        b.session.switch_tab(original.window, original.tab)
    }).unwrap();
    LINK_BACKGROUND
}
unsafe fn foreground_after_close(route: Binding) -> usize {
    no_native_borrows();
    close_native(route.owner.window(), false);
    no_native_borrows();
    LINK_FOREGROUND
}
unsafe fn pre_context_input(route: Binding) {
    no_native_borrows();
    SendMessageW(route.hwnd as Handle, 0x0200, 0, packed(80, 160));
}
unsafe fn pre_context_same_owner_reload(route: Binding) {
    no_native_borrows();
    let original = owner_for(route).unwrap();
    with_tab(original, |app| app.reload()).unwrap();
}
unsafe fn choose_foreground(_: Binding) -> usize { no_native_borrows(); LINK_FOREGROUND }
unsafe fn choose_window(_: Binding) -> usize { no_native_borrows(); LINK_WINDOW }
unsafe fn context(shell: &Shell) {
    let point = screen_point(shell.hwnd, client_point(shell.origin()));
    SendMessageW(shell.hwnd, 0x007b, shell.hwnd as usize, packed(point.x, point.y));
}

#[test]
fn hidden_link_release_envelope_preserves_posted_point_full_owner_and_is_one_shot() {
    ui_thread(|| unsafe {
        let shell = Shell::new(App::new());
        let route = binding(shell.hwnd).unwrap();
        let origin = shell.origin();
        let message = queued_up(shell.hwnd, 1, 71, 109, 0x0008 | 0x0004, (-345, 678));
        let release = release_envelope(&message).unwrap();
        assert_eq!(release.origin, origin);
        assert_eq!(release.route.key, route.key);
        assert_eq!((release.input.button, release.input.ctrl, release.input.shift), (1, true, true));
        assert_eq!((release.input.screen_x, release.input.screen_y), (-345.0, 678.0));
        MOUSE_RELEASE.with(|slot| slot.set(Some(release)));
        assert!(take_release(shell.hwnd, message.message, message.wparam, message.lparam + 1).is_none());
        assert!(take_release(shell.hwnd, message.message, message.wparam, message.lparam).is_some());
        assert!(take_release(shell.hwnd, message.message, message.wparam, message.lparam).is_none());
        let replacement = mutate(|b| b.session.new_tab(origin.window)).unwrap();
        assert_ne!(replacement, origin.tab);
        assert!(!exact_release(release, route, owner_for(route)), "Old release rebound to a new active owner");
        assert!(native_matches(route));
        no_native_borrows();
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_native_handlers_use_original_primary_and_middle_release_payload_once() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.addEventListener('click',function(e){if(e.button!==0||!e.ctrlKey||e.shiftKey||e.screenX!==-345||e.screenY!==678||e.clientX!==80||e.clientY!==16){document.title='BAD primary payload';e.preventDefault();return;}calls++;document.title='Primary '+calls;e.preventDefault();});go.addEventListener('auxclick',function(e){if(e.button!==1||e.ctrlKey||e.shiftKey||e.screenX!==121||e.screenY!==222){document.title='BAD middle payload';e.preventDefault();return;}calls++;document.title='Middle '+calls;e.preventDefault();});");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        let (x, y) = client_point(origin);
        // Arm the original pure gesture without sampling any user-held global keys.
        leased_down(binding(shell.hwnd).unwrap(), origin, (x, y), 0);
        let primary = queued_up(shell.hwnd, 0, x, y, 0x0008, (-345, 678));
        dispatch_native(&primary);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Primary 1"));
        dispatch_native(&primary);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Primary 1"));
        SendMessageW(shell.hwnd, 0x0207, 0x0010, packed(x, y));
        let middle = queued_up(shell.hwnd, 1, x, y, 0, (121, 222));
        dispatch_native(&middle);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Middle 2"));
        assert_eq!(edit(|b| Ok(b.session.tab_count())).unwrap(), 1);
        assert!(MOUSE_RELEASE.with(|slot| slot.get().is_none()));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_unpaired_sent_release_preserves_native_lease_until_its_queued_release() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(e){calls++;document.title='Genuine retained '+calls;e.preventDefault();};");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        let (x, y) = client_point(origin);
        let lease = leased_down(binding(shell.hwnd).unwrap(), origin, (x, y), 0);
        SendMessageW(shell.hwnd, 0x0202, 0, packed(x, y));
        assert!(current_capture(lease), "Sent up inferred and consumed a current lease");
        assert_eq!(GetCapture(), shell.hwnd);
        assert!(edit(|b| Ok(b.ui.has_link_pointer(&b.session, origin.window, origin.tab, 0))).unwrap());
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Native source"));
        let up = queued_up(shell.hwnd, 0, x, y, 0, (99, 88));
        dispatch_native(&up);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Genuine retained 1"));
        dispatch_native(&up);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Genuine retained 1"));
        assert_eq!(edit(|b| Ok(b.session.tab_count())).unwrap(), 1);
        assert_eq!(GetCapture(), null_mut());
        assert!(POINTER_CAPTURE.with(|slot| slot.get().is_none()));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_popup_capture_cancel_reenters_guard_free_and_background_keeps_original_owner() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(){document.title='BAD context forged author click';};");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        TEST_LINK_MENU_DROPS.with(|slot| slot.set(0));
        TEST_LINK_MENU.with(|slot| slot.set(Some(background_cancel_reentry)));
        context(&shell);
        assert_eq!(TEST_LINK_MENU_DROPS.with(|slot| slot.get()), 1);
        assert_eq!(edit(|b| b.session.active(origin.window)).unwrap(), Some(origin.tab));
        let tabs = edit(|b| Ok(b.session.window_tabs(origin.window)?.to_vec())).unwrap();
        assert_eq!(tabs.len(), 2);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Native source"));
        assert!(edit(|b| b.session.observe_tab(tabs[1], |app| app.title())).unwrap().contains("Native destination"));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_popup_intervening_input_owner_aba_and_keyboard_request_never_rehit() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(){document.title='BAD context click';};");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        for hook in [background_after_input as unsafe fn(Binding)->usize, background_after_move as unsafe fn(Binding)->usize] {
            TEST_LINK_MENU.with(|slot| slot.set(Some(hook)));
            context(&shell);
            assert_eq!(edit(|b| Ok(b.session.tab_count())).unwrap(), 1);
        }
        TEST_LINK_MENU.with(|slot| slot.set(Some(background_after_owner_aba)));
        context(&shell);
        assert_eq!(edit(|b| Ok(b.session.tab_count())).unwrap(), 2, "ABA must add only the deliberate peer, never a popup destination");
        assert_eq!(edit(|b| b.session.active(origin.window)).unwrap(), Some(origin.tab));
        let before = TEST_LINK_MENU_DROPS.with(|slot| slot.get());
        SendMessageW(shell.hwnd, 0x007b, shell.hwnd as usize, packed(-1, -1));
        assert_eq!(TEST_LINK_MENU_DROPS.with(|slot| slot.get()), before, "Keyboard request cursor-rehit a link");
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Native source"));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_popup_source_close_drops_original_context_without_touching_peer() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        let (other, peer_tab) = mutate(|b| b.session.new_window(640, 480)).unwrap();
        let peer = create_main(other, false).unwrap();
        TEST_LINK_MENU.with(|slot| slot.set(Some(foreground_after_close)));
        context(&shell);
        assert_eq!(IsWindow(shell.hwnd), 0);
        assert_eq!(IsWindow(peer), 1);
        assert!(binding(shell.hwnd).is_none());
        assert_eq!(edit(|b| Ok((b.session.window_count(), b.session.tab_count(), b.session.active(other)?))).unwrap(), (1, 1, Some(peer_tab)));
        assert!(with_tab(origin, |_| panic!("Closed context reached its old App")).is_err());
        assert!(!has_quit(false));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_new_window_uses_existing_host_pipeline_and_deterministic_refusal_rolls_back_only_destination() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        TEST_LINK_MENU.with(|slot| slot.set(Some(choose_window)));
        context(&shell);
        let windows = edit(|b| Ok(b.session.window_ids().collect::<Vec<_>>())).unwrap();
        assert_eq!(windows.len(), 2);
        let opened = *windows.iter().find(|window| **window != origin.window).unwrap();
        let opened_route = main_route(opened).expect("Admitted window did not receive its native host");
        assert_eq!(IsWindow(opened_route.hwnd as Handle), 1);
        assert!(edit(|b| b.session.observe_active(opened, |app| app.title())).unwrap().contains("Native destination"));
        assert_eq!(edit(|b| b.session.active(origin.window)).unwrap(), Some(origin.tab));
        close_native(opened, false);
        TEST_LINK_HOST_REFUSAL.with(|slot| slot.set(true));
        TEST_LINK_MENU.with(|slot| slot.set(Some(choose_window)));
        context(&shell);
        assert_eq!(edit(|b| Ok((b.session.window_count(), b.session.tab_count()))).unwrap(), (1, 1));
        assert_eq!(records().into_iter().filter(|route| matches!(route.owner, Owner::Main(_)) && !route.closing).count(), 1);
        assert!(native_matches(binding(shell.hwnd).unwrap()));
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Native source"));
        let expected = TEST_MODAL_ERRORS.with(|slot| slot.borrow_mut().as_mut().unwrap().pop()).unwrap();
        assert!(expected.contains("Task-created native host admission refusal"), "Unexpected rollback report: {expected}");
        assert!(!TEST_LINK_HOST_REFUSAL.with(|slot| slot.get()));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_pre_native_origin_stamp_refuses_input_or_same_owner_reload_before_popup_capture() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(){document.title='BAD rebound';};");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        TEST_LINK_MENU_DROPS.with(|slot| slot.set(0));
        for hook in [pre_context_input as unsafe fn(Binding), pre_context_same_owner_reload as unsafe fn(Binding)] {
            TEST_LINK_CONTEXT_REENTRY.with(|slot| slot.set(Some(hook)));
            TEST_LINK_MENU.with(|slot| slot.set(Some(background_cancel_reentry)));
            context(&shell);
            assert_eq!(edit(|b| Ok(b.session.tab_count())).unwrap(), 1, "Pre-native source change rebound to a new popup target");
            assert_eq!(TEST_LINK_MENU_DROPS.with(|slot| slot.get()), 0, "Stale origin created a popup before refusal");
            assert!(TEST_LINK_MENU.with(|slot| slot.take().is_some()), "Stale origin reached popup command selection");
            assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Native source"));
        }
        assert!(!TEST_LINK_CONTEXT_REENTRY.with(|slot| slot.get().is_some()));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_foreground_admission_purges_old_translated_owner_before_new_page_text() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        let scan = 0x001e0001;
        // Retain the exact old translated owner and its posted character, with
        // no keyboard device or fabricated DOM event. Foreground admission must
        // discard both before the replacement App can receive text.
        INPUT.with(|routes| routes.borrow_mut().translated(shell.hwnd as usize, scan, origin));
        assert_ne!(PostMessageW(shell.hwnd, 0x0102, usize::from(b'a'), scan), 0);
        let mut character: Message = zeroed();
        assert_ne!(PeekMessageW(&mut character, shell.hwnd, 0x0102, 0x0102, 0), 0);
        TEST_LINK_MENU.with(|slot| slot.set(Some(choose_foreground)));
        context(&shell);
        let active = edit(|b| b.session.active(origin.window)).unwrap().unwrap();
        assert_ne!(active, origin.tab);
        assert_eq!(INPUT.with(|routes| routes.borrow().text(shell.hwnd as usize, scan)), None);
        assert_eq!(PeekMessageW(&mut character, shell.hwnd, 0x0102, 0x0102, 0), 0);
        assert!(edit(|b| b.session.observe_tab(origin.tab, |app| app.title())).unwrap().contains("Native source"));
        assert!(edit(|b| b.session.observe_tab(active, |app| app.title())).unwrap().contains("Native destination"));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_unpaired_release_without_retained_press_preserves_replacement_surrogate_and_editor() {
    ui_thread(|| unsafe {
        let shell = Shell::new(App::new());
        let previous = shell.origin();
        let replacement = mutate(|b| b.session.new_tab(previous.window)).unwrap();
        let current = shell.origin();
        assert_eq!(current.tab, replacement);
        assert_ne!(current.tab, previous.tab);
        let control = |key_code| crate::scripting::KeyboardInput {
            down: true, ctrl: true, key_code, key: "Unidentified".into(),
            ..crate::scripting::KeyboardInput::default()
        };
        with_tab(current, |app| {
            app.key_event(control(0x4c));
            app.utf16_input(0xd83e);
        }).unwrap();
        // No retained gesture exists for either owner. A sent stale release
        // must not fall back to cancelling the replacement active editor.
        SendMessageW(shell.hwnd, 0x0202, 0, packed(17, 43));
        let copied = with_tab(current, |app| {
            app.utf16_input(0xdd89);
            app.key_event(control(0x41));
            app.key_event(control(0x43)).0
        }).unwrap();
        match copied {
            PlatformAction::Copy(text) => assert_eq!(text, "🦉", "Old release cleared replacement input state"),
            _ => panic!("Original App copy command did not expose the replacement address value"),
        }
        assert_eq!(edit(|b| b.session.active(current.window)).unwrap(), Some(replacement));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_old_owner_envelope_preserves_replacement_press_capture_and_translated_route() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(e){calls++;document.title='Retained press '+calls;e.preventDefault();};");
        let shell = Shell::new(files.app());
        let old = shell.origin();
        let point = client_point(old);
        leased_down(binding(shell.hwnd).unwrap(), old, point, 0);
        let old_message = queued_up(shell.hwnd, 0, point.0, point.1, 0, (17, 29));
        let old_release = release_envelope(&old_message).unwrap();
        mutate(|b| b.session.new_tab(old.window)).unwrap();
        let replacement = shell.origin();
        with_tab(replacement, |app| app.open(&files.0.join("source.html"))).unwrap();
        let point = client_point(replacement);
        leased_down(binding(shell.hwnd).unwrap(), replacement, point, 0);
        assert_eq!(GetCapture(), shell.hwnd);
        let scan = 0x001e0001;
        INPUT.with(|routes| routes.borrow_mut().translated(shell.hwnd as usize, scan, replacement));
        MOUSE_RELEASE.with(|slot| slot.set(Some(old_release)));
        SendMessageW(shell.hwnd, old_message.message, old_message.wparam, old_message.lparam);
        assert_eq!(GetCapture(), shell.hwnd, "Old source release stole replacement pointer capture");
        assert_eq!(INPUT.with(|routes| routes.borrow().text(shell.hwnd as usize, scan)), Some(replacement));
        let up = queued_up(shell.hwnd, 0, point.0, point.1, 0, (121, 222));
        dispatch_native(&up);
        assert!(observe_tab(replacement, |app| app.title()).unwrap().contains("Retained press 1"), "Old source release cancelled the new owner's gesture");
        assert_eq!(GetCapture(), null_mut());
        assert_eq!(INPUT.with(|routes| routes.borrow().text(shell.hwnd as usize, scan)), Some(replacement), "Capture loss cleared unchanged keyboard ownership");
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_capture_lease_retires_on_source_switch_before_late_up() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(){document.title='BAD late release';};");
        let shell = Shell::new(files.app());
        let original = shell.origin();
        let point = client_point(original);
        let old = leased_down(binding(shell.hwnd).unwrap(), original, point, 0);
        assert!(current_capture(old));
        mutate(|b| b.session.new_tab(original.window)).unwrap();
        let replacement = shell.origin();
        assert_ne!(replacement.tab, original.tab);
        assert_eq!(GetCapture(), null_mut(), "Source switch stranded its old HWND capture");
        assert!(POINTER_CAPTURE.with(|slot| slot.get().is_none()));
        let late = queued_up(shell.hwnd, 0, point.0, point.1, 0, (17, 29));
        dispatch_native(&late);
        assert_eq!(GetCapture(), null_mut());
        assert_eq!(shell.origin(), replacement);
        assert!(edit(|b| b.session.observe_tab(original.tab, |app| app.title())).unwrap().contains("Native source"));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_capture_release_reentry_preserves_higher_generation_replacement_until_its_up() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(e){calls++;document.title='Retained press '+calls;e.preventDefault();};");
        let shell = Shell::new(files.app());
        let original = shell.origin();
        let point = client_point(original);
        let old = leased_down(binding(shell.hwnd).unwrap(), original, point, 0);
        RECAPTURE_PATH.with(|slot| *slot.borrow_mut() = Some(files.0.join("source.html")));
        TEST_CAPTURE_REENTRY.with(|slot| slot.set(Some(recapture_replacement)));
        let up = queued_up(shell.hwnd, 0, point.0, point.1, 0, (17, 29));
        dispatch_native(&up);
        let replacement = shell.origin();
        assert_ne!(replacement.tab, original.tab);
        let fresh = POINTER_CAPTURE.with(|slot| slot.get()).expect("Old ReleaseCapture cleared nested replacement lease");
        assert_eq!(fresh.origin, replacement);
        assert!(fresh.generation > old.generation);
        assert!(!fresh.pending);
        assert_eq!(GetCapture(), shell.hwnd, "Old ReleaseCapture stole nested replacement capture");
        let point = client_point(replacement);
        let up = queued_up(shell.hwnd, 0, point.0, point.1, 0, (121, 222));
        dispatch_native(&up);
        assert_eq!(GetCapture(), null_mut());
        assert!(POINTER_CAPTURE.with(|slot| slot.get().is_none()));
        assert!(observe_tab(replacement, |app| app.title()).unwrap().contains("Retained press 1"));
        assert!(edit(|b| b.session.observe_tab(original.tab, |app| app.title())).unwrap().contains("Retained press 1"));
        RECAPTURE_PATH.with(|slot| slot.borrow_mut().take());
        assert!(!TEST_CAPTURE_REENTRY.with(|slot| slot.get().is_some()));
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_capture_generation_exhaustion_refuses_without_wrap_or_native_capture() {
    ui_thread(|| unsafe {
        let shell = Shell::new(App::new());
        NEXT_POINTER_CAPTURE.with(|slot| slot.set(u64::MAX));
        assert!(begin_pointer_capture(binding(shell.hwnd).unwrap(), shell.origin(), 0).is_none());
        assert_eq!(NEXT_POINTER_CAPTURE.with(|slot| slot.get()), u64::MAX);
        assert!(POINTER_CAPTURE.with(|slot| slot.get().is_none()));
        assert_eq!(GetCapture(), null_mut());
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_old_generation_envelope_preserves_newer_same_owner_press_until_its_up() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(e){calls++;document.title='Same owner press '+calls;e.preventDefault();};");
        let shell = Shell::new(files.app());
        let origin = shell.origin();
        let route = binding(shell.hwnd).unwrap();
        let point = client_point(origin);
        let old = leased_down(route, origin, point, 0);
        let message = queued_up(shell.hwnd, 0, point.0, point.1, 0, (17, 29));
        let old_release = release_envelope(&message).unwrap();
        let fresh = leased_down(route, origin, point, 0);
        assert!(fresh.generation > old.generation);
        assert_eq!(fresh.origin, old.origin);
        MOUSE_RELEASE.with(|slot| slot.set(Some(old_release)));
        SendMessageW(shell.hwnd, message.message, message.wparam, message.lparam);
        assert!(current_capture(fresh), "Old same-owner envelope cleared the newer lease");
        assert_eq!(GetCapture(), shell.hwnd, "Old same-owner envelope released the newer capture");
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Native source"));
        // An envelope without any original lease likewise cannot consume the
        // already retained newer same-owner native gesture.
        MOUSE_RELEASE.with(|slot| slot.set(Some(MouseRelease { capture: None, ..old_release })));
        SendMessageW(shell.hwnd, message.message, message.wparam, message.lparam);
        assert!(current_capture(fresh));
        assert_eq!(GetCapture(), shell.hwnd);
        let up = queued_up(shell.hwnd, 0, point.0, point.1, 0, (121, 222));
        dispatch_native(&up);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Same owner press 1"));
        dispatch_native(&up);
        assert!(observe_tab(origin, |app| app.title()).unwrap().contains("Same owner press 1"));
        assert!(POINTER_CAPTURE.with(|slot| slot.get().is_none()));
        assert_eq!(GetCapture(), null_mut());
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}

#[test]
fn hidden_link_unpaired_up_cannot_consume_programmatic_pure_press_without_native_lease() {
    ui_thread(|| unsafe {
        let files = LinkFiles::new("go.onclick=function(e){calls++;document.title='Pure retained '+calls;e.preventDefault();};");
        let shell = Shell::new(files.app());
        let previous = shell.origin();
        mutate(|b| b.session.new_tab(previous.window)).unwrap();
        let replacement = shell.origin();
        with_tab(replacement, |app| app.open(&files.0.join("source.html"))).unwrap();
        let point = client_point(replacement);
        mutate(|b| b.ui.link_pointer_down(&mut b.session, replacement.window, point.0, point.1, 0, false)).unwrap();
        assert!(POINTER_CAPTURE.with(|slot| slot.get().is_none()));
        assert_eq!(GetCapture(), null_mut());
        SendMessageW(shell.hwnd, 0x0202, 0, packed(point.0, point.1));
        assert!(edit(|b| Ok(b.ui.has_link_pointer(&b.session, replacement.window, replacement.tab, 0))).unwrap(), "Unpaired up cancelled a programmatic replacement press");
        assert!(observe_tab(replacement, |app| app.title()).unwrap().contains("Native source"));
        let result = mutate(|b| b.ui.link_pointer_up(&mut b.session, replacement.window, point.0, point.1,
            AnchorMouseInput { screen_x: 121.0, screen_y: 222.0, ..AnchorMouseInput::default() }));
        outcome(binding(shell.hwnd).unwrap(), result);
        assert!(observe_tab(replacement, |app| app.title()).unwrap().contains("Pure retained 1"));
        assert!(!edit(|b| Ok(b.ui.has_link_pointer(&b.session, replacement.window, replacement.tab, 0))).unwrap());
        assert!(POINTER_CAPTURE.with(|slot| slot.get().is_none()));
        assert_eq!(GetCapture(), null_mut());
        assert_eq!(edit(|b| Ok(b.session.tab_count())).unwrap(), 2);
        assert!(!TEST_REENTRY_BUSY.with(|slot| slot.get()));
    });
}
