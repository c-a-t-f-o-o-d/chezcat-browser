//! User-facing inspector checks use local documents, the real parser/layout/
//! runtime, and the same App mouse/keyboard routes as the Windows shell.
use super::*;
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let folder = std::env::temp_dir().join(format!(
            "chezcat-inspector-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&folder).unwrap();
        Self(folder)
    }
    fn write(&self, name: &str, contents: &str) -> PathBuf {
        let file = self.0.join(name);
        std::fs::write(&file, contents).unwrap();
        file
    }
    fn open(&self, contents: &str) -> App {
        let mut app = App::new();
        app.open(&self.write("inspected.html", contents));
        assert!(!app.failed, "{}", app.status);
        assert!(!app.is_loading());
        app
    }
    fn workshop(&self) -> App {
        self.write("workshop.css", "body { margin:28px; background:#f5f2e9; color:#203c4b } #card { width:240px; padding:14px; background:#d7f0e7; color:#235744 } .row { margin:8px }");
        self.write("workshop.js", "var counter = 4; console.log('fixture loaded'); document.getElementById('change').addEventListener('click', function(){ counter += 1; document.getElementById('card').textContent='clicked'; });");
        let rows = (0..80)
            .map(|i| format!("<p class=row>Sample row {i}</p>\n"))
            .collect::<String>();
        self.open(&format!("<!doctype html><html><head><title>Inspector workshop</title><link rel=stylesheet href=workshop.css></head><body><h1>Inspector workshop</h1><p>A live document, its original sources and the real runtime.</p><div id=card data-note='original attribute'>Before console</div><button id=change>Change this document</button>{rows}<script src=workshop.js></script></body></html>"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn node(app: &App, id: &str) -> usize {
    app.document
        .nodes
        .iter()
        .enumerate()
        .find_map(|(node, _)| (app.document.attr(node, "id") == Some(id)).then_some(node))
        .expect("fixture element exists")
}
fn run(app: &mut App, command: &str) {
    app.inspector_tab(Tab::Console);
    app.inspector.focused = true;
    app.inspector.command.select_all();
    app.paste(command);
    app.key(0x0d, false, false, false);
}
fn click(app: &mut App, action: Action) -> PlatformAction {
    let rect = app
        .inspector_buttons()
        .into_iter()
        .find_map(|(item, rect, _)| (item == action).then_some(rect))
        .unwrap();
    let (x, y) = (rect.x + rect.w / 2, rect.y + rect.h / 2);
    app.pointer_down(x, y, false);
    // A repaint during a press must preserve the pending inspector action.
    app.resize(app.width, app.height);
    let _ = app.draw();
    app.pointer_up(x, y)
}
fn report(app: &App) -> String {
    app.scripting.as_ref().unwrap().report()
}
fn shot(app: &App, name: &str, detached: bool) {
    let Some(folder) = std::env::var_os("OWN_BROWSER_INSPECTOR_SHOTS") else {
        return;
    };
    let folder = PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    let mut file = std::io::BufWriter::new(std::fs::File::create(folder.join(name)).unwrap());
    let canvas = if detached {
        app.draw_inspector_window()
    } else {
        app.draw()
    };
    write!(file, "P6\n{} {}\n255\n", canvas.width, canvas.height).unwrap();
    for pixel in canvas.pixels {
        file.write_all(&[(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8])
            .unwrap();
    }
}

#[test]
fn f12_reserves_a_real_page_viewport_without_navigating_or_reexecuting_scripts() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    let location = app.location.display();
    let raw = app.raw.clone();
    let full = app.viewport_height();
    let history = app.history.len();
    app.key(0x7b, false, false, false);
    assert_eq!(app.inspector.mode, Mode::Docked);
    assert_eq!(full - app.viewport_height(), app.inspector_dock_height());
    assert_eq!(app.history.len(), history);
    assert_eq!(app.location.display(), location);
    assert_eq!(app.raw, raw);
    assert_eq!(report(&app).matches("fixture loaded").count(), 1);
    run(&mut app, "document.getElementById('card').setAttribute('data-viewport', document.documentElement.clientHeight);");
    assert_eq!(
        app.document.attr(node(&app, "card"), "data-viewport"),
        Some(app.viewport_height().to_string().as_str())
    );
    app.key(0x49, true, true, false);
    assert_eq!(app.inspector.mode, Mode::Closed);
    assert_eq!(app.viewport_height(), full);
    assert_eq!(report(&app).matches("fixture loaded").count(), 1);
}

#[test]
fn dock_resize_detach_and_redock_keep_the_same_document_and_independent_window_size() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    let full = app.viewport_height();
    app.open_inspector();
    let top = app.content_bottom();
    assert!(app.inspector_resize_cursor(200, top + 1));
    app.pointer_down(200, top + 1, false);
    assert!(app.pointer_move(200, top - 69, true));
    app.pointer_up(200, top - 69);
    assert_eq!(app.inspector_dock_height(), 400);
    let location = app.location.display();
    click(&mut app, Action::Dock);
    assert!(app.inspector_detached());
    assert_eq!(app.viewport_height(), full);
    app.inspector_window_size(960, 560);
    let canvas = app.draw_inspector_window();
    assert_eq!((canvas.width, canvas.height), (960, 560));
    assert_eq!((app.width, app.height), (1080, 760));
    app.inspector_activate(Action::Dock);
    assert_eq!(app.inspector.mode, Mode::Docked);
    assert_eq!(app.viewport_height(), full - 400);
    assert_eq!(app.location.display(), location);
    app.close_inspector();
    assert_eq!(app.viewport_height(), full);
}

#[test]
fn tree_selection_reports_actual_attributes_computed_styles_and_live_client_dimensions() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    let card = node(&app, "card");
    app.inspector_activate(Action::Row(card, false));
    let details = app.inspector_snapshot().details.join("\n");
    assert!(
        details.contains("data-note = original attribute"),
        "{details}"
    );
    assert!(details.contains("color: #235744"), "{details}");
    assert!(details.contains("width: 240px"), "{details}");
    assert!(details.contains("clientWidth:"), "{details}");
    shot(&app, "elements-docked.ppm", false);
    run(&mut app, "document.getElementById('card').style.width='300px'; document.getElementById('card').style.color='red'; document.getElementById('card').textContent='Changed in the live document';");
    app.inspector_tab(Tab::Elements);
    let details = app.inspector_snapshot().details.join("\n");
    assert!(details.contains("width: 300px"), "{details}");
    assert!(details.contains("color: #ff0000"), "{details}");
    let copied = app.inspector_copy();
    assert!(copied.contains("Changed in the live document"));
    assert!(copied.contains("Selected node"));
    assert!(app.raw.contains("Before console"));
    assert!(!app.raw.contains("Changed in the live document"));
}

#[test]
fn source_cycles_loaded_html_scripts_and_css_without_loading_or_running_them_again() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    let before = report(&app);
    app.inspector_tab(Tab::Sources);
    assert!(app.inspector_copy().contains("<!doctype html>"));
    click(&mut app, Action::NextSource);
    assert!(app.inspector_copy().contains("var counter = 4"));
    click(&mut app, Action::NextSource);
    assert!(app.inspector_copy().contains("#card { width:240px"));
    click(&mut app, Action::NextSource);
    assert!(app.inspector_copy().contains("<!doctype html>"));
    assert_eq!(report(&app), before);
    assert!(!app.is_loading());
    shot(&app, "sources-docked.ppm", false);
}

#[test]
fn console_enters_the_live_realm_with_pasted_comments_and_recalls_command_history() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    run(
        &mut app,
        "// Keep this comment on its own line\ncounter += 2; counter",
    );
    assert!(report(&app).contains("=> 6"), "{}", report(&app));
    run(&mut app, "counter += 1; counter");
    assert!(report(&app).contains("=> 7"));
    app.inspector_paste("draft");
    app.key(0x26, false, false, false);
    assert_eq!(app.inspector.command.text(), "counter += 1; counter");
    app.key(0x26, false, false, false);
    assert!(app.inspector.command.text().starts_with("// Keep"));
    app.key(0x28, false, false, false);
    app.key(0x28, false, false, false);
    assert_eq!(app.inspector.command.text(), "draft");
    shot(&app, "console-docked.ppm", false);
    app.detach_inspector();
    app.inspector_window_size(1000, 620);
    shot(&app, "console-detached.ppm", true);
}

