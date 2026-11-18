//! Fixed-size observations only. Never schedule work, run JS or claim native
//! presentation. Unknown is distinct from an observed zero or a failed hit.
use super::*;
use std::cell::Cell;

#[derive(Clone, Copy)]
struct LayoutStamp { generation: Option<u64>, document: u64, dom: Option<u64>, page_view: bool }
#[derive(Clone, Copy)]
struct CanvasStamp { generation: Option<u64>, layout: LayoutStamp, document: u64, dom: Option<u64>, scroll: i32, width: i32, height: i32 }
#[derive(Clone, Copy)]
struct Hit { layout: LayoutStamp, document: u64, dom: Option<u64>, x: i32, y: i32, page_y: i32, scroll: i32, node: Option<usize>, matched: bool }

pub(super) struct Data {
    layout: LayoutStamp,
    canvas_generation: Cell<Option<u64>>,
    canvas: Cell<Option<CanvasStamp>>,
    hit: Option<Hit>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> App {
        let mut app = App::new();
        app.resize(640, 480);
        app.document = html::parse("<body style='margin:0'><p id='p'>Before</p><input id='q'><a href='#target'>Link</a><div style='height:1200px'></div><p id='target'>End</p></body>");
        app.document.scripting_enabled = true;
        app.forms = Forms::new(&app.document);
        let mut page = crate::scripting::Page::for_test();
        assert_eq!(page.start(&mut app.document, &mut app.forms, &[]).errors, 0);
        app.scripting = Some(page);
        app.reflow();
        app
    }
    fn eval(app: &mut App, source: &str, errors: usize) {
        let result = app.scripting.as_mut().unwrap().console_eval(&mut app.document, &mut app.forms, source);
        assert_eq!(result.errors, errors, "{}", app.script_activity());
        if result.layout_changed { app.reflow(); }
    }

    #[test]
    fn absent_attribute_removal_preserves_app_layout_pixels_and_allows_the_next_update() {
        let mut app = fixture();
        eval(&mut app, "var p=document.getElementById('p'), q=document.getElementById('q');q.value='edited';", 0);
        let dom = app.observed_dom();
        let layout = app.observations.layout.generation;
        let before = app.draw();
        let start = app.content_top() as usize * before.width;
        eval(&mut app, r#"
            p.removeAttribute('initial-state');p.removeAttribute('hidden');q.removeAttribute('value');
            if(q.value!=='edited'||q.defaultValue!==''||p.textContent!=='Before')throw 'lost state';
        "#, 0);
        assert_eq!(app.observed_dom(), dom);
        assert_eq!(app.observations.layout.generation, layout);
        assert!(before.pixels[start..] == app.draw().pixels[start..]);
        // A real content update after the same cleanup must still reach layout and paint.
        eval(&mut app, "p.removeAttribute('initial-state');p.textContent='Updated';", 0);
        assert!(app.page.runs.iter().any(|run| run.text == "Updated"));
        assert!(before.pixels[start..] != app.draw().pixels[start..]);
    }

    #[test]
    fn stage_observations_correlate_dom_commits_layout_canvas_without_running_work() {
        let mut app = fixture();
        assert_eq!(app.observed_dom(), Some(0));
        assert!(app.stage_report().contains("canvas=unknown"));
        eval(&mut app, "var p=document.getElementById('p');p.setAttribute('data-x','v');var a=p.getAttributeNode('data-x');p.removeAttribute('data-x');", 0);
        assert_eq!(app.observed_dom(), Some(2));
        eval(&mut app, "p.setAttributeNS(null,'x:y','bad');", 1);
        assert_eq!(app.observed_dom(), Some(2));
        eval(&mut app, "a.value='detached';", 0);
        assert_eq!(app.observed_dom(), Some(3));
        // Detached writes need no new displayed layout.
        assert_eq!(app.observations.layout.dom, Some(2));
        eval(&mut app, "p.removeAttribute('data-absent');document.getElementById('q').value='native';", 0);
        assert_eq!(app.observed_dom(), Some(3));
        eval(&mut app, "p.textContent='After';", 0);
        assert_eq!(app.observations.layout.dom, Some(4));
        let before = app.activity_report();
        assert_eq!(before, app.activity_report());
        assert!(before.contains("canvas=unknown"));
        assert!(!app.draw().pixels.is_empty());
        let first = app.observations.canvas.get().unwrap();
        assert_eq!(first.dom, Some(4));
        assert_eq!(first.layout.dom, first.dom);
        app.set_scroll(10);
        app.draw();
        let second = app.observations.canvas.get().unwrap();
        assert_eq!(second.layout.generation, first.layout.generation);
        assert_eq!(second.scroll, 10);
        assert_eq!(second.generation, first.generation.map(|n| n + 1));
        app.toggle_view(View::Source);
        app.draw();
        assert!(!app.observations.canvas.get().unwrap().layout.page_view);
        assert!(app.activity_report().contains("native presentation is unknown"));
    }

    #[test]
    fn stage_observations_record_real_link_up_misses_and_hits() {
        let mut app = fixture();
        let run = app.page.runs.iter().find(|run| run.text == "Link").unwrap();
        let (x, y) = (run.x + run.width() - 1, run.y + app.content_top() + run.height() / 2);
        assert_eq!(app.page.link_at(x, y - app.content_top()), Some("#target"));
        app.pointer_down(x, y, false);
        app.pointer_up(x + 3, y);
        let miss = app.observations.hit.unwrap();
        assert!(!miss.matched);
        assert_eq!(miss.node, None);
        app.pointer_down(x - 3, y, false);
        app.pointer_up(x - 3, y);
        let hit = app.observations.hit.unwrap();
        assert!(hit.matched);
        assert!(hit.node.is_some());
        assert!(app.stage_report().contains("matched_press=1"));
        assert_eq!(hit.scroll, 0); // Native fragment activation has since scrolled.
        assert!(app.scroll > 0);
    }
}
impl Data {
    pub(super) fn new() -> Self {
        // App::new has just built this bundled document's real initial layout.
        Self { layout: LayoutStamp { generation: Some(1), document: 0, dom: None, page_view: true },
            canvas_generation: Cell::new(Some(0)), canvas: Cell::new(None), hit: None }
    }
}
fn number<T: std::fmt::Display>(value: Option<T>) -> String {
    value.map_or_else(|| "unknown".into(), |value| value.to_string())
}
fn layout_fields(stamp: LayoutStamp) -> String {
    format!("layout={} layout_document={} layout_dom={} page_view={}",
        number(stamp.generation), stamp.document, number(stamp.dom), u8::from(stamp.page_view))
}
impl App {
    fn observed_dom(&self) -> Option<u64> { self.scripting.as_ref().and_then(crate::scripting::Page::dom_commit_generation) }

