//! Actual App inline SVG CSS geometry and used viewport reductions.
use super::*;
#[test]
fn svg_geometry_actual_app_external_root_sizing_preserves_viewbox_aspect_and_repaints_after_resize()
{
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let file = Fixture(std::env::temp_dir().join(format!(
        "chezcat-svg-css-viewport-{}.html",
        std::process::id()
    )));
    std::fs::write(&file.0,r##"<html><head><style>html,body{margin:0;background:#fff}#icon{position:absolute;left:10px;top:10px;width:80px;height:20px}#neighbor{position:absolute;left:10px;top:70px;width:160px;height:30px}#go{position:absolute;display:block;left:200px;top:10px;width:100px;height:30px;background:#2040e0}@media(max-width:400px){#icon{width:auto;height:auto}}</style></head><body><input id=neighbor value=seed><a id=go href=#destination></a><div id=destination></div><script>
var ns='http://www.w3.org/2000/svg',icon=document.createElementNS(ns,'svg'),rect=document.createElementNS(ns,'rect');icon.setAttribute('id','icon');icon.setAttribute('viewBox','0 0 40 20');rect.setAttribute('width','40');rect.setAttribute('height','20');rect.setAttribute('fill','#20a040');icon.appendChild(rect);document.body.appendChild(icon);
</script></body></html>"##).unwrap();
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
    let neighbor = id(&app, "neighbor");
    let go = id(&app, "go");
    assert_eq!(app.page.border_rects[icon].unwrap().width, 80);
    // A rendered viewport must never become the next intrinsic layout size.
    app.resize(360,260);assert_eq!(app.page.border_rects[icon].unwrap().width,40);assert_eq!(app.page.border_rects[icon].unwrap().height,20);
    app.resize(480,260);assert_eq!(app.page.border_rects[icon].unwrap().width,80);

    let canvas = app.draw();
    let y = (app.content_top() + 20 - app.scroll) as usize;
    assert_eq!(
        canvas.pixels[y * canvas.width + 20] & 0xffffff,
        0xffffff,
        "preserved SVG meet letterbox"
    );
    assert_eq!(canvas.pixels[y * canvas.width + 40] & 0xffffff, 0x20a040);
    assert_eq!(canvas.pixels[y * canvas.width + 80] & 0xffffff, 0xffffff);
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
    let outcome = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "icon.style.height='40px';",
    );
    assert_eq!(
        outcome.errors,
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    app.reflow_preserving_drag();
    let canvas = app.draw();
    let y = (app.content_top() + 20 - app.scroll) as usize;
    assert_eq!(canvas.pixels[y * canvas.width + 20] & 0xffffff, 0x20a040);
    assert_eq!(canvas.pixels[y * canvas.width + 80] & 0xffffff, 0x20a040);
    let v = &app.forms.controls[&neighbor].value;
    assert_eq!((v.text(), v.cursor, v.anchor, v.offset), editor);
    assert_eq!(app.page.link_node_at(220, 20), Some(go));
    let y = app.content_top() + 20 - app.scroll;
    app.pointer_down(220, y, false);
    app.pointer_up(220, y);
    assert_eq!(app.location.fragment, "destination");
}
