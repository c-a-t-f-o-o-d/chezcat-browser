//! Original live picture selection, distinct from initial-load selection tests.
use super::*;

const PAGE: &str = r#"<!doctype html><html><head><title>Original responsive card</title><style>
html,body{margin:0;padding:0;background:white}
#card{position:absolute;left:20px;top:20px}
#thumb{display:block;width:80px;height:48px;image-rendering:crisp-edges}
#neighbor{position:absolute;left:20px;top:100px;width:180px;height:30px}
</style></head><body><a id=card href=detail.html><picture>
<source type=image/webp media='(min-width:700px)' srcset=wide.webp>
<img id=thumb src=narrow.jpg width=80 height=48 alt='Original responsive image'>
</picture></a><form id=f><input id=neighbor name=keep value=seed></form></body></html>"#;

fn node(app: &App, name: &str) -> usize {
    (0..app.document.nodes.len())
        .find(|&node| app.document.attr(node, "id") == Some(name))
        .unwrap()
}

fn ready(app: &mut App) {
    app.wait_for_load();
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    let page = app.scripting.as_ref().unwrap();
    assert_eq!(page.error_count(), 0, "{}", page.report());
}

fn current_source(app: &mut App, source: &str) {
    let expected = navigation::resolve(source, &app.base_location).unwrap().display();
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        &format!("if(document.getElementById('thumb').currentSrc!=={expected:?})throw 'actual selected image source';"),
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
}

fn card_pixels(app: &App) -> Vec<u32> {
    let image = node(app, "thumb");
    let card = node(app, "card");
    let rect = app.page.border_rects[image].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, 20, 80, 48));
    for (x, y) in [(30, 30), (90, 58)] {
        assert_eq!(app.page.link_at(x, y), Some("detail.html"));
        assert_eq!(app.page.link_node_at(x, y), Some(card));
    }
    let frame = app.draw();
    (0..rect.height).flat_map(|y| {
        let row = (app.content_top() + rect.y + y - app.scroll) as usize;
        let start = row * frame.width + rect.x as usize;
        frame.pixels[start..start + rect.width as usize].iter().copied()
    }).collect()
}

#[test]
fn responsive_picture_reselects_webp_after_resize_preserving_card_and_native_editor() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.0.join("narrow.jpg"),
        include_bytes!("../../../pages/images/jpeg-landscape.jpg"),
    ).unwrap();
    std::fs::write(
        fixture.0.join("wide.webp"),
        include_bytes!("../../../pages/images/format-lossy.webp"),
    ).unwrap();
    let path = fixture.write("card.html", PAGE);
    fixture.write("detail.html", "<title>Original responsive detail</title><p>Original image destination</p>");

    // Cold-wide is an existing-capability control: the media source, WebP
    // decoder and actual card paint must work before testing a live resize.
    let mut wide = App::new();
    wide.resize(800, 360);
    wide.set_javascript_enabled(true);
    wide.open(&path);
    ready(&mut wide);
    current_source(&mut wide, "wide.webp");
    let wide_bitmap = wide.images[&node(&wide, "thumb")].clone();
    assert!(wide_bitmap.width > 0 && wide_bitmap.height > 0);
    let expected_paint = card_pixels(&wide);
    assert!(expected_paint.iter().any(|&pixel| pixel != 0xffffff));

    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    ready(&mut app);
    current_source(&mut app, "narrow.jpg");
    let image = node(&app, "thumb");
    let narrow_bitmap = &app.images[&image];
    assert!(narrow_bitmap.width > 0 && narrow_bitmap.height > 0);
    assert!(narrow_bitmap.pixels != wide_bitmap.pixels, "The original JPEG and WebP must distinguish the selected resource");
    assert!(card_pixels(&app) != expected_paint, "The two authored images must distinguish visible card paint");
    let neighbor = node(&app, "neighbor");
    let control = app.page.controls.iter().find(|control| control.node == neighbor).unwrap();
    let (x, y) = (control.x + 8, app.content_top() + control.y + 8 - app.scroll);
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    assert!(app.focus == Focus::Control(neighbor));
    app.key(0x41, true, false, false);
    app.paste("edited neighbor");
    let control = &app.forms.controls[&neighbor];
    let kept = (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner);
    assert_eq!(kept.0, "edited neighbor");
    assert_eq!(kept.5, Some(node(&app, "f")));
    let raw = app.raw.clone();
    let identity = (app.script_epoch, app.document.nodes.as_ptr());
    let location = app.location.clone();
    let history = (app.history_index, app.history.len());

    app.resize(800, 360);
    ready(&mut app);
    assert_eq!((app.script_epoch, app.document.nodes.as_ptr()), identity);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    assert_eq!((app.history_index, app.history.len()), history);
    assert_eq!(app.document.attr(image, "src"), Some("narrow.jpg"));
    let control = &app.forms.controls[&neighbor];
    assert_eq!((control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner), kept);
    assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "seed");

    // First new assertion: reflow alone retains the narrow decoded resource.
    let selected = &app.images[&image];
    assert!(
        selected.width == wide_bitmap.width && selected.height == wide_bitmap.height
            && selected.pixels == wide_bitmap.pixels,
        "LIVE RESIZE: the selected decoded image must match the proved cold-wide WebP",
    );
    current_source(&mut app, "wide.webp");
    assert!(card_pixels(&app) == expected_paint, "LIVE RESIZE: actual card paint must match cold-wide selection");

    // The real resized image remains the original link's visible hit target.
    let (x, y) = (30, app.content_top() + 30 - app.scroll);
    assert!(app.wants_hand(x, y));
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    ready(&mut app);
    assert_eq!(app.document.title, "Original responsive detail");
}

