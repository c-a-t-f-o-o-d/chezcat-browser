//! Original bounded RIFF/WAVE PCM parsing and incremental integer sample decode.
//! No external codec, process, network or device output is involved here.
use std::ops::Range;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub input_bytes: usize,
    pub channels: u16,
    pub sample_rate: u32,
    pub duration: Duration,
    pub block_frames: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            input_bytes: 512 * 1024 * 1024,
            channels: 32,
            sample_rate: 768_000,
            duration: Duration::from_secs(7 * 24 * 3600),
            block_frames: 16_384,
        }
    }
}
pub struct Clip {
    data: Vec<u8>,
    samples: Range<usize>,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits: u16,
    pub frames: u64,
    pub duration: Duration,
    align: usize,
    limits: Limits,
}
impl std::fmt::Debug for Clip {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PcmClip")
            .field("encoded_bytes", &self.data.len())
            .field("channels", &self.channels)
            .field("sample_rate", &self.sample_rate)
            .field("bits", &self.bits)
            .field("frames", &self.frames)
            .field("duration", &self.duration)
            .finish()
    }
}

fn word(data: &[u8], at: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(
        data.get(at..at + 2)
            .ok_or("Truncated WAVE format.")?
            .try_into()
            .unwrap(),
    ))
}
fn dword(data: &[u8], at: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        data.get(at..at + 4)
            .ok_or("Truncated RIFF chunk.")?
            .try_into()
            .unwrap(),
    ))
}
impl Clip {
    /// Real, already-demuxed continuous PCM16; no WAVE wrapper or resampling.
    pub(crate) fn from_pcm16(data:Vec<u8>,channels:u16,sample_rate:u32,limits:Limits,check:&dyn Fn()->Result<(),String>)->Result<Self,String> {
        check()?;
        let align=usize::from(channels).checked_mul(2).ok_or("Muxed PCM alignment overflow.")?;
        if data.len()>limits.input_bytes||channels==0||channels>limits.channels||sample_rate==0||sample_rate>limits.sample_rate||sample_rate>1_000_000_000||limits.block_frames==0||data.is_empty()||data.len()%align!=0 {return Err("Muxed PCM16 format/frame/byte allowance exceeded.".into());}
        let frames=u64::try_from(data.len()/align).map_err(|_|"Muxed PCM frame count overflow.")?;
        let nanos=(u128::from(frames)*1_000_000_000).div_ceil(u128::from(sample_rate));
        let duration=Duration::from_nanos(u64::try_from(nanos).map_err(|_|"Muxed PCM duration overflow.")?);
        if duration>limits.duration {return Err("Muxed PCM duration allowance exceeded.".into());}
        let samples=0..data.len();
        Ok(Self {data,samples,channels,sample_rate,bits:16,frames,duration,align,limits})
    }
    pub fn parse(
        data: Vec<u8>,
        limits: Limits,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Self, String> {
        check()?;
        if limits.block_frames == 0 {
            return Err("Audio decode-block allowance must permit progress.".into());
        }
        if data.len() > limits.input_bytes {
            return Err("Audio input byte allowance exceeded.".into());
        }
        if data.get(..4) != Some(b"RIFF") || data.get(8..12) != Some(b"WAVE") {
            return Err("Expected a little-endian RIFF/WAVE file.".into());
        }
        let end = usize::try_from(dword(&data, 4)?)
            .ok()
            .and_then(|size| size.checked_add(8))
            .filter(|&end| end == data.len())
            .ok_or("RIFF declared/actual size mismatch.")?;
        let (mut position, mut format, mut samples) = (12usize, None, None);
        while position < end {
            check()?;
            // Every chunk consumes at least its eight-byte header from the
            // admitted input. Ignored chunks retain no index or copied data.
            let begin = position
                .checked_add(8)
                .filter(|&at| at <= end)
                .ok_or("Truncated RIFF chunk header.")?;
            let size = usize::try_from(dword(&data, position + 4)?)
                .map_err(|_| "WAVE chunk size exceeds this machine.")?;
            let finish = begin
                .checked_add(size)
                .filter(|&at| at <= end)
                .ok_or("Truncated RIFF chunk payload.")?;
            let next = finish
                .checked_add(size & 1)
                .filter(|&at| at <= end)
                .ok_or("Missing RIFF chunk padding.")?;
            match &data[position..position + 4] {
                b"fmt " => {
                    if format.is_some() {
                        return Err("Multiple WAVE format chunks are not supported.".into());
                    }
                    let bytes = &data[begin..finish];
                    if bytes.len() < 16 {
                        return Err("WAVE PCM format is truncated.".into());
                    }
                    if word(bytes, 0)? != 1 {
                        return Err("This audio stage supports integer PCM only; compressed, extensible and floating-point WAVE are not implemented.".into());
                    }
                    if bytes.len() == 17
                        || bytes.len() >= 18
                            && usize::from(word(bytes, 16)?).checked_add(18) != Some(bytes.len())
                    {
                        return Err("WAVE format extension size mismatch.".into());
                    }
                    let (channels, rate, byte_rate, align, bits) = (
                        word(bytes, 2)?,
                        dword(bytes, 4)?,
                        dword(bytes, 8)?,
                        word(bytes, 12)?,
                        word(bytes, 14)?,
                    );
                    if channels == 0
                        || channels > limits.channels
                        || rate == 0
                        || rate > limits.sample_rate
                        || rate > 1_000_000_000
                        || !matches!(bits, 8 | 16 | 24 | 32)
                    {
                        return Err("Audio channels/sample-rate/PCM-width allowance or supported format exceeded.".into());
                    }
                    let expected = channels
                        .checked_mul(bits / 8)
                        .ok_or("PCM block alignment overflow.")?;
                    if align != expected || rate.checked_mul(u32::from(align)) != Some(byte_rate) {
                        return Err("PCM byte-rate or block alignment is inconsistent.".into());
                    }
                    format = Some((channels, rate, bits, usize::from(align)));
                }
                b"data" => {
                    if samples.replace(begin..finish).is_some() {
                        return Err(
                            "Multiple WAVE data chunks are not supported in this slice.".into()
                        );
                    }
                }
                _ => {}
            }
            position = next;
        }
        let (channels, sample_rate, bits, align) =
            format.ok_or("WAVE PCM format chunk is missing.")?;
        let samples = samples.ok_or("WAVE data chunk is missing.")?;
        if samples.is_empty() || samples.len() % align != 0 {
            return Err("WAVE data must contain complete, nonempty PCM frames.".into());
        }
        let frames =
            u64::try_from(samples.len() / align).map_err(|_| "Audio sample count overflow.")?;
        let nanos = u128::from(frames)
            .checked_mul(1_000_000_000)
            .ok_or("Audio duration overflow.")?
            .div_ceil(u128::from(sample_rate));
        let duration = Duration::from_nanos(
            u64::try_from(nanos).map_err(|_| "Audio duration exceeds the clock representation.")?,
        );
        if duration.is_zero() || duration > limits.duration {
            return Err("Audio duration allowance exceeded or below clock precision.".into());
        }
        Ok(Self {
            data,
            samples,
            channels,
            sample_rate,
            bits,
            frames,
            duration,
            align,
            limits,
        })
    }
    pub fn frame_at(&self, position: Duration) -> u64 {
        if position >= self.duration {
            return self.frames;
        }
        u64::try_from(
            position
                .as_nanos()
                .saturating_mul(u128::from(self.sample_rate))
                / 1_000_000_000,
        )
        .unwrap_or(u64::MAX)
        .min(self.frames)
    }
    pub fn decode_block_frames(&self) -> usize {
        self.limits.block_frames
    }
    pub fn time_at(&self, frame: u64) -> Duration {
        Duration::from_nanos(
            (u128::from(frame.min(self.frames)) * 1_000_000_000)
                .div_ceil(u128::from(self.sample_rate)) as u64,
        )
    }
    /// Decode only the requested bounded block to signed interleaved PCM16.
    /// Higher integer precision is quantized by dropping low bits, without
    /// resampling, channel mixing, dithering or pretending another codec works.
    pub fn decode(
        &self,
        frame: u64,
        maximum_frames: usize,
        check: &dyn Fn() -> Result<(), String>,
    ) -> Result<Vec<i16>, String> {
        check()?;
        if frame > self.frames {
            return Err("Audio seek frame is outside the clip.".into());
        }
        if maximum_frames > self.limits.block_frames {
            return Err("Audio decode-block allowance exceeded.".into());
        }
        let count = maximum_frames.min(usize::try_from(self.frames - frame).unwrap_or(usize::MAX));
        let length = count
            .checked_mul(usize::from(self.channels))
            .ok_or("Audio decode length overflow.")?;
        let offset = usize::try_from(frame)
            .ok()
            .and_then(|frame| frame.checked_mul(self.align))
            .and_then(|at| self.samples.start.checked_add(at))
            .ok_or("Audio data offset overflow.")?;
        let finish = offset
            .checked_add(
                count
                    .checked_mul(self.align)
                    .ok_or("Audio byte count overflow.")?,
            )
            .filter(|&end| end <= self.samples.end)
            .ok_or("Audio data range is invalid.")?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| "Could not reserve the audio decode block.")?;
        let width = usize::from(self.bits / 8);
        for (index, sample) in self.data[offset..finish].chunks_exact(width).enumerate() {
            if index % 256 == 0 {
                check()?;
            }
            let value = match self.bits {
                8 => (i16::from(sample[0]) - 128) << 8,
                16 => i16::from_le_bytes(sample.try_into().unwrap()),
                24 => i16::from_le_bytes([sample[1], sample[2]]),
                32 => i16::from_le_bytes([sample[2], sample[3]]),
                _ => unreachable!(),
            };
            output.push(value);
        }
        Ok(output)
    }
}

