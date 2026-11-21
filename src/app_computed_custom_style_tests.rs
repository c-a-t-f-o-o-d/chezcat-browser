//! Original App proof of live computed custom reads through author CSS and
//! trusted native input. No physical UI, clipboard access or framework source.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "chezcat-computed-custom-app-{}-{}.html",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, r#"<!doctype html><html><head><title>Computed pending</title><style>
html,body{margin:0;padding:0;background:#ffffff}
#theme{--Base:seed;--Value:var(--Base);--Case:Upper;--case:lower;--View:narrow;--Pad:4px 6px}
#theme.changed{--Base:changed;--Pad:6px 10px}
@media (min-width:800px){#theme{--View:wide}}
#neighbor{display:block;position:absolute;left:20px;top:20px;width:220px;height:32px;margin:0}
#read{display:block;position:absolute;left:280px;top:20px;width:180px;height:32px;margin:0;padding:0}
#out{position:absolute;left:20px;top:100px;width:500px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
#target{--Echo:var(--Value);position:absolute;left:20px;top:180px;width:80px;height:30px;box-sizing:content-box;padding:var(--Pad);border:0;background:#336699}
#stable-link{display:block;position:absolute;left:20px;top:270px;width:100px;height:24px;background:#cc3300}
</style></head><body><div id=theme><form id=f><input id=neighbor name=neighbor value="neighbor seed"><button id=read name=read type=button>Read live style</button></form><div id=target></div></div><p id=out>Fallback</p><a id=stable-link href=#destination>Go</a><div id=destination></div><script>
var theme=document.getElementById('theme'),target=document.getElementById('target');
var neighbor=document.getElementById('neighbor'),button=document.getElementById('read');
var saved=getComputedStyle(target),readCustom=saved.getPropertyValue,clicks=0,edits=0;
function report(stage){
 var value=readCustom.call(saved,'--Base').trim(),view=saved.getPropertyValue('--View').trim();
 if(saved.getPropertyValue('--Value').trim()!==value||saved.getPropertyValue('--Echo').trim()!==value)throw 'live inherited custom var chain';
 if(saved.getPropertyValue('--Case').trim()!=='Upper'||saved.getPropertyValue('--case').trim()!=='lower'||saved.getPropertyValue('--missing')!=='')throw 'custom name case or missing value';
 var text=stage+'_'+value+'_'+view;
 document.getElementById('out').textContent=text;document.title=text;
}
neighbor.oninput=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this!==neighbor||this.form!==document.getElementById('f'))throw 'trusted owned input required';
 edits++;report('Input'+edits);
};
button.onclick=function(event){
 if(!event.isTrusted||event.currentTarget!==this||this!==button||this.form!==document.getElementById('f'))throw 'trusted owned button required';
 clicks++;theme.className='changed';report('Click'+clicks);
};
report('Ready');
</script></body></html>"#).unwrap();
        Self(path)
    }
    fn open(&self) -> App {
        let mut app = App::new();
        app.resize(640, 480);
        app.set_javascript_enabled(true);
        app.open(&self.0);
        clean(&app);
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
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}
fn clean(app: &App) {
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let scripting = app.scripting.as_ref().unwrap();
    assert_eq!(scripting.error_count(), 0, "{}", scripting.report());
    assert!(!scripting.has_navigation_request());
}
fn click(app: &mut App, name: &str) {
    let node = id(app, name);
    let rect = app
        .page
        .controls
        .iter()
        .find(|rect| rect.node == node)
        .unwrap();
    let (x, page_y) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}
