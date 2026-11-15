// Original owned HTTP full-fetch/type/Plan controls. No TLS/device or live use.
use super::*;
const MKV: &[u8] = include_bytes!("../tools/fixtures/scene-inter-av.mkv");
const INTER_IVF: &[u8] = include_bytes!("../tools/fixtures/scene-inter.ivf");

fn decode(context: Context, target: &HttpUrl, muted: bool, audio: Option<crate::audio::Limits>) -> Result<crate::embedded_video::Asset, String> {
    let location = Location { url: Some(context.origin.clone()), path: None, root: None, fragment: String::new() };
    let crossorigin = match context.credentials { None => "", Some(http::MediaCredentials::SameOrigin) => "crossorigin=anonymous", Some(http::MediaCredentials::Include) => "crossorigin=use-credentials" };
    let doc = crate::html::parse(&format!("<video src='{}' controls {crossorigin} {}></video>",target.display(),if muted {"muted"}else{""}));
    let mut plan = crate::embedded_video::Plan::new(crate::embedded_video::authored_node(&doc).unwrap(), &doc,
        &location, &location, &context.policy, crate::media::Limits::default())?;
    plan.retired = context.retired.clone();
    if let Some(limits) = audio { plan = plan.with_audio_limits(limits); }
    let plan = Arc::new(plan.with_network_context(context)?);
    let worker = crate::embedded_video::Worker::new(Arc::new(|| {}))?;
    worker.submit(plan.clone());
    let until = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(completion) = worker.take() {
            assert!(Arc::ptr_eq(&completion.plan, &plan));
            return completion.result;
        }
        assert!(std::time::Instant::now() < until, "Real network source worker timed out");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn network_typed_mime_and_real_plan_choose_decoder_independently_of_suffix() {
    let server = Server::new(vec![
        ("/looks.ivf", response(200,"Content-Type: VIDEO/X-MATROSKA; note=\"original; clip\"\r\n",MKV)),
        ("/looks.mkv", response(200,"Content-Type: video/x-ivf\r\n",INTER_IVF)),
        ("/no-suffix", response(200,"Content-Type: video/x-matroska\r\n",MKV)),
    ]);
    for path in ["/looks.ivf", "/no-suffix"] {
        let asset = decode(server.context(), &server.url(path), false, Some(crate::audio::Limits::default())).unwrap();
        assert_eq!((asset.video.clip.width,asset.video.clip.height,asset.video.clip.frame_count()),(64,48,24));
        assert_eq!(asset.video.clip.duration,Duration::from_secs(3));
        let pcm = asset.audio.expect("Real network muxed payload lost PCM");
        assert_eq!((pcm.channels,pcm.sample_rate,pcm.bits,pcm.frames),(2,48000,16,144000));
        let actual = pcm.decode(0,32,&||Ok(())).unwrap();
        let expected: Vec<_> = include_bytes!("../tools/fixtures/scene-inter-av.pcm")[..128].chunks_exact(2)
            .map(|bytes|i16::from_le_bytes([bytes[0],bytes[1]])).collect();
        assert_eq!(actual,expected);
    }
    let asset = decode(server.context(), &server.url("/looks.mkv"), false, Some(crate::audio::Limits {input_bytes:1,..crate::audio::Limits::default()})).unwrap();
    assert!(asset.audio.is_none(),"An audio policy must not shrink or change video-only IVF");
    assert_eq!(asset.video.clip.frame_count(),24);
    let requests=server.requests();assert_eq!(requests.len(),3);
    assert!(requests.iter().all(|head|!head.contains("Range:")&&!head.contains("If-Range:")));
}

#[test]
fn network_wrong_container_mime_never_guesses_or_falls_back_to_another_decoder() {
    let server=Server::new(vec![
        ("/wrong-ivf",response(200,"Content-Type: video/x-ivf\r\n",MKV)),
        ("/wrong-mkv",response(200,"Content-Type: video/x-matroska\r\n",INTER_IVF)),
        ("/legacy",response(200,"Content-Type: video/x-matroska\r\n",MKV)),
    ]);
    for path in ["/wrong-ivf","/wrong-mkv"] {
        assert!(decode(server.context(),&server.url(path),false,Some(crate::audio::Limits::default())).is_err());
    }
    // Existing callers without captured audio policy retain strict IVF-only IO.
    assert!(server.context().load(&server.url("/legacy"),MKV.len(),&AtomicBool::new(false)).unwrap_err().contains("captured audio policy"));
    assert_eq!(server.requests().len(),3);
}

#[test]
fn network_muxed_muted_and_audio_profile_refusals_happen_after_bounded_get_before_install() {
    let server=Server::new(vec![("/clip",response(200,"Content-Type: video/x-matroska\r\n",MKV))]);
    assert!(decode(server.context(),&server.url("/clip"),true,Some(crate::audio::Limits::default()))
        .err().unwrap().contains("muting"));
    for limits in [
        crate::audio::Limits {input_bytes:1,..crate::audio::Limits::default()},
        crate::audio::Limits {channels:1,..crate::audio::Limits::default()},
        crate::audio::Limits {duration:Duration::from_millis(100),..crate::audio::Limits::default()},
    ] {
        assert!(decode(server.context(),&server.url("/clip"),false,Some(limits)).is_err());
    }
    assert_eq!(server.requests().len(),4,"These MIME-dependent refusals each need one full GET, never an automatic retry");
}

#[test]
fn network_muxed_mime_status_partial_truncation_and_requested_limit_refusals_stay_strict() {
    for (status,headers) in [
        (206,"Content-Type: video/x-matroska\r\n"),
        (200,"Content-Type: video/x-matroska\r\nContent-Range: bytes 0-3/4\r\n"),
        (200,"Content-Type: video/x-matroska\r\nContent-Type: video/x-matroska\r\n"),
        (200,"Content-Type: video/x-matroska; broken\r\n"),
        (200,"Content-Type: video/webm\r\n"),
        (200,"Content-Type: application/octet-stream\r\n"),
    ] {
        let server=Server::new(vec![("/clip.mkv",response(status,headers,MKV))]);
        assert!(server.context().load_resource(&server.url("/clip.mkv"),MKV.len(),&AtomicBool::new(false)).is_err(),"{headers}");
        assert_eq!(server.requests().len(),1);
    }
    let mut truncated=format!("HTTP/1.1 200 OK\r\nContent-Type: video/x-matroska\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",MKV.len()+1).into_bytes();truncated.extend_from_slice(MKV);
    let server=Server::new(vec![("/truncated",truncated),("/limit",response(200,"Content-Type: video/x-matroska\r\n",MKV))]);
    assert!(server.context().load_resource(&server.url("/truncated"),MKV.len()+1,&AtomicBool::new(false)).is_err());
    assert!(server.context().load_resource(&server.url("/limit"),MKV.len()-1,&AtomicBool::new(false)).is_err());
    assert_eq!(server.requests().len(),2);
}

#[test]
fn network_muxed_redirect_and_gzip_preserve_type_full_bytes_cookie_context_and_policy() {
    let server=Server::new(vec![
        ("/redirect",response(302,"Location: /final.ivf\r\nSet-Cookie: redirect_av=1; Path=/; SameSite=Strict\r\n",b"")),
        ("/final.ivf",response(200,"Content-Type: video/x-matroska\r\n",MKV)),
        ("/gzip",response(200,"Content-Type: video/x-matroska\r\nContent-Encoding: gzip\r\n",&gzip(MKV))),
    ]);
    let context=server.context();
    let resource=context.load_resource(&server.url("/redirect"),MKV.len(),&AtomicBool::new(false)).unwrap();
    assert_eq!(resource.format,Format::MatroskaPcm16);assert_eq!(resource.bytes,MKV);
    assert!(server.requests()[1].contains("Cookie: redirect_av=1\r\n"));
    let resource=context.load_resource(&server.url("/gzip"),512*1024*1024,&AtomicBool::new(false)).unwrap();
    assert_eq!(resource.format,Format::MatroskaPcm16);assert_eq!(resource.bytes,MKV);
    assert_eq!(server.requests().len(),3);
    let mut denied=server.context();denied.policy.add_header("media-src 'none'");
    assert!(denied.load_resource(&server.url("/never"),MKV.len(),&AtomicBool::new(false)).is_err());
    assert_eq!(server.requests().len(),3,"CSP denial must precede media contact");
}

#[test]
fn network_muxed_cors_credentials_and_denial_preserve_existing_cookie_ordering() {
    for (credentialed,denied) in [(false,false),(true,false),(true,true)] {
        let document=Server::new(Vec::new());
        let origin=document.url("/").display().trim_end_matches('/').to_owned();
        let allow=if denied {"*".to_owned()}else{origin.clone()};
        let headers=format!("Content-Type: video/x-matroska\r\nAccess-Control-Allow-Origin: {allow}\r\n{}Set-Cookie: av_response=stored; Path=/; SameSite=Strict; HttpOnly\r\n",if credentialed {"Access-Control-Allow-Credentials: true\r\n"}else{""});
        let media=Server::new(vec![("/clip",response(200,&headers,MKV))]);
        let mut context=document.context();
        context.policy=csp::PolicySet::new(&Location {url:Some(context.origin.clone()),path:None,root:None,fragment:String::new()});
        context.policy.add_header("media-src *");
        context.cookies.set_document(&context.origin,"av_seed=sent; Path=/; SameSite=Strict").unwrap();
        // Missing crossorigin still refuses before contacting the foreign port.
        assert!(context.load_resource(&media.url("/clip"),MKV.len(),&AtomicBool::new(false)).is_err());
        assert!(media.requests().is_empty());
        context.credentials=Some(if credentialed {http::MediaCredentials::Include}else{http::MediaCredentials::SameOrigin});
        let result=context.load_resource(&media.url("/clip"),MKV.len(),&AtomicBool::new(false));
        assert_eq!(result.is_err(),denied);
        if let Ok(resource)=result {assert_eq!(resource.format,Format::MatroskaPcm16);assert_eq!(resource.bytes,MKV);}
        let requests=media.requests();assert_eq!(requests.len(),1);assert!(requests[0].contains(&format!("Origin: {origin}\r\n")));
        assert_eq!(requests[0].contains("av_seed=sent"),credentialed);
        assert_eq!(context.cookies.request_header(&context.origin,true,false).contains("av_response=stored"),credentialed,
            "Credentialed Set-Cookie processing must precede a later CORS denial");
        assert!(!context.cookies.document(&context.origin).contains("av_response"));
    }
}

#[test]
fn network_muxed_retirement_fences_redirects_and_typed_body_publication() {
    for redirect in [false,true] {
        let retired=Arc::new(AtomicBool::new(false));
        let wire=if redirect {response(302,"Location: /must-not-contact\r\n",b"")}else{response(200,"Content-Type: video/x-matroska\r\n",MKV)};
        let server=Server::with_retirement(vec![("/retire",wire)],Some(retired.clone()));
        let mut context=server.context();context.retired=retired;
        assert!(context.load_resource(&server.url("/retire"),MKV.len(),&AtomicBool::new(false)).unwrap_err().contains("cancelled"));
        assert_eq!(server.requests().len(),1);
    }
}
