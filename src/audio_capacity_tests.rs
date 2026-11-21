use super::*;
use std::cell::Cell;

const FORMER_CHUNKS: usize = 1_000_000;

fn pcm_wave() -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WAVEfmt \x10\0\0\0".to_vec();
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&8_000u32.to_le_bytes());
    bytes.extend_from_slice(&32_000u32.to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data\x08\0\0\0");
    for value in [i16::MIN, 0, i16::MAX, -1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let size = u32::try_from(bytes.len() - 8).unwrap();
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes
}

fn with_junk(count: usize, tail: &[u8]) -> Vec<u8> {
    let mut bytes = pcm_wave();
    bytes.extend_from_slice(&b"JUNK\0\0\0\0".repeat(count));
    bytes.extend_from_slice(tail);
    let size = u32::try_from(bytes.len() - 8).unwrap();
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    bytes
}

#[test]
fn wave_source_bounded_chunks_preserve_real_pcm_samples_and_timing() {
    let clip = Clip::parse(with_junk(FORMER_CHUNKS + 1, &[]), Limits::default(), &|| Ok(())).unwrap();
    assert_eq!((clip.channels, clip.sample_rate, clip.bits, clip.frames), (2, 8_000, 16, 2));
    assert_eq!(clip.duration, Duration::from_micros(250));
    assert_eq!(clip.decode(0, 2, &|| Ok(())).unwrap(), [i16::MIN, 0, i16::MAX, -1]);
    assert_eq!(clip.decode(1, 1, &|| Ok(())).unwrap(), [i16::MAX, -1]);
    assert_eq!(clip.frame_at(Duration::from_micros(125)), 1);
}

#[test]
fn wave_source_bounded_chunks_preserve_late_malformed_and_duplicate_refusals() {
    let base = pcm_wave();
    for (tail, expected) in [
        (&b"JUNK"[..], "Truncated RIFF chunk header"),
        (&b"JUNK\x02\0\0\0x"[..], "Truncated RIFF chunk payload"),
        (&b"JUNK\x01\0\0\0x"[..], "Missing RIFF chunk padding"),
        (&base[12..36], "Multiple WAVE format chunks"),
        (&base[36..], "Multiple WAVE data chunks"),
    ] {
        let error = Clip::parse(with_junk(FORMER_CHUNKS + 1, tail), Limits::default(), &|| Ok(())).unwrap_err();
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn wave_chunk_scanning_remains_cancelable_past_the_former_ceiling() {
    let calls = Cell::new(0usize);
    let stop = FORMER_CHUNKS + 4;
    let error = Clip::parse(with_junk(FORMER_CHUNKS + 10, &[]), Limits::default(), &|| {
        calls.set(calls.get() + 1);
        if calls.get() >= stop { Err("late WAVE cancellation".into()) } else { Ok(()) }
    }).unwrap_err();
    assert_eq!(error, "late WAVE cancellation");
    assert_eq!(calls.get(), stop);
    assert_eq!(Clip::parse(pcm_wave(), Limits::default(), &|| Ok(())).unwrap().frames, 2);
}

#[test]
fn wave_source_bounded_chunks_retain_input_duration_and_output_block_admission() {
    let bytes = with_junk(FORMER_CHUNKS + 1, &[]);
    let small = Limits { input_bytes: bytes.len() - 1, ..Limits::default() };
    assert!(Clip::parse(bytes.clone(), small, &|| Ok(())).unwrap_err().contains("input byte allowance"));
    let short = Limits { duration: Duration::from_micros(249), ..Limits::default() };
    assert!(Clip::parse(bytes.clone(), short, &|| Ok(())).unwrap_err().contains("duration allowance"));
    let clip = Clip::parse(bytes, Limits { block_frames: 1, ..Limits::default() }, &|| Ok(())).unwrap();
    assert!(clip.decode(0, 2, &|| Ok(())).unwrap_err().contains("decode-block allowance"));
    assert_eq!(clip.decode(1, 1, &|| Ok(())).unwrap(), [i16::MAX, -1]);
}