fn editor(app: &App) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &app.forms.controls[&id(app, "neighbor")];
    (
        control.value.text(),
        control.value.cursor,
        control.value.anchor,
        control.value.offset,
        control.dirty_value,
        control.owner,
    )
}
fn stable_controls(app: &App) {
    let neighbor = id(app, "neighbor");
    let input = &app.forms.controls[&neighbor];
    assert_eq!(input.kind, Kind::Text);
    assert_eq!(input.name, "neighbor");
    assert_eq!(input.owner, Some(id(app, "f")));
    assert_eq!(
        app.forms.text_default_value(neighbor).unwrap(),
        "neighbor seed"
    );
    assert_eq!(app.forms.controls.len(), 2);
    assert_eq!(app.forms.controls[&id(app, "read")].kind, Kind::Button);
    assert_eq!(
        app.forms.controls[&id(app, "read")].owner,
        Some(id(app, "f"))
    );
    assert_eq!(app.page.control_at(130, 36), Some(neighbor));
    assert_eq!(app.page.control_at(19, 36), None);
    assert_eq!(app.page.control_at(240, 36), None);
    let link = id(app, "stable-link");
    for (x, y) in [(20, 270), (70, 282), (119, 293)] {
        assert_eq!(app.page.link_node_at(x, y), Some(link));
        assert_eq!(app.page.link_at(x, y), Some("#destination"));
    }
    assert_eq!(app.page.link_node_at(120, 282), None);
}
fn pixel(app: &App, canvas: &Canvas, x: i32, page_y: i32) -> u32 {
    let y = app.content_top() + page_y - app.scroll;
    assert!(x >= 0 && y >= 0 && (x as usize) < canvas.width && (y as usize) < canvas.height);
    canvas.pixels[y as usize * canvas.width + x as usize]
}
fn output_pixels(app: &App, expected: &str) -> Vec<u32> {
    assert_eq!(app.document.title, expected);
    let rect = app.page.border_rects[id(app, "out")].unwrap();
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (20, 100, 500, 40)
    );
    let runs: Vec<_> = app
        .page
        .runs
        .iter()
        .filter(|run| {
            run.x >= rect.x
                && run.x < rect.x + rect.width
                && run.y >= rect.y
                && run.y < rect.y + rect.height
        })
        .collect();
    assert_eq!(
        runs.iter().map(|run| run.text.as_str()).collect::<Vec<_>>(),
        [expected]
    );
    assert!(runs.iter().all(|run| run.color == 0x123456));
    let canvas = app.draw();
    let pixels: Vec<_> = (100..140)
        .flat_map(|y| (20..520).map(move |x| (x, y)))
        .map(|(x, y)| pixel(app, &canvas, x, y))
        .collect();
    assert!(
        pixels.iter().any(|&value| value != 0xffffff),
        "Live custom values must paint actual output glyphs"
    );
    assert_eq!(pixels.last(), Some(&0xffffff));
    pixels
}
fn painted_target(app: &App, width: i32, height: i32) {
    let rect = app.page.border_rects[id(app, "target")].unwrap();
    assert_eq!(
        (rect.x, rect.y, rect.width, rect.height),
        (20, 180, width, height)
    );
    let canvas = app.draw();
    for y in rect.y..rect.y + rect.height {
        for x in rect.x..rect.x + rect.width {
            assert_eq!(
                pixel(app, &canvas, x, y),
                0x336699,
                "Original opaque custom-padding box at ({x},{y})"
            );
        }
    }
    assert_eq!(pixel(app, &canvas, rect.x - 1, rect.y + 2), 0xffffff);
    assert_eq!(
        pixel(app, &canvas, rect.x + rect.width, rect.y + 2),
        0xffffff
    );
}

#[test]
fn computed_custom_style_actual_app_live_sheet_button_input_and_resized_media_preserve_editor() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    stable_controls(&app);
    painted_target(&app, 92, 38);
    let initial = output_pixels(&app, "Ready_seed_narrow");

    click(&mut app, "neighbor");
    assert!(matches!(
        app.key(0x41, true, false, false),
        PlatformAction::None
    ));
    app.paste("neighbor kept");
    clean(&app);
    assert_ne!(output_pixels(&app, "Input1_seed_narrow"), initial);
    assert!(matches!(
        app.key(0x24, false, false, false),
        PlatformAction::None
    ));
    for _ in 0..2 {
        assert!(matches!(
            app.key(0x27, false, false, false),
            PlatformAction::None
        ));
    }
    for _ in 0..3 {
        assert!(matches!(
            app.key(0x27, false, true, false),
            PlatformAction::None
        ));
    }
    let preserved = editor(&app);
    assert_eq!(
        (preserved.0.as_str(), preserved.1, preserved.2),
        ("neighbor kept", 5, 2)
    );
    assert!(preserved.4);

    click(&mut app, "read");
    let narrow = output_pixels(&app, "Click1_changed_narrow");
    painted_target(&app, 100, 42);
    assert_eq!(editor(&app), preserved);
    stable_controls(&app);

    app.resize(960, 480);
    click(&mut app, "read");
    let wide = output_pixels(&app, "Click2_changed_wide");
    assert_ne!(
        wide, narrow,
        "The retained declaration must read the resized media cascade"
    );
    painted_target(&app, 100, 42);
    assert_eq!(editor(&app), preserved);
    stable_controls(&app);

    app.resize(640, 480);
    click(&mut app, "read");
    assert_ne!(output_pixels(&app, "Click3_changed_narrow"), wide);
    painted_target(&app, 100, 42);
    assert_eq!(editor(&app), preserved);
    stable_controls(&app);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}

