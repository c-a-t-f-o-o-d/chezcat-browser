//! Native lifetime controls use only new hidden HWNDs on a dedicated UI thread.
//! No OS input, owner windows, clipboard or live sites.
use super::*;
use std::fs;
use std::sync::Mutex;
static SERIAL: Mutex<()> = Mutex::new(());
#[link(name = "user32")]
extern "system" {
    fn IsWindow(hwnd: Handle) -> i32;
}
struct Shell {
    instance: Handle,
    initial: WindowId,
    hwnd: Handle,
}
impl Shell {
    unsafe fn new(app: App) -> Self {
        let instance = GetModuleHandleW(null());
        assert!(!instance.is_null());
        let name = wide(CLASS);
        let class = WindowClass {
            style: 3,
            procedure: Some(procedure),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: null_mut(),
            cursor: null_mut(),
            background: null_mut(),
            menu_name: null(),
            class_name: name.as_ptr(),
        };
        assert_ne!(
            RegisterClassW(&class),
            0,
            "{}",
            std::io::Error::last_os_error()
        );
        inspector_window::register(instance, null_mut()).unwrap();
        let session = Session::adopt(Box::new(app), Limits::default())
            .map_err(|(_, e)| e)
            .unwrap();
        let initial = session.window_ids().next().unwrap();
        NATIVE.with(|s| *s.borrow_mut() = Some(Registry::new(40).unwrap()));
        BROWSER.with(|s| {
            *s.borrow_mut() = Some(Browser {
                session,
                ui: SessionUi::new(),
                instance: instance as usize,
                large: 0,
                small: 0,
                finalizing: false,
                final_error: None,
                show_auxiliary: false,
                scheduler_failed: false,
            })
        });
        TEST_REENTRY_BUSY.with(|s| s.set(false));
        let hwnd = create_main(initial, false).unwrap();
        assert_eq!(IsWindow(hwnd), 1);
        Self {
            instance,
            initial,
            hwnd,
        }
    }
    fn origin(&self) -> InputOwner {
        owner_for(unsafe { binding(self.hwnd) }.unwrap()).unwrap()
    }
}
impl Drop for Shell {
    fn drop(&mut self) {
        unsafe {
            let routes = registry(|r| r.close_all()).unwrap_or_default();
            for route in routes {
                destroy_route(route);
            }
            let _ = shutdown_attempt();
            inspector_window::unregister(self.instance);
            let name = wide(CLASS);
            UnregisterClassW(name.as_ptr(), self.instance);
            BROWSER.with(|s| s.borrow_mut().take());
            NATIVE.with(|s| s.borrow_mut().take());
            INPUT.with(|s| *s.borrow_mut() = InputRoutes::new());
            // The dedicated UI thread exits after this fixture. An unrestricted
            // queue drain can repeatedly retrieve an unvalidated WM_PAINT.
        }
    }
}
unsafe fn has_quit(remove: bool) -> bool {
    let mut message: Message = zeroed();
    PeekMessageW(&mut message, null_mut(), 0x12, 0x12, u32::from(remove)) != 0
}
fn ui_thread(body: impl FnOnce() + Send + 'static) {
    let _serial = SERIAL.lock().unwrap();
    std::thread::spawn(move || {
        TEST_MODAL_ERRORS.with(|slot| *slot.borrow_mut() = Some(Vec::new()));
        body();
        let errors = TEST_MODAL_ERRORS.with(|slot| slot.borrow_mut().take().unwrap());
        assert!(errors.is_empty(), "Unexpected native modal errors: {errors:?}");
    }).join().unwrap();
}

