use super::*;

fn canvas(width: usize, height: usize, color: u32) -> Canvas {
    let mut canvas = Canvas::new(width, height);
    canvas.fill_rect(0, 0, width as i32, height as i32, color);
    canvas
}

#[test]
fn rounded_border_canvas_equal_material_is_invariant_across_alpha_and_collapsed_interiors() {
    let (destination, material) = (0xc86432, 0x1464dc);
    let mut reference = canvas(24, 20, destination);
    reference.fill_rounded_rect_alpha(2, 2, 20, 16, 8.0, material, 255);
    assert!(reference.pixels.iter().any(|pixel| *pixel != destination && *pixel != material),
        "The independent material invariant must include partial edge coverage");
    for width in [0, 1, 8, 40] { for alpha in [0, 128, 255] {
        let mut actual = canvas(24, 20, destination);
        actual.fill_rounded_border_rect(2, 2, 20, 16, 8.0, material,
            UniformBorder { width, color: material, alpha });
        assert_eq!(actual.pixels, reference.pixels,
            "Equal border/background RGB cannot change any boundary material: width={width}, alpha={alpha}");
    }}
}

#[test]
fn rounded_border_canvas_nonwhite_boundary_and_square_zero_match_independent_controls() {
    let mut actual = canvas(84, 44, 0xc86432);
    actual.fill_rounded_border_rect(2, 2, 80, 40, 8.0, 0x64c832,
        UniformBorder { width: 4, color: 0, alpha: 51 });
    assert_eq!(actual.pixels[2 * 84 + 2], 0xc86432);
    assert_eq!(actual.pixels[2 * 84 + 42], 0x50a028);
    assert_eq!(actual.pixels[22 * 84 + 42], 0x64c832);
    // Circle center(8,8), pixel center(2.5,2.5) gives outer coverage184/255;
    // inner coverage0. Border-over-background material is(80,160,40), then
    // that material covers184/255 of the actual backdrop(200,100,50).
    assert_eq!(actual.pixels[4 * 84 + 4], 0x718f2b);
    let mut square = canvas(84, 44, 0xc86432);
    let mut previous = canvas(84, 44, 0xc86432);
    square.fill_rounded_border_rect(2, 2, 80, 40, 0.0, 0x64c832,
        UniformBorder { width: 4, color: 0, alpha: 51 });
    previous.fill_rect(2, 2, 80, 40, 0x64c832);
    for (x, y, w, h) in border_segments(2, 2, 80, 40, [4; 4], [51; 4]) {
        previous.fill_rect_alpha(x, y, w, h, 0, 51);
    }
    assert_eq!(square.pixels, previous.pixels, "Zero radius preserves the existing rectangular result");
}

#[test]
fn rounded_border_canvas_square_inner_corner_zero_interior_overlap_and_clip() {
    let mut thick = canvas(20, 16, 0xc86432);
    thick.fill_rounded_border_rect(0, 0, 20, 16, 2.0, 0x64c832,
        UniformBorder { width: 3, color: 0, alpha: 51 });
    assert_eq!(thick.pixels[3 * 20 + 3], 0x64c832, "Width>=radius gives a square inner corner");
    assert_eq!(thick.pixels[3 * 20 + 2], 0x50a028);
    let mut exhausted = canvas(20, 16, 0xc86432);
    let mut reduced = canvas(20, 16, 0xc86432);
    let border = UniformBorder { width: 8, color: 0, alpha: 51 };
    exhausted.fill_rounded_border_rect(0, 0, 20, 16, 1000.0, 0x64c832, border);
    reduced.fill_rounded_border_rect(0, 0, 20, 16, 8.0, 0x64c832, border);
    assert_eq!(exhausted.pixels, reduced.pixels, "Normalize the outer radius before deriving the inner one");
    assert_eq!(exhausted.pixels[8 * 20 + 10], 0x50a028, "A collapsed inner box has no invented interior coverage");

    let mut clipped = canvas(16, 16, 0xc86432);
    let clip = ClipRect::new(3, 2, 8, 7);
    clipped.set_clip(Some(clip));
    clipped.fill_rounded_border_rect(1, 1, 12, 12, 6.0, 0x64c832,
        UniformBorder { width: 2, color: 0, alpha: 51 });
    assert_eq!(clipped.clip(), clip);
    for y in 0..16 { for x in 0..16 {
        if !clip.contains(x, y) { assert_eq!(clipped.pixels[y as usize * 16 + x as usize], 0xc86432); }
    }}
    let before = clipped.pixels.clone();
    for (x, y, w, h, radius) in [(i32::MIN, i32::MIN, 20, 20, 8.0),
        (i32::MAX, i32::MAX, i32::MAX, i32::MAX, 8.0), (0, 0, 12, 12, f32::NAN)] {
        clipped.fill_rounded_border_rect(x, y, w, h, radius, 0x64c832, border);
    }
    assert_eq!(clipped.pixels, before);
}
