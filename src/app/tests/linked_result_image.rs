//! One original search -> decoded image link -> detail -> Back journey.
//! Reuse the parent module's loopback Site, decoder, escaper and App input helpers.
use super::*;

pub(super) const THUMBNAIL: &[u8] =
    b"P6\n2 2\n255\n\xff\x00\x00\x00\xff\x00\x00\x00\xff\xff\xff\x00";

pub(super) fn response(target: &str) -> Option<String> {
    let path = target.split('?').next().unwrap();
    if !matches!(path, "/home" | "/search" | "/watch") {
        return None;
    }
    let wire_query = target
        .split_once('?')
        .map_or("", |(_, query)| query)
        .split('&')
        .find_map(|pair| pair.strip_prefix("q="))
        .unwrap_or("");
    let query = decode(wire_query);
    Some(if path == "/watch" {
        format!("<!doctype html><title>Original watch-like detail</title><h1>Original detail</h1><p>Query {}</p>", escape(&query))
    } else {
        page(path == "/search", &query, wire_query)
    })
}

fn page(results: bool, query: &str, wire_query: &str) -> String {
    let result = if results {
        format!("<h1>Original results</h1><p>Query {}</p><a id=result href='/watch?v=original&amp;q={}'><img id=thumbnail src='/thumb.ppm' width=80 height=48 alt='Original color tile'></a>", escape(query), escape(wire_query))
    } else {
        "<h1>Original local search</h1>".into()
    };
    format!(
        r#"<!doctype html><html><head><title>Original {kind}</title><style>
html,body{{margin:0;background:white}}form{{height:45px}}input,button{{height:28px}}
#neighbor{{position:absolute;left:300px;top:4px;width:160px}}#thumbnail{{display:block;image-rendering:crisp-edges}}
</style></head><body><form id=f action=/search><input id=q name=q value='{query}'>
<input id=trusted type=hidden name=trusted value=pending><button id=send name=go value=search>Search</button></form>
<input id=neighbor value=seed>{result}<script>
document.getElementById('f').onsubmit=function(event){{
    if(!event.isTrusted||event.target!==this||event.currentTarget!==this){{event.preventDefault();throw 'real trusted submit required';}}
    document.getElementById('trusted').value='yes';
}};
</script></body></html>"#,
        kind = if results { "results" } else { "home" },
        query = escape(query)
    )
}

fn thumbnail(app: &App) -> (usize, i32, i32) {
    let node = id(app, "thumbnail");
    let image = &app.images[&node];
    assert_eq!((image.width, image.height), (2, 2));
    assert_eq!(
        image.pixels,
        [0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffff00]
    );
    let (x, y, width, height) = app
        .page
        .items
        .iter()
        .find_map(|item| match item {
            PaintItem::Image {
                node: owner,
                x,
                y,
                width,
                height,
                ..
            } if *owner == node => Some((*x, *y, *width, *height)),
            _ => None,
        })
        .unwrap();
    assert_eq!((width, height), (80, 48));
    let target = "/watch?v=original&q=alpha";
    let canvas = app.draw();
    for (dx, dy, color) in [
        (10, 6, 0xff0000),
        (60, 6, 0x00ff00),
        (10, 36, 0x0000ff),
        (60, 36, 0xffff00),
    ] {
        let screen_y = app.content_top() + y + dy - app.scroll;
        assert_eq!(
            canvas.pixels[screen_y as usize * canvas.width + (x + dx) as usize],
            color
        );
        assert_eq!(app.page.link_at(x + dx, y + dy), Some(target));
        assert_eq!(
            app.page.link_node_at(x + dx, y + dy),
            Some(id(app, "result"))
        );
    }
    (node, x, y)
}

