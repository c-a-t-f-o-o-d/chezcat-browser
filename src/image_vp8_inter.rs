//! Original finite visible version-zero VP8 interframe orchestration.
//! Reuses ChezCat's boolean reader, transforms, coefficients, filters and render.
use super::*;

struct Parameters {
    golden: bool,
    altref: bool,
    last: bool,
    copy_golden: u8,
    copy_altref: u8,
    sign_bias: [bool; 4],
    intra: u8,
    last_prob: u8,
    golden_prob: u8,
    ymode: [u8; 4],
    uvmode: [u8; 3],
    mv_prob: [[u8; 19]; 2],
}
#[derive(Default, Debug)]
pub(super) struct Audit {
    pub partitions: usize,
    pub references: [usize; 4],
    pub modes: [usize; 5],
    pub intra4: usize,
    pub skipped: usize,
    pub refresh: [bool; 3],
    pub copies: [u8; 2],
    pub entropy_refresh: bool,
}

fn header(bits: &mut Bool<'_>, previous: &Persistent) -> ImageResult<(Header, Parameters)> {
    let mut h = Header::read_common(bits, Some(previous))?;
    let golden = bits.bit(128)?;
    let altref = bits.bit(128)?;
    let copy_golden = if golden { 0 } else { bits.literal(2)? as u8 };
    let copy_altref = if altref { 0 } else { bits.literal(2)? as u8 };
    if copy_golden == 3 || copy_altref == 3 {
        return Err("Reserved VP8 reference copy selector.".into());
    }
    let sign_bias = [false, false, bits.bit(128)?, bits.bit(128)?];
    h.refresh_entropy = bits.bit(128)?;
    let last = bits.bit(128)?;
    h.read_tokens(bits)?;
    let intra = bits.literal(8)? as u8;
    let last_prob = bits.literal(8)? as u8;
    let golden_prob = bits.literal(8)? as u8;
    let mut ymode = previous.ymode;
    let mut uvmode = previous.uvmode;
    let mut mv_prob = previous.mv_prob;
    if bits.bit(128)? {
        for probability in &mut ymode {
            *probability = bits.literal(8)? as u8;
        }
    }
    if bits.bit(128)? {
        for probability in &mut uvmode {
            *probability = bits.literal(8)? as u8;
        }
    }
    motion::update_probabilities(&mut mv_prob, |p| bits.bit(p))?;
    Ok((
        h,
        Parameters {
            golden,
            altref,
            last,
            copy_golden,
            copy_altref,
            sign_bias,
            intra,
            last_prob,
            golden_prob,
            ymode,
            uvmode,
            mv_prob,
        },
    ))
}

fn clamp_predictor(
    mv: motion::Mv,
    x: usize,
    y: usize,
    columns: usize,
    rows: usize,
) -> ImageResult<motion::Mv> {
    let units = |n: usize| {
        i32::try_from(n)
            .ok()
            .and_then(|n| n.checked_mul(128))
            .ok_or("VP8 predictor rectangle overflow.")
    };
    Ok(motion::Mv {
        col: mv.col.clamp(-units(x + 1)?, units(columns - x)?),
        row: mv.row.clamp(-units(y + 1)?, units(rows - y)?),
    })
}

fn record(
    bits: &mut Bool<'_>,
    h: &Header,
    p: &Parameters,
    previous_segment: u8,
    neighbors: [Option<motion::Neighbor>; 3],
    x: usize,
    y: usize,
    columns: usize,
    rows: usize,
) -> ImageResult<Macroblock> {
    let mut block = Macroblock::default();
    if h.segments {
        block.segment = if h.segment_map {
            bits.tree(&[2, 4, 0, -1, -2, -3], &h.segment_probs)? as usize
        } else {
            usize::from(previous_segment)
        };
    }
    if let Some(probability) = h.skip_probability {
        block.skip = bits.bit(probability)?;
    }
    if !bits.bit(p.intra)? {
        block.luma = bits.tree(&[0, 2, 4, 6, -1, -2, -3, -4], &p.ymode)?;
        if block.luma == 4 {
            for mode in &mut block.modes {
                *mode = bits.tree(
                    &[
                        0, 2, -1, 4, -2, 6, 8, 12, -3, 10, -5, -6, -4, 14, -7, 16, -8, -9,
                    ],
                    &[120, 90, 79, 133, 87, 85, 80, 111, 151],
                )?;
            }
        }
        block.chroma = bits.tree(&[0, 2, -1, 4, -2, -3], &p.uvmode)?;
        return Ok(block);
    }
    block.reference = if !bits.bit(p.last_prob)? {
        1
    } else if !bits.bit(p.golden_prob)? {
        2
    } else {
        3
    };
    let near = motion::near_candidates(neighbors, block.reference, &p.sign_bias)?;
    let probabilities = near.probabilities;
    block.inter_mode = if !bits.bit(probabilities[0])? {
        0
    } else if !bits.bit(probabilities[1])? {
        1
    } else if !bits.bit(probabilities[2])? {
        2
    } else if !bits.bit(probabilities[3])? {
        3
    } else {
        4
    };
    block.mv = match block.inter_mode {
        0 => motion::Mv::default(),
        1 => clamp_predictor(near.nearest, x, y, columns, rows)?,
        2 => clamp_predictor(near.near, x, y, columns, rows)?,
        3 => {
            let best = clamp_predictor(near.best, x, y, columns, rows)?;
            let delta = motion::read_delta(&p.mv_prob, |probability| bits.bit(probability))?;
            motion::Mv {
                row: best
                    .row
                    .checked_add(delta.row)
                    .ok_or("VP8 NEWMV row overflow.")?,
                col: best
                    .col
                    .checked_add(delta.col)
                    .ok_or("VP8 NEWMV column overflow.")?,
            }
        }
        _ => {
            return Err(format!(
                "VP8 SPLITMV is not implemented in this finite profile (macroblock {x},{y})."
            ))
        }
    };
    // No final NEW clamp is invented. The raw sum remains its later-neighbor
    // predictor; the motion primitive checks its supported representation.
    if !(-32768..=32767).contains(&block.mv.row) || !(-32768..=32767).contains(&block.mv.col) {
        return Err(
            "VP8 stored motion vector is outside the format's signed displacement range.".into(),
        );
    }
    Ok(block)
}

