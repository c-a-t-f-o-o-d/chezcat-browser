use super::*;
use crate::css::{ImageRendering, ObjectFit};
use crate::image::Bitmap;

// Independent floating-point reference: direct two-dimensional integration of
// source pixel rectangles on shrinking axes, pixel-center bilinear on others.
// It intentionally does not use the painter's separable pass or integer spans.
fn reference(
    bitmap: &Bitmap,
    px: usize,
    py: usize,
    width: usize,
    height: usize,
    background: u32,
) -> u32 {
    fn weights(offset: usize, source: usize, destination: usize) -> Vec<(usize, f64)> {
        if destination < source {
            let left = offset as f64 * source as f64 / destination as f64;
            let right = (offset + 1) as f64 * source as f64 / destination as f64;
            (0..source)
                .filter_map(|index| {
                    let covered = right.min(index as f64 + 1.0) - left.max(index as f64);
                    (covered > 0.0).then_some((index, covered / (right - left)))
                })
                .collect()
        } else {
            let center = ((offset as f64 + 0.5) * source as f64 / destination as f64 - 0.5)
                .clamp(0.0, source as f64 - 1.0);
            let first = center.floor() as usize;
            let fraction = center - first as f64;
            vec![
                (first, 1.0 - fraction),
                ((first + 1).min(source - 1), fraction),
            ]
        }
    }
    let columns = weights(px, bitmap.width, width);
    let rows = weights(py, bitmap.height, height);
    let mut alpha = 0.0;
    let mut rgb = [0.0; 3];
    for (y, wy) in rows {
        for &(x, wx) in &columns {
            let pixel = bitmap.pixels[y * bitmap.width + x];
            let a = (pixel >> 24) as f64 / 255.0;
            alpha += a * wx * wy;
            for (channel, shift) in rgb.iter_mut().zip([0, 8, 16]) {
                *channel += ((pixel >> shift) & 255) as f64 * a * wx * wy;
            }
        }
    }
    let mut output = 0;
    for (value, shift) in rgb.into_iter().zip([0, 8, 16]) {
        let channel = (value + ((background >> shift) & 255) as f64 * (1.0 - alpha)).round() as u32;
        output |= channel << shift;
    }
    output
}
fn assert_channels_close(actual: u32, expected: u32) {
    for shift in [0, 8, 16] {
        let a = ((actual >> shift) & 255) as i32;
        let e = ((expected >> shift) & 255) as i32;
        assert!(
            (a - e).abs() <= 2,
            "actual {actual:06x}, reference {expected:06x}"
        );
    }
}
fn asymmetric() -> Bitmap {
    Bitmap {
        width: 3,
        height: 2,
        pixels: vec![
            0xffea1010, 0x4080f015, 0x0000ffff, 0x802010d0, 0xff305030, 0xffffffff,
        ],
    }
}

#[test]
fn smooth_images_match_independent_alpha_reference_at_larger_smaller_and_natural_sizes() {
    let bitmap = asymmetric();
    for (width, height) in [(3, 2), (9, 7), (2, 1), (1, 1), (7, 1), (1, 8)] {
        let mut canvas = Canvas::new(width, height);
        canvas.fill_rect(0, 0, width as i32, height as i32, 0x324b65);
        canvas.image(0, 0, width as i32, height as i32, &bitmap);
        for y in 0..height {
            for x in 0..width {
                assert_channels_close(
                    canvas.pixels[y * width + x],
                    reference(&bitmap, x, y, width, height, 0x324b65),
                );
            }
        }
    }
}

#[test]
fn smooth_alpha_does_not_mix_hidden_transparent_blue_into_opaque_red() {
    let bitmap = Bitmap {
        width: 2,
        height: 1,
        pixels: vec![0xffff0000, 0x000000ff],
    };
    let mut canvas = Canvas::new(3, 1);
    canvas.fill_rect(0, 0, 3, 1, 0xffffff);
    canvas.image(0, 0, 3, 1, &bitmap);
    assert_eq!(canvas.pixels, vec![0xff0000, 0xff8080, 0xffffff]);
    let mut dark = Canvas::new(3, 1);
    dark.image(0, 0, 3, 1, &bitmap);
    assert_eq!(dark.pixels, vec![0xff0000, 0x800000, 0x000000]);
}

