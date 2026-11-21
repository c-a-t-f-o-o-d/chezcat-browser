//! Actual mixed stamping through native stylesheet, control, image and link paths.
use super::*;
#[test]
fn style_import_actual_app_mixed_batch_paints_and_moves_without_losing_native_state() {
    struct Fixture(PathBuf, PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
            let _ = std::fs::remove_file(&self.1);
        }
    }
    let name = format!("chezcat-style-import-{}", std::process::id());
    let root = std::env::temp_dir();
    let fixture = Fixture(
        root.join(format!("{name}.html")),
        root.join(format!("{name}.ppm")),
    );
    std::fs::write(&fixture.1, b"P6\n1 1\n255\n\x00\xa0\xe0").unwrap();
    let html = format!(
        r##"<!doctype html><html><head><title>Style import ready</title><style>
html,body{{margin:0;background:#ffffff}}#run{{position:absolute;left:20px;top:20px;width:180px;height:32px}}#neighbor{{position:absolute;left:240px;top:20px;width:180px;height:32px}}#toggle{{position:absolute;left:20px;top:65px;width:180px;height:32px}}
#mount{{position:absolute;left:20px;top:115px;width:400px;height:70px}}#report{{position:absolute;left:20px;top:210px;width:400px;height:40px;font-size:24px;color:#000000}}#safe{{display:block;position:absolute;left:20px;top:280px;width:180px;height:36px;background:#204060}}
</style></head><body><button id=run type=button>Stamp mixed data</button><input id=neighbor value=seed><button id=toggle type=button>Move style</button><div id=mount></div><div id=report>Original</div><a id=safe href=#destination>Real link</a><div id=destination></div><script>
var sheet,parked=document.createDocumentFragment(),mount=document.getElementById('mount');document.getElementById('run').onclick=function(e){{if(!e.isTrusted)throw 'native trigger';var t=document.createElement('template');t.innerHTML='<style id="color-sheet" is="custom-style">#report{{color:#123456}}</style><style>#safe{{background:#406020}}#swatch{{position:absolute;left:450px;top:100px;width:16px;height:16px}}</style><original-controls><input id="stamped" value="native seed"><img id="swatch" src="{name}.ppm" width="16" height="16"></original-controls>';var f=document.importNode(t.content,true);sheet=f.querySelector('#color-sheet');var clone=sheet.cloneNode(true);if(clone.textContent!==sheet.textContent||clone.getAttribute('is')!=='custom-style')throw 'real style clone';mount.appendChild(f);document.getElementById('report').textContent='Styled actual stamp';document.title='Mixed style stamped';}};
document.getElementById('toggle').onclick=function(e){{if(!e.isTrusted)throw 'native style move';if(sheet.parentNode===mount)parked.appendChild(sheet);else mount.insertBefore(parked,mount.firstChild);}};
</script></body></html>"##
    );
    std::fs::write(&fixture.0, html).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let id = |app: &App, name: &str| {
        (0..app.document.nodes.len())
            .find(|&n| app.document.attr(n, "id") == Some(name))
            .unwrap()
    };
    let click = |app: &mut App, node: usize| {
        let c = app.page.controls.iter().find(|c| c.node == node).unwrap();
        let (x, y) = (c.x + 12, app.content_top() + c.y + 12 - app.scroll);
        app.pointer_down(x, y, false);
        app.pointer_up(x, y);
    };
    let neighbor = id(&app, "neighbor");
    click(&mut app, neighbor);
    app.paste("neighbor edit");
    let value = &app.forms.controls[&neighbor].value;
    let saved = (value.text(), value.cursor, value.anchor, value.offset);
    let run = id(&app, "run");
    click(&mut app, run);
    assert_eq!(
        app.document.title,
        "Mixed style stamped",
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let swatch = id(&app, "swatch");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !app.images.contains_key(&swatch) && std::time::Instant::now() < deadline {
        app.poll_load();
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(app.images[&swatch].pixels, [0xff00a0e0]);
    let has_color = |app: &mut App, color: u32| {
        let canvas = app.draw();
        (210..250).any(|y| {
            (20..420).any(|x| {
                canvas.pixels
                    [(app.content_top() + y - app.scroll) as usize * canvas.width + x as usize]
                    & 0xffffff
                    == color
            })
        })
    };
    assert!(has_color(&mut app, 0x123456));
    let toggle = id(&app, "toggle");
    click(&mut app, toggle);
    assert!(!has_color(&mut app, 0x123456));
    click(&mut app, toggle);
    assert!(has_color(&mut app, 0x123456));
    let value = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (value.text(), value.cursor, value.anchor, value.offset),
        saved
    );
    assert_eq!(
        app.forms.controls[&id(&app, "stamped")].value.text(),
        "native seed"
    );
    let link = id(&app, "safe");
    assert_eq!(app.page.link_node_at(40, 295), Some(link));
    let canvas = app.draw();
    assert_eq!(
        canvas.pixels[(app.content_top() + 290 - app.scroll) as usize * canvas.width + 30]
            & 0xffffff,
        0x406020
    );
    let y = app.content_top() + 295 - app.scroll;
    app.pointer_down(40, y, false);
    app.pointer_up(40, y);
    assert_eq!(app.location.fragment, "destination");
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
}
