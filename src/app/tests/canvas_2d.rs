use super::*;

const BACKDROP: u32 = 0x367c96;
const SHEET: &str = r#"<!doctype html><html><head><title>Starting</title><style>
html,body{margin:0;padding:0;background:#367c96}
canvas{position:absolute;top:20px}
#sheet{left:20px}#neighbor{left:40px}
button{position:absolute;left:20px;top:50px;width:100px;height:32px}
</style></head><body>
<canvas id=sheet width=4 height=3></canvas><canvas id=neighbor width=4 height=3></canvas>
<button id=advance type=button>Advance</button><script>
var sheet=document.getElementById('sheet'),neighbor=document.getElementById('neighbor');
var saved=sheet.getContext('2d'),other=neighbor.getContext('2d'),stage=0;
function checkOwners(){
  if(saved!==sheet.getContext('2d')||other!==neighbor.getContext('2d')||saved===other||
     saved.canvas!==sheet||other.canvas!==neighbor)throw 'context identity or owner';
}
checkOwners();
document.getElementById('advance').onclick=function(event){
  if(!event.isTrusted)throw 'expected a trusted click';
  if(stage===0){saved.fillRect(1,0,2,2);}
  if(stage===1){saved.clearRect(2,1,1,1);}
  if(stage===2){sheet.width=sheet.width;}
  if(stage===3){saved.fillRect(0,2,1,1);}
  checkOwners();
  stage++;
  document.title='Stage '+stage;
};
document.title='Stage 0';
</script></body></html>"#;

fn node(app: &App, id: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(id))
        .unwrap()
}

fn check_surface(app: &App, stage: usize, painted: &[(i32, i32)]) {
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "stage {stage}: {}", page.report());
    assert_eq!(app.document.title, format!("Stage {stage}"));
    let frame = app.draw();
    for (id, left) in [("sheet", 20), ("neighbor", 40)] {
        let rect = app.page.border_rects[node(app, id)].unwrap();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (left, 20, 4, 3));
        for y in 0..rect.height {
            for x in 0..rect.width {
                let row = (app.content_top() + rect.y + y - app.scroll) as usize;
                let pixel = frame.pixels[row * frame.width + (rect.x + x) as usize];
                let expected = if id == "sheet" && painted.contains(&(x, y)) {
                    0
                } else {
                    BACKDROP
                };
                assert_eq!(pixel, expected, "{id} pixel ({x}, {y}) at stage {stage}");
            }
        }
    }
}

#[test]
fn canvas_2d_app_draw_clear_and_same_width_reset_preserve_saved_context() {
    let fixture = Fixture::new();
    let path = fixture.write("canvas.html", SHEET);
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    check_surface(&app, 0, &[]);

    let stages: [&[(i32, i32)]; 4] = [
        &[(1, 0), (2, 0), (1, 1), (2, 1)],
        &[(1, 0), (2, 0), (1, 1)],
        &[],
        &[(0, 2)],
    ];
    let button = node(&app, "advance");
    for (stage, painted) in stages.iter().enumerate() {
        let rect = app.page.controls.iter().find(|r| r.node == button).unwrap();
        let x = rect.x + rect.width / 2;
        let page_y = rect.y + rect.height / 2;
        assert_eq!(app.page.control_at(x, page_y), Some(button));
        let y = app.content_top() + page_y - app.scroll;
        assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
        assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
        check_surface(&app, stage + 1, painted);
    }
}

#[test]
fn canvas_2d_app_object_fit_none_and_scale_down_paint_natural_pixels() {
    let fixture = Fixture::new();
    let path = fixture.write("canvas-fit.html", r#"<!doctype html><html><head><title>Waiting</title><style>
html,body{margin:0;padding:0;background:#367c96}
.box{position:absolute;top:20px;width:8px;height:7px}
#reference{left:20px}#mark{position:absolute;left:2px;top:2px;width:4px;height:3px;background:black}
#none{left:40px;object-fit:none}#down{left:60px;object-fit:scale-down}
#smaller{left:80px;width:4px;height:4px;object-fit:scale-down}
button{position:absolute;left:20px;top:50px;width:120px;height:32px}
</style></head><body><div id=reference class=box><div id=mark></div></div>
<canvas id=none class=box width=4 height=3></canvas><canvas id=down class=box width=4 height=3></canvas>
<canvas id=smaller class=box width=8 height=4></canvas><button id=draw type=button>Draw</button><script>
document.getElementById('draw').onclick=function(event){
  if(!event.isTrusted)throw 'trusted button required';
  for(var id of ['none','down','smaller']){
    var sheet=document.getElementById(id);
    sheet.getContext('2d').fillRect(0,0,sheet.width,sheet.height);
  }
  document.title='Painted';
};
</script></body></html>"#);
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let button = node(&app, "draw");
    let control = app.page.controls.iter().find(|r| r.node == button).unwrap();
    let (x, page_y) = (control.x + control.width / 2, control.y + control.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(button));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
    assert_eq!(app.document.title, "Painted");

    let frame = app.draw();
    for (id, left, width, height, ink_x, ink_y) in [
        ("reference", 20, 8, 7, 2..6, 2..5),
        ("none", 40, 8, 7, 2..6, 2..5),
        ("down", 60, 8, 7, 2..6, 2..5),
        ("smaller", 80, 4, 4, 0..4, 1..3),
    ] {
        let rect = app.page.border_rects[node(&app, id)].unwrap();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), (left, 20, width, height));
        for y in -1..=height {
            for x in -1..=width {
                let row = (app.content_top() + rect.y + y - app.scroll) as usize;
                let actual = frame.pixels[row * frame.width + (rect.x + x) as usize];
                let expected = if ink_x.contains(&x) && ink_y.contains(&y) { 0 } else { BACKDROP };
                assert_eq!(actual, expected, "{id} pixel ({x}, {y})");
            }
        }
    }
}