#[cfg(test)]
#[path = "audio_capacity_tests.rs"]
mod capacity_tests;

#[cfg(test)]
mod tests {
    use super::*;
    const SCENE: &[u8] = include_bytes!("../tools/fixtures/original-tone.wav");
    fn clip() -> Clip {
        Clip::parse(SCENE.to_vec(), Limits::default(), &|| Ok(())).unwrap()
    }
    fn wave(bits: u16, channels: u16, rate: u32, samples: &[u8], junk: bool) -> Vec<u8> {
        let mut data = b"RIFF\0\0\0\0WAVE".to_vec();
        let mut format = Vec::new();
        format.extend_from_slice(&1u16.to_le_bytes());
        format.extend_from_slice(&channels.to_le_bytes());
        format.extend_from_slice(&rate.to_le_bytes());
        let align = channels * (bits / 8);
        format.extend_from_slice(&(rate * u32::from(align)).to_le_bytes());
        format.extend_from_slice(&align.to_le_bytes());
        format.extend_from_slice(&bits.to_le_bytes());
        let mut chunk = |name: &[u8], bytes: &[u8]| {
            data.extend_from_slice(name);
            data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(bytes);
            if bytes.len() % 2 == 1 {
                data.push(0);
            }
        };
        if junk {
            chunk(b"JUNK", b"abc");
        }
        chunk(b"fmt ", &format);
        chunk(b"data", samples);
        let size = (data.len() - 8) as u32;
        data[4..8].copy_from_slice(&size.to_le_bytes());
        data
    }
    #[test]
    fn local_audio_all_integer_widths_decode_extrema_and_preserve_stereo_interleaving() {
        for (bits, data, expected) in [
            (8, vec![0, 128, 255, 127], vec![-32768, 0, 32512, -256]),
            (
                16,
                [-32768i16, 0, 32767, -1]
                    .into_iter()
                    .flat_map(i16::to_le_bytes)
                    .collect(),
                vec![-32768, 0, 32767, -1],
            ),
            (
                24,
                [-8_388_608i32, 0, 8_388_607, -256]
                    .into_iter()
                    .flat_map(|value| value.to_le_bytes()[..3].to_vec())
                    .collect(),
                vec![-32768, 0, 32767, -1],
            ),
            (
                32,
                [i32::MIN, 0, i32::MAX, -65536]
                    .into_iter()
                    .flat_map(i32::to_le_bytes)
                    .collect(),
                vec![-32768, 0, 32767, -1],
            ),
        ] {
            let clip = Clip::parse(
                wave(bits, 2, 44100, &data, true),
                Limits::default(),
                &|| Ok(()),
            )
            .unwrap();
            assert_eq!(clip.frames, 2);
            assert_eq!(
                clip.decode(0, 2, &|| Ok(())).unwrap(),
                expected,
                "PCM{bits}"
            );
            assert_eq!(clip.decode(1, 1, &|| Ok(())).unwrap(), expected[2..]);
        }
    }
    #[test]
    fn local_audio_frame_boundary_roundtrips_eof_padding_and_mid_decode_cancel_are_consistent() {
        for rate in [44100, 48000, 768000] {
            for frames in [1usize, 2, 13] {
                let clip = Clip::parse(
                    wave(8, 1, rate, &vec![128; frames], true),
                    Limits::default(),
                    &|| Ok(()),
                )
                .unwrap();
                for frame in 0..=clip.frames {
                    assert_eq!(
                        clip.frame_at(clip.time_at(frame)),
                        frame,
                        "rate {rate} frame {frame}"
                    );
                }
                assert_eq!(clip.frame_at(clip.duration), clip.frames);
            }
        }
        let mut data = wave(8, 1, 48000, &[128], false);
        data.pop();
        let size = (data.len() - 8) as u32;
        data[4..8].copy_from_slice(&size.to_le_bytes());
        assert!(Clip::parse(data, Limits::default(), &|| Ok(()))
            .unwrap_err()
            .contains("padding"));
        let calls = std::cell::Cell::new(0);
        assert!(clip()
            .decode(0, 4096, &|| {
                calls.set(calls.get() + 1);
                if calls.get() > 2 {
                    Err("mid-decode cancellation".into())
                } else {
                    Ok(())
                }
            })
            .unwrap_err()
            .contains("mid-decode"));
        assert!(!clip().decode(0, 4096, &|| Ok(())).unwrap().is_empty());
    }
    #[test]
    fn local_audio_original_pcm_samples_duration_seek_and_bounded_blocks_are_real() {
        let clip = clip();
        assert_eq!(
            (clip.channels, clip.sample_rate, clip.bits, clip.frames),
            (2, 48_000, 16, 96_000)
        );
        assert_eq!(clip.duration, Duration::from_secs(2));
        let samples = clip.decode(0, 4096, &|| Ok(())).unwrap();
        assert_eq!(samples.len(), 8192);
        assert!(samples.iter().any(|&sample| sample > 1000));
        assert!(samples.iter().any(|&sample| sample < -1000));
        assert_eq!(samples[0], 0);
        assert_eq!(clip.frame_at(Duration::from_millis(750)), 36_000);
        assert_eq!(clip.time_at(36_000), Duration::from_millis(750));
        assert!(clip.decode(clip.frames, 10, &|| Ok(())).unwrap().is_empty());
        assert!(clip
            .decode(0, Limits::default().block_frames + 1, &|| Ok(()))
            .is_err());
        assert!(clip.decode(clip.frames + 1, 1, &|| Ok(())).is_err());
    }
    #[test]
    fn local_audio_riff_truncation_format_alignment_small_profiles_and_cancellation_refuse() {
        for length in [0, 4, 11, 20, SCENE.len() - 1] {
            assert!(Clip::parse(SCENE[..length].to_vec(), Limits::default(), &|| Ok(())).is_err());
        }
        for (at, byte) in [
            (0, b'X'),
            (8, b'X'),
            (20, 3),
            (22, 0),
            (24, 0),
            (32, 1),
            (34, 12),
        ] {
            let mut data = SCENE.to_vec();
            data[at] = byte;
            if at == 24 {
                data[24..28].fill(0);
            }
            assert!(
                Clip::parse(data, Limits::default(), &|| Ok(())).is_err(),
                "offset {at}"
            );
        }
        for limits in [
            Limits {
                input_bytes: 64,
                ..Limits::default()
            },
            Limits {
                channels: 1,
                ..Limits::default()
            },
            Limits {
                sample_rate: 8000,
                ..Limits::default()
            },
            Limits {
                block_frames: 0,
                ..Limits::default()
            },
            Limits {
                duration: Duration::from_secs(1),
                ..Limits::default()
            },
        ] {
            assert!(Clip::parse(SCENE.to_vec(), limits, &|| Ok(())).is_err());
        }
        assert!(clip()
            .decode(0, 4096, &|| Err("cancelled".into()))
            .unwrap_err()
            .contains("cancelled"));
    }
}
