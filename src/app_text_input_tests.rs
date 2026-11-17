//! Original offline App proof of fragment INPUT editing and latest Reset defaults.
//! Software pointer/key delivery uses the native App paths; no clipboard or OS UI.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::temp_dir().join(format!(
            "chezcat-text-input-app-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        std::fs::write(directory.join("page.html"), r#"<!doctype html><html><head><title>Text input waiting</title><style>
html,body{margin:0;padding:0;background:#ffffff}
input{display:block;width:220px;height:32px;margin:0}
#neighbor{position:absolute;left:20px;top:20px}
#parsed{position:absolute;left:20px;top:70px}
#host{position:absolute;left:20px;top:120px;width:220px;height:32px}
button{position:absolute;left:280px;width:180px;height:32px}
#create{top:20px}#reset{top:70px}
#out{position:absolute;left:20px;top:180px;width:400px;height:40px;margin:0;padding:0;border:0;background:#ffffff;color:#123456;font-size:24px;line-height:32px}
</style></head><body><form id=f><input id=neighbor name=neighbor value="neighbor seed"><input id=parsed name=parsed value="parsed initial"><div id=host></div><button id=create name=create type=button>Create text input</button><button id=reset name=reset type=reset>Reset originals</button></form><p id=out>Before</p><script>
var parsed=document.getElementById('parsed'), dynamic=null, dynamicEdits=0, parsedEdits=0;
parsed.defaultValue='parsed latest';
parsed.oninput=function(event){
 if(!event.isTrusted)throw 'trusted parsed edit required';
 var live=this.value;this.defaultValue='parsed reset';
 if(this.value!==live)throw 'parsed default replaced edited live text';
 parsedEdits++;document.title='Parsed '+parsedEdits;document.getElementById('out').textContent='Parsed';
};
document.getElementById('create').onclick=function(event){
 if(!event.isTrusted||dynamic!==null)throw 'one trusted creation required';
 var host=document.getElementById('host');host.innerHTML='<input id="dynamic" name="dynamic" value="initial">';
 dynamic=host.firstChild;
 if(dynamic.type!=='text'||dynamic.value!=='initial'||dynamic.defaultValue!=='initial'||dynamic.form!==document.getElementById('f')||dynamic.ownerDocument!==document)throw 'genuine native fragment INPUT';
 dynamic.oninput=function(event){
  if(!event.isTrusted)throw 'trusted dynamic edit required';
  var live=this.value;this.defaultValue='dynamic reset';
  if(this.value!==live)throw 'dynamic default replaced edited live text';
  dynamicEdits++;document.title='Dynamic '+dynamicEdits;document.getElementById('out').textContent='Dynamic';
 };
 document.title='Text input created';
};
document.title='Text input ready';
</script></body></html>"#).unwrap();
        Self(directory)
    }
    fn open(&self) -> App {
        let mut app = App::new();
        app.resize(640, 480);
        app.set_javascript_enabled(true);
        app.open(&self.0.join("page.html"));
        assert!(!app.failed && !app.is_loading(), "{}", app.status);
        clean(&app);
        assert_eq!(app.document.title, "Text input ready");
        app
    }
    fn input(&self, app: &mut App, stage: &str, source: &str) {
        let script = self.0.join(format!("{stage}.input.txt"));
        std::fs::write(&script, source).unwrap();
        app.run_input_script(&script, &self.0.join(stage)).unwrap();
        clean(app);
    }
}
impl Drop for Fixture {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len()).find(|&node| app.document.attr(node, "id") == Some(name)).unwrap()
}
fn clean(app: &App) {
    assert!(!app.failed, "{}", app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", app.scripting.as_ref().unwrap().report());
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}
fn key(app: &mut App, code: u32, shift: bool) {
    assert!(matches!(app.key(code, false, shift, false), PlatformAction::None));
    clean(app);
}
fn click(app: &mut App, name: &str) {
    let node = id(app, name);
    let rect = app.page.controls.iter().find(|rect| rect.node == node).unwrap();
    let (x, page_y) = (rect.x + rect.width / 2, rect.y + rect.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(node));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    clean(app);
}
fn control_pixels(app: &App, name: &str, top: i32) -> Vec<u32> {
    let node = id(app, name);
    let control = &app.forms.controls[&node];
    assert_eq!(control.kind, Kind::Text);
    assert!(control.editable());
    assert_eq!(control.owner, Some(id(app, "f")));
    let rect = app.page.controls.iter().find(|rect| rect.node == node).unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, top, 220, 32));
    let border = app.page.border_rects[node].expect("Native editor must own its actual border box");
    assert_eq!((border.x, border.y, border.width, border.height), (20, top, 220, 32));
    assert_eq!(app.page.control_at(130, top + 16), Some(node));
    assert_eq!(app.page.control_at(19, top + 16), None);
    assert_eq!(app.page.control_at(240, top + 16), None);
    let canvas = app.draw();
    let y = app.content_top() + top - app.scroll;
    assert!(y >= 0 && (y + 32) as usize <= canvas.height && 240 <= canvas.width);
    let pixels: Vec<_> = (y as usize..y as usize + 32).flat_map(|row|
        canvas.pixels[row * canvas.width + 20..row * canvas.width + 240].iter().copied()).collect();
    assert!(pixels.iter().any(|&pixel| pixel != 0xffffff), "The native editor must actually paint");
    assert_eq!(pixels[16 * 220 + 216], 0xffffff, "The right-hand editor interior is clear white");
    pixels
}
fn neighbor(app: &App) -> (String, usize, usize, usize, bool, Option<usize>) {
    let control = &app.forms.controls[&id(app, "neighbor")];
    (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner)
}

