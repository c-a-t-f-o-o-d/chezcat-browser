//! Original finite local Matroska slice. RFC9559/RFC8794 and Matroska codec
//! mappings; no borrowed demuxer, external runtime or WebM claim.
use crate::{audio, media};
use std::{ops::Range, sync::Arc, time::Duration};
pub struct Asset {
    pub video: media::Asset,
    pub audio: Arc<audio::Clip>,
}
struct Element {
    id: u64,
    body: Range<usize>,
}
struct Reader<'a> {
    data: &'a [u8],
    left: usize,
    check: &'a dyn Fn() -> Result<(), String>,
}
fn vint(data: &[u8], at: &mut usize, id: bool) -> Result<u64, String> {
    let first = *data.get(*at).ok_or("Truncated EBML variable integer.")?;
    let n = first.leading_zeros() as usize + 1;
    if n > if id { 4 } else { 8 } {
        return Err("Invalid EBML integer width.".into());
    }
    let mut value = u64::from(if id {
        first
    } else {
        first & ((1u8 << (8 - n)) - 1)
    });
    for byte in data
        .get(*at + 1..at.checked_add(n).ok_or("EBML offset overflow.")?)
        .ok_or("Truncated EBML integer.")?
    {
        value = (value << 8) | u64::from(*byte);
    }
    *at += n;
    if !id && value == (1u64 << (7 * n)) - 1 {
        return Err("Unknown-sized EBML elements/track integers are unsupported.".into());
    }
    Ok(value)
}
impl Reader<'_> {
    fn next(&mut self, at: &mut usize, end: usize) -> Result<Option<Element>, String> {
        (self.check)()?;
        if *at == end {
            return Ok(None);
        }
        if *at > end || end > self.data.len() || self.left == 0 {
            return Err("Matroska element boundary/work allowance exceeded.".into());
        }
        self.left -= 1;
        let parent = &self.data[..end];
        let id = vint(parent, at, true)?;
        let len = usize::try_from(vint(parent, at, false)?)
            .map_err(|_| "EBML element length exceeds this machine.")?;
        let finish = at
            .checked_add(len)
            .filter(|&n| n <= end)
            .ok_or("EBML child exceeds its parent.")?;
        let body = *at..finish;
        *at = finish;
        if id == 0xbf {
            return Err("Matroska CRC elements are not implemented in this slice.".into());
        }
        Ok(Some(Element { id, body }))
    }
    fn uint(&self, e: &Element) -> Result<u64, String> {
        let bytes = &self.data[e.body.clone()];
        if bytes.len() > 8 {
            return Err("Matroska integer exceeds 64 bits.".into());
        }
        Ok(bytes
            .iter()
            .fold(0, |value, &b| (value << 8) | u64::from(b)))
    }
    fn float(&self, e: &Element) -> Result<f64, String> {
        let b = &self.data[e.body.clone()];
        let n = match b.len() {
            4 => f32::from_be_bytes(b.try_into().unwrap()) as f64,
            8 => f64::from_be_bytes(b.try_into().unwrap()),
            _ => return Err("Unsupported Matroska float width.".into()),
        };
        if !n.is_finite() {
            return Err("Nonfinite Matroska float.".into());
        }
        Ok(n)
    }
}
fn unique<T>(slot: &mut Option<T>, value: T) -> Result<(), String> {
    if slot.replace(value).is_some() {
        Err("Duplicate singleton Matroska element.".into())
    } else {
        Ok(())
    }
}
#[derive(Default)]
struct Track {
    number: Option<u64>,
    uid: Option<u64>,
    kind: Option<u64>,
    codec: Option<u8>,
    step: Option<u64>,
    width: Option<u64>,
    height: Option<u64>,
    channels: Option<u64>,
    rate: Option<u32>,
    bits: Option<u64>,
}
fn track(r: &mut Reader<'_>, range: Range<usize>) -> Result<Track, String> {
    let mut t = Track::default();
    let mut at = range.start;
    let mut seen = std::collections::HashSet::new();
    while let Some(e) = r.next(&mut at, range.end)? {
        if e.id == 0xec {
            continue;
        }
        if !seen.insert(e.id) {
            return Err("Duplicate Matroska track field.".into());
        }
        match e.id {
            0xd7=>t.number=Some(r.uint(&e)?),0x83=>t.kind=Some(r.uint(&e)?),
            0x86=>t.codec=Some(match &r.data[e.body] {b"V_VP8"=>1,b"A_PCM/INT/LIT"=>2,_=>return Err("Unsupported Matroska codec ID.".into())}),
            0x23e383=>t.step=Some(r.uint(&e)?),
            0xe0|0xe1=>{
                let mut pos=e.body.start;let mut fields=std::collections::HashSet::new();
                while let Some(f)=r.next(&mut pos,e.body.end)? {
                    if f.id==0xec {continue;}if !fields.insert(f.id){return Err("Duplicate Matroska media field.".into());}
                    match (e.id,f.id) {
                        (0xe0,0xb0)=>t.width=Some(r.uint(&f)?),(0xe0,0xba)=>t.height=Some(r.uint(&f)?),
                        (0xe0,0x9a) if matches!(r.uint(&f)?,0|2)=>{},
                        (0xe1,0x9f)=>t.channels=Some(r.uint(&f)?),(0xe1,0x6264)=>t.bits=Some(r.uint(&f)?),
                        (0xe1,0xb5)=>{let rate=r.float(&f)?;if rate<=0.0||rate>u32::MAX as f64||rate.fract()!=0.0{return Err("Muxed PCM requires an integer sample rate.".into());}t.rate=Some(rate as u32);}
                        _=>return Err(format!("Unsupported Matroska video/audio field {:x}.",f.id)),
                    }
                }
            }
            0x73c5=>{let uid=r.uint(&e)?;if uid==0{return Err("Zero Matroska track UID.".into());}t.uid=Some(uid);},
            0xb9 if r.uint(&e)?==1=>{},
            0x88|0x55aa|0x9c if r.uint(&e)?<=1=>{},
            0x23314f if r.float(&e)?==1.0=>{},
            0x56aa|0x56bb if r.uint(&e)?==0=>{},
            _=>return Err(format!("Unsupported Matroska track field {:x}; timing transforms/content encodings are not implemented.",e.id)),
        }
    }
    if t.number.is_none_or(|n| n == 0) || t.codec.is_none() || !seen.contains(&0x73c5) {
        return Err("Missing Matroska track identity.".into());
    }
    // A disabled selected track cannot silently become enabled playback.
    Ok(t)
}
fn append(target: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Result<Range<usize>, String> {
    let start = target.len();
    let end = start
        .checked_add(bytes.len())
        .filter(|&n| n <= limit)
        .ok_or("Demuxed payload byte allowance exceeded.")?;
    target
        .try_reserve(bytes.len())
        .map_err(|_| "Could not grow demuxed payload storage.")?;
    target.extend_from_slice(bytes);
    Ok(start..end)
}
pub fn load(
    data: Vec<u8>,
    vl: media::Limits,
    al: audio::Limits,
    check: &dyn Fn() -> Result<(), String>,
) -> Result<Asset, String> {
    check()?;
    if data.len() > vl.input_bytes.min(al.input_bytes) {
        return Err("Matroska input byte allowance exceeded.".into());
    }
    let mut r = Reader {
        data: &data,
        left: al.chunks,
        check,
    };
    let mut at = 0;
    let header = r
        .next(&mut at, data.len())?
        .filter(|e| e.id == 0x1a45dfa3)
        .ok_or("Missing Matroska EBML header.")?;
    let mut p = header.body.start;
    let mut doc = false;
    let mut seen = std::collections::HashSet::new();
    while let Some(e) = r.next(&mut p, header.body.end)? {
        if e.id == 0xec {
            continue;
        }
        if !seen.insert(e.id) {
            return Err("Duplicate EBML header field.".into());
        }
        match e.id {
            0x4282 if &data[e.body.clone()] == b"matroska" => doc = true,
            0x4286 | 0x42f7 if r.uint(&e)? == 1 => {}
            0x42f2 if r.uint(&e)? == 4 => {}
            0x42f3 if r.uint(&e)? == 8 => {}
            0x4287 | 0x4285 if (1..=4).contains(&r.uint(&e)?) => {}
            _ => return Err("Unsupported EBML/Matroska document version or header field.".into()),
        }
    }
    if !doc {
        return Err("This stage requires Matroska, not WebM or another EBML document.".into());
    }
    let segment = r
        .next(&mut at, data.len())?
        .filter(|e| e.id == 0x18538067)
        .ok_or("Missing finite Matroska Segment.")?;
    if at != data.len() {
        return Err("Multiple/trailing Matroska segments are unsupported.".into());
    }
    let (mut scale, mut info, mut tracks, mut declared) = (1_000_000u64, false, None, None);
    let mut pos = segment.body.start;
    // Read track metadata before the second, bounded pass over Clusters.
    while let Some(e) = r.next(&mut pos, segment.body.end)? {
        match e.id {
            0x1549a966 => {
                if info {
                    return Err("Duplicate Segment Info.".into());
                }
                info = true;
                let mut i = e.body.start;
                let mut seen = std::collections::HashSet::new();
                while let Some(f) = r.next(&mut i, e.body.end)? {
                    if f.id == 0xec {
                        continue;
                    }
                    if !seen.insert(f.id) {
                        return Err("Duplicate Segment Info field.".into());
                    }
                    match f.id {
                        0x2ad7b1 => scale = r.uint(&f)?,
                        0x4489 => declared = Some(r.float(&f)?),
                        0x4d80 | 0x5741 | 0x7ba9 | 0x4461 => {}
                        _ => return Err("Unsupported Segment Info field.".into()),
                    }
                }
            }
            0x1654ae6b => {
                if tracks.is_some() {
                    return Err("Duplicate Tracks.".into());
                }
                let mut ts = Vec::new();
                let mut i = e.body.start;
                while let Some(f) = r.next(&mut i, e.body.end)? {
                    if f.id == 0xec {
                        continue;
                    }
                    if f.id != 0xae || ts.len() >= 2 {
                        return Err("This Matroska slice requires exactly two tracks.".into());
                    }
                    ts.push(track(&mut r, f.body)?);
                }
                tracks = Some(ts);
            }
            0x1f43b675 | 0xec => {}
            _ => return Err(format!("Unsupported Matroska Segment field {:x}.", e.id)),
        }
    }
    if !info || scale == 0 {
        return Err("Missing Segment Info or zero Matroska timestamp scale.".into());
    }
    let tracks = tracks
        .filter(|t| t.len() == 2)
        .ok_or("Exactly one VP8 and one PCM track are required.")?;
    let v = tracks
        .iter()
        .find(|t| t.kind == Some(1) && t.codec == Some(1))
        .ok_or("Missing VP8 video track.")?;
    let a = tracks
        .iter()
        .find(|t| t.kind == Some(2) && t.codec == Some(2))
        .ok_or("Missing little-endian PCM audio track.")?;
    if v.number == a.number || v.uid == a.uid || a.bits != Some(16) {
        return Err("Duplicate track numbers/UIDs or non-PCM16 audio.".into());
    }
    if v.channels.is_some()
        || v.rate.is_some()
        || v.bits.is_some()
        || a.width.is_some()
        || a.height.is_some()
        || a.step.is_some()
    {
        return Err("Track type and audio/video metadata disagree.".into());
    }
    let channels =
        u16::try_from(a.channels.unwrap_or(1)).map_err(|_| "PCM channel count overflow.")?;
    let rate = a.rate.unwrap_or(8000);
    if channels == 0
        || channels > al.channels
        || rate == 0
        || rate > al.sample_rate
        || rate > 1_000_000_000
    {
        return Err("Muxed PCM format allowance exceeded.".into());
    }
    let step = v
        .step
        .filter(|&n| n > 0)
        .ok_or("This video slice requires DefaultDuration.")?;
    let width = usize::try_from(v.width.ok_or("Missing video width.")?)
        .map_err(|_| "Video width overflow.")?;
    let height = usize::try_from(v.height.ok_or("Missing video height.")?)
        .map_err(|_| "Video height overflow.")?;
    let (mut video, mut audio, mut packets, mut audio_frames) =
        (Vec::new(), Vec::new(), Vec::new(), 0u64);
    let mut pos = segment.body.start;
    while let Some(e) = r.next(&mut pos, segment.body.end)? {
        if e.id != 0x1f43b675 {
            continue;
        }
        let mut p = e.body.start;
        let mut stamp = None;
        while let Some(b) = r.next(&mut p, e.body.end)? {
            match b.id {
                0xe7 => unique(&mut stamp, r.uint(&b)?)?,
                0xec => {}
                0xa3 => {
                    let cluster =
                        stamp.ok_or("Cluster Timestamp must precede blocks in this slice.")?;
                    let bytes = &data[b.body];
                    let mut offset = 0;
                    let number = vint(bytes, &mut offset, false)?;
                    let h = bytes
                        .get(offset..offset + 3)
                        .ok_or("Truncated SimpleBlock header.")?;
                    if h[2]&!0x80!=0 {
                        return Err("Only unlaced visible non-discardable SimpleBlocks are implemented.".into());
                    }
                    let ticks = i128::from(cluster) + i128::from(i16::from_be_bytes([h[0], h[1]]));
                    let nanos = u64::try_from(ticks)
                        .ok()
                        .and_then(|t| t.checked_mul(scale))
                        .ok_or("Negative/overflowing Matroska timestamp.")?;
                    let payload = &bytes[offset + 3..];
                    if payload.is_empty() {
                        return Err("Empty Matroska packet.".into());
                    }
                    if Some(number) == v.number {
                        if (h[2]&0x80!=0)!=(payload[0]&1==0) {
                            return Err("Matroska keyframe flag disagrees with the actual VP8 packet.".into());
                        }
                        if packets.len() >= vl.frames || payload.len() > vl.frame_bytes {
                            return Err("Matroska video packet allowance exceeded.".into());
                        }
                        if u64::try_from(packets.len())
                            .ok()
                            .and_then(|n| n.checked_mul(step))
                            != Some(nanos)
                        {
                            return Err("This video slice requires zero-start continuous DefaultDuration timing.".into());
                        }
                        let range = append(&mut video, payload, vl.input_bytes)?;
                        packets
                            .try_reserve(1)
                            .map_err(|_| "Could not grow Matroska video index.")?;
                        packets.push((range, Duration::from_nanos(nanos)));
                    } else if Some(number) == a.number {
                        if h[2]!=0x80 {return Err("PCM SimpleBlocks must remain independent.".into());}
                        let align = usize::from(channels) * 2;
                        if payload.len() % align != 0
                            || u128::from(audio_frames) * 1_000_000_000
                                / u128::from(rate)
                                / u128::from(scale)
                                != u128::from(nanos / scale)
                        {
                            return Err("PCM packets must start at zero with continuous sample timing; gaps/overlaps are unsupported.".into());
                        }
                        audio_frames = audio_frames
                            .checked_add(
                                u64::try_from(payload.len() / align)
                                    .map_err(|_| "PCM packet frame count overflow.")?,
                            )
                            .ok_or("PCM sample count overflow.")?;
                        append(&mut audio, payload, al.input_bytes)?;
                    } else {
                        return Err("SimpleBlock refers to an unknown track.".into());
                    }
                }
                _ => return Err("Unsupported Matroska Cluster field/BlockGroup.".into()),
            }
        }
        if stamp.is_none() {
            return Err("Missing Cluster Timestamp.".into());
        }
    }
    let audio = Arc::new(audio::Clip::from_pcm16(audio, channels, rate, al, check)?);
    let end = u64::try_from(packets.len())
        .ok()
        .and_then(|n| n.checked_mul(step))
        .ok_or("Video end timestamp overflow.")?;
    if audio.duration != Duration::from_nanos(end)
        || declared
            .is_some_and(|d| d <= 0.0 || (d * scale as f64 - end as f64).abs() > scale as f64)
    {
        return Err("Muxed track end/duration mismatch is unsupported.".into());
    }
    let clip = Arc::new(media::Clip::from_packets(
        video,
        packets,
        width,
        height,
        audio.duration,
        vl,
        check,
    )?);
    let initial_index = clip.index_at(Duration::ZERO);
    let first = clip.decode(initial_index, check)?;
    Ok(Asset {
        video: media::Asset {
            clip,
            first,
            initial_index,
        },
        audio,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../tools/fixtures/scene-av.mkv");
    fn read(bytes: Vec<u8>) -> Result<Asset, String> {
        load(
            bytes,
            media::Limits::default(),
            audio::Limits::default(),
            &|| Ok(()),
        )
    }
    #[test]
    fn matroska_real_tracks_share_timestamps_and_decode_original_packets() {
        let asset = read(FIXTURE.to_vec()).unwrap();
        assert_eq!(asset.video.clip.frame_count(), 6);
        assert_eq!(asset.audio.frames, 144000);
        assert_eq!(asset.video.clip.duration, Duration::from_secs(3));
        assert_eq!(asset.audio.duration, Duration::from_secs(3));
        let video = media::Clip::parse(
            include_bytes!("../tools/fixtures/scene-intra.ivf").to_vec(),
            media::Limits::default(),
            &|| Ok(()),
        )
        .unwrap();
        for index in 0..6 {
            let actual = asset.video.clip.decode(index, &|| Ok(())).unwrap();
            let reference = video.decode(index, &|| Ok(())).unwrap();
            assert_eq!(actual.pixels, reference.pixels);
            assert_eq!(
                asset
                    .video
                    .clip
                    .index_at(Duration::from_millis(index as u64 * 500)),
                index
            );
        }
        // The independently authored PCM signal crosses 6,000-frame packet
        // boundaries without resetting its oscillator or manufacturing silence.
        let block = asset.audio.decode(5990, 25, &|| Ok(())).unwrap();
        for n in 5990..6015 {
            let t = n as f64 / 48000.0;
            let left = (2800.0 * (2.0 * std::f64::consts::PI * 220.0 * t).sin()).round() as i16;
            assert_eq!(block[(n - 5990) * 2], left);
        }
    }
    fn replace_first(bytes: &mut [u8], old: &[u8], new: &[u8]) {
        let p = bytes.windows(old.len()).position(|b| b == old).unwrap();
        assert_eq!(old.len(), new.len());
        bytes[p..p + old.len()].copy_from_slice(new);
    }
    #[test]
    fn matroska_rejects_timing_lacing_encoding_and_identity_changes() {
        for (old, new) in [
            (&b"matroska"[..], &b"webmxxxx"[..]),
            (&b"V_VP8"[..], &b"V_VP9"[..]),
            (&[0x42,0xf2,0x81,4][..],&[0x42,0xf2,0x81,1][..]),
            (&[0x42,0xf3,0x81,8][..],&[0x42,0xf3,0x81,1][..]),
            (&[0x73,0xc5,0x81,2][..],&[0x73,0xc5,0x81,1][..]),
            (&[0x81,0,0,0x80][..],&[0x81,0xff,0xff,0x80][..]),
            (&[0x9c, 0x81, 0][..], &[0xb9, 0x81, 0][..]),
            (&[0x81, 0, 0, 0x80][..], &[0x81, 0, 0, 0x82][..]),
            (&[0xe7, 0x81, 0][..], &[0xe7, 0x81, 1][..]),
            (&[0x62, 0x64, 0x81, 16][..], &[0x62, 0x64, 0x81, 24][..]),
        ] {
            let mut bytes = FIXTURE.to_vec();
            replace_first(&mut bytes, old, new);
            assert!(read(bytes).is_err(), "mutation {old:?}");
        }
        for length in [0, 1, 4, 16, 40, FIXTURE.len() - 1] {
            assert!(read(FIXTURE[..length].to_vec()).is_err());
        }
        let mut bytes = FIXTURE.to_vec();
        let mut at = 0;
        vint(&bytes, &mut at, true).unwrap();
        bytes[at] = 0xff;
        assert!(matches!(read(bytes),Err(e) if e.contains("Unknown-sized")));
    }
    #[test]
    fn matroska_small_profiles_cancellation_and_bad_vints_fail_cleanly() {
        let vl = media::Limits {
            frames: 2,
            ..media::Limits::default()
        };
        assert!(load(FIXTURE.to_vec(), vl, audio::Limits::default(), &|| Ok(())).is_err());
        let al = audio::Limits {
            chunks: 8,
            ..audio::Limits::default()
        };
        assert!(load(FIXTURE.to_vec(), media::Limits::default(), al, &|| Ok(())).is_err());
        let al = audio::Limits {
            input_bytes: 100,
            ..audio::Limits::default()
        };
        assert!(load(FIXTURE.to_vec(), media::Limits::default(), al, &|| Ok(())).is_err());
        let checks = std::cell::Cell::new(0);
        let result = load(
            FIXTURE.to_vec(),
            media::Limits::default(),
            audio::Limits::default(),
            &|| {
                checks.set(checks.get() + 1);
                if checks.get() > 15 {
                    Err("cancelled-original-fixture".into())
                } else {
                    Ok(())
                }
            },
        );
        assert!(matches!(result,Err(ref e) if e=="cancelled-original-fixture"));
        for data in [&[0u8][..], &[0x40][..], &[0x1f, 0xff][..]] {
            assert!(vint(data, &mut 0, false).is_err());
        }
        assert!(read(FIXTURE.to_vec()).is_ok());
    }

    #[test]
    fn matroska_dependent_vp8_and_pcm_match_original_standalone_tracks() {
        let bytes=include_bytes!("../tools/fixtures/scene-inter-av.mkv");
        let asset=load(bytes.to_vec(),media::Limits::default(),audio::Limits::default(),&||Ok(())).unwrap();
        let clip=media::Clip::parse(include_bytes!("../tools/fixtures/scene-inter.ivf").to_vec(),media::Limits::default(),&||Ok(())).unwrap();
        assert_eq!(asset.video.clip.frame_count(),24);assert_eq!(asset.video.clip.duration,Duration::from_secs(3));
        for index in [0,7,12,17,23,2] {
            assert_eq!(asset.video.clip.decode(index,&||Ok(())).unwrap().pixels,clip.decode(index,&||Ok(())).unwrap().pixels);
        }
        // The existing audio acceptance owns sample/device clock behavior; this
        // checks the exact demuxed original PCM data independently of video.
        assert_eq!(asset.audio.duration,Duration::from_secs(3));
        assert_eq!((asset.audio.channels,asset.audio.sample_rate,asset.audio.frames),(2,48000,144000));
        let pcm=include_bytes!("../tools/fixtures/scene-inter-av.pcm");let mut frame=0u64;
        while frame<asset.audio.frames {
            let block=asset.audio.decode(frame,asset.audio.decode_block_frames(),&||Ok(())).unwrap();
            assert!(!block.is_empty());let start=frame as usize*4;
            for(sample,bytes)in block.iter().zip(pcm[start..start+block.len()*2].chunks_exact(2)) {
                assert_eq!(*sample,i16::from_le_bytes([bytes[0],bytes[1]]));
            }
            frame+=block.len() as u64/2;
        }
        for index in 0..24 {assert_eq!(asset.audio.frame_at(Duration::from_millis(index*125)),index*6000);}

    }
}
