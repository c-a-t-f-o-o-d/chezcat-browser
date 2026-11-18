//! Original ordinary-text line-height geometry and color-paint controls.
use super::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-line-height-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }
    fn open(&self, width: i32, geometry_font: bool) -> App {
        let mut app = App::new();
        if geometry_font {
            app.base_fonts
                .add(
                    "Geometry".into(),
                    400,
                    false,
                    Arc::new(
                        crate::font::Font::parse(&crate::font::tests::fixture_font()).unwrap(),
                    ),
                )
                .unwrap();
            app.fonts = app.base_fonts.clone();
        }
        app.resize(width, 480);
        app.set_javascript_enabled(true);
        app.open(&self.0);
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        assert_eq!(
            app.scripting.as_ref().unwrap().error_count(),
            0,
            "{}",
            app.scripting.as_ref().unwrap().report()
        );
        app
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&n| app.document.attr(n, "id") == Some(name))
        .unwrap()
}
fn rect(app: &App, name: &str) -> crate::layout::BorderRect {
    app.page.border_rects[id(app, name)].unwrap()
}
fn pixel(app: &App, canvas: &Canvas, x: i32, y: i32) -> u32 {
    let y = app.content_top() + y - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn crop(app: &App, canvas: &Canvas, x: i32, y: i32, width: i32, height: i32) -> Vec<u32> {
    (0..height)
        .flat_map(|dy| (0..width).map(move |dx| (dx, dy)))
        .map(|(dx, dy)| pixel(app, canvas, x + dx, y + dy))
        .collect()
}

#[test]
fn line_height_variable_actual_app_original_three_boxes_match_literal_geometry_and_pixels() {
    let fixture = Fixture::new(ORIGINAL);
    for width in [640, 1280] {
        let app = fixture.open(width, false);
        let canvas = app.draw();
        assert_eq!(
            app.document.title,
            "Original variable line-height probe ready"
        );
        for (name, expected) in [
            ("origin", (0, 0, 32, 4)),
            ("variable", (20, 20, 180, 70)),
            ("literal", (220, 20, 180, 70)),
            ("normal", (420, 20, 180, 61)),
        ] {
            let r = rect(&app, name);
            assert_eq!((r.x, r.y, r.width, r.height), expected, "{name} at {width}");
        }
        assert_eq!(
            pixel(&app, &canvas, 0, 0),
            0xff00ff,
            "Actual content origin"
        );
        for (x, color) in [(20, 0x1464dc), (40, 0x204060), (60, 0xc86432)] {
            assert_eq!(pixel(&app, &canvas, x, 110), color);
        }
        for name in ["variable", "literal", "normal"] {
            let r = rect(&app, name);
            assert_eq!(pixel(&app, &canvas, r.x, r.y), 0x204060);
            assert_eq!(
                pixel(&app, &canvas, r.x + r.width / 2, r.y + r.height - 1),
                0x204060
            );
            assert_eq!(
                pixel(&app, &canvas, r.x + r.width - 8, r.y + r.height / 2),
                0x1464dc
            );
            assert_eq!(pixel(&app, &canvas, r.x - 1, r.y + r.height / 2), 0xc86432);
            assert_eq!(
                pixel(&app, &canvas, r.x + r.width / 2, r.y + r.height),
                0xc86432
            );
        }
        let variable = crop(&app, &canvas, 20, 20, 180, 70);
        let literal = crop(&app, &canvas, 220, 20, 180, 70);
        assert!(
            literal.iter().any(|&p| p == 0xffffff),
            "The twin crop includes the original text ink"
        );
        assert_eq!(
            variable, literal,
            "Resolved variable must match the entire actual literal twin"
        );
        assert_ne!(
            variable,
            crop(&app, &canvas, 420, 20, 180, 70),
            "Independent factor1.5 control remains visibly shorter"
        );
    }
}

#[test]
fn line_height_variable_actual_app_inheritance_live_reflow_hits_and_editor_stay_real() {
    let fixture = Fixture::new(
        r#"<!doctype html><html><head><style>
html{font-size:16px}body{margin:0;padding:0;background:#c86432;font-family:Geometry}
.host{position:absolute;top:20px;width:140px;font-size:12px}
.host a{display:block;box-sizing:border-box;width:140px;padding:2px;border:1px solid #204060;background:#1464dc;color:white;font-size:18px}
#factor{left:20px;--Leading:2;line-height:var(--Leading)}#factor.changed{--Leading:3}
#tall{left:180px;line-height:3}#relative{left:340px;--Leading:2em;line-height:var(--Leading)}
#percent{left:20px;top:100px;--Leading:200%;line-height:var(--Leading)}
#pixels{left:180px;top:100px;line-height:24px}
#neighbor{position:absolute;left:340px;top:100px;width:140px;height:32px;font-size:12px}
</style></head><body><div id=factor class=host><a id=variable href=#destination>AV</a></div>
<div id=tall class=host><a id=literal-tall href=#destination>AV</a></div>
<div id=relative class=host><a id=em-child href=#destination>AV</a></div>
<div id=percent class=host><a id=percent-child href=#destination>AV</a></div>
<div id=pixels class=host><a id=pixel-child href=#destination>AV</a></div>
<input id=neighbor value=seed><div id=destination></div></body></html>"#,
    );
    let mut app = fixture.open(640, true);
    for (name, expected) in [
        ("variable", (20, 20, 140, 42)),
        ("literal-tall", (180, 20, 140, 60)),
        ("em-child", (340, 20, 140, 30)),
        ("percent-child", (20, 100, 140, 30)),
        ("pixel-child", (180, 100, 140, 30)),
    ] {
        let r = rect(&app, name);
        assert_eq!((r.x, r.y, r.width, r.height), expected, "{name}");
    }
    let variable = id(&app, "variable");
    let neighbor = id(&app, "neighbor");
    assert_eq!(app.page.link_node_at(90, 61), Some(variable));
    assert_eq!(app.page.link_node_at(90, 62), None);
    let before = app.draw();
    assert_eq!(
        crop(&app, &before, 340, 20, 140, 30),
        crop(&app, &before, 180, 100, 140, 30),
        "em computed at parent12px then inherited by child18px"
    );
    assert_eq!(
        crop(&app, &before, 20, 100, 140, 30),
        crop(&app, &before, 180, 100, 140, 30),
        "Percentage inherits the parent's computed absolute length"
    );
    let outcome = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "document.getElementById('neighbor').value='edited';",
    );
    assert_eq!(
        outcome.errors,
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let editor = app.forms.controls.get_mut(&neighbor).unwrap();
    editor.value.cursor = 2;
    editor.value.anchor = 5;
    editor.value.offset = 1;
    let before_editor = (
        editor.value.text(),
        editor.dirty_value,
        editor.owner,
        (
            editor.value.cursor,
            editor.value.anchor,
            editor.value.offset,
        ),
    );
    let outcome = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "document.getElementById('factor').className='host changed';",
    );
    assert_eq!(
        outcome.errors,
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert!(outcome.layout_changed);
    app.reflow();
    assert_eq!(id(&app, "variable"), variable);
    let r = rect(&app, "variable");
    assert_eq!((r.x, r.y, r.width, r.height), (20, 20, 140, 60));
    assert_eq!(app.page.link_node_at(90, 79), Some(variable));
    assert_eq!(app.page.link_node_at(90, 80), None);
    let after = app.draw();
    assert_eq!(
        crop(&app, &after, 20, 20, 140, 60),
        crop(&app, &after, 180, 20, 140, 60)
    );
    let editor = &app.forms.controls[&neighbor];
    assert_eq!(
        (
            editor.value.text(),
            editor.dirty_value,
            editor.owner,
            (
                editor.value.cursor,
                editor.value.anchor,
                editor.value.offset
            )
        ),
        before_editor
    );
    assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "seed");
    assert_eq!(app.forms.controls.len(), 1);
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}

const ORIGINAL: &str = r#"<!doctype html><html><head><meta charset="utf-8"><title>Original variable line-height reduction</title><style>
html,body{margin:0;padding:0;background:#c86432;color:white;font-family:sans-serif;font-size:18px;line-height:1.5}
#origin{width:32px;height:4px;background:#ff00ff}
.box{display:block;position:absolute;top:20px;box-sizing:border-box;width:180px;margin:0;padding:16px;border:1px solid #204060;background:#1464dc;font-size:18px}
#variable{left:20px;--leading:2;line-height:var(--leading)}
#literal{left:220px;line-height:2}
#normal{left:420px;line-height:1.5}
.tile{position:absolute;top:110px;width:10px;height:10px}
#blue{left:20px;background:#1464dc}#edge{left:40px;background:#204060}#backdrop{left:60px;background:#c86432}
#report{position:absolute;left:20px;top:140px;width:600px;margin:0;white-space:pre;font-size:12px;line-height:14px}
</style></head><body><div id=origin></div><div id=variable class=box>Same text</div><div id=literal class=box>Same text</div><div id=normal class=box>Same text</div>
<div id=blue class=tile></div><div id=edge class=tile></div><div id=backdrop class=tile></div><pre id=report>MEASURE_PENDING</pre>
<script>
var ids=['origin','variable','literal','normal'];var output='';
for(var i=0;i<ids.length;i++){
  var r=document.getElementById(ids[i]).getBoundingClientRect();
  output+='M:'+ids[i]+':'+r.x+','+r.y+','+r.width+','+r.height+'\n';
}
document.getElementById('report').textContent=output;
document.title='Original variable line-height probe ready';
</script></body></html>
"#;
