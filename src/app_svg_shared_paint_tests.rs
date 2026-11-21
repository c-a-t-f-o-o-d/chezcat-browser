//! Original App external SVG paint and inherited variable/currentColor reduction.
use super::*;
#[test]
fn svg_shared_paint_actual_app_external_fill_color_variables_and_refusal_recover_with_editor_link()
{
    struct Fixture(PathBuf, PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(&self.1);
        }
    }
    let stem = format!("chezcat-svg-paint-{}", std::process::id());
    let dir = std::env::temp_dir();
    let file = Fixture(
        dir.join(format!("{stem}.html")),
        dir.join(format!("{stem}.css")),
    );
    std::fs::write(&file.1,"body{color:#203040;fill:#506070;--icon-paint:#e02040}#icon>.shape{fill:var(--icon-paint,currentColor);stroke:currentColor}#icon.refused>.shape{fill:url(https://unrequested.invalid/paint)}#icon.recovered>.shape{fill:#b03080!important}").unwrap();
    std::fs::write(&file.0,format!(r##"<html><head><link rel=stylesheet href='{stem}.css'><style>html,body{{margin:0;background:#fff}}#icon{{position:absolute;left:10px;top:10px;width:40px;height:20px}}#neighbor{{position:absolute;left:10px;top:70px;width:160px;height:30px}}#go{{position:absolute;display:block;left:200px;top:10px;width:100px;height:30px;background:#2040e0}}</style></head><body><input id=neighbor value=seed><a id=go href=#destination></a><div id=destination></div><script>
var ns='http://www.w3.org/2000/svg',icon=document.createElementNS(ns,'svg'),shape=document.createElementNS(ns,'rect');icon.setAttribute('id','icon');icon.setAttribute('viewBox','0 0 40 20');shape.setAttribute('class','shape');shape.setAttribute('width','40');shape.setAttribute('height','20');shape.setAttribute('fill','#20a040');shape.setAttribute('stroke-width','2');icon.appendChild(shape);document.body.appendChild(icon);
</script></body></html>"##)).unwrap();
    let mut app = App::new();
    app.resize(480, 300);
    app.set_javascript_enabled(true);
    app.open(&file.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let icon = id(&app, "icon");
    let neighbor = id(&app, "neighbor");
    let go = id(&app, "go");
    let pixel = |app: &mut App| {
        let y = (app.content_top() + 20 - app.scroll) as usize;
        let c = app.draw();
        c.pixels[y * c.width + 30] & 0xffffff
    };
    assert_eq!(
        pixel(&mut app),
        0xe02040,
        "external descendant fill must reach original painter"
    );
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
        let out =
            app.scripting
                .as_mut()
                .unwrap()
                .console_eval(&mut app.document, &mut app.forms, code);
        assert_eq!(
            out.errors,
            0,
            "{}",
            app.scripting.as_ref().unwrap().report()
        );
        app.reflow_preserving_drag();
    };
    eval(
        &mut app,
        "document.body.style.setProperty('--icon-paint','currentColor');",
    );
    assert_eq!(pixel(&mut app), 0x203040);
    eval(&mut app, "document.body.style.color='#1040d0';");
    assert_eq!(pixel(&mut app), 0x1040d0);
    eval(
        &mut app,
        "shape.style.setProperty('fill','var(--missing)');",
    );
    assert_eq!(
        pixel(&mut app),
        0x506070,
        "missing variable computes inherited fill from real HTML owner"
    );
    eval(
        &mut app,
        "shape.style.removeProperty('fill');icon.setAttribute('class','refused');",
    );
    assert!(
        !app.images.contains_key(&icon),
        "external paintserver never reuses stale paint"
    );
    assert_ne!(pixel(&mut app), 0x506070);
    eval(&mut app, "icon.setAttribute('class','recovered');");
    assert_eq!(pixel(&mut app), 0xb03080);
    eval(&mut app, "shape.style.fill='#2040d0';");
    assert_eq!(
        pixel(&mut app),
        0xb03080,
        "external important beats inline normal"
    );
    eval(
        &mut app,
        "shape.style.setProperty('fill','#a04010','important');",
    );
    assert_eq!(pixel(&mut app), 0xa04010, "inline important wins");
    let denied = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "shape.style.fill='url(https://unrequested.invalid/paint)';",
    );
    assert_eq!(denied.errors, 1, "CSSOM URL guard must precede mutation");
    app.reflow_preserving_drag();
    assert_eq!(pixel(&mut app), 0xa04010);

    assert!(!app.status.starts_with("Inline SVG"), "{}", app.status);
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    assert_eq!(app.page.link_node_at(220, 20), Some(go));
    let y = app.content_top() + 20 - app.scroll;
    app.pointer_down(220, y, false);
    app.pointer_up(220, y);
    assert_eq!(app.location.fragment, "destination");
}
