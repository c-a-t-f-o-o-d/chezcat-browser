use super::*;
use crate::css::{BackgroundSize, ImageRendering};
use crate::image::Bitmap;

fn presentation() -> BackgroundPresentation {
    BackgroundPresentation {
        size: BackgroundSize::Auto,
        repeat: [true, true],
        position: [(0.0, 0.0); 2],
        scale: 1.0,
        rendering: ImageRendering::CrispEdges,
    }
}

fn corners() -> Bitmap {
    Bitmap {
        width: 2,
        height: 2,
        pixels: vec![0xffff0000, 0xff00ff00, 0xff0000ff, 0xffffffff],
    }
}

#[test]
fn background_repeats_original_pixels_and_negative_offsets_wrap_euclidean() {
    let bitmap = corners();
    for offset in [0.0, -1.0, -5.0] {
        let mut canvas = Canvas::new(7, 5);
        let mut style = presentation();
        style.position = [(0.0, offset); 2];
        canvas.background_image(
            ClipRect::new(0, 0, 7, 5),
            ClipRect::new(0, 0, 7, 5),
            &bitmap,
            style,
        );
        for y in 0..5 {
            for x in 0..7 {
                let sx = (x as i32 - offset as i32).rem_euclid(2) as usize;
                let sy = (y as i32 - offset as i32).rem_euclid(2) as usize;
                assert_eq!(
                    canvas.pixels[y * 7 + x],
                    bitmap.pixels[sy * 2 + sx] & 0xffffff,
                    "{offset}/{x}/{y}"
                );
            }
        }
    }
}

#[test]
fn background_axis_repetition_does_not_fill_the_nonrepeating_axis() {
    let bitmap = corners();
    for repeat in [[false, false], [true, false], [false, true]] {
        let mut canvas = Canvas::new(6, 5);
        canvas.pixels.fill(0x123456);
        let mut style = presentation();
        style.repeat = repeat;
        canvas.background_image(
            ClipRect::new(0, 0, 6, 5),
            ClipRect::new(1, 1, 4, 3),
            &bitmap,
            style,
        );
        for y in 0..5 {
            for x in 0..6 {
                let inside =
                    (repeat[0] || (1..3).contains(&x)) && (repeat[1] || (1..3).contains(&y));
                let expected = if inside {
                    bitmap.pixels[(y as i32 - 1).rem_euclid(2) as usize * 2
                        + (x as i32 - 1).rem_euclid(2) as usize]
                        & 0xffffff
                } else {
                    0x123456
                };
                assert_eq!(canvas.pixels[y * 6 + x], expected, "{repeat:?}/{x}/{y}");
            }
        }
    }
}

#[test]
fn background_uses_padding_origin_border_clip_and_preserves_existing_canvas_clip() {
    let bitmap = corners();
    let mut canvas = Canvas::new(8, 8);
    canvas.pixels.fill(0x123456);
    let clip = ClipRect::new(3, 1, 3, 4);
    canvas.set_clip(Some(clip));
    let mut style = presentation();
    style.repeat = [false, false];
    canvas.background_image(
        ClipRect::new(1, 1, 5, 5),
        ClipRect::new(2, 2, 3, 3),
        &bitmap,
        style,
    );
    assert_eq!(canvas.clip(), clip);
    for y in 0..8 {
        for x in 0..8 {
            let expected = match (x, y) {
                (3, 2) => 0x00ff00,
                (3, 3) => 0xffffff,
                _ => 0x123456,
            };
            assert_eq!(canvas.pixels[y * 8 + x], expected, "{x}/{y}");
        }
    }
}

#[test]
fn background_alpha_blends_over_color_without_transparent_rgb_leak() {
    let bitmap = Bitmap {
        width: 2,
        height: 1,
        pixels: vec![0x80ff0000, 0x0000ff00],
    };
    let mut canvas = Canvas::new(2, 1);
    canvas.pixels.fill(0x0000ff);
    let mut style = presentation();
    style.rendering = ImageRendering::Auto;
    canvas.background_image(
        ClipRect::new(0, 0, 2, 1),
        ClipRect::new(0, 0, 2, 1),
        &bitmap,
        style,
    );
    assert_eq!(canvas.pixels, [0x80007f, 0x0000ff]);
}