#[test]
fn console_clear_removes_log_text_but_preserves_the_realm_and_error_count() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    run(&mut app, "missingInspectorVariable");
    assert!(report(&app).contains("missingInspectorVariable"));
    click(&mut app, Action::ClearConsole);
    let clean = report(&app);
    assert!(!clean.contains("missingInspectorVariable"), "{clean}");
    assert!(clean.contains("1 error"), "{clean}");
    run(&mut app, "counter");
    assert!(report(&app).contains("=> 4"));
}

#[test]
fn console_obeys_disabled_scripting_and_document_csp_instead_of_bypassing_them() {
    let fixture = Fixture::new();
    let mut app = fixture.open("<meta http-equiv='Content-Security-Policy' content=\"script-src 'unsafe-inline'\"><div id=card>guarded</div><script>var counter=5;</script>");
    app.open_inspector();
    run(
        &mut app,
        "document.getElementById('card').setAttribute('data-bypass', 'yes');",
    );
    assert_eq!(app.document.attr(node(&app, "card"), "data-bypass"), None);
    assert!(
        report(&app).contains("Content Security Policy"),
        "{}",
        report(&app)
    );
    app.set_javascript_enabled(false);
    run(
        &mut app,
        "document.getElementById('card').setAttribute('data-bypass', 'yes');",
    );
    assert_eq!(app.document.attr(node(&app, "card"), "data-bypass"), None);
    assert!(app.inspector.notice.contains("JavaScript is disabled"));
}