const DENSITY_PAGE: &str = r#"<!doctype html><html><head><title>Original density-only selection</title><style>
html,body{margin:0;padding:0;background:white}
#card{position:absolute;left:20px;top:20px}
img{display:block;image-rendering:crisp-edges}
#fixed{position:absolute;left:20px;top:400px}
#literal{position:absolute;left:140px;top:400px;width:80px;height:40px;background:#2468ac}
#natural{position:absolute;left:260px;top:400px}
#neighbor{position:absolute;left:20px;top:460px;width:180px;height:30px}
</style></head><body><a id=card href=detail.html>
<img id=thumb src=shared.ppm srcset='shared.ppm 4w' sizes=50vw alt='Original density card'>
</a><img id=fixed src=shared.ppm width=80 height=40 alt='Fixed dimensions'>
<div id=literal></div><img id=natural src=shared.ppm alt='Uncorrected natural dimensions'>
<form id=f><input id=neighbor name=keep value=seed></form></body></html>"#;

fn density_controls(app: &App, width: i32, height: i32) {
    let image = node(app, "thumb");
    let rect = app.page.border_rects[image].unwrap();
    assert_eq!((rect.x, rect.y, rect.width, rect.height), (20, 20, width, height));
    for (id, expected) in [
        ("fixed", (20, 400, 80, 40)),
        ("literal", (140, 400, 80, 40)),
        ("natural", (260, 400, 4, 2)),
    ] {
        let rect = app.page.border_rects[node(app, id)].unwrap();
        assert_eq!((rect.x, rect.y, rect.width, rect.height), expected, "{id}");
    }
    assert_eq!(app.page.link_at(30, 30), Some("detail.html"));
    assert_eq!(app.page.link_node_at(30, 30), Some(node(app, "card")));
    let frame = app.draw();
    let pixel = |x: i32, y: i32| {
        frame.pixels[(app.content_top() + y - app.scroll) as usize * frame.width + x as usize]
    };
    // Sample the centers of the eight authored source pixels, at either density.
    for (row, colors) in [
        [0xff0000, 0x00ff00, 0x0000ff, 0xffffff],
        [0x000000, 0xffff00, 0xff00ff, 0x00ffff],
    ].iter().enumerate() {
        for (column, &color) in colors.iter().enumerate() {
            let x = 20 + width * (2 * column as i32 + 1) / 8;
            let y = 20 + height * (2 * row as i32 + 1) / 4;
            assert_eq!(pixel(x, y), color, "source pixel {column},{row}");
        }
    }
    assert_eq!(pixel(150, 410), 0x2468ac);
    assert_eq!(pixel(260, 400), 0xff0000);
    assert_eq!(pixel(263, 401), 0x00ffff);
}