pub(super) fn decode(
    data: &[u8],
    previous: &VideoState,
    work: &Work<'_>,
    maximum_pixels: usize,
) -> ImageResult<(Bitmap, VideoState)> {
    decode_audited(data, previous, work, maximum_pixels).map(|(bitmap, state, _)| (bitmap, state))
}

pub(super) fn decode_audited(
    data: &[u8],
    previous: &VideoState,
    work: &Work<'_>,
    maximum_pixels: usize,
) -> ImageResult<(Bitmap, VideoState, Audit)> {
    (work.check)()?;
    if data.len() < 3 {
        return Err("Truncated VP8 interframe tag.".into());
    }
    let tag = usize::from(data[0]) | (usize::from(data[1]) << 8) | (usize::from(data[2]) << 16);
    if tag & 1 == 0 || (tag >> 1) & 7 != 0 || tag & 16 == 0 {
        return Err("VP8 video interframe profile requires visible version-zero P frames.".into());
    }
    let frame = &previous.references[0];
    let (width, height) = (frame.width, frame.height);
    let pixels = dimensions_with_limit(width, height, maximum_pixels)?;
    let columns = width.div_ceil(16);
    let rows = height.div_ceil(16);
    let blocks = columns
        .checked_mul(rows)
        .ok_or("VP8 macroblock count overflow.")?;
    if previous.persistent.segment_map.len() != blocks {
        return Err("VP8 retained segment map has incompatible dimensions.".into());
    }
    if previous
        .references
        .iter()
        .any(|reference| reference.width != width || reference.height != height)
    {
        return Err("VP8 reference dimensions do not match the current sequence.".into());
    }
    let first_end = 3usize
        .checked_add(tag >> 5)
        .filter(|&end| end <= data.len())
        .ok_or("Truncated VP8 interframe first partition.")?;
    let mut bits = Bool::new(&data[3..first_end], work);
    let (h, p) = header(&mut bits, &previous.persistent)?;
    let mut audit = Audit {
        partitions: h.partitions,
        refresh: [p.last, p.golden, p.altref],
        copies: [p.copy_golden, p.copy_altref],
        entropy_refresh: h.refresh_entropy,
        ..Audit::default()
    };
    let mut next = first_end
        .checked_add((h.partitions - 1) * 3)
        .filter(|&end| end <= data.len())
        .ok_or("Truncated VP8 interframe token directory.")?;
    let mut tokens = reserved(h.partitions)?;
    for partition in 0..h.partitions {
        let end = if partition + 1 == h.partitions {
            data.len()
        } else {
            let at = first_end + partition * 3;
            let size = usize::from(data[at])
                | (usize::from(data[at + 1]) << 8)
                | (usize::from(data[at + 2]) << 16);
            next.checked_add(size)
                .filter(|&end| end <= data.len())
                .ok_or("Truncated VP8 interframe token partition.")?
        };
        tokens.push(Bool::new(&data[next..end], work));
        next = end;
    }
    let mut planes = [
        Plane::new(columns * 16, rows * 16)?,
        Plane::new(columns * 8, rows * 8)?,
        Plane::new(columns * 8, rows * 8)?,
    ];
    let mut records: Vec<Macroblock> = reserved(blocks)?;
    let mut contexts = reserved(columns)?;
    contexts.resize(columns, [0u8; 9]);
    let mut segments = reserved(blocks)?;
    segments.extend_from_slice(&previous.persistent.segment_map);
    work.pixels(blocks)?;
    for y in 0..rows {
        let mut left = [0u8; 9];
        for x in 0..columns {
            (work.check)()?;
            work.pixels(2 * (16 * 16 + 2 * 8 * 8))?;
            let index = y * columns + x;
            let neighbor = |at: usize| {
                let b = records[at];
                Some(motion::Neighbor {
                    reference: b.reference,
                    mv: b.mv,
                    split: false,
                })
            };
            let neighbors = [
                if y != 0 {
                    neighbor(index - columns)
                } else {
                    None
                },
                if x != 0 { neighbor(index - 1) } else { None },
                if x != 0 && y != 0 {
                    neighbor(index - columns - 1)
                } else {
                    None
                },
            ];
            let mut block = record(
                &mut bits,
                &h,
                &p,
                segments[index],
                neighbors,
                x,
                y,
                columns,
                rows,
            )?;
            if h.segment_map {
                segments[index] = block.segment as u8;
            }
            audit.references[usize::from(block.reference)] += 1;
            audit.skipped += usize::from(block.skip);
            if block.reference != 0 {
                audit.modes[usize::from(block.inter_mode)] += 1;
            } else {
                audit.intra4 += usize::from(block.luma == 4);
            }
            let residue = residuals(
                &mut block,
                &h,
                &mut tokens[y % h.partitions],
                &mut left,
                &mut contexts[x],
            )?;
            if block.reference != 0 {
                let source = &previous.references[usize::from(block.reference) - 1];
                for component in 0..3 {
                    let size = if component == 0 { 16 } else { 8 };
                    let px = x * size;
                    let py = y * size;
                    let mv = if component == 0 {
                        block.mv
                    } else {
                        block.mv.chroma()?
                    };
                    let reference = &source.planes[component];
                    let plane = &mut planes[component];
                    let start = py * plane.width + px;
                    motion::predict_into(
                        &reference.data,
                        reference.width,
                        reference.width,
                        reference.height,
                        px,
                        py,
                        size,
                        mv,
                        &mut plane.data[start..],
                        plane.width,
                        &|| {
                            (work.check)()?;
                            work.step()
                        },
                    )?;
                }
            } else {
                if block.luma != 4 {
                    planes[0].predict(x * 16, y * 16, 16, block.luma);
                }
                planes[1].predict(x * 8, y * 8, 8, block.chroma);
                planes[2].predict(x * 8, y * 8, 8, block.chroma);
            }
            for (n, &coeff) in residue[..16].iter().enumerate() {
                let px = x * 16 + n % 4 * 4;
                let py = y * 16 + n / 4 * 4;
                if block.reference == 0 && block.luma == 4 {
                    planes[0].predict4(px, py, y * 16, block.modes[n]);
                }
                planes[0].add(px, py, coeff);
            }
            for component in 1..3 {
                for n in 0..4 {
                    planes[component].add(
                        x * 8 + n % 2 * 4,
                        y * 8 + n / 2 * 4,
                        residue[16 + (component - 1) * 4 + n],
                    );
                }
            }
            records.push(block);
        }
    }
    filtering(&mut planes, &records, &h, columns, work)?;
    let bitmap = render(&planes, width, height, pixels, work)?;
    for (index, plane) in planes.iter_mut().enumerate() {
        plane.extend_visible_edges(
            if index == 0 { width } else { width.div_ceil(2) },
            if index == 0 {
                height
            } else {
                height.div_ceil(2)
            },
            work,
        )?;
    }
    let current = Arc::new(YuvFrame {
        width,
        height,
        planes,
    });
    let mut references = [
        previous.references[0].clone(),
        previous.references[1].clone(),
        previous.references[2].clone(),
    ];
    // Complete-frame reference changes form an owned candidate. ALT copy
    // precedes GOLD copy; refreshes then select the actual new filtered YUV.
    if p.copy_altref != 0 {
        references[2] = previous.references[if p.copy_altref == 1 { 0 } else { 1 }].clone();
    }
    if p.copy_golden != 0 {
        references[1] = if p.copy_golden == 1 {
            previous.references[0].clone()
        } else {
            references[2].clone()
        };
    }
    if p.golden {
        references[1] = current.clone();
    }
    if p.altref {
        references[2] = current.clone();
    }
    if p.last {
        references[0] = current;
    }
    let persistent = Persistent {
        probabilities: if h.refresh_entropy {
            h.probabilities
        } else {
            previous.persistent.probabilities
        },
        absolute: h.absolute,
        quant: h.quant,
        filter: h.filter,
        segment_probs: h.segment_probs,
        segment_map: segments,
        reference_delta: h.reference_delta,
        mode_delta: h.mode_delta,
        ymode: if h.refresh_entropy {
            p.ymode
        } else {
            previous.persistent.ymode
        },
        uvmode: if h.refresh_entropy {
            p.uvmode
        } else {
            previous.persistent.uvmode
        },
        mv_prob: if h.refresh_entropy {
            p.mv_prob
        } else {
            previous.persistent.mv_prob
        },
    };
    (work.check)()?;
    Ok((
        bitmap,
        VideoState {
            references,
            persistent,
        },
        audit,
    ))
}

#[cfg(test)]
#[path = "image_vp8_inter_tests.rs"]
mod tests;
