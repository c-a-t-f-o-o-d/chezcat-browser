//! Original local payload/admission controls; no network or audio device use.
use super::*;

fn base() -> Location {
    crate::navigation::explicit(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/fixtures/authored-video.html")).unwrap()
}
fn plan(source: &str, attributes: &str, limits: media::Limits) -> Result<Plan, String> {
    let doc = crate::html::parse(&format!("<video src='{source}' {attributes}></video>"));
    let location = base();
    Plan::new(authored_node(&doc).unwrap(), &doc, &location, &location,
        &crate::csp::PolicySet::new(&location), limits)
}

#[test]
fn authored_local_matroska_payload_contains_actual_original_video_and_pcm_tracks() {
    let asset = plan("scene-inter-av.mkv", "controls", media::Limits::default()).unwrap()
        .with_audio_limits(crate::audio::Limits::default()).load(&AtomicBool::new(false)).unwrap();
    assert_eq!((asset.video.clip.width, asset.video.clip.height, asset.video.clip.frame_count()), (64, 48, 24));
    assert_eq!(asset.video.clip.duration, Duration::from_secs(3));
    let independent = media::Clip::load(include_bytes!("../tools/fixtures/scene-inter.ivf").to_vec(), media::Limits::default(), &|| Ok(())).unwrap();
    assert_eq!(asset.video.first.pixels, independent.first.pixels);
    let audio = asset.audio.expect("Typed muxed result lost its real PCM track");
    assert_eq!((audio.channels, audio.sample_rate, audio.bits, audio.frames), (2, 48000, 16, 144000));
    assert_eq!(audio.duration, asset.video.clip.duration);
    let samples = audio.decode(0, 32, &|| Ok(())).unwrap();
    let expected: Vec<_> = include_bytes!("../tools/fixtures/scene-inter-av.pcm")[..128].chunks_exact(2)
        .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]])).collect();
    assert_eq!(samples, expected, "The payload must contain decoded original PCM, not a placeholder");
}

#[test]
fn authored_muxed_load_requires_captured_host_audio_limits_and_recovers_from_small_profiles() {
    assert!(plan("scene-inter-av.mkv", "", media::Limits::default()).unwrap().load(&AtomicBool::new(false))
        .err().unwrap().contains("owning App audio limits"));
    for limits in [
        crate::audio::Limits { input_bytes: 1, ..crate::audio::Limits::default() },
        crate::audio::Limits { channels: 1, ..crate::audio::Limits::default() },
        crate::audio::Limits { sample_rate: 1, ..crate::audio::Limits::default() },
        crate::audio::Limits { duration: Duration::from_millis(100), ..crate::audio::Limits::default() },
    ] {
        assert!(plan("scene-inter-av.mkv", "", media::Limits::default()).unwrap().with_audio_limits(limits)
            .load(&AtomicBool::new(false)).is_err());
    }
    assert!(plan("scene-inter-av.mkv", "", media::Limits { frames: 1, ..media::Limits::default() }).unwrap()
        .with_audio_limits(crate::audio::Limits::default()).load(&AtomicBool::new(false)).is_err());
    assert!(plan("scene-inter-av.mkv", "", media::Limits::default()).unwrap()
        .with_audio_limits(crate::audio::Limits::default()).load(&AtomicBool::new(false)).is_ok());
}

#[test]
fn authored_muxed_policy_muting_and_unsupported_attributes_refuse_before_loading() {
    for attribute in ["muted", "muted=false", "loop", "poster=x", "autoplay", "crossorigin=anonymous"] {
        assert!(plan("scene-inter-av.mkv", attribute, media::Limits::default()).is_err(), "{attribute}");
    }
    assert!(plan("../../Cargo.toml", "", media::Limits::default()).is_err());
    assert!(plan("https://example.test/clip.mkv", "", media::Limits::default()).is_err());
    let location = base();
    let doc = crate::html::parse("<video src=scene-inter-av.mkv></video>");
    let mut policy = crate::csp::PolicySet::new(&location); policy.add_header("media-src 'none'");
    assert!(Plan::new(authored_node(&doc).unwrap(), &doc, &location, &location, &policy, media::Limits::default()).is_err());
    // Muted video-only IVF had no audio behavior and retains its old admission.
    assert!(plan("scene-inter.ivf", "muted", media::Limits::default()).is_ok());
}

#[test]
fn authored_muxed_cancellation_and_retired_source_never_return_a_payload() {
    let selected = plan("scene-inter-av.mkv", "", media::Limits::default()).unwrap()
        .with_audio_limits(crate::audio::Limits::default());
    assert!(selected.load(&AtomicBool::new(true)).err().unwrap().contains("cancelled"));
    selected.retired.store(true, Ordering::Relaxed);
    assert!(selected.load(&AtomicBool::new(false)).err().unwrap().contains("cancelled"));
}

#[test]
fn authored_network_mkv_spelling_stays_ivf_only_and_ivf_payload_has_no_audio() {
    let location = crate::navigation::address("https://example.test/page", &Location::home()).unwrap();
    let doc = crate::html::parse("<video src='/same-origin-spelling.mkv'></video>");
    let selected = Plan::new(authored_node(&doc).unwrap(), &doc, &location, &location,
        &crate::csp::PolicySet::new(&location), media::Limits::default()).unwrap()
        .with_audio_limits(crate::audio::Limits::default());
    assert_eq!(selected.kind, SourceKind::Ivf);
    assert!(selected.load(&AtomicBool::new(false)).err().unwrap().contains("no owning resource context"));
    let plain = plan("scene-inter.ivf", "", media::Limits::default()).unwrap().load(&AtomicBool::new(false)).unwrap();
    assert!(plain.audio.is_none()); assert_eq!(plain.video.clip.frame_count(), 24);
}
