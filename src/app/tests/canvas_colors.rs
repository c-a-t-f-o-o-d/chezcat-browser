use super::*;

const BACKDROP: u32 = 0x204080;
const SOLID: u32 = 0xaa33cc;
const BLENDED: u32 = 0x704040;
const NEIGHBOR: u32 = 0x0d6efd;

const PAGE: &str = r#"<!doctype html><html><head><title>Starting</title><style>
html,body{margin:0;padding:0;background:#204080}
canvas,.sample{position:absolute;top:20px}
#sheet{left:20px}#neighbor{left:40px}
.sample{width:1px;height:1px}
#solid{left:60px;background:#aa33cc}#blended{left:65px;background:#704040}#blue{left:70px;background:#0d6efd}
button{position:absolute;left:20px;top:48px;width:120px;height:32px}
</style></head><body><canvas id=sheet width=3 height=1></canvas><canvas id=neighbor width=1 height=1></canvas>
<div id=solid class=sample></div><div id=blended class=sample></div><div id=blue class=sample></div>
<button type=button id=advance>Paint samples</button><script>
var sheet=document.getElementById('sheet'),saved=sheet.getContext('2d');
var other=document.getElementById('neighbor').getContext('2d'),stage=0;
document.getElementById('advance').onclick=function(event){
  if(!event.isTrusted)throw 'expected a trusted button click';
  if(stage===0){
    if(saved.fillStyle!=='#000000'||other.fillStyle!=='#000000')throw 'initial solid fill';
    saved.fillStyle='#A3c';
    if(saved.fillStyle!=='#aa33cc'||other.fillStyle!=='#000000')throw 'canonical shorthand or separate state';
    saved.fillStyle='unknown sample color';
    if(saved.fillStyle!=='#aa33cc')throw 'invalid color replaced the prior setting';
    saved.fillRect(0,0,1,1);
    other.fillStyle='#0D6EfD';
    if(other.fillStyle!=='#0d6efd')throw 'canonical hex';
    other.fillRect(0,0,1,1);
    saved.fillStyle='rgba(192, 64, 0, 0.5)';
    saved.fillRect(1,0,1,1);
    document.title='Painted';
  }else{
    sheet.width=sheet.width;
    if(saved!==sheet.getContext('2d')||saved.fillStyle!=='#000000'||other.fillStyle!=='#0d6efd')
      throw 'same-width reset or neighboring state';
    saved.fillRect(2,0,1,1);
    document.title='Reset';
  }
  stage++;
};
document.title='Ready';
</script></body></html>"#;

fn node(app: &App, id: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&n| app.document.attr(n, "id") == Some(id))
        .unwrap()
}

fn pixels(app: &App, title: &str, sheet: &[u32], neighbor: u32) {
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{title}: {}", page.report());
    assert_eq!(app.document.title, title);
    let frame = app.draw();
    for (id, left, expected) in [
        ("solid", 60, &[SOLID][..]),
        ("blended", 65, &[BLENDED][..]),
        ("blue", 70, &[NEIGHBOR][..]),
        ("sheet", 20, sheet),
        ("neighbor", 40, std::slice::from_ref(&neighbor)),
    ] {
        let rect = app.page.border_rects[node(app, id)].unwrap();
        assert_eq!(
            (rect.x, rect.y, rect.width, rect.height),
            (left, 20, expected.len() as i32, 1)
        );
        let start =
            (app.content_top() + rect.y - app.scroll) as usize * frame.width + rect.x as usize;
        assert_eq!(
            &frame.pixels[start..start + expected.len()],
            expected,
            "{title}: {id}"
        );
    }
}

#[test]
fn canvas_color_app_fill_style_pixels_survive_invalid_color_and_reset_independently() {
    let fixture = Fixture::new();
    let path = fixture.write("colors.html", PAGE);
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    pixels(&app, "Ready", &[BACKDROP; 3], BACKDROP);
    let button = node(&app, "advance");
    for (title, expected) in [
        ("Painted", [SOLID, BLENDED, BACKDROP]),
        ("Reset", [BACKDROP, BACKDROP, 0]),
    ] {
        let control = app.page.controls.iter().find(|r| r.node == button).unwrap();
        let (x, page_y) = (
            control.x + control.width / 2,
            control.y + control.height / 2,
        );
        assert_eq!(app.page.control_at(x, page_y), Some(button));
        let y = app.content_top() + page_y - app.scroll;
        assert!(matches!(
            app.pointer_down(x, y, false),
            PlatformAction::None
        ));
        assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
        pixels(&app, title, &expected, NEIGHBOR);
    }
}