#[test]
fn area_reduction_preserves_fine_pattern_averages_and_transparent_coverage() {
    let stripes = Bitmap {
        width: 12,
        height: 12,
        pixels: (0..144)
            .map(|index| {
                if index % 3 == 0 {
                    0xffffffff
                } else {
                    0xff000000
                }
            })
            .collect(),
    };
    for rendering in [
        ImageRendering::Auto,
        ImageRendering::Smooth,
        ImageRendering::HighQuality,
    ] {
        let mut canvas = Canvas::new(2, 2);
        canvas.image_filtered(0, 0, 2, 2, &stripes, rendering);
        assert_eq!(canvas.pixels, [0x555555; 4]);
    }
    // Explicit pixel-art choices retain their prior sampling, including the
    // smooth fractional stage of Pixelated when its integer expansion is one.
    let mut pixelated = Canvas::new(2, 2);
    pixelated.image_filtered(0, 0, 2, 2, &stripes, ImageRendering::Pixelated);
    assert_eq!(pixelated.pixels, [0x808080; 4]);
    let mut crisp = Canvas::new(2, 2);
    crisp.image_filtered(0, 0, 2, 2, &stripes, ImageRendering::CrispEdges);
    assert_eq!(crisp.pixels, [0xffffff; 4]);
    let transparent = Bitmap {
        width: 12,
        height: 12,
        pixels: (0..144)
            .map(|index| {
                if index % 3 == 0 {
                    0xffff0000
                } else {
                    0x000000ff
                }
            })
            .collect(),
    };
    let mut white = Canvas::new(1, 1);
    white.fill_rect(0, 0, 1, 1, 0xffffff);
    white.image(0, 0, 1, 1, &transparent);
    assert_eq!(white.pixels, [0xffaaaa]);
    let mut black = Canvas::new(1, 1);
    black.image(0, 0, 1, 1, &transparent);
    assert_eq!(black.pixels, [0x550000]);
}

#[test]
fn area_reduction_matches_fractional_coverage_with_mixed_axis_scaling() {
    let bitmap = Bitmap {
        width: 17,
        height: 13,
        pixels: (0..221)
            .map(|index| {
                let x = index % 17;
                let y = index / 17;
                (((x * 47 + y * 59) % 256) << 24)
                    | (((x * 61 + y * 11) % 256) << 16)
                    | (((x * 13 + y * 97) % 256) << 8)
                    | ((x * 71 + y * 29) % 256)
            })
            .collect(),
    };
    for (width, height) in [(5, 4), (4, 25), (31, 3), (16, 12), (1, 40), (41, 1)] {
        let mut canvas = Canvas::new(width, height);
        canvas.fill_rect(0, 0, width as i32, height as i32, 0x324b65);
        canvas.image(0, 0, width as i32, height as i32, &bitmap);
        for y in 0..height {
            for x in 0..width {
                assert_channels_close(
                    canvas.pixels[y * width + x],
                    reference(&bitmap, x, y, width, height, 0x324b65),
                );
            }
        }
    }
}

#[test]
fn area_clipping_keeps_global_coverage_and_bounded_enormous_mixed_extents() {
    let bitmap = Bitmap {
        width: 71,
        height: 43,
        pixels: (0..3053)
            .map(|n| 0xff000000 | ((n * 17717) & 0xffffff))
            .collect(),
    };
    for (width, height) in [(23, 17), (23, 80), (80, 17)] {
        let mut complete = Canvas::new(40, 30);
        complete.fill_rect(0, 0, 40, 30, 0x234567);
        complete.image(-7, -5, width, height, &bitmap);
        let mut clipped = Canvas::new(40, 30);
        clipped.fill_rect(0, 0, 40, 30, 0x234567);
        let clip = ClipRect::new(3, 2, 13, 11);
        clipped.set_clip(Some(clip));
        clipped.image(-7, -5, width, height, &bitmap);
        for y in 0..30 {
            for x in 0..40 {
                assert_eq!(
                    clipped.pixels[y * 40 + x],
                    if clip.contains(x as i32, y as i32) {
                        complete.pixels[y * 40 + x]
                    } else {
                        0x234567
                    }
                );
            }
        }
    }
    let mut canvas = Canvas::new(8, 8);
    canvas.image(-999_999_990, 0, 2_000_000_000, 1, &bitmap);
    canvas.image(0, -999_999_990, 1, 2_000_000_000, &bitmap);
    assert!(canvas.pixels.iter().any(|&pixel| pixel != 0));
}

