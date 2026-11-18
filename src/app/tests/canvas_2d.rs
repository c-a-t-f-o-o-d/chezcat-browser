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
