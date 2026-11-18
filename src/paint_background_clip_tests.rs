use super::*;

fn canvas() -> Canvas {
    let mut canvas = Canvas::new(84, 44);
    canvas.fill_rect(0, 0, 84, 44, 0xc86432);
    canvas
}
fn border(width: i32, alpha: u8) -> UniformBorder {
    UniformBorder {
        width,
        color: 0,
        alpha,
    }
}

#[test]
fn background_clip_canvas_original_nonwhite_alpha_and_square_reference() {
    let mut actual = canvas();
    actual.fill_color_box(
        2,
        2,
        80,
        40,
        0.0,
        0x64c832,
        255,
        [4; 4],
        Some(border(4, 51)),
    );
    let mut literal = canvas();
    literal.fill_rect(6, 6, 72, 32, 0x64c832);
    for (x, y, w, h) in border_segments(2, 2, 80, 40, [4; 4], [51; 4]) {
        literal.fill_rect_alpha(x, y, w, h, 0, 51);
    }
    assert_eq!(actual.pixels, literal.pixels);
    assert_eq!(actual.pixels[2 * 84 + 42], 0xa05028);
    assert_eq!(actual.pixels[22 * 84 + 42], 0x64c832);
    let mut default = canvas();
    let mut previous = canvas();
    default.fill_color_box(
        2,
        2,
        80,
        40,
        8.0,
        0x64c832,
        255,
        [0; 4],
        Some(border(4, 51)),
    );
    previous.fill_rounded_border_rect(2, 2, 80, 40, 8.0, 0x64c832, border(4, 51));
    assert_eq!(
        default.pixels, previous.pixels,
        "Default clipping remains byte-identical"
    );
}

#[test]
fn background_clip_canvas_rounded_single_composite_and_padding_hole() {
    let mut actual = canvas();
    actual.fill_color_box(
        2,
        2,
        80,
        40,
        8.0,
        0x64c832,
        255,
        [4; 4],
        Some(border(4, 51)),
    );
    assert_eq!(actual.pixels[4 * 84 + 4], 0xab562b,
        "Outer coverage184 with alpha51 covers only the original backdrop; no green under the border");
    assert_eq!(actual.pixels[2 * 84 + 42], 0xa05028);
    assert_eq!(actual.pixels[22 * 84 + 42], 0x64c832);
    let mut content = canvas();
    content.fill_color_box(
        2,
        2,
        80,
        40,
        8.0,
        0x64c832,
        255,
        [8; 4],
        Some(border(4, 51)),
    );
    assert_eq!(
        content.pixels[8 * 84 + 42],
        0xc86432,
        "Padding inside the border stays unpainted"
    );
    assert_eq!(content.pixels[10 * 84 + 42], 0x64c832);
    let mut equal = canvas();
    let mut reference = canvas();
    equal.fill_color_box(
        2,
        2,
        80,
        40,
        8.0,
        0x1464dc,
        255,
        [4; 4],
        Some(UniformBorder {
            width: 4,
            color: 0x1464dc,
            alpha: 255,
        }),
    );
    reference.fill_rounded_rect_alpha(2, 2, 80, 40, 8.0, 0x1464dc, 255);
    assert_eq!(
        equal.pixels, reference.pixels,
        "Disjoint opaque materials share an antialiased edge without seams"
    );
}

#[test]
fn background_clip_canvas_zero_transparent_exhausted_alpha_and_ancestor_clip() {
    for edge in [border(0, 255), border(4, 0)] {
        let mut actual = canvas();
        actual.fill_color_box(2, 2, 80, 40, 8.0, 0x64c832, 128, [8; 4], Some(edge));
        assert_eq!(actual.pixels[8 * 84 + 42], 0xc86432);
        assert_eq!(actual.pixels[22 * 84 + 42], 0x969632);
    }
    let mut exhausted = canvas();
    exhausted.fill_color_box(
        2,
        2,
        80,
        40,
        1000.0,
        0x64c832,
        255,
        [40; 4],
        Some(border(40, 51)),
    );
    assert_eq!(exhausted.pixels[22 * 84 + 42], 0xa05028);
    let mut clipped = canvas();
    let clip = ClipRect::new(9, 8, 20, 15);
    clipped.set_clip(Some(clip));
    clipped.fill_color_box(
        2,
        2,
        80,
        40,
        8.0,
        0x64c832,
        255,
        [4; 4],
        Some(border(4, 51)),
    );
    assert_eq!(clipped.clip(), clip);
    for y in 0..44 {
        for x in 0..84 {
            if !clip.contains(x, y) {
                assert_eq!(clipped.pixels[y as usize * 84 + x as usize], 0xc86432);
            }
        }
    }
}