#[test]
fn responsive_picture_reselects_density_for_same_url_without_javascript_or_redecode() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.0.join("shared.ppm"),
        b"P6\n4 2\n255\n\xff\x00\x00\x00\xff\x00\x00\x00\xff\xff\xff\xff\x00\x00\x00\xff\xff\x00\xff\x00\xff\x00\xff\xff",
    ).unwrap();
    let path = fixture.write("density.html", DENSITY_PAGE);
    fixture.write("detail.html", "<title>Original density destination</title>");

    let mut app = App::new();
    app.resize(640, 800);
    app.set_javascript_enabled(false);
    app.open(&path);
    app.wait_for_load();
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert!(app.scripting.is_none());
    let image = node(&app, "thumb");
    let bitmap = app.images[&image].clone();
    assert_eq!((bitmap.width, bitmap.height), (4, 2));
    // Initial selection reserves App's existing 12 px content gutter. Derive
    // 50vw from that same observed viewport rather than the outer window width.
    let narrow_source_size = f64::from(app.load_options().viewport_width) / 2.0;
    assert_eq!(narrow_source_size, 314.0);
    assert_eq!(app.image_densities[&image], 4.0 / narrow_source_size);
    density_controls(&app, narrow_source_size as i32, narrow_source_size as i32 / 2);

    let neighbor = node(&app, "neighbor");
    let control = app.page.controls.iter().find(|control| control.node == neighbor).unwrap();
    let (x, y) = (control.x + 8, app.content_top() + control.y + 8 - app.scroll);
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    app.key(0x41, true, false, false);
    app.paste("density neighbor edited");
    let control = &app.forms.controls[&neighbor];
    let kept = (control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner);
    assert_eq!(kept.0, "density neighbor edited");
    assert_eq!(kept.5, Some(node(&app, "f")));
    let raw = app.raw.clone();
    let location = app.location.clone();

    app.resize(1280, 800);
    app.wait_for_load();
    assert!(!app.failed && !app.is_loading(), "{}", app.status);
    assert!(app.scripting.is_none());
    assert!(Arc::ptr_eq(&bitmap, &app.images[&image]), "same selected URL must retain its decoded image");
    assert_eq!(app.document.attr(image, "src"), Some("shared.ppm"));
    assert_eq!(app.document.attr(image, "srcset"), Some("shared.ppm 4w"));
    let wide_source_size = f64::from(app.load_options().viewport_width) / 2.0;
    assert_eq!(wide_source_size, 634.0);
    assert_eq!(app.image_densities[&image], 4.0 / wide_source_size);
    density_controls(&app, wide_source_size as i32, wide_source_size as i32 / 2);
    assert_eq!(app.raw, raw);
    assert_eq!(app.location, location);
    let control = &app.forms.controls[&neighbor];
    assert_eq!((control.value.text(), control.value.cursor, control.value.anchor, control.value.offset, control.dirty_value, control.owner), kept);
    assert_eq!(app.forms.text_default_value(neighbor).unwrap(), "seed");

    // An identical environment does not submit another resource job. These are
    // immediate state checks, rather than a timeout used as a no-request oracle.
    assert!(app.pending_images.is_empty());
    let events = app.events.clone();
    app.resize(1280, 800);
    assert!(!app.is_loading());
    assert!(app.pending_images.is_empty());
    assert_eq!(app.events, events);
    assert!(Arc::ptr_eq(&bitmap, &app.images[&image]));
    density_controls(&app, wide_source_size as i32, wide_source_size as i32 / 2);
}

#[test]
fn responsive_picture_reselects_same_url_density_with_one_load_and_fresh_natural_dimensions() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.0.join("shared.ppm"),
        b"P6\n4 2\n255\n\xff\x00\x00\x00\xff\x00\x00\x00\xff\xff\xff\xff\x00\x00\x00\xff\xff\x00\xff\x00\xff\x00\xff\xff",
    ).unwrap();
    let path = fixture.write("density-events.html", DENSITY_PAGE);

    let mut app = App::new();
    app.resize(640, 800);
    app.set_javascript_enabled(true);
    app.open(&path);
    ready(&mut app);
    current_source(&mut app, "shared.ppm");
    let image = node(&app, "thumb");
    let bitmap = app.images[&image].clone();
    assert_eq!((bitmap.width, bitmap.height), (4, 2));
    assert_eq!(app.load_options().viewport_width, 628);
    density_controls(&app, 314, 157);
    // Local open has already decoded and committed its initial images. Install
    // these observers after that checkpoint: a settled load must not replay.
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        r#"
var densityImage=document.getElementById('thumb');
if(!densityImage.complete||densityImage.naturalWidth!==314||densityImage.naturalHeight!==157)
  throw 'independently available initial density dimensions';
