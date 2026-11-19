use super::*;

const PAGE: &str = r#"<!doctype html><html><head><title>Ready</title><style>
body{margin:0;background:white}
.tile{position:absolute;top:16px;width:24px;height:24px}
#sample{left:16px}#reference{left:64px;background:#6080a0}
button{position:absolute;left:16px;top:64px;width:112px;height:32px}
</style></head><body>
<div id=sample class=tile style="background-color:#204060;color:black"></div>
<div id=reference class=tile></div><button id=apply type=button>Apply color</button>
<script>
document.getElementById('apply').onclick=function(event){
    if(!event.isTrusted)throw 'Expected trusted input';
    var root=document.documentElement.style,property='';
    for(var k in root){if(k==='backgroundColor')property=k;}
    if(property===''||Object.keys(root).length!==0)throw 'Empty inline style enumeration';
    var style=document.getElementById('sample').style;
    if(Object.keys(style).join(',')!=='0,1'||style[0]!=='background-color'||style.item(1)!=='color')
        throw 'Declared property order';
    style[property]='#6080a0';
    if(style.length!==2||style[0]!=='background-color')throw 'Live declaration changed position';
    document.title='Color applied';
};
</script></body></html>"#;

#[test]
fn inline_style_enumeration_app_uses_inherited_css_name_to_paint_on_trusted_input() {
    let files = Fixture::new();
    let path = files.write("style-properties.html", PAGE);
    let mut app = App::new();
    app.resize(400, 280);
    app.set_javascript_enabled(true);
    app.open(&path);
    let check_pixels = |app: &App, expected| {
        let frame = app.draw();
        for (x, color) in [(16, expected), (64, 0x6080a0)] {
            let top = (app.content_top() + 16 - app.scroll) as usize;
            for y in top..top + 24 {
                let start = y * frame.width + x;
                assert!(frame.pixels[start..start + 24]
                    .iter()
                    .all(|&pixel| pixel == color));
            }
        }
    };
    assert_eq!(app.document.title, "Ready");
    check_pixels(&app, 0x204060);
    let button = app
        .page
        .controls
        .iter()
        .find(|item| app.document.attr(item.node, "id") == Some("apply"))
        .unwrap();
    let (x, page_y) = (button.x + button.width / 2, button.y + button.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(button.node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    let script = app.scripting.as_ref().unwrap();
    assert_eq!(script.error_count(), 0, "{}", script.report());
    assert_eq!(app.document.title, "Color applied");
    check_pixels(&app, 0x6080a0);
}