#[test]
fn touch_action_actual_app_trusted_mutation_keeps_mouse_editor_pixels_and_link_hits() {
    let fixture=Fixture(std::env::temp_dir().join(format!("chezcat-touch-action-app-{}.html",std::process::id())));
    std::fs::write(&fixture.0,r#"<!doctype html><html><head><title>Touch policy ready</title><style>
html,body{margin:0;padding:0;background:#ffffff}#parent{touch-action:none}
#change{display:block;position:absolute;left:20px;top:20px;width:180px;height:32px}
#neighbor{position:absolute;left:240px;top:20px;width:180px;height:32px}
#out{position:absolute;left:20px;top:100px;width:400px;height:40px;color:#123456}
#link{display:block;position:absolute;left:20px;top:180px;width:180px;height:36px;background:#204060}
</style></head><body><div id=parent><button id=change type=button>Change touch policy</button><input id=neighbor value=seed><a id=link href=#destination>Real link</a><div id=out>Original text</div></div><div id=destination></div><script>
var button=document.getElementById('change'),neighbor=document.getElementById('neighbor'),link=document.getElementById('link'),out=document.getElementById('out');
if(getComputedStyle(button).touchAction!=='auto')throw 'touch-action is noninherited';
button.style.touchAction='none';neighbor.style.touchAction='none';link.style.touchAction='none';
button.onclick=function(e){if(!e.isTrusted)throw 'real click required';this.style.touchAction='pan-y pan-x';if(getComputedStyle(this).touchAction!=='pan-x pan-y')throw 'actual updated cascade';out.textContent='Actual touch policy';document.title='Touch policy changed';};
</script></body></html>"#).unwrap();
    let mut app=App::new();app.resize(640,360);app.set_javascript_enabled(true);app.open(&fixture.0);
    assert!(!app.failed&&!app.is_loading(),"{}",app.status);
    let id=|app:&App,name:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(name)).unwrap();
    let neighbor=id(&app,"neighbor");let link=id(&app,"link");let button=id(&app,"change");
    let link_rect=app.page.border_rects[link].unwrap();let before=app.draw();
    let pixel=|app:&App,canvas:&Canvas,x:i32,y:i32|canvas.pixels[(app.content_top()+y-app.scroll)as usize*canvas.width+x as usize];
    assert_eq!(pixel(&app,&before,22,182),0x204060);
    let control=app.page.controls.iter().find(|c|c.node==neighbor).unwrap();let (x,y)=(control.x+12,app.content_top()+control.y+12-app.scroll);
    app.pointer_down(x,y,false);app.pointer_up(x,y);app.key(0x24,false,false,false);app.key(0x23,false,true,false);app.paste("edited native");
    let editor=&app.forms.controls[&neighbor].value;let editor=(editor.cursor,editor.anchor,editor.offset);
    let control=app.page.controls.iter().find(|c|c.node==button).unwrap();let(x,y)=(control.x+12,app.content_top()+control.y+12-app.scroll);
    app.pointer_down(x,y,false);app.pointer_up(x,y);
    assert_eq!(app.document.title,"Touch policy changed");assert_eq!(app.scripting.as_ref().unwrap().error_count(),0,"{}",app.scripting.as_ref().unwrap().report());
    let current=&app.forms.controls[&neighbor].value;assert_eq!(current.text(),"edited native");assert_eq!((current.cursor,current.anchor,current.offset),editor);
    let after_rect=app.page.border_rects[link].unwrap();assert_eq!((after_rect.x,after_rect.y,after_rect.width,after_rect.height),(link_rect.x,link_rect.y,link_rect.width,link_rect.height));
    assert_eq!(app.page.link_node_at(40,195),Some(link));assert_eq!(app.page.link_at(40,195),Some("#destination"));
    let after=app.draw();assert_eq!(pixel(&app,&after,22,182),0x204060);assert!(app.page.runs.iter().any(|text|text.text.contains("Actual")));
    assert!((100..140).any(|y|(20..420).any(|x|pixel(&app,&after,x,y)==0x123456)),"Actual changed response glyphs must paint");
    let y=app.content_top()+195-app.scroll;app.pointer_down(40,y,false);app.pointer_up(40,y);
    assert_eq!(app.location.fragment,"destination");

}
