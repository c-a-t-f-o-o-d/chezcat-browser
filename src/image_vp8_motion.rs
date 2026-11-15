//! Original finite VP8 motion primitives from RFC 6386 sections 16–18.
//! Only format-defined numeric probability/filter tables are transcribed.
//! No external implementation, FFI, allocator, reference-frame owner or codec.

type Result<T> = std::result::Result<T, String>;

/// Signed eighth-luma-pixel units (also used for already-derived chroma MVs).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Mv {
    pub row: i32,
    pub col: i32,
}
impl Mv {
    fn valid(self) -> bool {
        (-32768..=32767).contains(&self.row) && (-32768..=32767).contains(&self.col)
    }
    /// Single-MV chroma conversion; signed nearest-half, ties away from zero.
    pub(super) fn chroma(self) -> Result<Self> {
        if !self.valid() {
            return Err("VP8 motion vector exceeds the supported signed range.".into());
        }
        let half = |value: i32| (value + value.signum()) / 2;
        Ok(Self {
            row: half(self.row),
            col: half(self.col),
        })
    }
}

// RFC 6386 §17.2: rows first, columns second; nineteen positions each.
pub(super) const MV_DEFAULTS: [[u8; 19]; 2] = [
    [
        162, 128, 225, 146, 172, 147, 214, 39, 156, 128, 129, 132, 75, 145, 178, 206, 239, 254, 254,
    ],
    [
        164, 128, 204, 170, 119, 235, 140, 230, 228, 128, 130, 130, 74, 148, 180, 203, 236, 254,
        254,
    ],
];
pub(super) const MV_UPDATES: [[u8; 19]; 2] = [
    [
        237, 246, 253, 253, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 250, 250, 252, 254,
        254,
    ],
    [
        231, 243, 245, 253, 254, 254, 254, 254, 254, 254, 254, 254, 254, 254, 251, 251, 254, 254,
        254,
    ],
];
// RFC 6386 §16.3: weighted zero/nearest/near/split context columns.
pub(super) const MODE_CONTEXTS: [[u8; 4]; 6] = [
    [7, 1, 1, 143],
    [14, 18, 14, 107],
    [135, 64, 57, 68],
    [60, 56, 128, 65],
    [159, 134, 128, 34],
    [234, 188, 128, 28],
];

/// Decode one signed quarter-luma-pixel differential. The caller's bit reader
/// owns partition truncation, arithmetic decoding, work and cancellation.
pub(super) fn read_component(
    prob: &[u8; 19],
    mut read: impl FnMut(u8) -> Result<bool>,
) -> Result<i32> {
    let magnitude = if read(prob[0])? {
        let mut value = 0;
        for position in [0usize, 1, 2, 9, 8, 7, 6, 5, 4] {
            if read(prob[9 + position])? {
                value |= 1 << position;
            }
        }
        if value < 16 || read(prob[12])? {
            value |= 8;
        }
        value
    } else {
        // Three binary decisions; each prefix selects its format probability.
        let high = usize::from(read(prob[2])?);
        let middle = usize::from(read(prob[if high == 0 { 3 } else { 6 }])?);
        let low_probability = (if high == 0 { 4 } else { 7 }) + middle;
        let low = usize::from(read(prob[low_probability])?);
        (high * 4 + middle * 2 + low) as i32
    };
    Ok(if magnitude != 0 && read(prob[1])? {
        -magnitude
    } else {
        magnitude
    })
}

/// Read row before column and convert each quarter-luma differential to the
/// stored eighth-luma units. Do not double this result again in the decoder.
pub(super) fn read_delta(
    prob: &[[u8; 19]; 2],
    mut read: impl FnMut(u8) -> Result<bool>,
) -> Result<Mv> {
    let row = read_component(&prob[0], &mut read)? * 2;
    let col = read_component(&prob[1], &mut read)? * 2;
    Ok(Mv { row, col })
}