#[test]
fn background_contain_cover_and_auto_zoom_use_the_positioning_area() {
    let bitmap = Bitmap {
        width: 2,
        height: 1,
        pixels: vec![0xffff0000, 0xff00ff00],
    };
    for (size, rows) in [
        (BackgroundSize::Contain, 1..3),
        (BackgroundSize::Cover, 0..4),
    ] {
        let mut canvas = Canvas::new(4, 4);
        canvas.pixels.fill(0x123456);
        let mut style = presentation();
        style.size = size;
        style.repeat = [false, false];
        style.position = [(0.5, 0.0); 2];
        canvas.background_image(
            ClipRect::new(0, 0, 4, 4),
            ClipRect::new(0, 0, 4, 4),
            &bitmap,
            style,
        );
        for y in 0..4 {
            for x in 0..4 {
                let expected = if rows.contains(&y) {
                    if x < 2 {
                        0xff0000
                    } else {
                        0x00ff00
                    }
                } else {
                    0x123456
                };
                assert_eq!(canvas.pixels[y * 4 + x], expected, "{size:?}/{x}/{y}");
            }
        }
    }
    let mut canvas = Canvas::new(6, 3);
    canvas.pixels.fill(0x123456);
    let mut style = presentation();
    style.repeat = [false, false];
    style.scale = 2.0;
    canvas.background_image(
        ClipRect::new(0, 0, 6, 3),
        ClipRect::new(0, 0, 6, 3),
        &bitmap,
        style,
    );
    assert_eq!(
        canvas.pixels,
        [
            0xff0000, 0xff0000, 0x00ff00, 0x00ff00, 0x123456, 0x123456, 0xff0000, 0xff0000,
            0x00ff00, 0x00ff00, 0x123456, 0x123456, 0x123456, 0x123456, 0x123456, 0x123456,
            0x123456, 0x123456,
        ]
    );
}

#[test]
fn background_rejects_invalid_extents_and_nonfinite_geometry_without_mutation() {
    let mut canvas = Canvas::new(3, 3);
    canvas.pixels.fill(0x123456);
    for bitmap in [
        Bitmap {
            width: 2,
            height: 2,
            pixels: vec![0xffffffff],
        },
        Bitmap {
            width: 0,
            height: 1,
            pixels: vec![],
        },
        Bitmap {
            width: usize::MAX,
            height: 2,
            pixels: vec![0xffffffff],
        },
        Bitmap {
            width: 16_385,
            height: 1,
            pixels: vec![0xffffffff],
        },
    ] {
        canvas.background_image(
            ClipRect::new(0, 0, 3, 3),
            ClipRect::new(0, 0, 3, 3),
            &bitmap,
            presentation(),
        );
    }
    for (scale, position) in [
        (f32::NAN, 0.0),
        (f32::INFINITY, 0.0),
        (0.0, 0.0),
        (1.0, f32::NAN),
        (1.0, f32::MAX),
        (f32::MAX, 0.0),
    ] {
        let mut style = presentation();
        style.scale = scale;
        style.position[0].1 = position;
        canvas.background_image(
            ClipRect::new(0, 0, 3, 3),
            ClipRect::new(0, 0, 3, 3),
            &corners(),
            style,
        );
    }
    assert!(canvas.pixels.iter().all(|pixel| *pixel == 0x123456));
}

#[test]
fn background_extreme_clip_only_visits_the_small_surface() {
    let mut canvas = Canvas::new(2, 2);
    canvas.background_image(
        ClipRect {
            left: i32::MIN,
            top: i32::MIN,
            right: i32::MAX,
            bottom: i32::MAX,
        },
        ClipRect::new(0, 0, 2, 2),
        &corners(),
        presentation(),
    );
    assert_eq!(canvas.pixels, [0xff0000, 0x00ff00, 0x0000ff, 0xffffff]);
}