#[test]
fn text_input_actual_app_fragment_fill_delete_and_reset_preserve_native_editors() {
    let fixture = Fixture::new();
    let mut app = fixture.open();
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    fixture.input(&mut app, "neighbor-fill", "fill\tneighbor\tneighbor kept\n");
    key(&mut app, 0x24, false);
    for _ in 0..2 { key(&mut app, 0x27, false); }
    for _ in 0..3 { key(&mut app, 0x27, true); }
    let preserved = neighbor(&app);
    assert_eq!((preserved.1, preserved.2), (5, 2));
    assert!(preserved.4);
    click(&mut app, "create");
    assert_eq!(app.document.title, "Text input created");
    assert_eq!(neighbor(&app), preserved, "Fragment insertion cannot reconstruct neighboring native editors");
    let keep_pixels = control_pixels(&app, "neighbor", 20);
    let initial_pixels = control_pixels(&app, "dynamic", 120);
    let dynamic = id(&app, "dynamic");
    assert_eq!(app.forms.controls[&dynamic].value.text(), "initial");
    assert!(!app.forms.controls[&dynamic].dirty_value);
    let nodes = (id(&app,"neighbor"),id(&app,"parsed"),dynamic,id(&app,"f"));
    fixture.input(&mut app, "dynamic-fill", "fill\tdynamic\tEdited\n");
    assert_eq!(app.document.title, "Dynamic 6");
    assert_eq!(app.forms.controls[&dynamic].value.text(), "Edited");
    assert!(app.forms.controls[&dynamic].dirty_value);
    key(&mut app, 0x08, false);
    assert_eq!(app.forms.controls[&dynamic].value.text(), "Edite");
    key(&mut app, 0x24, false);
    key(&mut app, 0x2e, false);
    assert_eq!(app.forms.controls[&dynamic].value.text(), "dite");
    assert_eq!(app.document.title, "Dynamic 8", "Backspace and Delete each dispatch one genuine input callback");
    assert_eq!(app.document.attr(dynamic, "value"), Some("dynamic reset"));
    assert_eq!(neighbor(&app), preserved);
    fixture.input(&mut app, "parsed-fill", "fill\tparsed\tParsed\n");
    let parsed = id(&app, "parsed");
    assert_eq!(app.forms.controls[&parsed].value.text(), "Parsed");
    assert!(app.forms.controls[&parsed].dirty_value);
    assert_eq!(app.document.attr(parsed, "value"), Some("parsed reset"));
    assert_eq!(app.document.title, "Parsed 6");
    assert_eq!(neighbor(&app), preserved);
    assert_eq!(control_pixels(&app, "neighbor", 20), keep_pixels, "Unfocused neighboring editor paint must stay identical");
    assert_ne!(control_pixels(&app, "dynamic", 120), initial_pixels, "Edited text must visibly change the same unfocused native box");
    control_pixels(&app, "parsed", 70);
    click(&mut app, "reset");
    assert_eq!(app.document.title, "Parsed 6", "Reset must not dispatch authored input callbacks");
    for (name, expected) in [("dynamic", "dynamic reset"), ("parsed", "parsed reset"), ("neighbor", "neighbor seed")] {
        let control = &app.forms.controls[&id(&app, name)];
        assert_eq!(control.value.text(), expected);
        assert!(!control.dirty_value, "Reset clears the native dirty flag for {name}");
    }
    control_pixels(&app, "dynamic", 120);
    control_pixels(&app, "parsed", 70);
    // Arena allocation can grow as callbacks replace text; genuine node
    // identities and native ownership must survive that ordinary growth.
    assert_eq!((id(&app,"neighbor"),id(&app,"parsed"),id(&app,"dynamic"),id(&app,"f")), nodes);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}

