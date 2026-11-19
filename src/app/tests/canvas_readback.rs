//! Original paint/readback acceptance through normal trusted App input.
use super::*;

const BACKDROP: u32 = 0x204080;
const SOLID: u32 = 0x112233;
const BLENDED: u32 = 0x303048;
const PAGE: &str = r#"<!doctype html><html><head><title>Readback ready</title><style>
html,body{margin:0;padding:0;background:#204080;color:white}
#origin{width:32px;height:4px;background:#ff00ff}
canvas,.sample{position:absolute;top:20px}#sheet{left:20px}.sample{width:1px;height:1px}
#solid{left:60px;background:#112233}#blended{left:65px;background:#303048}#untouched{left:70px;background:#204080}
button{position:absolute;left:20px;top:48px;width:160px;height:32px}
input{position:absolute;left:20px;top:100px;width:200px;height:30px}
#status{position:absolute;left:20px;top:150px;width:580px;margin:0}
</style></head><body><div id=origin></div><canvas id=sheet width=3 height=1></canvas>
<div id=solid class=sample></div><div id=blended class=sample></div><div id=untouched class=sample></div>
<button id=advance name=readback type=button>Advance readback</button>
<form id=f><input id=neighbor name=keep value=seed></form><p id=status>Readback ready</p><script>
var sheet=document.getElementById('sheet'),context=sheet.getContext('2d');
var button=document.getElementById('advance'),neighbor=document.getElementById('neighbor');
var stage=0,snapshot,old;
var expected=[0,0,0,0,17,34,51,255,64,32,16,128,0,0,0,0,0,0,0,0];
function check(value,message){if(!value)throw message;}
function exact(image){
    check(Object.getPrototypeOf(image)===ImageData.prototype&&image instanceof ImageData&&
          image.width===5&&image.height===1,'genuine main-realm ImageData');
    check(image.data===image.data&&image.data instanceof Uint8ClampedArray&&
          Object.getPrototypeOf(image.data)===Uint8ClampedArray.prototype&&
          image.data.buffer instanceof ArrayBuffer&&image.data.length===20&&image.data.byteLength===20,
          'genuine stable typed data');
    for(var i=0;i<20;i++)check(image.data[i]===expected[i],'exact row-major RGBA byte '+i);
}
function show(text){document.title=text;document.getElementById('status').textContent=text;}
button.onclick=function(event){
    check(event.isTrusted&&event.target===button&&event.currentTarget===button&&this===button,'trusted native button');
    check(context.canvas===sheet&&context===sheet.getContext('2d'),'actual canvas/context identity');
    check(neighbor===document.getElementById('neighbor')&&neighbor.value==='edited'&&neighbor.defaultValue==='seed'&&
          neighbor.form===document.getElementById('f')&&neighbor.ownerDocument===document,'native neighbor retained');
    if(stage===0){
        context.fillStyle='#112233';context.fillRect(0,0,1,1);
        context.fillStyle='#40201080';context.fillRect(1,0,1,1);
        show('Readback painted');stage=1;
    }else if(stage===1){
        show('Readback attempted');
        snapshot=context.getImageData(-1,0,5,1);
        exact(snapshot);
        snapshot.data[0]=-4;snapshot.data[1]=257;
        snapshot.data[4]=99;snapshot.data[8]=12;snapshot.data[11]=0;
        check(snapshot.data[0]===0&&snapshot.data[1]===255&&snapshot.data[4]===99&&
              snapshot.data[8]===12&&snapshot.data[11]===0,'mutable clamped returned array');
        old=context.getImageData(-1,0,5,1);exact(old);
        check(old!==snapshot&&old.data!==snapshot.data&&old.data.buffer!==snapshot.data.buffer,'fresh independent snapshot/buffer');
        show('Readback independent');stage=2;
    }else{
        context.clearRect(0,0,3,1);
        exact(old);
        check(snapshot.data[1]===255&&snapshot.data[4]===99&&snapshot.data[8]===12&&snapshot.data[11]===0,
              'older mutable snapshot survived clear');
        var fresh=context.getImageData(-1,0,5,1);
        check(fresh!==old&&fresh!==snapshot&&fresh.data!==old.data&&fresh.data!==snapshot.data&&
              fresh.data.buffer!==old.data.buffer&&fresh.data.buffer!==snapshot.data.buffer&&
              fresh.width===5&&fresh.height===1,'fresh read after clear');
        for(var i=0;i<20;i++)check(fresh.data[i]===0,'cleared backing and outside pixels are transparent black');
        check(snapshot.data[1]===255&&snapshot.data[4]===99&&snapshot.data[8]===12&&snapshot.data[11]===0,
              'fresh allocation must not reuse a retained mutable snapshot');
        show('Readback cleared');stage=3;
    }
};
</script></body></html>"#;

fn node(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}

fn click(app: &mut App, node: usize) {
    let control = app.page.controls.iter().find(|control| control.node == node).unwrap();
    let (x, page_y) = (control.x + control.width / 2, control.y + control.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
}

fn pixels(app: &App, title: &str, sheet: &[u32]) {
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{title}: {}", page.report());
    assert_eq!(app.document.title, title);
    let frame = app.draw();
    for (name, left, expected) in [
        ("solid", 60, &[SOLID][..]),
        ("blended", 65, &[BLENDED][..]),
        ("untouched", 70, &[BACKDROP][..]),
        ("sheet", 20, sheet),
    ] {
        let rect = app.page.border_rects[node(app, name)].unwrap();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (left, 20, expected.len() as i32, 1));
        let start = (app.content_top() + rect.y - app.scroll) as usize * frame.width + rect.x as usize;
        assert_eq!(&frame.pixels[start..start + expected.len()], expected, "{title}: {name}");
    }
}

#[test]
fn canvas_readback_app_exact_rgba_snapshots_are_mutable_independent_and_retained() {
    let fixture = Fixture::new();
    let path = fixture.write("readback.html", PAGE);
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    pixels(&app, "Readback ready", &[BACKDROP; 3]);

    let neighbor = node(&app, "neighbor");
    click(&mut app, neighbor);
    assert!(app.focus == Focus::Control(neighbor));
    assert!(matches!(app.key(0x41, true, false, false), PlatformAction::None));
    app.paste("edited");
    let control = &app.forms.controls[&neighbor];
    let kept = (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner);
    assert_eq!(kept.0, "edited");
    assert_eq!(kept.5, Some(node(&app, "f")));
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    let count = app.forms.controls.len();
    let button = node(&app, "advance");
    for (title, expected) in [
        ("Readback painted", [SOLID, BLENDED, BACKDROP]),
        ("Readback independent", [SOLID, BLENDED, BACKDROP]),
        ("Readback cleared", [BACKDROP; 3]),
    ] {
        click(&mut app, button);
        pixels(&app, title, &expected);
        let control = &app.forms.controls[&neighbor];
        assert_eq!((control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner), kept);
        assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "seed");
        assert_eq!(app.forms.controls.len(), count);
        assert_eq!(app.raw, raw);
        assert_eq!(app.location, location);
        assert_eq!((app.history_index, app.history.len()), history);
    }
}
