//! Original SVG CSS geometry, units, cascade and paint checks.
use super::*;
fn image(source: &str) -> Bitmap {
    super::decode(source.as_bytes(), &|| Ok(())).unwrap()
}
fn pixel(image: &Bitmap, x: usize, y: usize) -> u32 {
    image.pixels[y * image.width + x]
}
#[test]
fn svg_geometry_css_root_and_rect_percentage_values_override_presentation_attributes() {
    let img = image(
        r#"<svg width="10" height="10" viewBox="0 0 40 20"><style>svg{width:40px;height:20px}rect{width:50%;height:100%;fill:red}</style><rect width="1" height="1"/></svg>"#,
    );
    assert_eq!((img.width, img.height), (40, 20));
    assert_eq!(pixel(&img, 10, 10), 0xffff0000);
    assert_eq!(pixel(&img, 30, 10), 0);
}
#[test]
fn svg_geometry_noninherited_fields_and_explicit_inherit_have_element_specific_effects() {
    let img = image(
        r#"<svg width="32" height="32" viewBox="0 0 32 32"><g style="x:20px;y:20px;width:32px;height:32px"><rect width="16" height="16" fill="red"/></g></svg>"#,
    );
    assert_eq!(pixel(&img, 4, 4), 0xffff0000);
    assert_eq!(pixel(&img, 24, 24), 0);
    let img = image(
        r#"<svg style="width:32px;height:16px" viewBox="0 0 32 16"><rect style="width:inherit;height:inherit" fill="blue"/></svg>"#,
    );
    assert_eq!(pixel(&img, 24, 8), 0xff0000ff);
    let img = image(r#"<svg width="32" height="16"><rect height="16" fill="red"/></svg>"#);
    assert!(img.pixels.iter().all(|p| *p == 0));
}
#[test]
fn svg_geometry_font_relative_absolute_units_and_late_font_declarations_are_computed() {
    let img = image(
        r#"<svg style="width:1in;height:24px;font-size:20px" viewBox="0 0 96 24"><rect style="width:2em;height:1em;font-size:8px" fill="blue"/><rect x="30" style="width:1rem;height:1rem" fill="red"/></svg>"#,
    );
    assert_eq!((img.width, img.height), (96, 24));
    assert_eq!(pixel(&img, 12, 4), 0xff0000ff);
    assert_eq!(pixel(&img, 18, 4), 0);
    assert_eq!(pixel(&img, 46, 16), 0xffff0000);
    assert_eq!(pixel(&img, 52, 16), 0);
}
#[test]
fn svg_geometry_important_stylesheet_beats_normal_inline_and_important_inline_wins() {
    let img = image(
        r#"<svg width="32" height="32"><style>rect{width:24px!important;height:24px;fill:red!important}</style><rect width="1" height="1" style="width:4px;height:12px!important;fill:blue"/></svg>"#,
    );
    assert_eq!(pixel(&img, 20, 8), 0xffff0000);
    assert_eq!(pixel(&img, 20, 16), 0);
    assert_eq!(pixel(&img, 28, 8), 0);
}
#[test]
fn svg_geometry_shape_centers_auto_radii_and_negative_sizes_follow_initial_values() {
    let img = image(
        r#"<svg width="32" height="32" viewBox="0 0 32 32"><circle style="cx:50%;cy:50%;r:25%" fill="red"/></svg>"#,
    );
    assert_eq!(pixel(&img, 16, 16), 0xffff0000);
    assert_eq!(pixel(&img, 2, 2), 0);
    let img = image(
        r#"<svg width="32" height="32"><ellipse style="cx:16px;cy:16px;rx:auto;ry:8px" fill="blue"/></svg>"#,
    );
    assert_eq!(pixel(&img, 16, 16), 0xff0000ff);
    assert_eq!(pixel(&img, 6, 16), 0);
    let img = image(
        r#"<svg width="32" height="32"><circle cx="16" cy="16" r="10" style="r:-1px"/><rect width="20" height="20" style="width:-5px"/></svg>"#,
    );
    assert!(img.pixels.iter().all(|p| *p == 0));
}
#[test]
fn svg_geometry_inapplicable_declarations_do_not_disable_unrelated_path_paint() {
    let img = image(
        r#"<svg width="20" height="20"><path d="M0 0H20V20H0Z" fill="green" style="width:var(--unused);height:100px;rx:calc(1px + 2px)"/></svg>"#,
    );
    assert_eq!(pixel(&img, 10, 10), 0xff008000);
}

#[test]
fn svg_geometry_context_uses_typed_values_without_rewriting_source_and_preserves_refusals() {
    let source = b"<svg width='20' height='10'><rect width='20' height='10' fill='red'/></svg>";
    let mut root = ComputedGeometry::new(16., 16.);
    root.apply(None, "svg", "width", "20", false, true).unwrap();
    root.apply(None, "svg", "height", "10", false, true)
        .unwrap();
    let mut rect = ComputedGeometry::new(16., 16.);
    rect.apply(Some(&root), "rect", "width", "5px", true, false)
        .unwrap();
    rect.apply(Some(&root), "rect", "height", "10px", true, false)
        .unwrap();
    let context = [root.clone(), rect.clone()];
    let image = decode_context(source, None, Some(&context), &|| Ok(())).unwrap();
    assert_eq!((image.width, image.height), (20, 10));
    assert_eq!(pixel(&image, 2, 5), 0xffff0000);
    assert_eq!(pixel(&image, 10, 5), 0);
    assert!(
        decode_context(source, None, Some(&context[..1]), &|| Ok(()))
            .unwrap_err()
            .contains("does not match")
    );
    assert!(
        decode_context(source, None, Some(&context), &|| Err("cancelled".into()))
            .unwrap_err()
            .contains("cancelled")
    );
    rect.mark_unknown("width");
    assert!(
        decode_context(source, None, Some(&[root.clone(), rect]), &|| Ok(()))
            .unwrap_err()
            .contains("unavailable")
    );
    let mut path = ComputedGeometry::new(16., 16.);
    path.mark_unknown("width");
    let path_image = decode_context(
        b"<svg width='20' height='10'><path d='M0 0L10 0L0 10Z' fill='red'/></svg>",
        None,
        Some(&[root.clone(), path]),
        &|| Ok(()),
    )
    .unwrap();
    assert_eq!(
        pixel(&path_image, 2, 2),
        0xffff0000,
        "inapplicable width does not refuse path paint"
    );
    let bad=b"<svg width='20' height='10'><rect width='20' height='10' fill='url(https://unrequested.invalid/p)'/></svg>";
    assert!(
        decode_context(bad, None, Some(&context), &|| Ok(())).is_err(),
        "typed geometry cannot bypass local-paint policy"
    );
}
