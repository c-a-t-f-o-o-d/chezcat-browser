//! Original, narrow CORS state for explicit CORS-mode media GET transfers.
//! HTTP owns TLS, message framing, redirects, cancellation and bounded retries.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Credentials { SameOrigin, Include }

#[derive(Clone, Copy)]
pub(super) struct Config<'a> { pub origin: &'a HttpUrl, pub credentials: Credentials }

pub(super) struct State<'a> {
    origin: &'a HttpUrl,
    credentials: Credentials,
    serialized: String,
    cors: bool,
    redirect_tainted: bool,
    cookies: crate::cookies::NavigationContext<'a>,
}
impl<'a> State<'a> {
    pub(super) fn new(config: Config<'a>) -> Result<Self, String> {
        let host=config.origin.host_header();
        let prefix=if config.origin.secure {"https://"}else{"http://"};
        let size=prefix.len().checked_add(host.len()).ok_or("Media Origin length overflow.")?;
        if size>MAX_LINE_BYTES {return Err("Media Origin exceeds the HTTP field limit.".into());}
        let mut serialized=String::new();serialized.try_reserve(size).map_err(|_|"Could not retain the media Origin.")?;
        serialized.push_str(prefix);serialized.push_str(&host);
        Ok(Self {origin:config.origin,credentials:config.credentials,serialized,cors:false,redirect_tainted:false,
            cookies:crate::cookies::NavigationContext::new(Some(config.origin),None)})
    }
    pub(super) fn prepare(&mut self,target:&HttpUrl)->Result<(),String> {
        if self.origin.secure&&!target.secure {return Err("HTTPS media cannot use an unencrypted HTTP source.".into());}
        self.cors|=!self.origin.same_origin(target);
        Ok(())
    }
    pub(super) fn request_origin(&self)->Option<&str> {
        self.cors.then_some(if self.redirect_tainted {"null"}else{self.serialized.as_str()})
    }
    pub(super) fn credentials_allowed(&self)->bool {self.credentials==Credentials::Include||!self.cors}
    pub(super) fn same_site(&self,target:&HttpUrl)->bool {self.credentials_allowed()&&self.cookies.is_same_site(target)}
    pub(super) fn redirect(&mut self,from:&HttpUrl,to:&HttpUrl) {
        // A -> B retains A. An origin change from already-foreign B makes the
        // serialized request origin null, including a return to A.
        self.redirect_tainted|=!self.origin.same_origin(from)&&!from.same_origin(to);
        self.cookies.redirect(from,to);
    }
    pub(super) fn check_response(&self,headers:&[(String,String)])->Result<(),String> {
        let Some(origin)=self.request_origin() else{return Ok(());};
        let allow=one_header(headers,"access-control-allow-origin")?
            .ok_or("Media CORS response is missing Access-Control-Allow-Origin.")?;
        if allow=="*" {
            if self.credentials==Credentials::Include {return Err("Media CORS wildcard origin is invalid with include credentials.".into());}
            return Ok(());
        }
        if allow!=origin {return Err("Media CORS Access-Control-Allow-Origin does not match the serialized request origin.".into());}
        if self.credentials==Credentials::Include
            && one_header(headers,"access-control-allow-credentials")?!=Some("true") {
            return Err("Media CORS include credentials require Access-Control-Allow-Credentials: true.".into());
        }
        Ok(())
    }
}
fn one_header<'a>(headers:&'a[(String,String)],name:&str)->Result<Option<&'a str>,String> {
    let mut matching=headers.iter().filter(|(key,_)|key.eq_ignore_ascii_case(name));
    let value=matching.next().map(|(_,value)|value.as_str());
    if matching.next().is_some() {return Err("Media CORS response has duplicate access-control fields.".into());}
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{net::TcpListener,thread::{self,JoinHandle}};
    struct Server {
        origin:HttpUrl,routes:Arc<Mutex<Vec<(String,Vec<u8>)>>>,requests:Arc<Mutex<Vec<String>>>,
        stop:Arc<AtomicBool>,worker:Option<JoinHandle<()>>,
    }
    impl Server {
        fn new(host:&str)->Self {
            let listener=TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();
            let origin=crate::url::parse(&format!("http://{host}:{}/document",listener.local_addr().unwrap().port())).unwrap().0;
            let routes=Arc::new(Mutex::new(Vec::<(String,Vec<u8>)>::new()));let served=routes.clone();
            let requests=Arc::new(Mutex::new(Vec::new()));let seen=requests.clone();let stop=Arc::new(AtomicBool::new(false));let done=stop.clone();
            let worker=thread::spawn(move||while !done.load(Ordering::Acquire) {
                let(mut stream,_)=match listener.accept(){Ok(pair)=>pair,Err(e) if e.kind()==io::ErrorKind::WouldBlock=>{thread::sleep(Duration::from_millis(1));continue;},Err(e)=>panic!("owned CORS loopback accept: {e}")};
                stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(2))).unwrap();stream.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
                let mut head=Vec::new();let mut byte=[0];while !head.ends_with(b"\r\n\r\n") {if stream.read_exact(&mut byte).is_err(){break;}head.push(byte[0]);assert!(head.len()<=MAX_HEADER_BYTES);}
                let request=String::from_utf8(head).unwrap();let path=request.split_whitespace().nth(1).unwrap_or("").to_owned();seen.lock().unwrap().push(request);
                let routes=served.lock().unwrap();let wire=routes.iter().find(|(key,_)|*key==path).map(|(_,wire)|wire.as_slice())
                    .unwrap_or(b"HTTP/1.1 404 Missing\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");let _=stream.write_all(wire);
            });
            Self{origin,routes,requests,stop,worker:Some(worker)}
        }
        fn url(&self,path:&str)->HttpUrl{self.origin.resolve(path).unwrap().0}
        fn put(&self,path:&str,status:u16,headers:&str){
            let body=if status==200 {b"original transport body".as_slice()}else{b"".as_slice()};
            let mut wire=format!("HTTP/1.1 {status} Test\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n",body.len()).into_bytes();wire.extend_from_slice(body);self.routes.lock().unwrap().push((path.into(),wire));
        }
        fn requests(&self)->Vec<String>{self.requests.lock().unwrap().clone()}
        fn serialized(&self)->String{format!("http://{}",self.origin.host_header())}
    }
    impl Drop for Server{fn drop(&mut self){self.stop.store(true,Ordering::Release);self.worker.take().unwrap().join().unwrap();}}
    fn value<'a>(request:&'a str,name:&str)->Option<&'a str>{request.lines().skip(1).find_map(|line|line.split_once(':').filter(|(key,_)|key.eq_ignore_ascii_case(name)).map(|(_,value)|value.trim()))}
    fn fetch(pool:&Pool<'_>,source:&HttpUrl,origin:&HttpUrl,credentials:Credentials)->Result<Response,String>{pool.fetch_cors_media(source,1024,origin,credentials,FetchMode::Normal,&|_|Ok(()))}

    #[test]
    fn media_cors_same_origin_basic_sends_credentials_without_origin_or_cors_requirement(){
        let a=Server::new("127.0.0.1");a.put("/basic",200,"Access-Control-Allow-Origin: invalid\r\nAccess-Control-Allow-Origin: duplicate\r\n");
        let cookies=Arc::new(crate::cookies::Session::default());cookies.set_document(&a.origin,"same_site=1; Path=/; SameSite=Strict").unwrap();let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust).with_cookie_session(cookies,Some(a.origin.clone()));
        for credentials in [Credentials::SameOrigin,Credentials::Include]{assert_eq!(fetch(&pool,&a.url("/basic"),&a.origin,credentials).unwrap().body,b"original transport body");}
        for request in a.requests(){assert_eq!(value(&request,"Origin"),None);assert_eq!(value(&request,"Cookie"),Some("same_site=1"));}
    }

    #[test]
    fn media_cors_anonymous_foreign_omits_credentials_and_accepts_exact_or_wildcard(){
        let a=Server::new("127.0.0.1");let b=Server::new("127.0.0.1");
        b.put("/exact",200,&format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: bad\r\nAccess-Control-Allow-Credentials: duplicate-unused\r\nSet-Cookie: omitted=1; Path=/\r\n",a.serialized()));b.put("/wild",200,"Access-Control-Allow-Origin: *\r\n");
        let cookies=Arc::new(crate::cookies::Session::default());cookies.set_document(&a.origin,"existing=1; Path=/; SameSite=Strict").unwrap();let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust).with_cookie_session(cookies.clone(),Some(a.origin.clone()));
        for path in ["/exact","/wild"]{fetch(&pool,&b.url(path),&a.origin,Credentials::SameOrigin).unwrap();}
        for request in b.requests(){assert_eq!(value(&request,"Origin"),Some(a.serialized().as_str()));assert_eq!(value(&request,"Cookie"),None);}
        assert!(!cookies.document(&a.origin).contains("omitted="));
    }

    #[test]
    fn media_cors_include_foreign_same_site_sends_cookie_and_stores_before_denial(){
        let a=Server::new("127.0.0.1");let b=Server::new("127.0.0.1");
        b.put("/allowed",200,&format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: true\r\n",a.serialized()));b.put("/denied",200,"Set-Cookie: processed_before_cors=1; Path=/\r\n");
        let cookies=Arc::new(crate::cookies::Session::default());cookies.set_document(&a.origin,"existing=1; Path=/; SameSite=Strict").unwrap();let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust).with_cookie_session(cookies.clone(),Some(a.origin.clone()));
        fetch(&pool,&b.url("/allowed"),&a.origin,Credentials::Include).unwrap();assert!(fetch(&pool,&b.url("/denied"),&a.origin,Credentials::Include).unwrap_err().contains("missing Access-Control-Allow-Origin"));
        for request in b.requests(){assert_eq!(value(&request,"Origin"),Some(a.serialized().as_str()));assert!(value(&request,"Cookie").is_some_and(|cookie|cookie.contains("existing=1")));}
        assert!(cookies.document(&a.origin).contains("processed_before_cors=1"));
    }

    #[test]
    fn media_cors_include_cross_site_response_cannot_delete_strict_cookie_before_cors_failure(){
        let a=Server::new("127.0.0.1");let b=Server::new("localhost");
        b.put("/denied",200,"Set-Cookie: protected=deleted; Path=/; Max-Age=0; SameSite=Strict\r\n");
        let cookies=Arc::new(crate::cookies::Session::default());cookies.set_document(&b.origin,"protected=kept; Path=/; SameSite=Strict").unwrap();
        let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust).with_cookie_session(cookies.clone(),Some(a.origin.clone()));
        assert!(fetch(&pool,&b.url("/denied"),&a.origin,Credentials::Include).unwrap_err().contains("CORS"));
        assert_eq!(value(&b.requests()[0],"Cookie"),None);assert_eq!(cookies.document(&b.origin),"protected=kept");
    }

    #[test]
    fn media_cors_required_headers_are_unique_exact_and_include_disallows_wildcard_without_cookies(){
        let a=Server::new("127.0.0.1");let b=Server::new("127.0.0.1");
        let cases=[format!("Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Credentials: true\r\n"),format!("Access-Control-Allow-Origin: {}\r\n",a.serialized()),format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: True\r\n",a.serialized()),format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: true\r\nAccess-Control-Allow-Credentials: true\r\n",a.serialized()),format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: true\r\n",a.serialized(),a.serialized()),"Access-Control-Allow-Origin: null\r\nAccess-Control-Allow-Credentials: true\r\n".into()];
        let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust);
        for (index,headers) in cases.iter().enumerate(){let path=format!("/invalid-{index}");b.put(&path,200,headers);assert!(fetch(&pool,&b.url(&path),&a.origin,Credentials::Include).is_err(),"case {index}");}assert_eq!(b.requests().len(),cases.len());
    }

    #[test]
    fn media_cors_anonymous_a_b_a_keeps_taint_null_origin_and_omits_return_cookies(){
        let a=Server::new("127.0.0.1");let b=Server::new("127.0.0.1");
        a.put("/start",302,&format!("Location: {}\r\n",b.url("/hop").display()));b.put("/hop",302,&format!("Access-Control-Allow-Origin: {}\r\nLocation: {}\r\nSet-Cookie: skipped_foreign=1; Path=/\r\n",a.serialized(),a.url("/return").display()));a.put("/return",200,"Access-Control-Allow-Origin: null\r\nSet-Cookie: skipped_return=1; Path=/\r\n");
        let cookies=Arc::new(crate::cookies::Session::default());cookies.set_document(&a.origin,"start_cookie=1; Path=/; SameSite=Strict").unwrap();let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust).with_cookie_session(cookies.clone(),Some(a.origin.clone()));
        let response=fetch(&pool,&a.url("/start"),&a.origin,Credentials::SameOrigin).unwrap();assert!(response.redirected);let ar=a.requests();let br=b.requests();assert_eq!((ar.len(),br.len()),(2,1));assert_eq!(value(&ar[0],"Origin"),None);assert!(value(&ar[0],"Cookie").is_some());assert_eq!(value(&br[0],"Origin"),Some(a.serialized().as_str()));assert_eq!(value(&ar[1],"Origin"),Some("null"));assert_eq!(value(&br[0],"Cookie"),None);assert_eq!(value(&ar[1],"Cookie"),None);assert_eq!(cookies.document(&a.origin),"start_cookie=1");
    }

    #[test]
    fn media_cors_include_a_cross_site_b_a_keeps_null_origin_and_strict_cookie_history(){
        let a=Server::new("127.0.0.1");let b=Server::new("localhost");
        a.put("/start",302,&format!("Location: {}\r\n",b.url("/hop").display()));b.put("/hop",302,&format!("Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Credentials: true\r\nLocation: {}\r\n",a.serialized(),a.url("/return").display()));a.put("/return",200,"Access-Control-Allow-Origin: null\r\nAccess-Control-Allow-Credentials: true\r\n");
        let cookies=Arc::new(crate::cookies::Session::default());cookies.set_document(&a.origin,"strict_a=1; Path=/; SameSite=Strict").unwrap();cookies.set_document(&b.origin,"strict_b=1; Path=/; SameSite=Strict").unwrap();let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust).with_cookie_session(cookies,Some(a.origin.clone()));
        fetch(&pool,&a.url("/start"),&a.origin,Credentials::Include).unwrap();let ar=a.requests();let br=b.requests();assert!(value(&ar[0],"Cookie").is_some());assert_eq!(value(&br[0],"Cookie"),None);assert_eq!(value(&ar[1],"Cookie"),None);assert_eq!(value(&br[0],"Origin"),Some(a.serialized().as_str()));assert_eq!(value(&ar[1],"Origin"),Some("null"));
    }

    #[test]
    fn media_cors_a_b_c_serializes_null_and_checks_every_tainted_redirect(){
        let a=Server::new("127.0.0.1");let b=Server::new("127.0.0.1");let c=Server::new("127.0.0.1");
        a.put("/start",302,&format!("Location: {}\r\n",b.url("/hop").display()));b.put("/hop",302,&format!("Access-Control-Allow-Origin: {}\r\nLocation: {}\r\n",a.serialized(),c.url("/final").display()));c.put("/final",200,"Access-Control-Allow-Origin: null\r\n");
        let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust);fetch(&pool,&a.url("/start"),&a.origin,Credentials::SameOrigin).unwrap();assert_eq!(value(&b.requests()[0],"Origin"),Some(a.serialized().as_str()));assert_eq!(value(&c.requests()[0],"Origin"),Some("null"));
        a.put("/denied-start",302,&format!("Location: {}\r\n",b.url("/denied-hop").display()));b.put("/denied-hop",302,&format!("Location: {}\r\n",c.url("/must-not-request").display()));
        assert!(fetch(&pool,&a.url("/denied-start"),&a.origin,Credentials::SameOrigin).unwrap_err().contains("CORS"));assert_eq!(c.requests().len(),1);
    }

    #[test]
    fn media_cors_return_home_still_requires_header_and_literal_null_is_not_missing(){
        let a=Server::new("127.0.0.1");let b=Server::new("127.0.0.1");a.put("/start",302,&format!("Location: {}\r\n",b.url("/hop").display()));b.put("/hop",302,&format!("Access-Control-Allow-Origin: {}\r\nLocation: {}\r\n",a.serialized(),a.url("/missing").display()));a.put("/missing",200,"");
        let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust);assert!(fetch(&pool,&a.url("/start"),&a.origin,Credentials::SameOrigin).unwrap_err().contains("missing Access-Control-Allow-Origin"));assert_eq!(value(&a.requests()[1],"Origin"),Some("null"));
    }

    #[test]
    fn media_cors_guards_every_destination_and_rejects_initial_mixed_content_or_redirect_userinfo(){
        let a=Server::new("127.0.0.1");let b=Server::new("127.0.0.1");a.put("/start",302,&format!("Location: {}\r\n",b.url("/blocked").display()));a.put("/userinfo",302,&format!("Location: http://user@{}/blocked\r\n",b.origin.host_header()));
        let cancel=AtomicBool::new(false);let trust=trust::Store::unconfigured();let pool=Pool::new(&cancel,&trust);let calls=AtomicUsize::new(0);let guard=|target:&HttpUrl|{calls.fetch_add(1,Ordering::Relaxed);if target.same_origin(&b.origin){Err("Original media CSP guard blocked destination.".into())}else{Ok(())}};
        assert!(pool.fetch_cors_media(&a.url("/start"),1024,&a.origin,Credentials::SameOrigin,FetchMode::Normal,&guard).unwrap_err().contains("guard blocked"));assert_eq!(calls.load(Ordering::Relaxed),2);assert!(b.requests().is_empty());
        let mut secure=a.origin.clone();secure.secure=true;assert!(fetch(&pool,&a.url("/never"),&secure,Credentials::SameOrigin).unwrap_err().contains("unencrypted"));assert_eq!(a.requests().len(),1);
        assert!(fetch(&pool,&a.url("/userinfo"),&a.origin,Credentials::SameOrigin).unwrap_err().contains("credentials"));assert!(b.requests().is_empty());
    }
}
