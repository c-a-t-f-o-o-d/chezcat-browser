use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn stylesheet_cache_actual_app_report_views_preserve_document_cache_and_navigation_releases_it() {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Files(PathBuf);
    impl Drop for Files {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let files = Files(std::env::temp_dir().join(format!(
        "chezcat-sheet-cache-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    std::fs::create_dir(&files.0).unwrap();
    let html="<!doctype html><title>Cache lifetime</title><style>body{margin:0}#box{width:40px;height:20px;background:#228844;opacity:1}</style><div id=box></div>";
    let first = files.0.join("first.html");
    let second = files.0.join("second.html");
    std::fs::write(&first, html).unwrap();
    std::fs::write(&second, html).unwrap();
    let mut app = App::new();
    app.resize(640, 480);
    app.open(&first);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let original = app.stylesheet_cache.clone().unwrap();
    app.reflow();
    assert!(original.shares_rules(app.stylesheet_cache.as_ref().unwrap()));
    let canvas = app.draw();
    let top = app.content_top() as usize;
    assert_eq!(canvas.pixels[(top + 5) * canvas.width + 5], 0x228844);
    app.toggle_view(View::Resources);
    assert!(app.alternate.is_some());
    assert!(original.shares_rules(app.stylesheet_cache.as_ref().unwrap()));
    app.toggle_view(View::Resources);
    assert!(app.alternate.is_none());
    assert!(original.shares_rules(app.stylesheet_cache.as_ref().unwrap()));
    let restored = app.draw();
    assert_eq!(restored.pixels[(top + 5) * restored.width + 5], 0x228844);
    app.open(&second);
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert!(
        !original.shares_rules(app.stylesheet_cache.as_ref().unwrap()),
        "an identical source in a new Document does not keep the prior document's generation"
    );
    assert_eq!(app.document.title, "Cache lifetime");
}