    pub(super) fn observe_layout(&mut self) {
        self.observations.layout = LayoutStamp {
            generation: self.observations.layout.generation.and_then(|n| n.checked_add(1)),
            document: self.script_epoch, dom: self.observed_dom(), page_view: self.alternate.is_none(),
        };
    }
    pub(super) fn observe_canvas(&self, canvas: &Canvas) {
        if canvas.pixels.is_empty() { return; }
        let generation = self.observations.canvas_generation.get().and_then(|n| n.checked_add(1));
        self.observations.canvas_generation.set(generation);
        self.observations.canvas.set(Some(CanvasStamp { generation, layout: self.observations.layout,
            document: self.script_epoch, dom: self.observed_dom(), scroll: self.scroll, width: self.width, height: self.height }));
    }
    pub(super) fn observe_link_hit(&mut self, x: i32, y: i32, page_y: i32, node: Option<usize>, matched: bool) {
        self.observations.hit = Some(Hit { layout: self.observations.layout, document: self.script_epoch,
            dom: self.observed_dom(), x, y, page_y, scroll: self.scroll, node, matched });
    }

    pub(super) fn stage_report(&self) -> String {
        let mut text = format!("\nStage document={} dom_commit={} navigation_queued={} navigation_committed={} pending_request={} pending_committed={}\nStage {} viewport_link_regions={}\n",
            self.script_epoch, number(self.observed_dom()), number((self.next_request != 0).then_some(self.next_request)),
            number(self.committed_request), number(self.pending.as_ref().map(|p| p.id)),
            number(self.pending.as_ref().map(|p| u8::from(p.committed))), layout_fields(self.observations.layout),
            self.page.viewport_link_regions(self.width, self.scroll, self.viewport_height()));
        if let Some(stamp) = self.observations.canvas.get() {
            text.push_str(&format!("Stage canvas={} document={} dom_commit={} {} scroll={} width={} height={}\n",
                number(stamp.generation), stamp.document, number(stamp.dom), layout_fields(stamp.layout), stamp.scroll, stamp.width, stamp.height));
        } else { text.push_str("Stage canvas=unknown\n"); }
        if let Some(hit) = self.observations.hit {
            text.push_str(&format!("Stage last_link_hit document={} dom_commit={} {} x={} y={} page_y={} scroll={} node={} matched_press={}\n",
                hit.document, number(hit.dom), layout_fields(hit.layout), hit.x, hit.y, hit.page_y, hit.scroll, number(hit.node), u8::from(hit.matched)));
        } else { text.push_str("Stage last_link_hit=unknown\n"); }
        text.push_str("Stage scope: DOM commits include detached writes; native form values are separate. Link regions are viewport candidates, may be occluded. Canvas means generated pixels; native presentation is unknown.\n");
        text
    }
}