#[test]
fn hidden_native_numeric_keys_reenter_close_and_retire_pinned_inspector_without_quitting_peer() {
    ui_thread(|| unsafe {
        let shell = Shell::new(App::new());
        let initial = shell.origin();
        let main = binding(shell.hwnd).unwrap();
        assert_eq!(GetWindowLongPtrW(shell.hwnd, -21) as usize, main.key);
        let request = with_tab(initial, |a| a.platform_request(PlatformAction::Paste)).unwrap();
        with_tab(initial, |a| a.detach_inspector()).unwrap();
        inspector_window::sync(shell.initial);
        let tools = inspector_routes(shell.initial)
            .into_iter()
            .find(|r| r.owner == Owner::Inspector(initial.window, initial.tab))
            .unwrap();
        assert_eq!(IsWindow(tools.hwnd as Handle), 1);
        let inspected = owner_for(tools).unwrap();
        assert!(inspected.inspector);
        let (other, tab) = mutate(|b| b.session.new_window(700, 500)).unwrap();
        let peer = create_main(other, false).unwrap();
        let peer_route = binding(peer).unwrap();
        assert_ne!(peer_route.key, main.key);
        let armed = records()
            .into_iter()
            .filter(|r| r.timer.is_some())
            .collect::<Vec<_>>();
        assert_eq!(
            armed.len(),
            1,
            "Only one browser scheduling timer may be armed."
        );
        let deadline = armed[0].timer;
        arm_scheduler();
        assert_eq!(
            registry(|r| r.get(armed[0].key)).flatten().unwrap().timer,
            deadline,
            "Ordinary activity postponed an already armed browser deadline."
        );
        // DestroyWindow synchronously reenters DESTROY/NCDESTROY. The production
        // callback records Browser/registry borrows still held at that boundary.
        SendMessageW(shell.hwnd, 0x0010, 0, 0);
        assert_eq!(IsWindow(shell.hwnd), 0);
        assert_eq!(IsWindow(tools.hwnd as Handle), 0);
        assert_eq!(IsWindow(peer), 1);
        let armed = records()
            .into_iter()
            .filter(|r| r.timer.is_some())
            .collect::<Vec<_>>();
        assert_eq!(armed.len(), 1);
        assert_eq!(armed[0].owner, Owner::Main(other));
        assert!(binding(shell.hwnd).is_none());
        assert!(binding(tools.hwnd as Handle).is_none());
        assert!(with_tab(inspected, |_| panic!("Closed inspector reached an App.")).is_err());
        assert!(with_tab(initial, |a| a.complete_platform_request(
            request,
            PlatformCompletion::Paste("late".into())
        ))
        .is_err());
        assert!(!TEST_REENTRY_BUSY.with(|s| s.get()));
        assert!(!has_quit(false));
        assert_eq!(
            edit(|b| Ok((b.session.window_count(), b.session.active(other)?))).unwrap(),
            (1, Some(tab))
        );
        assert!(!native_matches(main));
        assert!(native_matches(peer_route));
    });
}

#[test]
fn hidden_native_translated_text_is_purged_before_tab_replacement() {
    ui_thread(|| unsafe {
        let shell = Shell::new(App::new());
        edit(|b| b.ui.frame(&b.session, shell.initial))
            .expect("A newly created native window must have a UI binding before input.");
        let first = shell.origin();
        let key = Message {
            window: shell.hwnd,
            message: 0x0100,
            wparam: 0x41,
            lparam: 0x001e0001,
            time: 17,
            point: Point { x: 0, y: 0 },
            private: 0,
        };
        dispatch_native(&key);
        let mut character: Message = zeroed();
        assert_ne!(
            PeekMessageW(&mut character, shell.hwnd, 0x0102, 0x0102, 0),
            0,
            "TranslateMessage did not post text for this private HWND."
        );
        assert_eq!(
            INPUT.with(|r| r.borrow().text(shell.hwnd as usize, character.lparam)),
            Some(first)
        );
        let next = mutate(|b| b.session.new_tab(shell.initial)).unwrap();
        assert_ne!(next, first.tab);
        assert_eq!(
            PeekMessageW(&mut character, shell.hwnd, 0x0102, 0x0102, 0),
            0,
            "Old translated text survived the logical tab switch."
        );
        assert!(INPUT
            .with(|r| r.borrow().text(shell.hwnd as usize, 0x001e0001))
            .is_none());
        dispatch_native(&key);
        assert_ne!(
            PeekMessageW(&mut character, shell.hwnd, 0x0102, 0x0102, 1),
            0
        );
        let origin = INPUT
            .with(|r| r.borrow().text(shell.hwnd as usize, character.lparam))
            .unwrap();
        assert_eq!(origin.tab, next);
        dispatch_native(&character);
        assert!(!TEST_REENTRY_BUSY.with(|s| s.get()));
        assert!(!has_quit(false));
    });
}