#[test]
fn inspector_filter_copy_and_wheel_do_not_scroll_or_change_the_page() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    let location = app.location.display();
    let top = app.content_bottom();
    app.wheel_at(50, top + 110, -120, false, false);
    assert!(app.inspector.scroll > 0);
    assert_eq!(app.scroll, 0);
    let inspector_scroll = app.inspector.scroll;
    app.wheel_at(50, app.content_top() + 20, -120, false, false);
    assert!(app.scroll > 0);
    assert_eq!(app.inspector.scroll, inspector_scroll);
    let zoom = app.zoom;
    app.wheel_at(50, top + 110, 120, true, false);
    assert_eq!(app.zoom, zoom);
    assert_eq!(app.inspector.font_scale, 3);
    app.key(0x46, true, false, false);
    app.paste("SAMPLE ROW 42");
    let PlatformAction::Copy(text) = click(&mut app, Action::Copy) else {
        panic!("copy view")
    };
    assert!(text.contains("Sample row 42"), "{text}");
    assert!(!text.contains("Sample row 43"));
    assert_eq!(app.location.display(), location);
}

#[test]
fn navigation_replaces_the_tree_and_sources_and_cancels_pending_node_clicks() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    let card = node(&app, "card");
    app.inspector_activate(Action::Row(card, false));
    app.inspector.pressed = Some(Action::Row(card, true));
    app.open(&fixture.write(
        "other.html",
        "<title>Other</title><h1 id=new>New document</h1>",
    ));
    assert!(app.inspector.selected.is_none());
    assert!(app.inspector.pressed.is_none());
    assert!(app.inspector.scripts.is_empty());
    assert!(!app.inspector_copy().contains("data-note"));
    app.inspector_tab(Tab::Sources);
    assert!(app.inspector_copy().contains("New document"));
    assert!(!app.inspector_copy().contains("workshop.js"));
}

#[test]
fn native_browser_panels_coexist_with_the_dock_without_capturing_inspector_input() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    app.show_settings();
    assert!(app.browser_panel.is_some());
    assert!(!app.inspector.focused);
    click(&mut app, Action::Tab(Tab::Console));
    assert_eq!(app.inspector.tab, Tab::Console);
    assert!(app.inspector.focused);
    app.paste("counter + 3");
    app.key(0x0d, false, false, false);
    assert!(report(&app).contains("=> 7"));
    assert!(app.browser_panel.is_some());
    shot(&app, "settings-with-inspector.ppm", false);
    app.close_inspector();
    app.key(0x7b, false, false, false);
    assert!(
        app.browser_panel.is_some(),
        "F12 must preserve a staged native settings panel"
    );
}

#[test]
fn source_and_network_labels_distinguish_text_from_generated_image_viewer_markup() {
    let fixture = Fixture::new();
    let text = fixture.write(
        "note.txt",
        "<h1>This is original text, not parsed HTML.</h1>",
    );
    let image = fixture.write("picture.ppm", "P3\n1 1\n255\n42 199 188\n");
    let mut app = App::new();
    app.open(&text);
    app.open_inspector();
    app.inspector_tab(Tab::Sources);
    assert!(app
        .inspector_snapshot()
        .label
        .contains("Original document text (text/plain)"));
    assert!(app.inspector_copy().starts_with("<h1>"));
    app.open(&image);
    assert!(app
        .inspector_snapshot()
        .label
        .contains("Generated image viewer (image/"));
    app.inspector_tab(Tab::Network);
    assert!(app.inspector_copy().contains("CONTENT TYPE: image/"));
}

