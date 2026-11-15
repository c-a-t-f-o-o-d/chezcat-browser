//! Original complete-header P-frame controls and real sequence/state failures.
//! Wire as a cfg(test) child of image_vp8_inter; no production fixture encoder.
use super::*;

struct Decision {
    probability: u8,
    one: bool,
    split: u32,
    shifts: usize,
}
struct Writer {
    range: u32,
    decisions: Vec<Decision>,
}
impl Writer {
    fn new() -> Self {
        Self {
            range: 255,
            decisions: Vec::new(),
        }
    }
    fn bit(&mut self, probability: u8, one: bool) {
        assert!(self.decisions.len() < 10_000);
        let split = 1 + ((self.range - 1) * u32::from(probability) >> 8);
        self.range = if one { self.range - split } else { split };
        let mut shifts = 0;
        while self.range < 128 {
            self.range *= 2;
            shifts += 1;
        }
        self.decisions.push(Decision {
            probability,
            one,
            split,
            shifts,
        });
    }
    fn literal(&mut self, value: u32, bits: usize) {
        for at in (0..bits).rev() {
            self.bit(128, value & (1 << at) != 0);
        }
    }
    fn finish(self) -> Vec<u8> {
        // Reverse the decoder's bounded fractional interval equations from a
        // legal final value. This avoids copied encoder/carry-buffer routines.
        let mut value = 0u32;
        let mut chunks = Vec::new();
        for decision in self.decisions.iter().rev() {
            let mask = (1u32 << decision.shifts) - 1;
            chunks.push((value & mask, decision.shifts));
            value >>= decision.shifts;
            if decision.one {
                value += decision.split;
            }
        }
        assert!(value < 256);
        let mut bytes = vec![value as u8];
        let mut at = 8;
        for (value, bits) in chunks.into_iter().rev() {
            for bit in (0..bits).rev() {
                if at % 8 == 0 {
                    bytes.push(0);
                }
                if value & (1 << bit) != 0 {
                    let last = bytes.len() - 1;
                    bytes[last] |= 1 << (7 - at % 8);
                }
                at += 1;
            }
        }
        bytes.extend_from_slice(&[0; 4]);
        // Check fixture intent before using it as an input to state tests.
        let remaining = Cell::new(1_000_000);
        let check = || Ok(());
        let work = Work {
            remaining: &remaining,
            check: &check,
            video: true,
        };
        let mut reader = Bool::new(&bytes, &work);
        for decision in self.decisions {
            assert_eq!(reader.bit(decision.probability).unwrap(), decision.one);
        }
        bytes
    }
}
#[derive(Clone, Copy)]
enum Segments<'a> {
    Off,
    Keep,
    Update(&'a [u8]),
}
#[derive(Clone, Copy)]
struct Options<'a> {
    segments: Segments<'a>,
    entropy: bool,
    update_entropy: bool,
    update_filter: bool,
    golden: bool,
    altref: bool,
    last: bool,
    copy_golden: u8,
    copy_altref: u8,
    split_first: bool,
}
impl Default for Options<'_> {
    fn default() -> Self {
        Self {
            segments: Segments::Off,
            entropy: true,
            update_entropy: false,
            update_filter: false,
            golden: false,
            altref: false,
            last: true,
            copy_golden: 0,
            copy_altref: 0,
            split_first: false,
        }
    }
}
fn synthetic(previous: &VideoState, options: Options<'_>) -> Vec<u8> {
    let frame = &previous.references[0];
    let columns = frame.width.div_ceil(16);
    let rows = frame.height.div_ceil(16);
    let mut w = Writer::new();
    match options.segments {
        Segments::Off => w.bit(128, false),
        Segments::Keep => {
            w.bit(128, true);
            w.bit(128, false);
            w.bit(128, false);
        }
        Segments::Update(map) => {
            assert_eq!(map.len(), columns * rows);
            w.bit(128, true);
            w.bit(128, true);
            w.bit(128, true);
            w.bit(128, true);
            for value in [10, 20, 30, 40] {
                w.bit(128, true);
                w.literal(value, 7);
                w.bit(128, false);
            }
            for value in [0, 1, 2, 3] {
                w.bit(128, true);
                w.literal(value, 6);
                w.bit(128, false);
            }
            for _ in 0..3 {
                w.bit(128, true);
                w.literal(128, 8);
            }
        }
    }
    w.bit(128, false);
    w.literal(0, 6);
    w.literal(0, 3); // no filtering of this predicted control
    w.bit(128, options.update_filter);
    if options.update_filter {
        w.bit(128, true);
        for value in [1, 2, 3, 4, 4, 3, 2, 1] {
            w.bit(128, true);
            w.literal(value, 6);
            w.bit(128, false);
        }
    }
    w.literal(0, 2);
    w.literal(10, 7);
    for _ in 0..5 {
        w.bit(128, false);
    }
    w.bit(128, options.golden);
    w.bit(128, options.altref);
    if !options.golden {
        w.literal(u32::from(options.copy_golden), 2);
    }
    if !options.altref {
        w.literal(u32::from(options.copy_altref), 2);
    }
    w.bit(128, false);
    w.bit(128, false);
    w.bit(128, options.entropy);
    w.bit(128, options.last);
    for (index, probability) in COEFF_UPDATES.into_iter().enumerate() {
        let update = options.update_entropy && index == 0;
        w.bit(probability, update);
        if update {
            w.literal(
                u32::from(previous.persistent.probabilities[0].wrapping_add(37)),
                8,
            );
        }
    }
    w.bit(128, true);
    w.literal(128, 8); // each MB really skips coefficients
    for _ in 0..3 {
        w.literal(128, 8);
    } // intra/last/golden probability
    w.bit(128, options.update_entropy);
    if options.update_entropy {
        for value in previous.persistent.ymode {
            w.literal(u32::from(value.wrapping_add(1)), 8);
        }
    }
    w.bit(128, options.update_entropy);
    if options.update_entropy {
        for value in previous.persistent.uvmode {
            w.literal(u32::from(value.wrapping_add(1)), 8);
        }
    }
    for component in 0..2 {
        for position in 0..19 {
            let update = options.update_entropy && component == 0 && position == 0;
            w.bit(motion::MV_UPDATES[component][position], update);
            if update {
                w.literal(
                    if previous.persistent.mv_prob[0][0] == 128 {
                        32
                    } else {
                        64
                    },
                    7,
                );
            }
        }
    }
    for y in 0..rows {
        for x in 0..columns {
            if let Segments::Update(map) = options.segments {
                let segment = map[y * columns + x];
                assert!(segment < 4);
                w.bit(128, segment >= 2);
                w.bit(128, segment & 1 != 0);
            }
            w.bit(128, true);
            w.bit(128, true);
            w.bit(128, false); // skip, inter, LAST
            let zero =
                usize::from(y != 0) * 2 + usize::from(x != 0) * 2 + usize::from(x != 0 && y != 0);
            if options.split_first && x == 0 && y == 0 {
                for probability in motion::MODE_CONTEXTS[zero] {
                    w.bit(probability, true);
                }
            } else {
                w.bit(motion::MODE_CONTEXTS[zero][0], false);
            }
        }
    }
    let header = w.finish();
    assert!(header.len() < 1 << 19);
    let tag = (header.len() as u32) * 32 + 17;
    let mut packet = tag.to_le_bytes()[..3].to_vec();
    packet.extend_from_slice(&header);
    packet.extend_from_slice(&[0; 4]);
    packet
}
fn packets() -> Vec<&'static [u8]> {
    let bytes = include_bytes!("../tools/fixtures/scene-inter.ivf");
    let mut at = 32;
    let mut out = Vec::new();
    while at < bytes.len() {
        let size = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        at += 12;
        out.push(&bytes[at..at + size]);
        at += size;
    }
    out
}
fn seed(count: usize) -> VideoDecoder {
    let mut d = VideoDecoder::new(64 * 48, 1_000_000_000);
    for packet in packets().into_iter().take(count) {
        d.decode(packet, &|| Ok(())).unwrap();
    }
    d
}
fn visible(frame: &YuvFrame) -> Vec<u8> {
    let mut out = Vec::new();
    for (component, plane) in frame.planes.iter().enumerate() {
        let w = if component == 0 {
            frame.width
        } else {
            frame.width.div_ceil(2)
        };
        let h = if component == 0 {
            frame.height
        } else {
            frame.height.div_ceil(2)
        };
        for row in 0..h {
            out.extend_from_slice(&plane.data[row * plane.width..row * plane.width + w]);
        }
    }
    out
}
fn assert_yuv(decoder: &VideoDecoder, index: usize) {
    let oracle = include_bytes!("../tools/fixtures/scene-inter.yuv");
    assert_eq!(
        visible(&decoder.state.as_ref().unwrap().references[0]),
        oracle[index * 4608..(index + 1) * 4608],
        "frame {index}"
    );
}
fn fingerprint(p: &Persistent) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    let mut add = |byte: u8| {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    };
    for &byte in p
        .probabilities
        .iter()
        .chain(&p.segment_probs)
        .chain(&p.segment_map)
        .chain(&p.ymode)
        .chain(&p.uvmode)
        .chain(p.mv_prob.iter().flatten())
    {
        add(byte);
    }
    add(u8::from(p.absolute));
    for value in p
        .quant
        .iter()
        .chain(&p.filter)
        .chain(&p.reference_delta)
        .chain(&p.mode_delta)
    {
        for byte in value.to_le_bytes() {
            add(byte);
        }
    }
    hash
}
fn snapshot(decoder: &VideoDecoder) -> ([Arc<YuvFrame>; 3], u64) {
    let s = decoder.state.as_ref().unwrap();
    (
        [
            s.references[0].clone(),
            s.references[1].clone(),
            s.references[2].clone(),
        ],
        fingerprint(&s.persistent),
    )
}
fn unchanged(decoder: &VideoDecoder, before: &([Arc<YuvFrame>; 3], u64)) {
    let s = decoder.state.as_ref().unwrap();
    assert!(s
        .references
        .iter()
        .zip(&before.0)
        .all(|(a, b)| Arc::ptr_eq(a, b)));
    assert_eq!(fingerprint(&s.persistent), before.1);
}

