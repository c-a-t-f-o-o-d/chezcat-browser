//! Trusted specified-style reflection with real pixels and a native editor.
use super::*;

const PAGE: &str = r#"<!doctype html><html><head><title>Original easing ready</title><style>
html,body{margin:0;padding:0;background:white}
#tile,#literal{position:absolute;top:20px;width:24px;height:24px;background:#0d6efd}
#tile{left:20px}#literal{left:64px}
#check{position:absolute;left:20px;top:64px;width:160px;height:30px}
#neighbor{position:absolute;left:20px;top:112px;width:200px;height:30px}
</style></head><body><div id=tile></div><div id=literal></div>
<button id=check name=easing type=button>Reflect easing</button>
<form id=f><input id=neighbor name=keep value=seed></form><script>
document.getElementById('check').onclick=function(event){
  if(!event.isTrusted||event.currentTarget!==this||event.target!==this)throw 'trusted native activation';
  var editor=document.getElementById('neighbor');
  if(editor.value!=='edited easing neighbor'||editor.defaultValue!=='seed'||editor.form!==document.getElementById('f'))throw 'native neighbor before reflection';
  var fresh=document.createElement('div'),s=fresh.style;
  s.animationTimingFunction='';s.animationTimingFunction='linear';
  if(s.animationTimingFunction!=='linear')throw 'real detached validity probe';
  var tile=document.getElementById('tile');
  tile.style.animationTimingFunction='STEPS(4,END), EASE-IN';
  if(tile.style.animationTimingFunction!=='steps(4), ease-in'||tile.style.length!==1||tile.style[0]!=='animation-timing-function')throw 'live canonical easing declaration';
  var raw=tile.getAttribute('style');tile.style.animationTimingFunction='steps(0)';
  if(tile.getAttribute('style')!==raw)throw 'invalid declaration published';
  if(typeof tile.animate!=='undefined')throw 'declaration reflection must not create animation API';
  document.title='Original easing reflected';
};
</script></body></html>"#;

fn node(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}

fn pixels(app: &App) {
    let frame = app.draw();
    for (name, left) in [("tile", 20), ("literal", 64)] {
        let rect = app.page.border_rects[node(app, name)].unwrap();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (left, 20, 24, 24));
        for y in 0..24 {
            let start = (app.content_top() + 20 + y - app.scroll) as usize * frame.width + left as usize;
            assert!(frame.pixels[start..start + 24].iter().all(|&pixel| pixel == 0x0d6efd));
        }
    }
}

#[test]
fn inline_easing_app_trusted_reflection_keeps_pixels_and_edited_native_state() {
    let fixture = Fixture::new();
    let path = fixture.write("easing.html", PAGE);
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    app.wait_for_load();
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0);
    pixels(&app);
    let neighbor = node(&app, "neighbor");
    let control = app.page.controls.iter().find(|control| control.node == neighbor).unwrap();
    let (x, y) = (control.x + 8, app.content_top() + control.y + 8 - app.scroll);
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    app.key(0x41, true, false, false);
    app.paste("edited easing neighbor");
    let control = &app.forms.controls[&neighbor];
    let kept = (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner);
    assert_eq!(kept.0, "edited easing neighbor");
    let button = node(&app, "check");
    let control = app.page.controls.iter().find(|control| control.node == button).unwrap();
    let (x, page_y) = (control.x + control.width / 2, control.y + control.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(button));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", app.scripting.as_ref().unwrap().report());
    assert_eq!(app.document.title, "Original easing reflected");
    pixels(&app);
    let control = &app.forms.controls[&neighbor];
    assert_eq!((control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner), kept);
    assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "seed");
    assert_eq!(app.document.attr(neighbor, "value"), Some("seed"));
    let current = app.location.path.as_ref().expect("local fixture is still open");
    assert_eq!(current.canonicalize().unwrap(), path.canonicalize().unwrap());
}
