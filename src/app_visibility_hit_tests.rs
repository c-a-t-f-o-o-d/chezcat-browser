//! Retained geometry, actual paint and native input eligibility are separate.
use super::*;
use std::time::{Duration,Instant};
fn eval(app: &mut App, source: &str) {
    let p = app.scripting.as_mut().unwrap();
    let out = p.console_eval(&mut app.document, &mut app.forms, source);
    assert_eq!(out.errors, 0, "{}", p.report());
    if out.layout_changed {
        app.reflow();
        app.sync_editors();
    }
}
#[test]
fn visibility_hit_actual_app_hidden_boxes_keep_geometry_and_visible_descendants_work() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-visibility-hit-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r#"<html><head><style>html,body{margin:0;background:#fff}#under,#panel{position:absolute;left:20px;top:110px;width:240px;height:80px}#under{display:block;background:#2040e0}#panel{--state:hidden;visibility:var(--state);background:#e03040}#child{position:absolute;left:140px;top:10px;display:block;width:60px;height:40px;visibility:visible;background:#20a040}#hiddenInput{position:absolute;left:10px;top:10px;width:100px;height:30px}#shownInput{position:absolute;left:10px;top:45px;width:100px;height:30px;visibility:visible}#neighbor{position:absolute;left:300px;top:120px;width:140px;height:32px}</style></head><body><a id=under href=#under-destination></a><div id=panel><input id=hiddenInput value=kept><input id=shownInput value=visible><a id=child href=#child-destination></a></div><input id=neighbor value=seed><div id=under-destination></div><div id=child-destination></div></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 440);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let panel = id(&app, "panel");
    let hidden = id(&app, "hiddenInput");
    let shown = id(&app, "shownInput");
    let under = id(&app, "under");
    let child = id(&app, "child");
    assert_eq!(app.page.border_rects[panel].unwrap().width, 240);
    assert_eq!(app.page.link_node_at(135, 130), Some(under));
    assert_eq!(app.page.link_node_at(175, 135), Some(child));
    assert!(!app.page.controls.iter().any(|r| r.node == hidden));
    assert!(app.page.controls.iter().any(|r| r.node == shown));
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 130 - app.scroll) as usize * canvas.width + 135]
            & 0xffffff,
        0x2040e0
    );
    assert_eq!(
        canvas.pixels[(app.content_top() + 135 - app.scroll) as usize * canvas.width + 175]
            & 0xffffff,
        0x20a040
    );
    eval(&mut app,"var panel=document.getElementById('panel'),hiddenInput=document.getElementById('hiddenInput'),child=document.getElementById('child');if(getComputedStyle(panel).visibility!=='hidden'||getComputedStyle(child).visibility!=='visible'||panel.getBoundingClientRect().width!==240)throw 'actual visibility and retained box';panel.style.visibility='visible';if(getComputedStyle(hiddenInput).visibility!=='visible')throw 'live inherited visibility';");
    assert!(app.page.controls.iter().any(|r| r.node == hidden));
    app.focus_control(hidden);
    assert!(app.focus == Focus::Control(hidden));
    eval(&mut app,"panel.style.visibility='hidden';if(getComputedStyle(hiddenInput).visibility!=='hidden'||panel.getBoundingClientRect().width!==240)throw 'hidden computed box';");
    assert!(app.focus == Focus::Page);
    assert!(!app.page.controls.iter().any(|r| r.node == hidden));
    assert_eq!(app.forms.controls[&hidden].value.text(), "kept");
    assert!(app.page.controls.iter().any(|r| r.node == shown));
}
#[test]
fn visibility_hit_actual_app_pointer_none_paints_but_native_hits_and_keyboard_focus_are_genuine() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(
        std::env::temp_dir().join(format!("chezcat-pointer-hit-{}.html", std::process::id())),
    );
    std::fs::write(&file.0,r#"<html><head><style>html,body{margin:0}#under,#cover{position:absolute;left:20px;top:110px;width:240px;height:80px}#under{display:block;background:#2040e0}#cover{--pointer:none;pointer-events:var(--pointer);background:#e03040}#child{position:absolute;left:140px;top:10px;display:block;width:60px;height:40px;pointer-events:auto;background:#20a040}#field{position:absolute;left:10px;top:10px;width:100px;height:30px}</style></head><body><a id=under href=#destination></a><div id=cover><input id=field value=seed><a id=child href=#child-destination></a></div><div id=destination></div><div id=child-destination></div></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 440);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let field = id(&app, "field");
    assert_eq!(app.page.link_node_at(135, 130), Some(id(&app, "under")));
    assert_eq!(app.page.link_node_at(175, 135), Some(id(&app, "child")));
    assert_eq!(app.page.control_at(40, 130), None);
    assert!(app.page.controls.iter().any(|r| r.node == field));
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 130 - app.scroll) as usize * canvas.width + 135]
            & 0xffffff,
        0xe03040
    );
    eval(&mut app,"var cover=document.getElementById('cover'),child=document.getElementById('child');if(getComputedStyle(cover).pointerEvents!=='none'||getComputedStyle(child).getPropertyValue('pointer-events')!=='auto')throw 'real inherited hit readback';cover.style.pointerEvents='auto';");
    assert_eq!(app.page.link_node_at(135, 130), None);
    eval(&mut app, "cover.style.pointerEvents='none';");
    app.tab_focus(false);
    assert!(app.focus == Focus::Control(field));
    app.paste("native");
    assert!(app.forms.controls[&field].value.text().contains("native"));
    let y = app.content_top() + 130 - app.scroll;
    app.pointer_down(135, y, false);
    app.pointer_up(135, y);
    assert_eq!(app.location.fragment, "destination");
}

