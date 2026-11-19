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
    assert_eq!(app.document.title, "Ready");
    check_pixels(&app, 0x204060);
    click_apply(&mut app);
    let script = app.scripting.as_ref().unwrap();
    assert_eq!(script.error_count(), 0, "{}", script.report());
    assert_eq!(app.document.title, "Color applied");
    check_pixels(&app, 0x6080a0);
}

fn check_pixels(app: &App, expected: u32) {
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
}

fn click_apply(app: &mut App) {
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
}

#[test]
fn element_inline_style_property_paints_through_saved_handle_and_restores_on_trusted_clicks() {
    let files = Fixture::new();
    let path = files.write("saved-element-style.html", r#"<!doctype html><html><head><title>Ready</title>
<style>
body{margin:0;background:white}
.tile{position:absolute;top:16px;width:24px;height:24px}
#sample{left:16px}#reference{left:64px;background:#6080a0}
button{position:absolute;left:16px;top:64px;width:112px;height:32px}
</style></head><body><div id=sample class=tile style="background-color:#204060"></div>
<div id=reference class=tile></div><button id=apply type=button>Change color</button>
<script>
var tile=document.getElementById('sample'),native=tile.style,shadow={},stage=0;
document.getElementById('apply').onclick=function(e){
    if(!e.isTrusted||e.currentTarget!==this)throw 'trusted activation';
    if(stage===0){
        Object.defineProperty(tile,'style',{get:function(){return shadow;},configurable:true});
        native.backgroundColor='#6080a0';
        if(tile.style!==shadow||tile.getAttribute('style').indexOf('#6080a0')<0)throw 'real backing declaration';
        stage++;document.title='Saved style painted';
    }else{
        if(!delete tile.style||tile.style!==native)throw 'real native identity after deletion';
        tile.style.backgroundColor='#204060';
        document.title='Restored style painted';
    }
};
</script></body></html>"#);
    let mut app = App::new();
    app.resize(400, 280);
    app.set_javascript_enabled(true);
    app.open(&path);
    app.wait_for_load();
    assert_eq!(app.document.title, "Ready");
    check_pixels(&app, 0x204060);
    for (title, color) in [("Saved style painted", 0x6080a0), ("Restored style painted", 0x204060)] {
        click_apply(&mut app);
        let script = app.scripting.as_ref().unwrap();
        assert_eq!(script.error_count(), 0, "{}", script.report());
        assert_eq!(app.document.title, title);
        check_pixels(&app, color);
    }
}

#[test]
fn opaque_functional_background_paints_authored_and_live_declarations() {
    let files = Fixture::new();
    let path = files.write("background-functions.html", r#"<!doctype html><html><head><title>Ready</title><style>
html,body{margin:0;background:#f4f1ec}
.tile{position:absolute;top:16px;width:8px;height:6px;background-color:#192b3d}
#literal{left:12px;background:#2584d7}#triplet{left:28px;background-color:rgb(37,132,215)}
#live{left:44px}#sheet{left:60px;background-color:rgba(37,132,215,1);background-color:rgba(37,,215,1)}
#short{left:76px;background:rgba(37 132 215 / 100%)}
#clipped{left:92px;background-color:rgba(-13,300,178,1.3)}
#partial{left:108px;background-color:rgba(37,132,215,.5)}
#clear{left:124px;background-color:transparent}
button{position:absolute;left:12px;top:50px;width:116px;height:28px}
</style></head><body>
<div id=literal class=tile></div><div id=triplet class=tile></div><div id=live class=tile></div>
<div id=sheet class=tile></div><div id=short class=tile></div><div id=clipped class=tile></div>
<div id=partial class=tile></div><div id=clear class=tile></div>
<button id=apply type=button>Change color</button><script>
var live=document.getElementById('live'),decl=live.style,phase=0;
document.getElementById('apply').onclick=function(event){
    if(!event.isTrusted)throw 'Expected a trusted click';
    if(phase===0){
        decl.backgroundColor='rgba(37,132,215,1)';
        var raw=live.getAttribute('style');
        if(decl.getPropertyValue('background-color')!=='rgba(37,132,215,1)'||
           raw!=='background-color:rgba(37,132,215,1);')throw 'Native CSS declaration did not retain the value';
        decl.backgroundColor='rgba(37,,215,1)';
        if(live.getAttribute('style')!==raw)throw 'Invalid value replaced the prior declaration';
        document.title='Applied';
    }else{
        decl.removeProperty('background-color');
        if(decl.backgroundColor!=='')throw 'Removed declaration remained visible';
        document.title='Removed';
    }
    phase++;
};
</script></body></html>"#);
    let mut app = App::new();
    app.resize(400, 280);
    app.set_javascript_enabled(true);
    app.open(&path);
    app.wait_for_load();
    let blue = 0x2584d7;
    let base = 0x192b3d;
    for (title, live) in [("Applied", blue), ("Removed", base)] {
        click_apply(&mut app);
        let script = app.scripting.as_ref().unwrap();
        assert_eq!(script.error_count(), 0, "{}", script.report());
        assert_eq!(app.document.title, title);
        let frame = app.draw();
        let top = (app.content_top() + 16 - app.scroll) as usize;
        for (name, x, expected) in [
            ("literal", 12, blue), ("triplet", 28, blue), ("live", 44, live),
            ("sheet", 60, blue), ("short", 76, blue), ("clipped", 92, 0x00ffb2),
            ("partial", 108, base), ("clear", 124, 0xf4f1ec),
        ] {
            for y in top..top + 6 {
                let start = y * frame.width + x;
                assert_eq!(&frame.pixels[start..start + 8], &[expected; 8], "{title}: {name}");
            }
        }
    }
}
