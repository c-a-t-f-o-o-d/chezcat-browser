//! Complete same-origin IVF media transfers through the browser's HTTP stack.
//! No ranges, helper processes, codec substitution or automatic media retries.
use crate::{blocking, cookies, csp, http, loader, navigation::Location, network_trace, trust, url::HttpUrl};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};

#[derive(Clone)]
pub(crate) struct Context {
    pub(crate) origin: HttpUrl,
    pub(crate) trust: Arc<trust::Store>,
    pub(crate) cookies: Arc<cookies::Session>,
    pub(crate) blocking: Arc<blocking::Policy>,
    pub(crate) policy: csp::PolicySet,
    pub(crate) policy_scope: usize,
    pub(crate) network: network_trace::Trace,
    pub(crate) retired: Arc<AtomicBool>,
}

impl Context {
    pub(crate) fn load(&self, source: &HttpUrl, limit: usize, cancel: &AtomicBool) -> Result<Vec<u8>, String> {
        let guard = |target: &HttpUrl| {
            if cancel.load(Ordering::Acquire) || self.retired.load(Ordering::Acquire) {
                return Err("Network media source cancelled.".into());
            }
            if self.origin.secure && !target.secure {
                return Err("HTTPS media cannot use an unencrypted HTTP source.".into());
            }
            if !self.origin.same_origin(target) {
                return Err("Cross-origin media navigation or redirect was blocked.".into());
            }
            self.blocking.check(target)?;
            self.policy.allows_media(self.policy_scope, &Location {
                url: Some(target.clone()), path: None, root: None, fragment: String::new(),
            })
        };
        let pool = http::Pool::new(cancel, &self.trust)
            .with_cookie_session(self.cookies.clone(), Some(self.origin.clone()))
            .with_network_trace(self.network.clone());
        let response = pool.fetch_kind_checked(source, limit, &self.origin, http::FetchMode::Normal, "media", &guard)?;
        // Job cancellation is checked inside HTTP framing/decompression. The
        // immutable retirement token also gates each redirect and this final
        // return; retirement alone does not wake an in-progress socket read.
        guard(&response.url)?;
        if response.status != 200 {
            return Err(format!("Network media requires HTTP 200, received {}.", response.status));
        }
        if response.headers.iter().any(|(name, _)| name.eq_ignore_ascii_case("content-range")) {
            return Err("Network media does not accept Content-Range or partial representations.".into());
        }
        let mut types = response.headers.iter().filter(|(name, _)| name.eq_ignore_ascii_case("content-type"));
        let content_type = types.next().map(|(_, value)| value.as_str())
            .ok_or("Network media requires a video/x-ivf Content-Type.")?;
        if types.next().is_some() || !ivf_content_type(content_type) {
            return Err("Network media requires one valid video/x-ivf Content-Type.".into());
        }
        Ok(response.body)
    }
}