#[test]
fn detached_character_messages_never_leak_into_address_or_page_editors() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    app.detach_inspector();
    app.focus_address();
    let address = app.address.text();
    app.inspector_focus();
    app.inspector_tab(Tab::Console);
    app.inspector_key(0x1b, false, false, false);
    app.inspector_utf16_input(b'x' as u16);
    assert_eq!(app.address.text(), address);
    assert!(app.inspector.command.chars.is_empty());
    app.inspector.pane = Pane::Command;
    app.inspector_utf16_input(0xd83d);
    app.inspector_utf16_input(0xde3a);
    assert_eq!(app.inspector.command.text(), "😺");
    app.main_window_focus();
    assert!(!app.inspector.focused);
}

#[test]
fn side_button_navigation_uses_real_history_and_cancels_a_pending_link_release() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    let one = app.location.display();
    let two = fixture.write(
        "two.html",
        "<title>Two</title><a href='unexpected.html'>pending link</a>",
    );
    fixture.write("unexpected.html", "<title>Unexpected</title>");
    app.open(&two);
    let second = app.location.display();
    app.drag = Drag::Text {
        x: 30,
        y: 100,
        href: Some("unexpected.html".into()),
        moved: false,
    };
    app.mouse_navigation(false);
    assert_eq!(app.location.display(), one);
    app.pointer_up(30, 100);
    assert_eq!(app.location.display(), one);
    app.mouse_navigation(true);
    assert_eq!(app.location.display(), second);
    app.mouse_navigation(true);
    assert_eq!(app.location.display(), second);
    app.open_inspector();
    app.mouse_navigation(false);
    assert_eq!(app.location.display(), one);
    assert_eq!(app.inspector.mode, Mode::Docked);
}

#[test]
fn network_view_reports_the_actual_current_resources_and_filters_without_requests() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    app.inspector_tab(Tab::Network);
    let text = app.inspector_copy();
    assert!(text.contains(&app.location.display()));
    assert!(text.contains("workshop.js"), "{text}");
    assert!(text.contains("workshop.css"), "{text}");
    assert!(text.contains("LAST FAILED NAVIGATION: None"));
    let events = app.events.clone();
    click(&mut app, Action::Refresh);
    assert_eq!(events, app.events);
    assert!(!app.is_loading());
    shot(&app, "network-docked.ppm", false);
}

#[test]
fn source_and_console_limits_keep_large_text_bounded_and_utf8_intact() {
    let mut snapshot = Snapshot::default();
    add_lines(&mut snapshot, &"é".repeat(MAX_VIEW_BYTES), true);
    assert!(snapshot.truncated);
    assert_eq!(snapshot.rows.len(), 1);
    assert!(snapshot.rows[0].text.len() < MAX_VIEW_BYTES);
    assert!(snapshot.rows[0].text.ends_with("..."));
    let mut editor = Editor::default();
    insert_command(&mut editor, &"😺".repeat(MAX_COMMAND));
    assert_eq!(editor.text().len(), MAX_COMMAND);
    editor.select_all();
    insert_command(&mut editor, "// line one\r\n'line two'\0");
    assert_eq!(editor.text(), "// line one\n'line two'");
}

#[test]
fn real_tree_pointer_hits_select_the_node_and_fold_only_the_requested_branch() {
    let fixture = Fixture::new();
    let mut app = fixture.workshop();
    app.open_inspector();
    let card = node(&app, "card");
    let index = app
        .inspector_snapshot()
        .rows
        .iter()
        .position(|row| row.node == Some(card))
        .unwrap();
    app.inspector.scroll = index.min(app.inspector_max_scroll(false));
    let body = app.inspector_panes().0;
    let x = body.x + body.w - 30;
    let y = body.y + (index - app.inspector.scroll) as i32 * app.inspector.line_height() + 7;
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(app.inspector.selected, Some(card));
    assert!(app
        .inspector_snapshot()
        .details
        .join("\n")
        .contains("data-note = original attribute"));

    app.inspector.scroll = 0;
    let count = app.inspector_snapshot().rows.len();
    let x = body.x + 10;
    let y = body.y + 7;
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(app.inspector_snapshot().rows.len(), 1);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    assert_eq!(app.inspector_snapshot().rows.len(), count);
    assert!(!report(&app).contains("JavaScript error"));
}
