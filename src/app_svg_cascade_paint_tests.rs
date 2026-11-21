//! Original App external SVG geometry/variable and repaint reduction.
use super::*;
#[test]
fn svg_shared_cascade_actual_app_external_variables_media_refusal_recovery_keep_editor_and_link() {
    struct Fixture(PathBuf, PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(&self.1);
        }
    }
    let stem = format!("chezcat-svg-shared-{}", std::process::id());
    let dir = std::env::temp_dir();
    let file = Fixture(
        dir.join(format!("{stem}.html")),
        dir.join(format!("{stem}.css")),
    );
    std::fs::write(&file.1,"body{--shape-width:10px;--shape-x:10px}#icon>.shape{x:var(--shape-x);width:var(--shape-width);height:100%}@media(max-width:400px){#icon>.shape{height:50%}}").unwrap();
    std::fs::write(&file.0,format!(r##"<html><head><link rel=stylesheet href='{stem}.css'><style>html,body{{margin:0;background:#fff}}#icon{{position:absolute;left:10px;top:10px;width:40px;height:20px}}#neighbor{{position:absolute;left:10px;top:70px;width:160px;height:30px}}#go{{position:absolute;display:block;left:200px;top:10px;width:100px;height:30px;background:#2040e0}}</style></head><body><input id=neighbor value=seed><a id=go href=#destination></a><div id=destination></div><script>
var ns='http://www.w3.org/2000/svg',icon=document.createElementNS(ns,'svg'),shape=document.createElementNS(ns,'rect');icon.setAttribute('id','icon');icon.setAttribute('viewBox','0 0 40 20');shape.setAttribute('id','shape');shape.setAttribute('class','shape');shape.setAttribute('width','40');shape.setAttribute('height','20');shape.setAttribute('fill','#20a040');icon.appendChild(shape);document.body.appendChild(icon);
</script></body></html>"##)).unwrap();
    let mut app = App::new();
    app.resize(480, 260);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let icon = id(&app, "icon");
    let shape = id(&app, "shape");
    let neighbor = id(&app, "neighbor");
    let go = id(&app, "go");
    let pixel = |app: &mut App, x: usize, y: i32| {
        let y = (app.content_top() + y - app.scroll) as usize;
        let c = app.draw();
        c.pixels[y * c.width + x] & 0xffffff
    };
    assert_eq!(
        pixel(&mut app, 15, 20),
        0xffffff,
        "external descendant geometry must reach actual paint"
    );
    assert_eq!(pixel(&mut app, 25, 20), 0x20a040);
    assert_eq!(pixel(&mut app, 35, 20), 0xffffff);
    let r = app
        .page
        .controls
        .iter()
        .find(|r| r.node == neighbor)
        .unwrap();
    let (x, y) = (r.x + 10, app.content_top() + r.y + 10 - app.scroll);
    app.pointer_down(x, y, false);
    app.pointer_up(x, y);
    app.paste("edited");
    let v = &app.forms.controls[&neighbor].value;
    let editor = (v.text(), v.cursor, v.anchor, v.offset);
    let eval = |app: &mut App, code: &str| {
        let o =
            app.scripting
                .as_mut()
                .unwrap()
                .console_eval(&mut app.document, &mut app.forms, code);
        assert_eq!(o.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
        app.reflow_preserving_drag();
    };
    eval(
        &mut app,
        "document.body.style.setProperty('--shape-width','20px');",
    );
    assert_eq!(pixel(&mut app, 35, 20), 0x20a040);
    assert_eq!(app.document.attr(shape, "width"), Some("40"));
    assert!(app.document.attr(shape, "style").is_none());
    app.resize(360, 260);
    assert_eq!(pixel(&mut app, 25, 25), 0xffffff);
    assert_eq!(pixel(&mut app, 25, 15), 0x20a040);
    app.resize(480, 260);
    assert_eq!(pixel(&mut app, 25, 25), 0x20a040);
    eval(&mut app, "shape.style.width='calc(2px + 3%)';");
    assert!(
        !app.images.contains_key(&icon),
        "unsupported winner discards old paint"
    );
    assert_ne!(
        pixel(&mut app, 25, 20),
        0x20a040,
        "existing unavailable-image indicator replaces stale paint"
    );
    assert!(
        app.status.contains("SVG computed geometry is unavailable"),
        "{}",
        app.status
    );
    eval(&mut app, "shape.style.width='10px';");
    assert!(!app.status.starts_with("Inline SVG"),"resolved paint must clear its obsolete failure: {}",app.status);
    assert_eq!(pixel(&mut app, 25, 20), 0x20a040);
    assert_eq!(pixel(&mut app, 35, 20), 0xffffff);
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    assert_eq!(app.page.link_node_at(220, 20), Some(go));
    let y = app.content_top() + 20 - app.scroll;
    app.pointer_down(220, y, false);
    app.pointer_up(220, y);
    assert_eq!(app.location.fragment, "destination");
}
