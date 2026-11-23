//! Fixed native completion witnesses, including live LINK environment selection.
use super::*;

pub(super) fn response(path:&str)->Option<(&'static str,Vec<u8>)> {
    let source="#target[data-name='あ']{width:211px;height:40px;background:#123456}";
    let mut sjis=b"#target[data-name='".to_vec();sjis.extend([0x82,0xa0]);sjis.extend_from_slice(b"']{width:211px;height:40px;background:#123456}");
    Some(match path {
        "/encoding-page"|"/encoding-sjis-page" => (
            if path=="/encoding-sjis-page"{"text/html;charset=Shift_JIS"}else{"text/html;charset=utf-8"},
            b"<!doctype html><title>Ready</title><style>body{margin:0}#field{position:absolute;left:10px;top:10px;width:160px;height:30px}#target{position:absolute;left:10px;top:65px;width:200px;height:40px}#go{position:absolute;left:250px;top:10px;width:100px;height:30px}#done{position:absolute;left:250px;top:65px}</style><body><input id=field value=seed><div id=target data-name='&#x3042;'>Visible target</div><a id=go href=#done>Real link</a><div id=done>Destination</div>".to_vec()),
        "/encoding-sjis.css" => ("text/css",sjis),
        "/encoding-header.css" => ("text/css;charset=Shift_JIS",sjis),
        "/encoding-declaration.css" => {let mut body=b"@charset \"Shift_JIS\";".to_vec();body.extend(sjis);("text/css;charset=unknown",body)},
        "/encoding-utf16.css" => {let mut body=vec![0xff,0xfe];body.extend(source.encode_utf16().flat_map(u16::to_le_bytes));("text/css;charset=windows-1252",body)},
        "/encoding-cp81.css" => ("text/css;charset=windows-1252",b"#target[data-name='\x81']{width:211px;height:40px;background:#123456}".to_vec()),
        "/encoding-expansion.css" => {let mut body=b"/*".to_vec();body.extend([0x80;1000]);body.extend_from_slice(b"*/#target{width:999px}");("text/css;charset=windows-1252",body)},
        _=>return None,
    })
}

fn ready(site:&Site,path:&str)->App {
    let mut app=open(site,path);wait(&mut app,|app|!app.is_loading());app
}
fn request(app:&mut App,path:&str,charset:&str) {
    queue_base_change(app,&format!("var sheet=document.createElement('link');sheet.rel='stylesheet';sheet.setAttribute('charset','{charset}');sheet.href='{path}';sheet.onload=function(){{if(document.getElementById('target').clientWidth!==211)throw 'decoded geometry before load';document.title='Decoded';}};sheet.onerror=function(){{document.title='Refused';}};document.head.appendChild(sheet);"));
}
fn node(app:&App,name:&str)->usize {
    (0..app.document.nodes.len()).find(|&n|app.document.attr(n,"id")==Some(name)).unwrap()
}
fn assert_app(app:&mut App) {
    assert_eq!(app.document.title,"Decoded","{}",app.scripting.as_ref().unwrap().report());
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    let target=node(app,"target");let rect=app.page.border_rects[target].unwrap();assert_eq!(rect.width,211);
    let canvas=app.draw();let x=rect.x+rect.width-8;let y=app.content_top()+rect.y+rect.height-8-app.scroll;
    assert_eq!(canvas.pixels[y as usize*canvas.width+x as usize]&0xffffff,0x123456);
    let field=node(app,"field");let rect=app.page.controls.iter().find(|rect|rect.node==field).unwrap();
    let (x,y)=(rect.x+12,app.content_top()+rect.y+12-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);
    app.key(0x41,true,false,false);app.paste("edited Ω");
    let go=node(app,"go");let rect=app.page.border_rects[go].unwrap();let (x,y)=(rect.x+12,rect.y+12);
    assert_eq!(app.page.link_node_at(x,y),Some(go));let y=app.content_top()+y-app.scroll;app.pointer_down(x,y,false);app.pointer_up(x,y);
    wait(app,|app|!app.is_loading());assert!(app.activity_report().lines().any(|line|line.starts_with("DOCUMENT: ")&&line.ends_with("#done")));
    let rect=app.page.controls.iter().find(|rect|rect.node==field).unwrap();let (x,y)=(rect.x+12,app.content_top()+rect.y+12-app.scroll);app.pointer_down(x,y,false);app.pointer_up(x,y);
    app.key(0x41,true,false,false);match app.key(0x43,true,false,false) {PlatformAction::Copy(text)=>assert_eq!(text,"edited Ω"),_=>panic!("Native editor did not retain edited text")}
}