#[test]
fn vp8_inter_original_complete_sequence_matches_independent_visible_yuv_and_key_resets() {
    let mut d = VideoDecoder::new(64 * 48, 1_000_000_000);
    for (index, packet) in packets().into_iter().enumerate() {
        d.decode(packet, &|| Ok(())).unwrap();
        assert_yuv(&d, index);
        if index == 12 {
            let refs = &d.state.as_ref().unwrap().references;
            assert!(refs
                .iter()
                .all(|reference| Arc::ptr_eq(reference, &refs[0])));
        }
    }
    assert!(d.remaining_work() < 1_000_000_000);
}

#[test]
fn vp8_inter_real_header_prefixes_refuse_orphan_hidden_version_split_and_truncated_tokens() {
    let packets = packets();
    let mut orphan = VideoDecoder::new(64 * 48, 1_000_000_000);
    assert!(orphan
        .decode(packets[1], &|| Ok(()))
        .unwrap_err()
        .contains("earlier decoded keyframe"));
    assert!(orphan.state.is_none());
    let mut d = seed(1);
    let before = snapshot(&d);
    let mut hidden = packets[1].to_vec();
    hidden[0] &= !16;
    assert!(d
        .decode(&hidden, &|| Ok(()))
        .unwrap_err()
        .contains("visible version-zero"));
    unchanged(&d, &before);
    let mut version = packets[1].to_vec();
    version[0] |= 2;
    assert!(d
        .decode(&version, &|| Ok(()))
        .unwrap_err()
        .contains("visible version-zero"));
    unchanged(&d, &before);
    let split = synthetic(
        d.state.as_ref().unwrap(),
        Options {
            split_first: true,
            ..Options::default()
        },
    );
    assert!(d
        .decode(&split, &|| Ok(()))
        .unwrap_err()
        .contains("SPLITMV"));
    unchanged(&d, &before);
    // A complete updated control partition with missing residual tokens must
    // not publish its new entropy, segment data, or any reference identity.
    let tag = usize::from(packets[1][0])
        | (usize::from(packets[1][1]) << 8)
        | (usize::from(packets[1][2]) << 16);
    let first_end = 3 + (tag >> 5);
    assert!(first_end > 3);
    assert!(d.decode(&packets[1][..first_end], &|| Ok(())).is_err());
    unchanged(&d, &before);
    d.decode(packets[1], &|| Ok(())).unwrap();
    assert_yuv(&d, 1);
    d.decode(packets[2], &|| Ok(())).unwrap();
    assert_yuv(&d, 2);
}