// Validate this one accepted MIME type and its ordinary token/quoted-string
// parameters. Normalization remains the loader's shared media_type operation.
fn ivf_content_type(value: &str) -> bool {
    let essence = value.split(';').next().unwrap_or("").trim_matches([' ', '\t']);
    if !essence.eq_ignore_ascii_case("video/x-ivf") || loader::media_type(value) != "video/x-ivf" { return false; }
    let bytes=value.as_bytes();let mut i=value.find(';').unwrap_or(bytes.len());
    let ows=|b:u8| matches!(b,b' '|b'\t');
    let token=|b:u8| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b);
    while i<bytes.len() {
        while i<bytes.len()&&ows(bytes[i]) {i+=1;}
        if i==bytes.len() {return true;}
        if bytes[i]!=b';' {return false;}i+=1;
        while i<bytes.len()&&ows(bytes[i]) {i+=1;}
        let start=i;while i<bytes.len()&&token(bytes[i]) {i+=1;}
        if i==start {return false;}
        while i<bytes.len()&&ows(bytes[i]) {i+=1;}
        if bytes.get(i)!=Some(&b'=') {return false;}i+=1;
        while i<bytes.len()&&ows(bytes[i]) {i+=1;}
        if bytes.get(i)==Some(&b'"') {
            i+=1;let mut closed=false;
            while i<bytes.len() {
                let mut b=bytes[i];i+=1;
                if b==b'"' {closed=true;break;}
                if b==b'\\' {let Some(&escaped)=bytes.get(i) else{return false;};b=escaped;i+=1;}
                if b<32&&b!=b'\t'||b==127 {return false;}
            }
            if !closed {return false;}
        } else {
            let start=i;while i<bytes.len()&&token(bytes[i]) {i+=1;}
            if i==start {return false;}
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::{Read, Write}, net::TcpListener, sync::Mutex, thread::{self, JoinHandle}, time::Duration};
    const IVF:&[u8]=include_bytes!("../tools/fixtures/scene-intra.ivf");
    struct Server { origin:HttpUrl, requests:Arc<Mutex<Vec<String>>>, stop:Arc<AtomicBool>, worker:Option<JoinHandle<()>> }
    impl Server {
        fn new(routes:Vec<(&'static str,Vec<u8>)>)->Self {
            Self::with_retirement(routes,None)
        }
        fn with_retirement(routes:Vec<(&'static str,Vec<u8>)>,retired:Option<Arc<AtomicBool>>)->Self {
            let listener=TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();
            let origin=crate::url::parse(&format!("http://127.0.0.1:{}/document",listener.local_addr().unwrap().port())).unwrap().0;
            let requests=Arc::new(Mutex::new(Vec::new()));let captured=requests.clone();
            let stop=Arc::new(AtomicBool::new(false));let done=stop.clone();
            let worker=thread::spawn(move||while !done.load(Ordering::Acquire) {
                let (mut stream,_)=match listener.accept() {Ok(pair)=>pair,Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>{thread::sleep(Duration::from_millis(1));continue;},Err(error)=>panic!("loopback accept: {error}")};
                // Accepted Winsock sockets inherit listener nonblocking mode.
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut head=Vec::new();let mut byte=[0];
                while !head.ends_with(b"\r\n\r\n") {if stream.read_exact(&mut byte).is_err(){break;}head.push(byte[0]);assert!(head.len()<=32768);}
                let request=String::from_utf8(head).unwrap();let path=request.split_whitespace().nth(1).unwrap_or("").to_owned();
                captured.lock().unwrap().push(request);
                if let Some(retired)=&retired {retired.store(true,Ordering::Release);}
                if let Some((_,wire))=routes.iter().find(|(route,_)|*route==path) {let _=stream.write_all(wire);}
                else {let _=stream.write_all(b"HTTP/1.1 404 Missing\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");}
            });
            Self {origin,requests,stop,worker:Some(worker)}
        }
        fn url(&self,path:&str)->HttpUrl {self.origin.resolve(path).unwrap().0}
        fn context(&self)->Context {
            let location=Location {url:Some(self.origin.clone()),path:None,root:None,fragment:String::new()};
            let mut policy=csp::PolicySet::new(&location);policy.add_header("media-src 'self'");
            Context {origin:self.origin.clone(),trust:Arc::new(trust::Store::unconfigured()),cookies:Arc::new(cookies::Session::default()),
                blocking:Arc::new(blocking::Policy::default()),policy,policy_scope:usize::MAX,network:network_trace::Trace::default(),retired:Arc::new(AtomicBool::new(false))}
        }
        fn requests(&self)->Vec<String> {self.requests.lock().unwrap().clone()}
    }
    impl Drop for Server {fn drop(&mut self){self.stop.store(true,Ordering::Release);self.worker.take().unwrap().join().unwrap();}}
    fn response(status:u16,headers:&str,body:&[u8])->Vec<u8> {
        let mut wire=format!("HTTP/1.1 {status} Test\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();wire.extend_from_slice(body);wire
    }
    fn gzip(body:&[u8])->Vec<u8> {
        let mut bytes=vec![31,139,8,0,0,0,0,0,0,255];let count=body.len().div_ceil(65535);
        for (i,chunk) in body.chunks(65535).enumerate() {let len=chunk.len() as u16;bytes.push(u8::from(i+1==count));bytes.extend_from_slice(&len.to_le_bytes());bytes.extend_from_slice(&(!len).to_le_bytes());bytes.extend_from_slice(chunk);}
        bytes.extend_from_slice(&crate::compression::crc32(body).to_le_bytes());bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());bytes
    }

    #[test]
    fn network_media_complete_original_ivf_uses_normal_cookies_trace_and_real_decoder() {
        let server=Server::new(vec![("/movie.ivf?original=1",response(200,"Content-Type: VIDEO/X-IVF; profile=original; note=\"two; words\"\r\nSet-Cookie: media_response=1; Path=/; HttpOnly\r\n",IVF))]);
        let context=server.context();context.cookies.set_document(&context.origin,"fixture=sent; Path=/; SameSite=Strict").unwrap();
        let bytes=context.load(&server.url("/movie.ivf?original=1"),IVF.len(),&AtomicBool::new(false)).unwrap();assert_eq!(bytes,IVF);
        let asset=crate::media::Clip::load(bytes,crate::media::Limits::default(),&||Ok(())).unwrap();assert!(asset.clip.duration>Duration::ZERO);assert_eq!((asset.first.width,asset.first.height),(64,48));
        let requests=server.requests();assert_eq!(requests.len(),1);let request=&requests[0];assert!(request.starts_with("GET /movie.ivf?original=1 HTTP/1.1\r\n"));
        assert!(request.contains("Cookie: fixture=sent\r\n"));assert!(!request.contains("Range:")&&!request.contains("If-Range:")&&!request.contains("Cache-Control:"));
        assert!(context.cookies.request_header(&context.origin,true,false).contains("media_response=1"));assert!(!context.cookies.document(&context.origin).contains("media_response"));
        let rows=context.network.snapshot().rows;assert_eq!(rows.len(),1);assert_eq!(rows[0].kind,"media");assert_eq!(rows[0].status,Some(200));
    }

    #[test]
    fn network_media_same_origin_redirect_keeps_cookie_context_and_full_body() {
        let server=Server::new(vec![("/redirect",response(302,"Location: /final.ivf\r\nSet-Cookie: redirect_fixture=1; Path=/; SameSite=Strict\r\n",b"")),("/final.ivf",response(200,"Content-Type: video/x-ivf\r\n",IVF))]);
        let context=server.context();let bytes=context.load(&server.url("/redirect"),IVF.len(),&AtomicBool::new(false)).unwrap();assert_eq!(bytes,IVF);
        let requests=server.requests();assert_eq!(requests.len(),2);assert!(requests[0].starts_with("GET /redirect "));assert!(requests[1].starts_with("GET /final.ivf "));assert!(requests[1].contains("Cookie: redirect_fixture=1\r\n"));
    }

    #[test]
    fn network_media_redirect_guards_and_initial_csp_block_before_destination_request() {
        let server=Server::new(vec![("/redirect",response(302,"Location: /blocked/final.ivf\r\n",b""))]);
        let mut context=server.context();context.blocking=Arc::new(blocking::Policy::new(&blocking::Config {enabled:true,builtins:false,rules:vec!["path:/blocked".into()]}));
        assert!(context.load(&server.url("/redirect"),IVF.len(),&AtomicBool::new(false)).is_err());assert_eq!(server.requests().len(),1);
        context.policy.add_header("media-src 'none'");assert!(context.load(&server.url("/initial.ivf"),IVF.len(),&AtomicBool::new(false)).unwrap_err().contains("Content Security Policy"));assert_eq!(server.requests().len(),1);
        assert!(context.network.snapshot().rows.iter().any(|row|row.outcome==network_trace::Outcome::Blocked));
    }

    #[test]
    fn network_media_cross_origin_and_https_downgrade_do_not_send_requests() {
        let server=Server::new(Vec::new());let context=server.context();
        let mut other=server.url("/movie.ivf");other.host="localhost".into();
        assert!(context.load(&other,IVF.len(),&AtomicBool::new(false)).unwrap_err().contains("Cross-origin"));
        let mut secure=context.clone();secure.origin.secure=true;
        assert!(secure.load(&server.url("/movie.ivf"),IVF.len(),&AtomicBool::new(false)).unwrap_err().contains("unencrypted"));
        assert!(server.requests().is_empty());
    }

    #[test]
    fn network_media_cross_origin_redirect_never_requests_second_owned_origin() {
        let other=Server::new(Vec::new());
        let server=Server::new(vec![("/redirect",response(302,&format!("Location: {}\r\n",other.url("/movie.ivf").display()),b""))]);
        let context=server.context();
        assert!(context.load(&server.url("/redirect"),IVF.len(),&AtomicBool::new(false)).unwrap_err().contains("Cross-origin"));
        assert_eq!(server.requests().len(),1);assert!(other.requests().is_empty());
    }

    #[test]
    fn network_media_status_partial_and_invalid_mime_never_reach_decoder() {
        let cases=[(206,"Content-Type: video/x-ivf\r\n"),(404,"Content-Type: video/x-ivf\r\n"),(200,"Content-Type: video/x-ivf\r\nContent-Range: bytes 0-3/4\r\n"),
            (200,""),(200,"Content-Type: text/html\r\n"),(200,"Content-Type: video/x-ivf\r\nContent-Type: video/x-ivf\r\n"),(200,"Content-Type: video/x-ivf; broken\r\n"),(200,"Content-Type: video/x-ivf; note=\"unterminated\r\n")];
        for (status,headers) in cases {
            let server=Server::new(vec![("/movie.ivf",response(status,headers,IVF))]);let context=server.context();
            assert!(context.load(&server.url("/movie.ivf"),IVF.len(),&AtomicBool::new(false)).is_err(),"status={status} headers={headers:?}");assert_eq!(server.requests().len(),1);
        }
    }

    #[test]
    fn network_media_chunked_and_gzip_full_transfers_decode_with_generous_requested_limit() {
        let mut chunked=b"HTTP/1.1 200 OK\r\nContent-Type: video/x-ivf\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n".to_vec();
        for chunk in IVF.chunks(1024) {chunked.extend_from_slice(format!("{:x}\r\n",chunk.len()).as_bytes());chunked.extend_from_slice(chunk);chunked.extend_from_slice(b"\r\n");}chunked.extend_from_slice(b"0\r\n\r\n");
        let server=Server::new(vec![("/chunked.ivf",chunked),("/gzip.ivf",response(200,"Content-Type: video/x-ivf\r\nContent-Encoding: gzip\r\n",&gzip(IVF)))]);let context=server.context();
        for path in ["/chunked.ivf","/gzip.ivf"] {let bytes=context.load(&server.url(path),512*1024*1024,&AtomicBool::new(false)).unwrap();assert_eq!(bytes,IVF);crate::media::Clip::load(bytes,crate::media::Limits::default(),&||Ok(())).unwrap();}
        assert_eq!(server.requests().len(),2);
    }

    #[test]
    fn network_media_truncation_and_small_encoded_or_decoded_limits_fail_once() {
        let mut truncated=format!("HTTP/1.1 200 OK\r\nContent-Type: video/x-ivf\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",IVF.len()+1).into_bytes();truncated.extend_from_slice(IVF);
        // Original existing HTTP fixture: 24 encoded bytes expand to 256 x's.
        // This negative resource never reaches the media decoder.
        let expanding=b"\x1f\x8b\x08\x00\x00\x00\x00\x00\x02\x03\xab\xa8\x18\xd9\x00\x00\x10\x1a\xee\x73\x00\x01\x00\x00";
        let compressed=gzip(IVF);let server=Server::new(vec![("/truncated.ivf",truncated),("/too-large.ivf",response(200,"Content-Type: video/x-ivf\r\n",IVF)),("/encoded.ivf",response(200,"Content-Type: video/x-ivf\r\nContent-Encoding: gzip\r\n",&compressed)),("/decoded.ivf",response(200,"Content-Type: video/x-ivf\r\nContent-Encoding: gzip\r\n",expanding))]);let context=server.context();
        assert!(context.load(&server.url("/truncated.ivf"),IVF.len()+1,&AtomicBool::new(false)).is_err());
        assert!(context.load(&server.url("/too-large.ivf"),IVF.len()-1,&AtomicBool::new(false)).is_err());
        assert!(context.load(&server.url("/encoded.ivf"),compressed.len()-1,&AtomicBool::new(false)).is_err());
        assert!(context.load(&server.url("/decoded.ivf"),64,&AtomicBool::new(false)).unwrap_err().contains("output limit"));
        assert_eq!(server.requests().len(),4);
    }

    #[test]
    fn network_media_retirement_and_cancel_before_admission_are_request_free() {
        let server=Server::new(Vec::new());let context=server.context();context.retired.store(true,Ordering::Release);
        assert!(context.load(&server.url("/movie.ivf"),IVF.len(),&AtomicBool::new(false)).unwrap_err().contains("cancelled"));
        context.retired.store(false,Ordering::Release);
        assert!(context.load(&server.url("/movie.ivf"),IVF.len(),&AtomicBool::new(true)).unwrap_err().contains("cancelled"));assert!(server.requests().is_empty());
    }

    #[test]
    fn network_media_retirement_blocks_next_redirect_and_completed_body_publication() {
        for redirect in [false,true] {
            let retired=Arc::new(AtomicBool::new(false));
            let wire=if redirect {response(302,"Location: /must-not-request.ivf\r\n",b"")}else{response(200,"Content-Type: video/x-ivf\r\n",IVF)};
            let server=Server::with_retirement(vec![("/retire.ivf",wire)],Some(retired.clone()));
            let mut context=server.context();context.retired=retired;
            assert!(context.load(&server.url("/retire.ivf"),IVF.len(),&AtomicBool::new(false)).unwrap_err().contains("cancelled"));
            assert_eq!(server.requests().len(),1);
        }
    }
}
