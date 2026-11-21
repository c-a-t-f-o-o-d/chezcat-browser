//! Real pointer hover uses the same clipped/topmost regions as native activation.
use super::*;
fn eval(app: &mut App, source: &str) {
    let p = app.scripting.as_mut().unwrap();
    let out = p.console_eval(&mut app.document, &mut app.forms, source);
    assert_eq!(out.errors, 0, "{}", p.report());
}
#[test]
fn native_hover_actual_app_transparent_cover_ancestors_clips_editor_and_leave() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-native-hover-app-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r#"<html><head><style>html,body{margin:0;background:#fff}#wrap{position:absolute;left:20px;top:110px;width:240px;height:80px;color:#e03040}#wrap:hover{color:#20a040}#under,#cover{position:absolute;left:10px;top:10px;width:100px;height:40px}#under{display:block;background:#2040e0}#cover:hover{background:#20a040}#empty{position:absolute;left:140px;top:10px;width:60px;height:40px}#empty:hover{background:#e03040}#clip{position:absolute;left:20px;top:220px;width:60px;height:40px;overflow:hidden}#inside{position:absolute;left:45px;top:0;display:block;width:50px;height:30px;background:#2040e0}#neighbor{position:absolute;left:300px;top:120px;width:140px;height:32px}</style></head><body><div id=wrap><a id=under href=#destination></a><div id=cover></div><div id=empty></div></div><div id=clip><a id=inside href=#destination></a></div><input id=neighbor value=seed><div id=destination></div></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 440);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    eval(&mut app,"var wrap=document.getElementById('wrap'),under=document.getElementById('under'),cover=document.getElementById('cover'),empty=document.getElementById('empty'),inside=document.getElementById('inside');");
    let neighbor = (0..app.document.nodes.len())
        .find(|&n| app.document.attr(n, "id") == Some("neighbor"))
        .unwrap();
    let r = app
        .page
        .controls
        .iter()
        .find(|r| r.node == neighbor)
        .unwrap();
    let (x, y) = (r.x + 12, app.content_top() + r.y + 12 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("edited native");
    let v = &app.forms.controls[&neighbor].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    let y = app.content_top() + 135 - app.scroll;
    assert!(app.pointer_move(40, y, false));
    assert_eq!(app.page.link_node_at(40, 135), None);
    eval(&mut app,"if(!cover.matches(':hover')||!wrap.matches(':hover')||under.matches(':hover')||getComputedStyle(wrap).color!=='rgb(32, 160, 64)')throw 'paint-owned covered hover';under.dispatchEvent(new Event('mouseover',{bubbles:true}));if(!cover.matches(':hover')||under.matches(':hover'))throw 'synthetic hover changed native pointer';");
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 135 - app.scroll) as usize * canvas.width + 40]
            & 0xffffff,
        0x20a040
    );
    let y = app.content_top() + 135 - app.scroll;
    app.pointer_move(175, y, false);
    eval(&mut app,"if(!empty.matches(':hover')||cover.matches(':hover')||!wrap.matches(':hover'))throw 'transparent ordinary box hover';");
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 135 - app.scroll) as usize * canvas.width + 175]
            & 0xffffff,
        0xe03040
    );
    let y = app.content_top() + 230 - app.scroll;
    app.pointer_move(70, y, false);
    eval(
        &mut app,
        "if(!inside.matches(':hover'))throw 'clipped visible target';",
    );
    app.pointer_move(95, y, false);
    eval(
        &mut app,
        "if(inside.matches(':hover'))throw 'hover escaped overflow clip';",
    );
    assert_eq!(app.page.link_node_at(95, 230), None);
    assert!(app.pointer_leave());
    eval(&mut app,"if(cover.matches(':hover')||empty.matches(':hover')||wrap.matches(':hover')||inside.matches(':hover'))throw 'native window leave';");
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
}

#[test]
fn native_hover_without_hover_rules_tracks_queries_without_invalidating_native_geometry() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-native-hover-plain-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,"<html><head><style>html,body{margin:0}p{height:40px}</style></head><body><p id=target>Plain native text</p></body></html>").unwrap();
    let mut app = App::new();
    app.resize(640, 400);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    let node = (0..app.document.nodes.len())
        .find(|&n| app.document.attr(n, "id") == Some("target"))
        .unwrap();
    let r = app.page.border_rects[node].unwrap();
    let scope = std::rc::Rc::clone(&app.anchor_geometry_scope);
    app.pointer_move(r.x + 3, app.content_top() + r.y + 3 - app.scroll, false);
    assert!(std::rc::Rc::ptr_eq(&scope, &app.anchor_geometry_scope));
    eval(&mut app,"if(!document.getElementById('target').matches(':hover'))throw 'real hover without CSS consumer';");
}
#[test]
fn native_hover_without_scripting_keeps_captured_text_selection_on_window_leave() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-native-hover-selection-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,"<html><head><style>p{color:#e03040}p:hover{color:#20a040}</style></head><body><p>Selectable native words stay stable.</p></body></html>").unwrap();
    let mut app = App::new();
    app.resize(640, 400);
    app.set_javascript_enabled(false);
    app.open(&file.0);
    assert!(!app.document.scripting_enabled);
    let run = &app.page.runs[0];
    let (x, y, end) = (
        run.x + 2,
        app.content_top() + run.y + run.height() / 2 - app.scroll,
        run.x + run.width().min(60) - 2,
    );
    app.pointer_down(x, y, false);
    app.pointer_move(end, y, true);
    assert_eq!(app.page.runs[0].color, 0x20a040);
    let before = app.selection.map(|s| s.ordered());
    assert!(before.is_some());
    app.pointer_leave();
    assert!(matches!(app.drag, Drag::Text { .. }));
    assert_eq!(app.selection.map(|s| s.ordered()), before);
    assert_eq!(app.page.runs[0].color, 0xe03040);
}

#[test]
fn native_hover_rechecks_a_stationary_pointer_when_native_scroll_changes_the_displayed_target() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-native-hover-scroll-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,"<html><head><style>html,body{margin:0}#target{position:absolute;left:20px;top:80px;width:100px;height:40px}#target:hover{background:#20a040}#tail{position:absolute;top:1000px;height:20px}</style></head><body><div id=target></div><div id=tail></div></body></html>").unwrap();
    let mut app = App::new();
    app.resize(640, 400);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    let y = app.content_top() + 95;
    app.pointer_move(40, y, false);
    eval(&mut app,"var target=document.getElementById('target');if(!target.matches(':hover'))throw 'initial native hover';");
    app.scroll_by(200);
    eval(
        &mut app,
        "if(target.matches(':hover'))throw 'scroll left stale hover';",
    );
    app.scroll_by(-200);
    eval(
        &mut app,
        "if(!target.matches(':hover'))throw 'scroll did not restore actual hover';",
    );
}