#[test]
fn text_input_actual_app_reflow_retires_focused_editor_and_preserves_visible_neighbor() {
    fn editor(app: &App, node: usize) -> (String, usize, usize, usize, bool) {
        let control = &app.forms.controls[&node];
        (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value)
    }
    fn remove_and_reflow(app: &mut App, source: &str) {
        // The resource/media completion paths can enter this exact App reflow
        // boundary after author callbacks. Do not call focus validation here:
        // that would conceal whether reflow retires stale native admission.
        let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, source);
        assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
        assert!(outcome.layout_changed, "Removing a connected native control must change layout");
        app.reflow();
    }
    let fixture = Fixture::new();
    let mut app = fixture.open();
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    click(&mut app, "create");
    let dynamic = id(&app, "dynamic");
    let neighbor = id(&app, "neighbor");
    let parsed = id(&app, "parsed");

    // Positive control: retirement of a different native input must not steal
    // the visible editor's focus, dirty text, cursor, selection or scroll.
    fixture.input(&mut app, "retirement-neighbor", "fill\tneighbor\tneighbor kept\n");
    key(&mut app, 0x24, false);
    for _ in 0..2 { key(&mut app, 0x27, false); }
    for _ in 0..3 { key(&mut app, 0x27, true); }
    assert!(app.focus == Focus::Control(neighbor));
    let preserved = editor(&app, neighbor);
    remove_and_reflow(&mut app, "parsed.parentNode.removeChild(parsed);");
    assert!(!app.page.controls.iter().any(|rect| rect.node == parsed));
    assert_eq!(app.forms.controls[&parsed].owner, None);
    assert!(app.focus == Focus::Control(neighbor), "A visible unaffected neighbor must retain focus");
    assert_eq!(editor(&app, neighbor), preserved);

    // Enter the fragment-created editor through actual pointer delivery, then
    // type/paste literal data through the same App admission used by the shell.
    click(&mut app, "dynamic");
    key(&mut app, 0x24, false);
    app.text_input('R');
    app.paste("etired ");
    clean(&app);
    assert_eq!(app.forms.controls[&dynamic].value.text(), "Retired initial");
    assert_eq!(app.document.title, "Dynamic 2");
    assert!(app.focus == Focus::Control(dynamic));
    let old_rect = app.page.controls.iter().find(|rect| rect.node == dynamic).unwrap().clone();
    let retired = editor(&app, dynamic);
    remove_and_reflow(&mut app, "dynamic.parentNode.removeChild(dynamic);");
    assert!(app.forms.controls.contains_key(&dynamic), "Retirement preserves the actual native editor identity");
    assert_eq!(app.forms.controls[&dynamic].owner, None);
    assert!(!app.forms.controls[&dynamic].disabled, "The negative must exercise an enabled retained editor");
    assert!(!app.page.controls.iter().any(|rect| rect.node == dynamic));
    assert!(app.page.border_rects[dynamic].is_none());
    assert_eq!(app.page.control_at(old_rect.x + old_rect.width / 2, old_rect.y + old_rect.height / 2), None);
    assert_eq!(editor(&app, dynamic), retired, "Detachment itself preserves the editor");
    let retired_title = app.document.title.clone();
    app.text_input('X');
    app.paste("NO RETIRED PASTE");
    assert_eq!(editor(&app, dynamic), retired,
        "Retired editor accepted native input: focus_page={}, title={:?}, script={} ",
        app.focus == Focus::Page, app.document.title, app.scripting.as_ref().unwrap().report());
    assert!(app.focus == Focus::Page, "Reflow must retire focus before later native input admission");
    assert_eq!(app.document.title, retired_title, "Retired input cannot dispatch an authored input callback");
    assert_eq!(editor(&app, neighbor), preserved, "Rejected retired input cannot edit a neighboring control");
    let outcome = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms,
        "if(dynamicEdits!==2||parsedEdits!==0)throw 'retired native input callback';");
    assert_eq!(outcome.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    clean(&app);
}
