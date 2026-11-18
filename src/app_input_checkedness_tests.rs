//! Canceled native radio activation restores selection without an IDL write.
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn id(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}

#[test]
fn input_checkedness_actual_app_canceled_peer_keeps_previous_pristine_and_author_changes() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "chezcat-radio-checkedness-{}-{}.html",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::write(&fixture.0, r#"<!doctype html><html><head><title>Radio ready</title></head><body>
<form><input id=neighbor value=seed><input id=first type=radio name=choice value=one checked><input id=peer type=radio name=choice value=two><input id=other type=checkbox value=keep></form>
<script>
var first=document.getElementById('first'),peer=document.getElementById('peer'),other=document.getElementById('other');
peer.onclick=function(event){
 if(!event.isTrusted||!peer.checked||first.checked)throw 'native radio preactivation';
 other.checked=true;event.preventDefault();document.title='Radio canceled';
};
</script></body></html>"#).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.set_javascript_enabled(true);
    app.open(&fixture.0);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let first = id(&app, "first");
    let peer = id(&app, "peer");
    let other = id(&app, "other");
    let neighbor = id(&app, "neighbor");
    app.forms
        .script_set_value(neighbor, "edited neighbor")
        .unwrap();
    let value = &mut app.forms.controls.get_mut(&neighbor).unwrap().value;
    value.cursor = 2;
    value.anchor = 5;
    value.offset = 1;
    let before = (
        value.chars.clone(),
        value.cursor,
        value.anchor,
        value.offset,
    );
    assert!(!app.forms.controls[&first].dirty_checked);
    app.activate_control(peer);
    assert_eq!(app.document.title, "Radio canceled");
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
    assert!(app.forms.controls[&first].checked);
    assert!(
        !app.forms.controls[&first].dirty_checked,
        "default rollback must not simulate an IDL setter"
    );
    assert!(!app.forms.controls[&peer].checked);
    assert!(
        app.forms.controls[&other].checked && app.forms.controls[&other].dirty_checked,
        "author change to another control was rolled back"
    );
    let outcome=app.scripting.as_mut().unwrap().console_eval(&mut app.document,&mut app.forms,r#"
        first.removeAttribute('checked');
        if(first.checked||!other.checked||peer.checked)throw 'canceled activation dirtied the previous radio';
    "#);
    assert_eq!(
        outcome.errors,
        0,
        "{}",
        app.scripting.as_ref().unwrap().report()
    );
    let value = &app.forms.controls[&neighbor].value;
    assert_eq!(
        (
            value.chars.clone(),
            value.cursor,
            value.anchor,
            value.offset
        ),
        before
    );
    assert!(!app.scripting.as_ref().unwrap().has_navigation_request());
}