var densityLoads=0,densityErrors=0,densitySizes=[];
densityImage.onload=function(event){
  if(this!==densityImage||event.target!==densityImage||event.type!=='load')throw 'real density load';
  densityLoads++;
  densitySizes.push(densityImage.naturalWidth,densityImage.naturalHeight);
};
densityImage.onerror=function(){densityErrors++;};
if(densityLoads!==0||densityErrors!==0||densitySizes.length!==0)
  throw 'late observers must not replay initial load';
"#,
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());

    app.resize(1280, 800);
    ready(&mut app);
    assert!(Arc::ptr_eq(&bitmap, &app.images[&image]), "density-only change must retain its original decoded bitmap");
    assert_eq!(app.load_options().viewport_width, 1268);
    density_controls(&app, 634, 317);
    current_source(&mut app, "shared.ppm");
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "if(densityLoads!==1||densityErrors!==0||densitySizes.length!==2||densitySizes[0]!==634||densitySizes[1]!==317||densityImage.naturalWidth!==634||densityImage.naturalHeight!==317)throw 'one environment load with fresh density dimensions';",
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());

    app.resize(1280, 800);
    ready(&mut app);
    assert!(Arc::ptr_eq(&bitmap, &app.images[&image]));
    density_controls(&app, 634, 317);
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "if(densityLoads!==1||densityErrors!==0||densitySizes.length!==2||densityImage.naturalWidth!==634||densityImage.naturalHeight!==317)throw 'identical environment must not load again';",
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
}

const FAILED_CANDIDATE_PAGE: &str = r#"<!doctype html><html><head><title>Original failed responsive candidate</title><style>
html,body{margin:0;padding:0;background:white}
#card{position:absolute;left:20px;top:20px}
#thumb{display:block;width:80px;height:48px;image-rendering:crisp-edges}
#neighbor{position:absolute;left:20px;top:100px;width:180px;height:30px}
</style></head><body><a id=card href=detail.html><picture>
<source type=image/webp media='(min-width:700px)' srcset=broken.webp>
<img id=thumb src=narrow.jpg width=80 height=48 alt='Original retained image'>
</picture></a><form id=f><input id=neighbor name=keep value=seed></form></body></html>"#;

