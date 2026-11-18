//! The original nonwhite reduction plus real sizing, translated clips and hits.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new(source: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-background-clip-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, source).unwrap();
        Self(path)
    }
    fn open(&self, width: i32) -> App {
        let mut app = App::new();
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
    canvas.pixels[(y + app.content_top() - app.scroll) as usize * canvas.width + x as usize]
}

#[test]
fn background_clip_actual_app_original_nonwhite_padding_reference_at_both_widths() {
    let fixture = Fixture::new(ORIGINAL);
    for width in [640, 1280] {
        let app = fixture.open(width);
        let canvas = app.draw();
        for (name, expected) in [
            ("origin", (0, 0, 32, 4)),
            ("borderbox", (20, 20, 80, 40)),
            ("paddingbox", (140, 20, 80, 40)),
            ("reference", (260, 20, 80, 40)),
            ("inner", (264, 24, 72, 32)),
        ] {
            let r = rect(&app, name);
            assert_eq!((r.x, r.y, r.width, r.height), expected, "{name} at {width}");
        }
        assert_eq!(
            pixel(&app, &canvas, 0, 0),
            0xff00ff,
            "Actual content-origin witness"
        );
        assert_eq!(pixel(&app, &canvas, 10, 20), 0xc86432);
        assert_eq!(pixel(&app, &canvas, 60, 20), pixel(&app, &canvas, 380, 20));
        assert_eq!(pixel(&app, &canvas, 180, 20), pixel(&app, &canvas, 380, 40));
        assert_eq!(pixel(&app, &canvas, 180, 40), pixel(&app, &canvas, 380, 60));
        assert_eq!(pixel(&app, &canvas, 380, 20), 0x50a028);
        assert_eq!(pixel(&app, &canvas, 380, 40), 0xa05028);
        assert_eq!(pixel(&app, &canvas, 380, 60), 0x64c832);
        let mut changed = 0;
        for y in 0..40 {
            for x in 0..80 {
                let actual = pixel(&app, &canvas, 140 + x, 20 + y);
                assert_eq!(
                    actual,
                    pixel(&app, &canvas, 260 + x, 20 + y),
                    "Literal twin at {x},{y}"
                );
                changed += usize::from(actual != pixel(&app, &canvas, 20 + x, 20 + y));
                if (4..76).contains(&x) && (4..36).contains(&y) {
                    assert_eq!(actual, 0x64c832);
                }
            }
        }
        assert_eq!(changed, 896, "Only the true border area changes");
    }
}