#[test]
fn vp8_inter_final_cancellation_preserves_distinct_refs_entropy_and_recovers() {
    let packets = packets();
    let mut measured = seed(8);
    let calls = Cell::new(0);
    measured
        .decode(packets[8], &|| {
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    let final_check = calls.get();
    let mut d = seed(8);
    let before = snapshot(&d);
    assert!(!Arc::ptr_eq(&before.0[0], &before.0[2]));
    let used = d.remaining_work();
    let calls = Cell::new(0);
    assert_eq!(
        d.decode(packets[8], &|| {
            calls.set(calls.get() + 1);
            if calls.get() == final_check {
                Err("original inter final cancellation".into())
            } else {
                Ok(())
            }
        })
        .unwrap_err(),
        "original inter final cancellation"
    );
    unchanged(&d, &before);
    assert!(d.remaining_work() < used);
    d.decode(packets[8], &|| Ok(())).unwrap();
    assert_yuv(&d, 8);
}

#[test]
fn vp8_inter_cumulative_replay_work_failure_retains_state_and_new_decoder_recovers() {
    let packets = packets();
    let mut measured = seed(3);
    let prefix = 1_000_000_000 - measured.remaining_work();
    let before = measured.remaining_work();
    measured.decode(packets[3], &|| Ok(())).unwrap();
    let frame = before - measured.remaining_work();
    let mut d = VideoDecoder::new(64 * 48, prefix + frame / 2);
    for packet in &packets[..3] {
        d.decode(packet, &|| Ok(())).unwrap();
    }
    let retained = snapshot(&d);
    assert!(d
        .decode(packets[3], &|| Ok(()))
        .unwrap_err()
        .contains("work limit"));
    assert_eq!(d.remaining_work(), 0);
    unchanged(&d, &retained);
    assert!(d.decode(packets[3], &|| Ok(())).is_err());
    let mut fresh = VideoDecoder::new(64 * 48, prefix + frame);
    for packet in &packets[..4] {
        fresh.decode(packet, &|| Ok(())).unwrap();
    }
    assert_yuv(&fresh, 3);
    assert_eq!(fresh.remaining_work(), 0);
}

#[test]
fn vp8_inter_refresh_false_restores_all_entropy_but_keeps_new_feature_and_filter_state() {
    for refresh in [false, true] {
        let mut d = seed(1);
        let old = &d.state.as_ref().unwrap().persistent;
        let probabilities = old.probabilities;
        let ymode = old.ymode;
        let uvmode = old.uvmode;
        let mv = old.mv_prob;
        let map = [0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3];
        let packet = synthetic(
            d.state.as_ref().unwrap(),
            Options {
                entropy: refresh,
                update_entropy: true,
                update_filter: true,
                segments: Segments::Update(&map),
                ..Options::default()
            },
        );
        let expected = d.decode(&packet, &|| Ok(())).unwrap();
        assert_eq!(expected.width, 64);
        let current = &d.state.as_ref().unwrap().persistent;
        assert_eq!(
            current.probabilities[0],
            if refresh {
                probabilities[0].wrapping_add(37)
            } else {
                probabilities[0]
            }
        );
        assert_eq!(&current.probabilities[1..], &probabilities[1..]);
        assert_eq!(
            current.ymode,
            if refresh {
                ymode.map(|p| p.wrapping_add(1))
            } else {
                ymode
            }
        );
        assert_eq!(
            current.uvmode,
            if refresh {
                uvmode.map(|p| p.wrapping_add(1))
            } else {
                uvmode
            }
        );
        let mut expected_mv = mv;
        if refresh {
            expected_mv[0][0] = if mv[0][0] == 128 { 64 } else { 128 };
        }
        assert_eq!(current.mv_prob, expected_mv);
        assert_eq!(current.segment_map, map);
        assert_eq!(current.quant, [10, 20, 30, 40]);
        assert_eq!(current.filter, [0, 1, 2, 3]);
        assert!(current.absolute);
        assert_eq!(current.reference_delta, [1, 2, 3, 4]);
        assert_eq!(current.mode_delta, [4, 3, 2, 1]);
        let next = synthetic(d.state.as_ref().unwrap(), Options::default());
        d.decode(&next, &|| Ok(())).unwrap();
        let after = &d.state.as_ref().unwrap().persistent;
        assert_eq!(after.mv_prob, expected_mv);
        assert_eq!(after.reference_delta, [1, 2, 3, 4]);
        assert_eq!(after.segment_map, map);
    }
}

#[test]
fn vp8_inter_segment_ids_survive_disabled_frame_and_reenable_without_map_update() {
    let mut d = seed(1);
    let map = [3, 2, 1, 0, 3, 2, 1, 0, 3, 2, 1, 0];
    let expected = visible(&d.state.as_ref().unwrap().references[0]);
    for segments in [Segments::Update(&map), Segments::Off, Segments::Keep] {
        let packet = synthetic(
            d.state.as_ref().unwrap(),
            Options {
                segments,
                entropy: false,
                ..Options::default()
            },
        );
        d.decode(&packet, &|| Ok(())).unwrap();
        let s = d.state.as_ref().unwrap();
        assert_eq!(s.persistent.segment_map, map);
        assert_eq!(s.persistent.quant, [10, 20, 30, 40]);
        assert_eq!(visible(&s.references[0]), expected);
    }
}

#[test]
fn vp8_inter_real_packets_apply_alt_copy_before_gold_copy_and_refresh_after_both() {
    for mode in 0..3 {
        let mut d = seed(1);
        let packet = synthetic(d.state.as_ref().unwrap(), Options::default());
        d.decode(&packet, &|| Ok(())).unwrap();
        let packet = synthetic(
            d.state.as_ref().unwrap(),
            Options {
                golden: true,
                last: false,
                ..Options::default()
            },
        );
        d.decode(&packet, &|| Ok(())).unwrap();
        let old = snapshot(&d);
        assert!(
            !Arc::ptr_eq(&old.0[0], &old.0[1])
                && !Arc::ptr_eq(&old.0[0], &old.0[2])
                && !Arc::ptr_eq(&old.0[1], &old.0[2])
        );
        let options = match mode {
            0 => Options {
                last: false,
                copy_golden: 2,
                copy_altref: 2,
                ..Options::default()
            },
            1 => Options {
                last: false,
                copy_golden: 2,
                copy_altref: 1,
                ..Options::default()
            },
            _ => Options {
                golden: true,
                altref: true,
                last: true,
                ..Options::default()
            },
        };
        let packet = synthetic(d.state.as_ref().unwrap(), options);
        d.decode(&packet, &|| Ok(())).unwrap();
        let current = &d.state.as_ref().unwrap().references;
        match mode {
            0 => {
                assert!(Arc::ptr_eq(&current[0], &old.0[0]));
                assert!(Arc::ptr_eq(&current[1], &old.0[1]) && Arc::ptr_eq(&current[2], &old.0[1]));
            }
            1 => {
                assert!(current.iter().all(|r| Arc::ptr_eq(r, &old.0[0])));
            }
            _ => {
                assert!(current.iter().all(|r| Arc::ptr_eq(r, &current[0])));
                assert!(old.0.iter().all(|r| !Arc::ptr_eq(r, &current[0])));
            }
        }
    }
}