#[test]
fn linked_result_image_trusted_search_decodes_clicks_query_detail_and_back_remains_usable() {
    let site = Site::with_linked_result_image();
    let mut app = App::new();
    app.resize(800, 600);
    app.set_javascript_enabled(true);
    app.open_address(&site.url("/home"));
    wait(&mut app);
    assert!(!app.is_loading());
    assert_eq!(app.document.title, "Original home");
    edit_query(&mut app, "alpha");
    assert!(matches!(
        app.key(0x0d, false, false, false),
        PlatformAction::None
    ));
    wait(&mut app);
    assert!(!app.is_loading());
    let first = "/search?q=alpha&trusted=yes&go=search";
    assert!(site.seen(first), "{:?}", site.requests.lock().unwrap());
    assert_eq!(app.location.display(), site.url(first));
    assert_eq!(app.document.title, "Original results");
    assert_eq!(app.forms.controls[&id(&app, "q")].value.text(), "alpha");
    assert!(site.seen("/thumb.ppm"));
    let (_, x, y) = thumbnail(&app);
    click_control(&mut app, "neighbor");
    assert!(app.focus == Focus::Control(id(&app, "neighbor")));
    assert!(matches!(
        app.key(0x41, true, false, false),
        PlatformAction::None
    ));
    app.paste("neighbor edited");
    assert_eq!(
        app.forms.controls[&id(&app, "neighbor")].value.text(),
        "neighbor edited"
    );
    assert_eq!(
        app.forms.text_default_value(id(&app, "neighbor")).unwrap(),
        "seed"
    );
    let raw = app.raw.clone();
    let results_index = app.history_index;

    // Hit the decoded image itself, not a text run or a directly invoked URL.
    let (x, y) = (x + 10, app.content_top() + y + 6 - app.scroll);
    assert!(app.wants_hand(x, y));
    assert!(matches!(
        app.pointer_down(x, y, false),
        PlatformAction::None
    ));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    wait(&mut app);
    assert!(!app.is_loading());
    let destination = "/watch?v=original&q=alpha";
    assert!(site.seen(destination));
    assert_eq!(app.location.display(), site.url(destination));
    assert_eq!(app.document.title, "Original watch-like detail");
    assert_eq!(app.history_index, results_index + 1);
    assert!(app
        .page
        .runs
        .iter()
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .contains("Query alpha"));

    assert!(matches!(
        app.key(0x25, false, false, true),
        PlatformAction::None
    ));
    wait(&mut app);
    assert!(!app.is_loading());
    assert_eq!(app.history_index, results_index);
    assert_eq!(app.location.display(), site.url(first));
    assert_eq!(app.raw, raw);
    assert_eq!(app.forms.controls[&id(&app, "q")].value.text(), "alpha");
    // Full-document Back loads a fresh document in the current history model.
    // Observe optional non-query value restoration independently of usability.
    println!(
        "Full-document Back non-query editor: live={:?}, default={:?}",
        app.forms.controls[&id(&app, "neighbor")].value.text(),
        app.forms.text_default_value(id(&app, "neighbor")).unwrap(),
    );
    assert_eq!(
        app.forms.text_default_value(id(&app, "neighbor")).unwrap(),
        "seed"
    );
    thumbnail(&app);
    click_control(&mut app, "neighbor");
    assert!(app.focus == Focus::Control(id(&app, "neighbor")));
    assert!(matches!(
        app.key(0x41, true, false, false),
        PlatformAction::None
    ));
    app.paste("edited after Back");
    assert_eq!(
        app.forms.controls[&id(&app, "neighbor")].value.text(),
        "edited after Back"
    );
    assert_eq!(
        app.forms.text_default_value(id(&app, "neighbor")).unwrap(),
        "seed"
    );
    assert_eq!(app.forms.controls[&id(&app, "q")].value.text(), "alpha");
    edit_query(&mut app, "beta + caf\u{e9}");
    assert!(matches!(
        app.key(0x0d, false, false, false),
        PlatformAction::None
    ));
    wait(&mut app);
    assert!(!app.is_loading());
    let second = "/search?q=beta+%2B+caf%C3%A9&trusted=yes&go=search";
    assert!(site.seen(second));
    assert_eq!(app.location.display(), site.url(second));
    assert_eq!(app.document.title, "Original results");
    assert_eq!(
        app.forms.controls[&id(&app, "q")].value.text(),
        "beta + caf\u{e9}"
    );
}