struct Fixture {
    root: PathBuf,
    file: PathBuf,
    permissions: Option<std::fs::Permissions>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "chezcat-hidden-native-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        Self {
            file: root.join("history.txt"),
            root,
            permissions: None,
        }
    }
    fn readonly(&mut self) {
        let original = fs::metadata(&self.file).unwrap().permissions();
        let mut changed = original.clone();
        changed.set_readonly(true);
        fs::set_permissions(&self.file, changed).unwrap();
        self.permissions = Some(original);
    }
    fn writable(&mut self) {
        if let Some(p) = self.permissions.take() {
            fs::set_permissions(&self.file, p).unwrap();
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.writable();
        let _ = fs::remove_dir_all(&self.root);
    }
}
#[test]
fn hidden_native_final_history_failure_keeps_retry_host_and_posts_quit_only_after_success() {
    ui_thread(|| unsafe {
        let mut fixture = Fixture::new();
        let page = fixture.root.join("page.html");
        fs::write(&page, "<title>Native lifetime</title><p>offline</p>").unwrap();
        let mut store = crate::history::Store::open(fixture.file.clone()).unwrap();
        store
            .configure(
                crate::history::Policy {
                    enabled: true,
                    retention_days: None,
                    clear_on_exit: true,
                },
                now(),
            )
            .unwrap();
        let mut app = App::new();
        app.attach_browsing_history(store);
        app.open(&page);
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        while app.is_loading() {
            assert!(Instant::now() < deadline, "Offline fixture did not commit.");
            app.poll_load();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert!(
            app.title().contains("Native lifetime"),
            "The committed App document was not the offline fixture."
        );
        let shell = Shell::new(app);
        assert_eq!(
            crate::history::Store::open(fixture.file.clone())
                .unwrap()
                .data()
                .entries
                .len(),
            1
        );
        let before = fs::read(&fixture.file).unwrap();
        fixture.readonly();
        TEST_HISTORY_CHOICE.with(|s| s.set(Some(2))); // test-owned Cancel; no modal UI.
        SendMessageW(shell.hwnd, 0x0010, 0, 0);
        assert_eq!(IsWindow(shell.hwnd), 1);
        assert!(binding(shell.hwnd).is_none());
        assert!(!has_quit(false));
        assert!(edit(|b| b.session.new_window(1, 1)).is_err());
        assert_eq!(fs::read(&fixture.file).unwrap(), before);
        assert!(edit(|b| Ok(b.finalizing && b.final_error.is_some())).unwrap());
        assert!(!TEST_REENTRY_BUSY.with(|s| s.get()));
        fixture.writable();
        SendMessageW(shell.hwnd, 0x0010, 0, 0);
        assert_eq!(IsWindow(shell.hwnd), 0);
        assert!(has_quit(true));
        assert!(crate::history::Store::open(fixture.file.clone())
            .unwrap()
            .data()
            .entries
            .is_empty());
        let after = fs::read(&fixture.file).unwrap();
        shutdown_attempt().unwrap();
        assert_eq!(fs::read(&fixture.file).unwrap(), after);
        assert!(!TEST_REENTRY_BUSY.with(|s| s.get()));
    });
}