#[test]
fn responsive_picture_reselects_failed_candidate_preserving_current_image_and_source() {
    let fixture = Fixture::new();
    std::fs::write(
        fixture.0.join("narrow.jpg"),
        include_bytes!("../../../pages/images/jpeg-landscape.jpg"),
    ).unwrap();
    std::fs::write(fixture.0.join("broken.webp"), b"RIFF\x04\x00\x00\x00WEBP").unwrap();
    let path = fixture.write("failed.html", FAILED_CANDIDATE_PAGE);
    fixture.write("detail.html", "<title>Original retained image destination</title>");

    let mut app = App::new();
    app.resize(640, 360);
    app.set_javascript_enabled(true);
    app.open(&path);
    ready(&mut app);
    current_source(&mut app, "narrow.jpg");
    let image = node(&app, "thumb");
    let bitmap = app.images[&image].clone();
    let paint = card_pixels(&app);
    assert!(paint.iter().any(|&pixel| pixel != 0xffffff));
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        r#"
var image=document.getElementById('thumb');
if(!image.complete||!image.naturalWidth||!image.naturalHeight)throw 'initial available decoded image';
var oldWidth=image.naturalWidth,oldHeight=image.naturalHeight,successfulLoads=0,imageErrors=0;
image.onload=function(event){
  if(this!==image||event.target!==image||event.type!=='load')throw 'real image load event';
  successfulLoads++;
};
image.onerror=function(){imageErrors++;};
if(successfulLoads!==0||imageErrors!==0)throw 'initial load must not replay to late observers';
"#,
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    let raw = app.raw.clone();
    let events_before = app.events.len();

    app.resize(800, 360);
    ready(&mut app);
    assert!(
        app.events[events_before..].iter().any(|event| event.contains("WebP image chunk is missing")),
        "the wide candidate must actually reach the decoder before testing failure retention: {:?}",
        &app.events[events_before..],
    );
    assert!(Arc::ptr_eq(&bitmap, &app.images[&image]), "failed environment candidate must retain the available image");
    assert_eq!(card_pixels(&app), paint);
    current_source(&mut app, "narrow.jpg");
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "if(successfulLoads!==0||imageErrors!==0||!image.complete||image.naturalWidth!==oldWidth||image.naturalHeight!==oldHeight)throw 'failed environment candidate must preserve the available request without load or error';",
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    assert_eq!(app.raw, raw);
    assert_eq!(app.document.attr(image, "src"), Some("narrow.jpg"));
    assert_eq!(app.forms.text_default_value(node(&app, "neighbor")).unwrap(), "seed");

    // Recover the task-owned child resource while it is still selected. A
    // different wide viewport must suppress another attempt at that failed
    // choice; only leaving the choice permits a later retry.
    std::fs::write(
        fixture.0.join("broken.webp"),
        include_bytes!("../../../pages/images/format-lossy.webp"),
    ).unwrap();
    let mut cold_wide = App::new();
    cold_wide.resize(800, 360);
    cold_wide.set_javascript_enabled(true);
    cold_wide.open(&path);
    ready(&mut cold_wide);
    current_source(&mut cold_wide, "broken.webp");
    let recovered_bitmap = cold_wide.images[&node(&cold_wide, "thumb")].clone();
    let recovered_paint = card_pixels(&cold_wide);
    assert!(recovered_paint != paint, "the recovered WebP must visibly distinguish a retry from retained JPEG");

    app.resize(840, 360);
    ready(&mut app);
    assert!(Arc::ptr_eq(&bitmap, &app.images[&image]), "the same failed choice must not retry merely because its bytes recovered");
    assert_eq!(card_pixels(&app), paint);
    current_source(&mut app, "narrow.jpg");
    assert_eq!(
        app.events[events_before..].iter().filter(|event| event.contains("WebP image chunk is missing")).count(),
        1,
        "one actual failed decode, with no duplicate failed-choice attempts",
    );
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "if(successfulLoads!==0||imageErrors!==0)throw 'repeated failed choice must stay silent';",
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());

    app.resize(640, 360);
    ready(&mut app);
    assert!(Arc::ptr_eq(&bitmap, &app.images[&image]));
    assert_eq!(card_pixels(&app), paint);
    current_source(&mut app, "narrow.jpg");
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        "if(successfulLoads!==0||imageErrors!==0)throw 'returning to the displayed choice must stay silent';",
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());

    app.resize(800, 360);
    ready(&mut app);
    let selected = &app.images[&image];
    assert!(!Arc::ptr_eq(&bitmap, selected));
    assert!(selected.width == recovered_bitmap.width && selected.height == recovered_bitmap.height
        && selected.pixels == recovered_bitmap.pixels,
        "leaving and returning must really decode the recovered WebP child");
    assert_eq!(card_pixels(&app), recovered_paint);
    current_source(&mut app, "broken.webp");
    let result = app.scripting.as_mut().unwrap().console_eval(
        &mut app.document,
        &mut app.forms,
        &format!("if(successfulLoads!==1||imageErrors!==0||!image.complete||image.naturalWidth!=={}||image.naturalHeight!=={})throw 'one successful recovered resource load';", recovered_bitmap.width, recovered_bitmap.height),
    );
    assert_eq!(result.errors, 0, "{}", app.scripting.as_ref().unwrap().report());
    assert_eq!(app.raw, raw);
    assert_eq!(app.document.attr(image, "src"), Some("narrow.jpg"));
    assert_eq!(app.forms.text_default_value(node(&app, "neighbor")).unwrap(), "seed");

    // The recovered rendered resource remains the original actual link target.
    let (x, y) = (30, app.content_top() + 30 - app.scroll);
    assert!(matches!(app.pointer_down(x, y, false), PlatformAction::None));
    assert!(matches!(app.pointer_up(x, y), PlatformAction::None));
    ready(&mut app);
    assert_eq!(app.document.title, "Original retained image destination");
}