#[test]
fn dynamic_stylesheet_encoding_live_link_environment_after_response() {
    let site=Site::new();let mut app=ready(&site,"/encoding-page");
    request(&mut app,"/encoding-sjis.css","windows-1252");let completion=take_base_completion(&app);
    // The HTTP bytes are already complete; changing charset must still affect decoding.
    queue_base_change(&mut app,"sheet.charset='Shift_JIS';var charsetAttr=sheet.getAttributeNode('charset');charsetAttr.value='windows-1252';sheet.removeAttribute('charset');var replacement=document.createAttribute('charset');replacement.value='Shift_JIS';sheet.setAttributeNode(replacement);if(sheet.charset!=='Shift_JIS'||sheet.getAttributeNode('charset')!==replacement)throw 'live charset identity';");
    assert!(app.accept_dynamic_stylesheet(completion));wait(&mut app,|app|!app.is_loading());assert_app(&mut app);
    let css=app.styles.clone();queue_base_change(&mut app,"sheet.charset='utf-8';");
    assert_eq!(app.pending_stylesheets,0);assert_eq!(app.styles,css);
    assert_eq!(app.page.border_rects[node(&app,"target")].unwrap().width,211);
    assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/encoding-sjis.css").count(),1);
}

#[test]
fn dynamic_stylesheet_encoding_precedence_and_native_app_effects() {
    for (page,path,charset,cp81) in [
        ("/encoding-page","/encoding-header.css","windows-1252",false),
        ("/encoding-page","/encoding-declaration.css","windows-1252",false),
        ("/encoding-sjis-page","/encoding-sjis.css","unknown",false),
        ("/encoding-page","/encoding-utf16.css","windows-1252",false),
        ("/encoding-page","/encoding-cp81.css","Shift_JIS",true),
    ] {
        let site=Site::new();let mut app=ready(&site,page);
        if cp81 {eval(&mut app,r"document.getElementById('target').setAttribute('data-name','\u0081');");}
        request(&mut app,path,charset);wait(&mut app,|app|!app.is_loading());assert_app(&mut app);
    }
}

#[test]
fn dynamic_stylesheet_encoding_output_refusal_recovers_without_partial_css() {
    let site=Site::new();let mut app=ready(&site,"/encoding-page");
    app.document.stylesheet_limits=crate::css::StylesheetLimits{sheet_bytes:1500,document_bytes:8192,rules:16};
    request(&mut app,"/encoding-expansion.css","utf-8");wait(&mut app,|app|!app.is_loading());
    assert_eq!(app.document.title,"Refused");assert!(!app.styles.values().any(|s|s.contains("width:999px")));
    assert_eq!(app.page.border_rects[node(&app,"target")].unwrap().width,200);
    let page=app.scripting.as_ref().unwrap();assert_eq!(page.error_count(),1);assert!(page.report().contains("Decoded text exceeds the remaining 1500 byte allowance."),"{}",page.report());
    queue_base_change(&mut app,"sheet.href='/encoding-header.css';");wait(&mut app,|app|!app.is_loading());
    assert_eq!(app.document.title,"Decoded");assert_eq!(app.page.border_rects[node(&app,"target")].unwrap().width,211);
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),1);
}

#[test]
fn dynamic_stylesheet_encoding_current_document_bytes_and_shrinking_input() {
    let source="#target[data-name='あ']{width:211px;height:40px;background:#123456}";
    assert!(response("/encoding-utf16.css").unwrap().1.len()>source.len());
    for refuse in [false,true] {
        let site=Site::new();let mut app=ready(&site,"/encoding-page");
        let used=crate::css::stylesheet_usage(&app.document,&app.styles).bytes;
        app.document.stylesheet_limits=crate::css::StylesheetLimits{sheet_bytes:1024,document_bytes:used+source.len(),rules:32};
        request(&mut app,"/encoding-utf16.css","windows-1252");let completion=take_base_completion(&app);
        if refuse {app.document.stylesheet_limits.document_bytes-=1;}
        assert!(app.accept_dynamic_stylesheet(completion));wait(&mut app,|app|!app.is_loading());
        if refuse {
            assert_eq!(app.document.title,"Refused");assert_eq!(app.page.border_rects[node(&app,"target")].unwrap().width,200);
            assert!(app.styles.is_empty());let page=app.scripting.as_ref().unwrap();assert_eq!(page.error_count(),1);
            assert!(page.report().contains(&format!("Decoded text exceeds the remaining {} byte allowance.",source.len()-1)),"{}",page.report());
            app.document.stylesheet_limits.document_bytes+=1;
            queue_base_change(&mut app,"sheet.href='/encoding-header.css';");wait(&mut app,|app|!app.is_loading());
            assert_eq!(app.document.title,"Decoded");assert_eq!(app.page.border_rects[node(&app,"target")].unwrap().width,211);
            assert_eq!(app.scripting.as_ref().unwrap().error_count(),1);
        } else {assert_app(&mut app);}
    }
}