#[test]
fn pixelated_uses_virtual_integer_expansion_and_crisp_edges_uses_nearest_centers() {
    let bitmap = asymmetric();
    let mut expanded = Bitmap {
        width: 6,
        height: 4,
        pixels: Vec::new(),
    };
    for y in 0..4 {
        for x in 0..6 {
            expanded.pixels.push(bitmap.pixels[(y / 2) * 3 + x / 2]);
        }
    }
    let mut canvas = Canvas::new(7, 5);
    canvas.fill_rect(0, 0, 7, 5, 0x123456);
    canvas.image_filtered(0, 0, 7, 5, &bitmap, ImageRendering::Pixelated);
    for y in 0..5 {
        for x in 0..7 {
            assert_channels_close(
                canvas.pixels[y * 7 + x],
                reference(&expanded, x, y, 7, 5, 0x123456),
            );
        }
    }
    let opaque = Bitmap {
        width: 2,
        height: 1,
        pixels: vec![0xffff0000, 0xff0000ff],
    };
    let mut sharp = Canvas::new(5, 1);
    sharp.image_filtered(0, 0, 5, 1, &opaque, ImageRendering::CrispEdges);
    assert_eq!(
        sharp.pixels,
        vec![0xff0000, 0xff0000, 0x0000ff, 0x0000ff, 0x0000ff]
    );
    let mut exact = Canvas::new(6, 3);
    exact.image_filtered(0, 0, 6, 3, &opaque, ImageRendering::Pixelated);
    assert_eq!(
        &exact.pixels[..6],
        &[0xff0000, 0xff0000, 0xff0000, 0x0000ff, 0x0000ff, 0x0000ff]
    );
}

#[test]
fn filtered_image_clipping_and_object_fit_preserve_global_sampling_coordinates() {
    let bitmap = asymmetric();
    let mut complete = Canvas::new(16, 12);
    complete.fill_rect(0, 0, 16, 12, 0x234567);
    complete.image(-3, -2, 19, 15, &bitmap);
    let mut clipped = Canvas::new(16, 12);
    clipped.fill_rect(0, 0, 16, 12, 0x234567);
    let clip = ClipRect::new(4, 3, 7, 5);
    clipped.set_clip(Some(clip));
    clipped.image(-3, -2, 19, 15, &bitmap);
    for y in 0..12 {
        for x in 0..16 {
            assert_eq!(
                clipped.pixels[y * 16 + x],
                if clip.contains(x as i32, y as i32) {
                    complete.pixels[y * 16 + x]
                } else {
                    0x234567
                }
            );
        }
    }
    let mut fit = Canvas::new(12, 12);
    fit.fill_rect(0, 0, 12, 12, 0x234567);
    fit.fitted_image(
        ClipRect::new(2, 2, 8, 8),
        &bitmap,
        ImagePresentation {
            fit: ObjectFit::Cover,
            rendering: ImageRendering::Smooth,
            position: [(0.5, 0.0); 2],
            natural: (3.0, 2.0),
        },
    );
    let mut expected = Canvas::new(12, 12);
    expected.fill_rect(0, 0, 12, 12, 0x234567);
    expected.set_clip(Some(ClipRect::new(2, 2, 8, 8)));
    expected.image(0, 2, 12, 8, &bitmap);
    assert_eq!(fit.pixels, expected.pixels);
}

#[test]
fn enormous_css_expansion_is_bounded_by_visible_pixels_and_invalid_bitmaps_are_ignored() {
    let bitmap = asymmetric();
    let mut canvas = Canvas::new(32, 32);
    canvas.set_clip(Some(ClipRect::new(3, 3, 4, 4)));
    let start = std::time::Instant::now();
    for _ in 0..200 {
        canvas.image_filtered(
            -999_999_990,
            -999_999_990,
            2_000_000_000,
            2_000_000_000,
            &bitmap,
            ImageRendering::Pixelated,
        );
    }
    assert!(
        start.elapsed() < std::time::Duration::from_secs(3),
        "a tiny visible clip must not walk the CSS-sized virtual bitmap"
    );
    let before = canvas.pixels.clone();
    canvas.image(
        0,
        0,
        4,
        4,
        &Bitmap {
            width: usize::MAX,
            height: 3,
            pixels: vec![0],
        },
    );
    canvas.image(
        0,
        0,
        4,
        4,
        &Bitmap {
            width: 3,
            height: 3,
            pixels: vec![0],
        },
    );
    assert_eq!(canvas.pixels, before);
}
