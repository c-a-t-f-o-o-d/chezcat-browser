use super::*;

pub(crate) fn fixture() -> FontSpec {
    let mut book = Book::empty();
    let font = Arc::new(crate::font::Font::parse(&crate::font::tests::fixture_font()).unwrap());
    book.add("Geometry".into(), 400, false, font).unwrap();
    FontSpec::new(&book, &["Geometry".into()], 400, false, 40.0).unwrap()
}

#[test]
fn proportional_advances_kerning_and_caret_boundaries_agree() {
    let font = fixture();
    let shape = font.shape("AV oé");
    assert_eq!(shape.width(), font.measure("AV oé"));
    assert!(font.measure("AV") < font.measure("A") + font.measure("V"));
    assert_ne!(font.measure("A"), font.measure(" "));
    for i in 0..=shape.len() {
        assert_eq!(shape.hit(shape.x(i)), i);
    }
    assert_eq!(shape.fitting_count(0, shape.x(2)), 2);
    assert_eq!(shape.fitting_count(2, shape.x(4) - shape.x(2)), 2);
    assert!(font.measure(&font.fit("AV oé AV oé", 90)) <= 90);
}

#[test]
fn glyph_painting_keeps_coverage_and_respects_the_existing_clip() {
    let font = fixture();
    let shape = font.shape("AV oé");
    let mut canvas = Canvas::new(180, 90);
    canvas.fill_rect(0, 0, 180, 90, 0xffffff);
    let clip = crate::paint::ClipRect::new(20, 10, 55, 40);
    canvas.set_clip(Some(clip));
    shape.draw(&mut canvas, 10, 3, 0);
    assert!(canvas
        .pixels
        .iter()
        .any(|&pixel| pixel != 0 && pixel != 0xffffff));
    assert!(canvas.pixels.contains(&0));
    for (i, &pixel) in canvas.pixels.iter().enumerate() {
        if !clip.contains((i % 180) as i32, (i / 180) as i32) {
            assert_eq!(pixel, 0xffffff);
        }
    }
    assert_eq!(canvas.clip(), clip);
}

#[test]
fn ellipsis_fitting_includes_the_new_boundary_kerning_pair() {
    let mut book = Book::empty();
    book.add(
        "Boundary".into(),
        400,
        false,
        Arc::new(
            crate::font::Font::parse(&crate::font::tests::positive_boundary_fixture()).unwrap(),
        ),
    )
    .unwrap();
    let font = FontSpec::new(&book, &["Boundary".into()], 400, false, 40.0).unwrap();
    assert!(font.measure("A") + font.measure("...") <= 64);
    assert!(font.measure("A...") > 64);
    assert_eq!(font.fit("AAAA", 64), "...");
}
