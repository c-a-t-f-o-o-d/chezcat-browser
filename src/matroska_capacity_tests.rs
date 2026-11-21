use super::*;
use std::cell::Cell;

const FIXTURE: &[u8] = include_bytes!("../tools/fixtures/scene-av.mkv");
const FORMER_ELEMENTS: usize = 1_000_000;
const SEGMENT: u64 = 0x18538067;

fn element(id: u64, body: &[u8]) -> Vec<u8> {
    let id_bytes = id.to_be_bytes();
    let first = id_bytes.iter().position(|&byte| byte != 0).unwrap();
    let mut out = id_bytes[first..].to_vec();
    // A finite eight-byte size keeps fixture construction independent of the
    // existing fixture's original size-field widths.
    out.extend_from_slice(&(0x0100_0000_0000_0000u64 | body.len() as u64).to_be_bytes());
    out.extend_from_slice(body);
    out
}

fn inject(data: &[u8], path: &[u64], prefix: &[u8], suffix: &[u8]) -> Vec<u8> {
    if path.is_empty() {
        let mut out = prefix.to_vec();
        out.extend_from_slice(data);
        out.extend_from_slice(suffix);
        return out;
    }
    let mut out = Vec::new();
    let mut at = 0;
    let mut changed = false;
    while at < data.len() {
        let start = at;
        let id = vint(data, &mut at, true).unwrap();
        let size = usize::try_from(vint(data, &mut at, false).unwrap()).unwrap();
        let end = at.checked_add(size).filter(|&end| end <= data.len()).unwrap();
        if !changed && id == path[0] {
            out.extend_from_slice(&element(id, &inject(&data[at..end], &path[1..], prefix, suffix)));
            changed = true;
        } else {
            out.extend_from_slice(&data[start..end]);
        }
        at = end;
    }
    assert!(changed, "fixture path {path:x?}");
    out
}

fn voids(count: usize) -> Vec<u8> { [0xec, 0x80].repeat(count) }

fn read(bytes: Vec<u8>) -> Result<Asset, String> {
    load(bytes, media::Limits::default(), audio::Limits::default(), &|| Ok(()))
}

fn assert_real_tracks(asset: Asset) {
    let expected = read(FIXTURE.to_vec()).unwrap();
    assert_eq!((asset.video.clip.width, asset.video.clip.height, asset.video.clip.frame_count()), (64, 48, 6));
    assert_eq!((asset.audio.channels, asset.audio.sample_rate, asset.audio.bits, asset.audio.frames), (2, 48_000, 16, 144_000));
    assert_eq!(asset.video.clip.duration, Duration::from_secs(3));
    assert_eq!(asset.audio.duration, Duration::from_secs(3));
    for frame in [0, 5] {
        assert_eq!(asset.video.clip.decode(frame, &|| Ok(())).unwrap().pixels, expected.video.clip.decode(frame, &|| Ok(())).unwrap().pixels);
        assert_eq!(asset.video.clip.index_at(Duration::from_millis(frame as u64 * 500)), frame);
    }
    for frame in [0, 5_990, 143_970] {
        assert_eq!(asset.audio.decode(frame, 25, &|| Ok(())).unwrap(), expected.audio.decode(frame, 25, &|| Ok(())).unwrap());
    }
    // This authored sample is away from the fixture's start/end fade.
    let sample = (2_800.0 * (2.0 * std::f64::consts::PI * 220.0 * 5_990.0 / 48_000.0).sin()).round() as i16;
    assert_eq!(asset.audio.decode(5_990, 1, &|| Ok(())).unwrap()[0], sample);
}

#[test]
fn matroska_source_bounded_segment_records_preserve_both_passes_and_real_tracks() {
    // The former shared counter charged each of these records twice, so this
    // roughly 1 MiB prefix could fail before reaching real cluster payloads.
    let bytes = inject(FIXTURE, &[SEGMENT], &voids(FORMER_ELEMENTS / 2 + 1), &[]);
    assert_real_tracks(read(bytes).unwrap());
}

#[test]
fn matroska_source_bounded_nested_records_preserve_real_tracks() {
    let bytes = inject(FIXTURE, &[SEGMENT, 0x1654ae6b, 0xae, 0xe0], &voids(FORMER_ELEMENTS + 1), &[]);
    assert_real_tracks(read(bytes).unwrap());
}

#[test]
fn matroska_source_bounded_records_preserve_late_extent_and_format_refusals() {
    let prefix = voids(FORMER_ELEMENTS + 1);
    for (tail, expected) in [
        (&[0xec][..], "Truncated EBML variable integer"),
        (&[0xec, 0x81][..], "EBML child exceeds its parent"),
        (&[0xec, 0xff][..], "Unknown-sized EBML"),
        (&[0][..], "Invalid EBML integer width"),
        (&[0xbf, 0x84, 0, 0, 0, 0][..], "CRC elements are not implemented"),
        (&[0x15, 0x49, 0xa9, 0x66, 0x80][..], "Duplicate Segment Info"),
        (&[0xe7, 0x80][..], "Unsupported Matroska Segment field"),
    ] {
        let error = match read(inject(FIXTURE, &[SEGMENT], &prefix, tail)) {
            Err(error) => error,
            Ok(_) => panic!("malformed suffix accepted: {tail:?}"),
        };
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn matroska_second_pass_remains_cancelable_past_the_former_allowance() {
    let bytes = inject(FIXTURE, &[SEGMENT], &voids(600_000), &[]);
    let calls = Cell::new(0usize);
    let stop = FORMER_ELEMENTS + 256;
    let result = load(bytes, media::Limits::default(), audio::Limits::default(), &|| {
        calls.set(calls.get() + 1);
        if calls.get() >= stop { Err("late Matroska cancellation".into()) } else { Ok(()) }
    });
    assert!(matches!(result, Err(ref error) if error == "late Matroska cancellation"));
    assert_eq!(calls.get(), stop);
    assert_eq!(read(FIXTURE.to_vec()).unwrap().video.clip.frame_count(), 6);
}

#[test]
fn matroska_source_bounded_records_retain_input_packet_and_decode_work_admission() {
    let bytes = inject(FIXTURE, &[SEGMENT], &voids(FORMER_ELEMENTS / 2 + 1), &[]);
    let small = audio::Limits { input_bytes: bytes.len() - 1, ..audio::Limits::default() };
    let result = load(bytes.clone(), media::Limits::default(), small, &|| Ok(()));
    assert!(matches!(result, Err(ref error) if error.contains("input byte allowance")));
    let frames = media::Limits { frames: 2, ..media::Limits::default() };
    let result = load(bytes.clone(), frames, audio::Limits::default(), &|| Ok(()));
    assert!(matches!(result, Err(ref error) if error.contains("video packet allowance")));
    let work = media::Limits { decode_work: 1, ..media::Limits::default() };
    let result = load(bytes, work, audio::Limits::default(), &|| Ok(()));
    assert!(matches!(result, Err(ref error) if error.contains("work limit")));
}