#[test]
fn dynamic_stylesheet_encoding_raw_completion_capacity_and_retirement() {
    let site=Site::new();let mut app=ready(&site,"/encoding-page");
    request(&mut app,"/encoding-sjis.css","Shift_JIS");let mut completion=take_base_completion(&app);
    let response=completion.result.as_mut().unwrap();
    assert!(response.bytes.windows(2).any(|bytes|bytes==[0x82,0xa0]));assert_eq!(response.content_type,"text/css");
    // Spare byte/header capacity is retained payload too, even before decoding.
    let mut bytes=Vec::with_capacity(16384);bytes.extend_from_slice(&response.bytes);response.bytes=bytes;
    let mut header=String::with_capacity(4096);header.push_str(&response.content_type);response.content_type=header;
    let minimum=response.bytes.capacity()+response.content_type.capacity()+response.base.payload_bytes();
    assert!(completion_payload(&completion)>=minimum.saturating_sub(COMPLETION_ERROR_BYTES));
    // A retired response would exceed the present output limit if decoded.
    app.document.stylesheet_limits.document_bytes=0;
    queue_base_change(&mut app,"sheet.disabled=true;");assert!(app.accept_dynamic_stylesheet(completion));
    wait(&mut app,|app|!app.is_loading());assert!(app.styles.is_empty()&&app.style_bases.is_empty());
    assert_eq!(app.scripting.as_ref().unwrap().error_count(),0);
    assert_eq!(app.dynamic_stylesheets.as_ref().unwrap().state.0.lock().unwrap().completed_payload,0);
}

#[test]
fn link_charset_domstring_app_exact_reflection_attribute_and_observation() {
    let site=Site::new();let mut app=ready(&site,"/encoding-page");
    eval(&mut app,r#"
var sheet=document.createElement('link');sheet.charset='Shift_JIS';document.head.appendChild(sheet);
var attr=sheet.getAttributeNode('charset'),observer=new MutationObserver(function(){});
observer.observe(sheet,{attributes:true,attributeOldValue:true,attributeFilter:['charset']});
var calls=0;sheet.charset={toString:function(){calls++;sheet.setAttribute('charset','during');return 'x\ud800';}};
var records=observer.takeRecords();
if(calls!==1||sheet.charset!=='x\ud800'||attr.value!==sheet.charset||sheet.getAttribute('charset')!==sheet.charset||records.length!==2||records[0].oldValue!=='Shift_JIS'||records[1].oldValue!=='during'||records[1].target!==sheet)throw 'exact LINK reflection and old value';
attr.value='\udfff';records=observer.takeRecords();
if(sheet.charset!=='\udfff'||records.length!==1||records[0].oldValue!=='x\ud800'||sheet.getAttributeNode('charset')!==attr)throw 'live raw Attr';
sheet.setAttribute('charset','\ud800\ufffd');records=observer.takeRecords();
if(sheet.charset!=='\ud800\ufffd'||records.length!==1||records[0].oldValue!=='\udfff')throw 'raw content attribute';
var replacement=document.createAttribute('charset');replacement.value='\udfff\ud800';sheet.setAttributeNode(replacement);records=observer.takeRecords();
if(sheet.charset!=='\udfff\ud800'||attr.ownerElement!==null||replacement.ownerElement!==sheet||records.length!==1||records[0].oldValue!=='\ud800\ufffd')throw 'raw Attr attachment';
sheet.removeAttribute('charset');records=observer.takeRecords();
if(sheet.charset!==''||replacement.ownerElement!==null||records.length!==1||records[0].oldValue!=='\udfff\ud800')throw 'raw removal';
sheet.charset='Shift_JIS';document.title='Charset metadata retained';
"#);
    assert_eq!(app.document.title,"Charset metadata retained");assert_eq!(app.pending_stylesheets,0);
    assert_eq!(site.requests.lock().unwrap().as_slice(),["/encoding-page"]);
}

#[test]
fn link_charset_domstring_app_raw_label_uses_live_document_fallback() {
    let site=Site::new();let mut app=ready(&site,"/encoding-sjis-page");
    request(&mut app,"/encoding-sjis.css","windows-1252");let completion=take_base_completion(&app);
    queue_base_change(&mut app,r#"sheet.charset='Shift_JIS\ud800';if(sheet.charset.charCodeAt(9)!==0xd800)throw 'raw label';"#);
    assert!(app.accept_dynamic_stylesheet(completion));wait(&mut app,|app|!app.is_loading());assert_app(&mut app);
    assert_eq!(site.requests.lock().unwrap().iter().filter(|path|path.as_str()=="/encoding-sjis.css").count(),1);
}
