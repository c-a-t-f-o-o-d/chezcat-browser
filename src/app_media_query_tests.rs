//! Actual App/layout/pixel controls over original local markup, no OS input,
//! JavaScript, public service, external asset, audio device or reference renderer.
use super::*;
use std::sync::atomic::{AtomicUsize,Ordering};

const PAGE:&str=r#"<!doctype html><html><head><title>Original media layout</title><style>
html,body{margin:0;padding:0}#navigation{padding-left:120px;background:#eeeeee}
#navigation ul{margin:0;padding:0}#navigation li{display:block;height:24px}
#result{width:160px;height:32px;background:#aa2200;color:white}#after{height:16px;background:#224488}
@media screen and (min-width:1024px) and (hover:hover){#navigation{display:none}#result{background:#008844}}
@media screen and (max-width:1023px){#result{width:96px}}
@media print{#result{display:none}}@media screen and (original-unknown:1){#after{height:300px}}
</style></head><body><nav id=navigation><ul><li><a href=#result>First original link</a></li><li><a href=#after>Second original link</a></li></ul></nav>
<div id=result>Conditional paint</div><div id=after>Following content</div></body></html>"#;

struct Fixture(PathBuf);
impl Fixture {
    fn new()->Self {
        static NEXT:AtomicUsize=AtomicUsize::new(0);
        let folder=std::env::temp_dir().join(format!("chezcat-media-query-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
        std::fs::create_dir(&folder).unwrap();std::fs::write(folder.join("page.html"),PAGE).unwrap();Self(folder)
    }
    fn open(&self)->App {let mut app=App::new();app.scripts_enabled=false;app.open(&self.0.join("page.html"));assert!(!app.failed,"{}",app.status);assert!(!app.is_loading());app}
}
impl Drop for Fixture {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
fn id(app:&App,value:&str)->usize {(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"id")==Some(value)).unwrap()}
fn sample(app:&App,node:usize)->u32 {
    let rect=app.page.border_rects[node].unwrap();let canvas=app.draw();
    let x=rect.x+rect.width-2;let y=app.content_top()+rect.y+rect.height-2;
    assert!(x>=0&&y>=0&&(x as usize)<canvas.width&&(y as usize)<canvas.height);
    canvas.pixels[y as usize*canvas.width+x as usize]
}

// Normal whitespace creates separate native text runs. Bind every expected
// piece and actual hit to the one genuine href/DOM owner; do not join arbitrary
// page text or substitute a presence-only link-target assertion.
fn visible_link(app:&App,href:&str,owner:usize,words:&[&str])->(i32,i32) {
    let target=app.page.link_targets().position(|value|value==href).unwrap();
    let runs:Vec<_>=app.page.runs.iter().filter(|run|run.link==Some(target)).collect();
    assert_eq!(runs.iter().map(|run|run.text.as_str()).collect::<Vec<_>>(),words);
    for run in &runs {
        assert!(run.width()>1&&run.height()>1);
        assert_eq!(app.page.link_at(run.x+1,run.y+1),Some(href));
        assert_eq!(app.page.link_node_at(run.x+1,run.y+1),Some(owner));
    }
    (runs[0].x+1,runs[0].y+1)
}
fn hidden_navigation(app:&App) {
    assert!(app.page.link_targets().all(|href|href!="#result"&&href!="#after"));
    assert!(app.page.runs.iter().all(|run|run.link.is_none()));
}

#[test]
fn css_media_actual_app_resize_uses_full_viewport_and_preserves_document_and_pixels(){
    let fixture=Fixture::new();let mut app=fixture.open();
    let result=id(&app,"result");let navigation=id(&app,"navigation");let after=id(&app,"after");
    let anchor=|href:&str|(0..app.document.nodes.len()).find(|&node|app.document.attr(node,"href")==Some(href)).unwrap();
    let first=anchor("#result");let second=anchor("#after");
    let nodes=app.document.nodes.as_ptr();let raw=app.raw.clone();let location=app.location.display();let events=app.events.clone();
    app.resize(1024,760);
    assert_eq!(app.document.media_environment.unwrap().width,1024.0);
    assert!(app.page.border_rects[navigation].is_none(),"line width1012 must not replace media viewport1024");
    let wide=app.page.border_rects[result].unwrap();assert_eq!((wide.y,wide.width,wide.height),(0,160,32));
    assert_eq!(app.page.border_rects[after].unwrap().y,32);assert_eq!(sample(&app,result),0x008844);
    hidden_navigation(&app);
    app.resize(1023,760);
    let narrow=app.page.border_rects[result].unwrap();assert_eq!((narrow.y,narrow.width,narrow.height),(48,96,32));
    assert_eq!(app.page.border_rects[after].unwrap().y,80);assert_eq!(sample(&app,result),0xaa2200);
    let old_hit=visible_link(&app,"#result",first,&["First","original","link"]);
    visible_link(&app,"#after",second,&["Second","original","link"]);
    app.resize(1024,760);assert_eq!(sample(&app,result),0x008844);assert_eq!(app.page.border_rects[result].unwrap().y,0);
    hidden_navigation(&app);
    assert_eq!(app.page.link_at(old_hit.0,old_hit.1),None);
    assert_eq!(app.page.link_node_at(old_hit.0,old_hit.1),None);
    assert_eq!(app.document.attr(first,"href"),Some("#result"));assert_eq!(app.document.attr(second,"href"),Some("#after"));
    assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.raw,raw);assert_eq!(app.location.display(),location);assert_eq!(app.events,events);
    assert!(app.scripting.is_none());assert!(app.styles.is_empty());assert!(!app.is_loading());
}

#[test]
fn css_media_actual_app_zoom_recomputes_css_viewport_without_reloading_source(){
    let fixture=Fixture::new();let mut app=fixture.open();let result=id(&app,"result");
    app.resize(1280,760);let nodes=app.document.nodes.as_ptr();let raw=app.raw.clone();let events=app.events.clone();
    app.set_zoom(1.25);assert_eq!(app.document.media_environment.unwrap().width,1024.0);
    assert_eq!(sample(&app,result),0x008844);assert_eq!(app.page.border_rects[result].unwrap().y,0);
    app.resize(1279,760);assert!(app.document.media_environment.unwrap().width<1024.0);
    assert_eq!(sample(&app,result),0xaa2200);assert!(app.page.border_rects[result].unwrap().y>0);
    app.set_zoom(1.0);assert_eq!(sample(&app,result),0x008844);assert_eq!(app.page.border_rects[result].unwrap().y,0);
    assert_eq!(app.document.nodes.as_ptr(),nodes);assert_eq!(app.raw,raw);assert_eq!(app.events,events);assert!(app.scripting.is_none());
}