#[test]
fn visibility_hit_hidden_background_really_loads_and_reveal_uses_the_decoded_pixels() {
    struct Fixture(Vec<PathBuf>);
    impl Drop for Fixture {
        fn drop(&mut self) {
            for p in &self.0 {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    let dir = std::env::temp_dir();
    let name = format!("chezcat-visibility-resource-{}", std::process::id());
    let html = dir.join(format!("{name}.html"));
    let ppm = dir.join(format!("{name}.ppm"));
    let _files = Fixture(vec![html.clone(), ppm.clone()]);
    std::fs::write(&ppm, b"P6\n1 1\n255\n\x20\xa0\x40").unwrap();
    std::fs::write(&html,format!("<html><head><style>html,body{{margin:0;background:#fff}}#image{{position:absolute;left:20px;top:110px;width:80px;height:50px;visibility:hidden;background-image:url({name}.ppm);background-size:cover}}</style></head><body><div id=image></div></body></html>")).unwrap();
    let mut app = App::new();
    app.resize(640, 400);
    app.set_javascript_enabled(true);
    app.set_images_enabled(true);
    app.open(&html);
    let node = (0..app.document.nodes.len())
        .find(|&n| app.document.attr(n, "id") == Some("image"))
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while !app.backgrounds.contains_key(&node) {
        assert!(Instant::now() < deadline, "{:?}", app.events);
        app.poll_load();
        std::thread::sleep(Duration::from_millis(2));
    }
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 125 - app.scroll) as usize * canvas.width + 35]
            & 0xffffff,
        0xffffff
    );
    assert_eq!(app.page.border_rects[node].unwrap().width, 80);
    eval(
        &mut app,
        "document.getElementById('image').style.visibility='visible';",
    );
    assert!(app.backgrounds.contains_key(&node));
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 125 - app.scroll) as usize * canvas.width + 35]
            & 0xffffff,
        0x20a040
    );
}
#[test]
fn visibility_hit_visible_override_keeps_hidden_parent_opacity_without_painting_parent() {
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-visibility-opacity-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,"<html><head><style>html,body{margin:0;background:#fff}.parent{position:absolute;top:110px;width:100px;height:60px;opacity:.5}#hidden{left:20px;visibility:hidden;background:#e03040}#literal{left:180px}.child{position:absolute;left:10px;top:10px;width:40px;height:30px;background:#20a040;visibility:visible}</style></head><body><div id=hidden class=parent><div class=child></div></div><div id=literal class=parent><div class=child></div></div></body></html>").unwrap();
    let mut app = App::new();
    app.resize(640, 400);
    app.open(&file.0);
    let canvas = app.draw();
    let sample = |x: i32| {
        canvas.pixels[(app.content_top() + 135 - app.scroll) as usize * canvas.width + x as usize]
            & 0xffffff
    };
    assert_eq!(sample(45), sample(205));
    assert_ne!(sample(45), 0xffffff);
    assert_ne!(sample(45), 0x20a040);
    assert_eq!(sample(95), 0xffffff);
    assert!(!app.page.opacity_draw_refused.get());
}