/// Stage all optional updates; a truncated partition never publishes a prefix.
/// Frame entropy-refresh semantics and committing this table remain external.
pub(super) fn update_probabilities(
    prob: &mut [[u8; 19]; 2],
    mut read: impl FnMut(u8) -> Result<bool>,
) -> Result<()> {
    let mut next = *prob;
    for component in 0..2 {
        for position in 0..19 {
            if read(MV_UPDATES[component][position])? {
                let mut value = 0u8;
                for _ in 0..7 {
                    value = value * 2 + u8::from(read(128)?);
                }
                next[component][position] = if value == 0 { 1 } else { value * 2 };
            }
        }
    }
    *prob = next;
    Ok(())
}

/// Reference identifiers: zero intra, one LAST, two GOLDEN, three ALTREF.
/// Unavailable border neighbors are None, not inter-coded zero voters.
#[derive(Clone, Copy, Debug)]
pub(super) struct Neighbor {
    pub reference: u8,
    pub mv: Mv,
    pub split: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Near {
    pub nearest: Mv,
    pub near: Mv,
    pub best: Mv,
    pub counts: [u8; 4],
    pub probabilities: [u8; 4],
}

/// Survey above, left, above-left. Sign bias precedes comparison; equal
/// nonzero candidates merge their weights and ties preserve encounter order.
/// Returned predictors are RAW: the caller must apply the format's per-MB
/// predictor bounds before NEAREST/NEAR or adding a NEWMV differential.
pub(super) fn near_candidates(
    neighbors: [Option<Neighbor>; 3],
    reference: u8,
    sign_bias: &[bool; 4],
) -> Result<Near> {
    if !(1..=3).contains(&reference) {
        return Err("VP8 inter prediction requires a real reference frame.".into());
    }
    let mut zero = 0u8;
    let mut split = 0u8;
    let mut candidates = [(Mv::default(), 0u8); 3];
    let mut used = 0;
    for (neighbor, weight) in neighbors.into_iter().zip([2u8, 2, 1]) {
        let Some(neighbor) = neighbor else {
            continue;
        };
        if neighbor.reference > 3
            || !neighbor.mv.valid()
            || neighbor.reference == 0 && neighbor.split
        {
            return Err("Invalid VP8 neighboring motion record.".into());
        }
        if neighbor.split {
            split += weight;
        }
        if neighbor.reference == 0 {
            continue;
        }
        let mut mv = neighbor.mv;
        if sign_bias[usize::from(neighbor.reference)] != sign_bias[usize::from(reference)] {
            // A valid stored -32768 becomes +32768 before the caller's
            // predictor clamp. Keep this intermediate wide; never wrap it.
            mv = Mv {
                row: mv
                    .row
                    .checked_neg()
                    .ok_or("VP8 motion sign-bias overflow.")?,
                col: mv
                    .col
                    .checked_neg()
                    .ok_or("VP8 motion sign-bias overflow.")?,
            };
        }
        if mv == Mv::default() {
            zero += weight;
            continue;
        }
        if let Some(index) = (0..used).find(|&index| candidates[index].0 == mv) {
            candidates[index].1 += weight;
        } else {
            candidates[used] = (mv, weight);
            used += 1;
        }
    }
    // Stable insertion sort over at most three weighted candidates.
    for index in 1..used {
        let mut at = index;
        while at > 0 && candidates[at].1 > candidates[at - 1].1 {
            candidates.swap(at, at - 1);
            at -= 1;
        }
    }
    let nearest = candidates[0].0;
    let near = candidates[1].0;
    let counts = [zero, candidates[0].1, candidates[1].1, split];
    let mut probabilities = [0; 4];
    for column in 0..4 {
        probabilities[column] = MODE_CONTEXTS[usize::from(counts[column])][column];
    }
    Ok(Near {
        nearest,
        near,
        best: if counts[1] >= zero {
            nearest
        } else {
            Mv::default()
        },
        counts,
        probabilities,
    })
}

// Format coefficients, RFC 6386 §18.3 (version-0 reconstruction).
const SIX_TAP: [[i32; 6]; 8] = [
    [0, 0, 128, 0, 0, 0],
    [0, -6, 123, 12, -1, 0],
    [2, -11, 108, 36, -8, 1],
    [0, -9, 93, 50, -6, 0],
    [3, -16, 77, 77, -16, 3],
    [0, -6, 50, 93, -9, 0],
    [1, -8, 36, 108, -11, 2],
    [0, -1, 12, 123, -6, 0],
];
fn rounded_pixel(sum: i32) -> u8 {
    ((sum + 64) >> 7).clamp(0, 255) as u8
}

/// Predict one complete 16x16 luma or 8x8 chroma block. Width/height describe
/// the actual padded reconstruction plane, excluding any stride-only padding.
/// Caller must first replicate visible edges through nonvisible right/bottom
/// macroblock padding; arbitrary reconstructed offscreen pixels are not a halo.
/// `mv` is already in eighths of THIS plane's pixels; derive UV with chroma().
/// `dest` starts at the output block's top-left. Each synthesized pixel calls
/// check: wrap cancellation AND the caller's checked work charge in it.
/// Fixed scratch space and staged writes make every error leave dest intact.
#[allow(clippy::too_many_arguments)]
pub(super) fn predict_into(
    source: &[u8],
    stride: usize,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    size: usize,
    mv: Mv,
    dest: &mut [u8],
    dest_stride: usize,
    check: &dyn Fn() -> Result<()>,
) -> Result<()> {
    check()?;
    if !matches!(size, 8 | 16)
        || width == 0
        || height == 0
        || width > 16384
        || height > 16384
        || stride < width
        || dest_stride < size
        || !mv.valid()
        || x.checked_add(size).is_none_or(|end| end > width)
        || y.checked_add(size).is_none_or(|end| end > height)
    {
        return Err("Invalid bounded VP8 motion prediction geometry.".into());
    }
    let needed_source = (height - 1)
        .checked_mul(stride)
        .and_then(|n| n.checked_add(width))
        .ok_or("VP8 reference plane layout overflow.")?;
    let needed_dest = (size - 1)
        .checked_mul(dest_stride)
        .and_then(|n| n.checked_add(size))
        .ok_or("VP8 prediction destination layout overflow.")?;
    if source.len() < needed_source || dest.len() < needed_dest {
        return Err("Truncated VP8 motion prediction plane.".into());
    }
    let origin_x = x as i64 + i64::from(mv.col.div_euclid(8));
    let origin_y = y as i64 + i64::from(mv.row.div_euclid(8));
    let horizontal = &SIX_TAP[mv.col.rem_euclid(8) as usize];
    let vertical = &SIX_TAP[mv.row.rem_euclid(8) as usize];
    // Six samples per output: two before, three after. Extending by per-sample
    // edge replication avoids forming pointers into unallocated border memory.
    let mut intermediate = [0u8; 21 * 16];
    for row in 0..size + 5 {
        let sy = (origin_y + row as i64 - 2).clamp(0, height as i64 - 1) as usize;
        for col in 0..size {
            check()?;
            let mut sum = 0;
            for (tap, coefficient) in horizontal.iter().enumerate() {
                let sx =
                    (origin_x + col as i64 + tap as i64 - 2).clamp(0, width as i64 - 1) as usize;
                sum += i32::from(source[sy * stride + sx]) * coefficient;
            }
            intermediate[row * 16 + col] = rounded_pixel(sum);
        }
    }
    let mut output = [0u8; 16 * 16];
    for row in 0..size {
        for col in 0..size {
            check()?;
            let sum = vertical
                .iter()
                .enumerate()
                .map(|(tap, coefficient)| {
                    i32::from(intermediate[(row + tap) * 16 + col]) * coefficient
                })
                .sum();
            output[row * 16 + col] = rounded_pixel(sum);
        }
    }
    check()?;
    for row in 0..size {
        dest[row * dest_stride..row * dest_stride + size]
            .copy_from_slice(&output[row * 16..row * 16 + size]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn scripted(prob: &[u8; 19], steps: &[(usize, bool)]) -> i32 {
        let mut at = 0;
        let value = read_component(prob, |p| {
            let &(index, bit) = steps.get(at).ok_or("unexpected component read")?;
            assert_eq!(p, prob[index], "component probability order at {at}");
            at += 1;
            Ok(bit)
        })
        .unwrap();
        assert_eq!(at, steps.len());
        value
    }
    #[test]
    fn motion_component_short_long_sign_and_implicit_bit_have_exact_reads() {
        let prob = std::array::from_fn(|index| index as u8 + 1);
        assert_eq!(
            scripted(&prob, &[(0, false), (2, false), (3, false), (4, false)]),
            0
        );
        assert_eq!(
            scripted(
                &prob,
                &[(0, false), (2, true), (6, true), (8, true), (1, true)]
            ),
            -7
        );
        assert_eq!(
            scripted(
                &prob,
                &[
                    (0, true),
                    (9, false),
                    (10, false),
                    (11, false),
                    (18, false),
                    (17, false),
                    (16, false),
                    (15, false),
                    (14, false),
                    (13, false),
                    (1, false)
                ]
            ),
            8
        );
        assert_eq!(
            scripted(
                &prob,
                &[
                    (0, true),
                    (9, false),
                    (10, false),
                    (11, false),
                    (18, false),
                    (17, false),
                    (16, false),
                    (15, false),
                    (14, false),
                    (13, true),
                    (12, false),
                    (1, false)
                ]
            ),
            16
        );
        assert_eq!(
            scripted(
                &prob,
                &[
                    (0, true),
                    (9, true),
                    (10, true),
                    (11, true),
                    (18, true),
                    (17, true),
                    (16, true),
                    (15, true),
                    (14, true),
                    (13, true),
                    (12, true),
                    (1, true)
                ]
            ),
            -1023
        );
        assert!(read_component(&prob, |_| Err("original reader exhausted".into())).is_err());
    }
    #[test]
    fn motion_delta_reads_row_then_column_and_scales_quarter_pixels_once() {
        let probabilities = [[17; 19], [91; 19]];
        let bits = [
            false, true, true, true, true, false, false, true, false, false,
        ];
        let mut at = 0;
        let delta = read_delta(&probabilities, |probability| {
            assert_eq!(probability, if at < 5 { 17 } else { 91 });
            let bit = bits
                .get(at)
                .copied()
                .ok_or("unexpected original delta bit")?;
            at += 1;
            Ok(bit)
        })
        .unwrap();
        assert_eq!(at, bits.len());
        assert_eq!(delta, Mv { row: -14, col: 4 });
    }
    #[test]
    fn motion_probability_update_is_atomic_and_zero_maps_to_one() {
        let mut prob = MV_DEFAULTS;
        let mut calls = 0;
        update_probabilities(&mut prob, |p| {
            assert_eq!(p, MV_UPDATES[calls / 19][calls % 19]);
            calls += 1;
            Ok(false)
        })
        .unwrap();
        assert_eq!(calls, 38);
        assert_eq!(prob, MV_DEFAULTS);
        let mut calls = 0;
        assert!(update_probabilities(&mut prob, |_| {
            calls += 1;
            if calls == 12 {
                Err("truncated original update".into())
            } else {
                Ok(true)
            }
        })
        .is_err());
        assert_eq!(prob, MV_DEFAULTS);
        let mut calls = 0;
        update_probabilities(&mut prob, |_| {
            calls += 1;
            Ok(calls == 1)
        })
        .unwrap();
        assert_eq!(prob[0][0], 1);
        assert_eq!(prob[0][1..], MV_DEFAULTS[0][1..]);
        assert_eq!(prob[1], MV_DEFAULTS[1]);
    }
    #[test]
    fn motion_near_context_merges_biases_stable_ties_and_zero_census() {
        let a = Mv { row: 8, col: -16 };
        let b = Mv { row: -24, col: 32 };
        let n = |mv| {
            Some(Neighbor {
                reference: 1,
                mv,
                split: false,
            })
        };
        let context = near_candidates([n(a), n(b), n(a)], 1, &[false; 4]).unwrap();
        assert_eq!((context.nearest, context.near, context.best), (a, b, a));
        assert_eq!(context.counts, [0, 3, 2, 0]);
        assert_eq!(context.probabilities, [7, 56, 57, 143]);
        assert_eq!(
            near_candidates([n(a), n(b), None], 1, &[false; 4])
                .unwrap()
                .nearest,
            a
        );
        let zero =
            near_candidates([n(Mv::default()), n(a), n(Mv::default())], 1, &[false; 4]).unwrap();
        assert_eq!(zero.counts, [3, 2, 0, 0]);
        assert_eq!(zero.best, Mv::default());
        let opposite = Some(Neighbor {
            reference: 2,
            mv: a,
            split: false,
        });
        assert_eq!(
            near_candidates([opposite, None, None], 1, &[false, false, true, false])
                .unwrap()
                .best,
            Mv { row: -8, col: 16 }
        );
        assert_eq!(
            near_candidates([None; 3], 1, &[false; 4])
                .unwrap()
                .probabilities,
            [7, 1, 1, 143]
        );
        assert!(near_candidates([n(a), None, None], 0, &[false; 4]).is_err());
    }
    #[test]
    fn motion_sign_bias_keeps_wide_raw_candidate_until_caller_clamp() {
        let neighbor = Some(Neighbor {
            reference: 2,
            mv: Mv {
                row: -32768,
                col: -32768,
            },
            split: false,
        });
        let raw = near_candidates([neighbor, None, None], 1, &[false, false, true, false]).unwrap();
        let wide = Mv {
            row: 32768,
            col: 32768,
        };
        assert_eq!(
            (raw.nearest, raw.near, raw.best),
            (wide, Mv::default(), wide)
        );
        assert_eq!(raw.counts, [0, 2, 0, 0]);
        // One-MB reference geometry clamps this raw candidate to +128;
        // the helper must not prematurely reject or narrow the raw census.
        let clamped = Mv {
            row: raw.best.row.clamp(-128, 128),
            col: raw.best.col.clamp(-128, 128),
        };
        assert_eq!(clamped, Mv { row: 128, col: 128 });
        assert!(clamped.chroma().is_ok());
        assert!(wide.chroma().is_err(), "used-vector guard remains active");
        for invalid in [-32769, 32768, i32::MIN] {
            let bad = Some(Neighbor {
                reference: 2,
                mv: Mv {
                    row: invalid,
                    col: 0,
                },
                split: false,
            });
            assert!(
                near_candidates([bad, None, None], 1, &[false, false, true, false]).is_err(),
                "stored neighbor {invalid}"
            );
        }
    }
    #[test]
    fn motion_single_vector_chroma_rounds_signed_halves_away_from_zero() {
        for (value, expected) in [
            (-7, -4),
            (-3, -2),
            (-2, -1),
            (-1, -1),
            (0, 0),
            (1, 1),
            (2, 1),
            (3, 2),
            (7, 4),
        ] {
            assert_eq!(
                Mv {
                    row: value,
                    col: -value
                }
                .chroma()
                .unwrap(),
                Mv {
                    row: expected,
                    col: -expected
                }
            );
        }
        assert!(Mv {
            row: i32::MIN,
            col: 0
        }
        .chroma()
        .is_err());
    }
    fn predict(source: &[u8], x: usize, y: usize, mv: Mv) -> [u8; 64] {
        let mut output = [0; 64];
        predict_into(source, 16, 16, 16, x, y, 8, mv, &mut output, 8, &|| Ok(())).unwrap();
        output
    }
    #[test]
    fn motion_filters_preserve_constants_and_signed_floor_with_numeric_phase_oracle() {
        for row in 0..8 {
            for col in 0..8 {
                assert_eq!(predict(&[37; 256], 4, 4, Mv { row, col }), [37; 64]);
            }
        }
        let ramp: Vec<u8> = (0..256).map(|index| (index % 16 * 16) as u8).collect();
        let vertical_ramp: Vec<u8> = (0..256).map(|index| (index / 16 * 16) as u8).collect();
        // Independently evaluated linear-ramp dot products for all eight phases.
        for (phase, expected) in [64, 66, 68, 70, 72, 74, 76, 78].into_iter().enumerate() {
            assert_eq!(
                predict(
                    &ramp,
                    4,
                    4,
                    Mv {
                        row: 0,
                        col: phase as i32
                    }
                )[0],
                expected
            );
            assert_eq!(
                predict(
                    &vertical_ramp,
                    4,
                    4,
                    Mv {
                        row: phase as i32,
                        col: 0
                    }
                )[0],
                expected
            );
        }
        assert_eq!(predict(&ramp, 4, 4, Mv { row: 0, col: -1 })[0], 62);
    }
    #[test]
    fn motion_two_pass_clips_intermediate_before_vertical_negative_tap() {
        let mut source = [100; 256];
        for x in 2..8 {
            source[3 * 16 + x] = if matches!(x, 4 | 5) { 255 } else { 0 };
        }
        // Horizontal row3 = min(255,round(2*77*255/128)) =255.
        // Vertical = round((128*100-16*(255-100))/128) =81.
        assert_eq!(predict(&source, 4, 4, Mv { row: 4, col: 4 })[0], 81);
        let mut stripes = [0; 256];
        for x in 0..16 {
            stripes[4 * 16 + x] = 255;
            stripes[5 * 16 + x] = 255;
        }
        assert_eq!(predict(&stripes, 4, 4, Mv { row: 4, col: 0 })[0], 255);
        stripes.fill(0);
        for x in 0..16 {
            stripes[3 * 16 + x] = 255;
            stripes[6 * 16 + x] = 255;
        }
        assert_eq!(predict(&stripes, 4, 4, Mv { row: 4, col: 0 })[0], 0);
    }
    #[test]
    fn motion_edge_extension_uses_plane_extent_not_stride_padding() {
        let mut source = [201; 256];
        source[0] = 37;
        source[255] = 93;
        assert_eq!(
            predict(
                &source,
                0,
                0,
                Mv {
                    row: -32768,
                    col: -32768
                }
            ),
            [37; 64]
        );
        assert_eq!(
            predict(
                &source,
                0,
                0,
                Mv {
                    row: 32767,
                    col: 32767
                }
            ),
            [93; 64]
        );
        let mut padded = [255; 320];
        for y in 0..16 {
            for x in 0..16 {
                padded[y * 20 + x] = 73;
            }
        }
        let mut dest = [199; 96];
        predict_into(
            &padded,
            20,
            16,
            16,
            0,
            0,
            8,
            Mv { row: 0, col: 32767 },
            &mut dest,
            12,
            &|| Ok(()),
        )
        .unwrap();
        for row in dest.chunks(12) {
            assert_eq!(&row[..8], &[73; 8]);
            assert_eq!(&row[8..], &[199; 4]);
        }
    }
    #[test]
    fn motion_invalid_layout_work_exhaustion_and_cancellation_never_publish_pixels() {
        let source = [37; 256];
        let mut output = [77; 256];
        for (stride, width, height, x, size, mv) in [
            (15, 16, 16, 0, 8, Mv::default()),
            (16, 16, 17, 0, 8, Mv::default()),
            (16, 16, 16, usize::MAX, 8, Mv::default()),
            (16, 16, 16, 0, 4, Mv::default()),
            (16, 16, 16, 0, 8, Mv { row: 32768, col: 0 }),
        ] {
            assert!(predict_into(
                &source,
                stride,
                width,
                height,
                x,
                0,
                size,
                mv,
                &mut output,
                16,
                &|| Ok(())
            )
            .is_err());
            assert_eq!(output, [77; 256]);
        }
        let remaining = Cell::new(10usize);
        assert!(predict_into(
            &source,
            16,
            16,
            16,
            0,
            0,
            16,
            Mv::default(),
            &mut output,
            16,
            &|| {
                let next = remaining
                    .get()
                    .checked_sub(1)
                    .ok_or("original work allowance exhausted")?;
                remaining.set(next);
                Ok(())
            }
        )
        .is_err());
        assert_eq!(output, [77; 256]);
        assert!(predict_into(
            &source,
            16,
            16,
            16,
            0,
            0,
            16,
            Mv::default(),
            &mut output,
            16,
            &|| Err("original cancellation".into())
        )
        .is_err());
        assert_eq!(output, [77; 256]);
        predict_into(
            &source,
            16,
            16,
            16,
            0,
            0,
            16,
            Mv::default(),
            &mut output,
            16,
            &|| Ok(()),
        )
        .unwrap();
        assert_eq!(output, [37; 256]);
    }
}