#[test]
fn background_clip_actual_app_content_geometry_resize_translation_hits_and_live_cascade() {
    let fixture = Fixture::new(
        r#"<!doctype html><html><head><style>
html,body{margin:0;padding:0;background:#c86432}
:root{--clip:content-box}
#clip{position:absolute;left:20px;top:20px;width:60px;height:50px;overflow:hidden}
#moved{display:block;box-sizing:border-box;width:80px;height:40px;padding:4px;background:#64c832;border:4px solid rgba(0,0,0,.2);border-radius:8px;background-clip:var(--clip);transform:translate(12px,5px)}
#moved.padding{background-clip:padding-box}
#row{position:absolute;left:120px;top:20px;display:flex;align-items:stretch;width:80px;height:60px}
#stretch{box-sizing:border-box;width:30px;padding:4px;background:#64c832;border:2px solid rgba(0,0,0,.2);border-radius:1000px;background-clip:content-box}
#asymmetric{position:absolute;left:220px;top:20px;box-sizing:border-box;width:80px;height:40px;padding:2px 6px 4px 8px;border:4px solid black;background:#64c832;background-clip:content-box}
</style></head><body><div id=clip><a id=moved href=#target></a></div><div id=row><div id=stretch></div></div><div id=asymmetric></div></body></html>"#,
    );
    let mut app = fixture.open(640);
    let check_geometry = |app: &App| {
        let r = rect(app, "moved");
        assert_eq!((r.x, r.y, r.width, r.height), (20, 20, 80, 40));
        let v = app.page.visual_border_rects[id(app, "moved")].unwrap();
        assert_eq!((v.x, v.y), (32.0, 25.0));
        let r = rect(app, "stretch");
        assert_eq!((r.x, r.y, r.width, r.height), (120, 20, 30, 60));
        let r = rect(app, "asymmetric");
        assert_eq!((r.x, r.y, r.width, r.height), (220, 20, 80, 40));
    };
    check_geometry(&app);
    let canvas = app.draw();
    assert_eq!(pixel(&app, &canvas, 72, 25), 0xa05028);
    assert_eq!(
        pixel(&app, &canvas, 72, 31),
        0xc86432,
        "Padding is actual unpainted space"
    );
    assert_eq!(pixel(&app, &canvas, 72, 35), 0x64c832);
    assert_eq!(
        pixel(&app, &canvas, 80, 45),
        0xc86432,
        "Rectangular ancestor clips still apply"
    );
    assert_eq!(
        app.page.link_node_at(72, 31),
        Some(id(&app, "moved")),
        "Color clipping does not shrink the real link box"
    );
    assert_eq!(app.page.link_node_at(80, 45), None);
    assert_eq!(
        pixel(&app, &canvas, 135, 24),
        0xc86432,
        "Final stretched box retains used padding insets"
    );
    assert_eq!(pixel(&app, &canvas, 135, 50), 0x64c832);
    assert_eq!(pixel(&app, &canvas, 231, 40), 0xc86432);
    assert_eq!(pixel(&app, &canvas, 232, 40), 0x64c832);
    assert_eq!(pixel(&app, &canvas, 290, 40), 0xc86432);
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "document.getElementById('moved').className='padding'",
    );
    assert_eq!(
        result.errors,
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    assert!(result.layout_changed);
    app.reflow();
    check_geometry(&app);
    let canvas = app.draw();
    assert_eq!(pixel(&app, &canvas, 72, 31), 0x64c832);
    assert_eq!(pixel(&app, &canvas, 72, 25), 0xa05028);
}

const ORIGINAL: &str = r#"<!doctype html><html><head><meta charset="utf-8"><title>Original background clip reduction</title><style>
html,body{margin:0;padding:0;background:#c86432;color:black;font-size:12px}
#origin{width:32px;height:4px;background:#ff00ff}
.box{position:absolute;top:20px;box-sizing:border-box;width:80px;height:40px;margin:0;padding:0;border:4px solid rgba(0,0,0,.2);border-radius:0}
#borderbox{left:20px;background:#64c832;background-clip:border-box}
#paddingbox{left:140px;background:#64c832;background-clip:padding-box}
#reference{left:260px;background:none}
#inner{height:32px;background:#64c832}
.tile{position:absolute;left:380px;width:10px;height:10px}
#literal-border{top:20px;background:#50a028}#literal-padding{top:40px;background:#a05028}#literal-fill{top:60px;background:#64c832}
#report{position:absolute;left:20px;top:100px;width:580px;margin:0;white-space:pre;font-size:12px;line-height:14px}
</style></head><body><div id=origin></div><div id=borderbox class=box></div><div id=paddingbox class=box></div>
<div id=reference class=box><div id=inner></div></div><div id=literal-border class=tile></div><div id=literal-padding class=tile></div><div id=literal-fill class=tile></div><pre id=report>MEASURE_PENDING</pre>
<script>
var ids=['origin','borderbox','paddingbox','reference','inner'];var output='';
for(var i=0;i<ids.length;i++){
  var r=document.getElementById(ids[i]).getBoundingClientRect();
  output+='M:'+ids[i]+':'+r.x+','+r.y+','+r.width+','+r.height+'\n';
}
document.getElementById('report').textContent=output;
document.title='Original clip probe ready';
</script></body></html>
"#;
