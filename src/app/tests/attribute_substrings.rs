//! Original local selectors: inert metadata, shared CSS paint and trusted input.
use super::*;

const PAGE: &str = r#"<!doctype html><html><head><title>Substring selector ready</title><style>
html,body{margin:0;padding:0;background:white}
#marker,#literal{position:absolute;top:20px;width:80px;height:30px}
#marker{left:20px}
.marker{background:#aa2233}
#literal{left:120px;background:#0d6efd}
div[data-module*="assets/widget-"]{background:#0d6efd}
#check{position:absolute;left:20px;top:70px;width:160px;height:30px}
#neighbor{position:absolute;left:20px;top:120px;width:180px;height:30px}
</style></head><body><div id=scope><div id=marker class=marker data-module='assets/widget-panel.data'></div></div>
<div id=literal></div><button id=check name=selector-check type=button>Check selectors</button>
<form id=f><input id=neighbor name=keep value=seed></form>
<script id=unrelated type=application/json src='assets/other.data'></script>
<script id=first type=application/json src='assets/widget-core.data'></script>
<script id=second type=application/json src='assets/widget-tools.data?rev=1'></script>
<script id=case-sensitive type=application/json src='assets/Widget-core.data'></script>
<script>
var first=document.getElementById('first'),second=document.getElementById('second');
var marker=document.getElementById('marker'),scope=document.getElementById('scope');
var button=document.getElementById('check');
if(document.querySelectorAll('script[src]').length!==4||
   document.querySelector('script[src="assets/widget-core.data"]')!==first||
   first.getAttribute('src')!=='assets/widget-core.data'||
   marker.parentNode!==scope||marker.ownerDocument!==document)
  throw 'existing exact, presence and physical identity prefix';
button.onclick=function(event){
  if(!event.isTrusted||this!==button||event.target!==button||event.currentTarget!==button)
    throw 'real native button activation';
  document.title='Trusted selector prefix';
  var selected=document.querySelector('script[src*="assets/widget-"]');
  if(selected!==first)throw 'quoted substring first actual node';
  var all=document.querySelectorAll('script[src*="assets/widget-"]');
  if(all.length!==2||all[0]!==first||all[1]!==second)
    throw 'static substring list order and case-sensitive negatives';
  if(!first.matches('script[src*="assets/widget-"]')||
     document.getElementById('unrelated').matches('script[src*="assets/widget-"]')||
     document.getElementById('case-sensitive').matches('script[src*="assets/widget-"]'))
    throw 'matches must use the same literal substring operation';
  if(document.querySelector('div[data-module*="assets/widget-"]')!==marker||
     scope.querySelector('div[data-module*="assets/widget-"]')!==marker||
     scope.querySelectorAll('div[data-module*="assets/widget-"]').length!==1||
     !marker.matches('div[data-module*="assets/widget-"]'))
    throw 'the CSS marker must also be the actual scoped DOM result';
  document.title='Substring selectors passed';
};
</script></body></html>"#;

fn node(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}

fn marker_pixels(app: &App, name: &str, left: i32, color: u32) {
    let rect = app.page.border_rects[node(app, name)].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (left, 20, 80, 30));
    let frame = app.draw();
    for y in 0..rect.height {
        let start = (app.content_top() + rect.y + y - app.scroll) as usize * frame.width
            + rect.x as usize;
        assert!(frame.pixels[start..start + rect.width as usize].iter().all(|&pixel| pixel == color));
    }
}

#[test]
fn attribute_substring_app_trusted_queries_and_css_share_literal_inert_sources() {
    let fixture = Fixture::new();
    let path = fixture.write("selectors.html", PAGE);
    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    app.wait_for_load();
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", app.scripting.as_ref().unwrap().report());
    assert_eq!(app.document.title, "Substring selector ready");
    marker_pixels(&app, "literal", 120, 0x0d6efd);

    let neighbor = node(&app, "neighbor");
    let control = app.page.controls.iter().find(|control| control.node == neighbor).unwrap();
    let (x, y) = (control.x + 8, app.content_top() + control.y + 8 - app.scroll);
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    assert!(app.focus == Focus::Control(neighbor));
    app.key(0x41, true, false, false);
    app.paste("edited selector neighbor");
    let control = &app.forms.controls[&neighbor];
    let kept = (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner);
    assert_eq!(kept.0, "edited selector neighbor");
    assert_eq!(kept.5, Some(node(&app, "f")));
    assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "seed");
    let raw = app.raw.clone();
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());
    let count = app.forms.controls.len();

    let button = node(&app, "check");
    let control = app.page.controls.iter().find(|control| control.node == button).unwrap();
    let (x, page_y) = (control.x + control.width / 2, control.y + control.height / 2);
    assert_eq!(app.page.control_at(x, page_y), Some(button));
    let y = app.content_top() + page_y - app.scroll;
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    assert_eq!(app.scripting.as_ref().unwrap().error_count(), 0, "{}", app.scripting.as_ref().unwrap().report());
    assert_eq!(app.document.title, "Substring selectors passed");
    marker_pixels(&app, "marker", 20, 0x0d6efd);
    marker_pixels(&app, "literal", 120, 0x0d6efd);
    let control = &app.forms.controls[&neighbor];
    assert_eq!((control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner), kept);
    assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "seed");
    assert_eq!(app.forms.controls.len(), count);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    assert!(app.pending_images.is_empty());
}
