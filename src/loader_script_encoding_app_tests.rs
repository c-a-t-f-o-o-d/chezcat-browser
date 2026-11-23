//! Original linked-script byte witnesses through Windows App loading/execution.
use super::*;

fn script_bytes(bytes: &[u8]) -> Vec<u8> {
    replace_bytes(b"document.getElementById('probe').textContent='_VALUE_';document.title=document.getElementById('probe').textContent;", b"_VALUE_", bytes)
}

#[test]
fn linked_script_app_initial_charset_precedence_and_real_execution() {
    for (header, attr, bytes, expected) in [
        ("text/javascript;charset=Shift_JIS", "windows-1252", script_bytes(&[0x82,0xa0]), "あ"),
        ("text/javascript", "Shift_JIS", script_bytes(&[0x82,0xa0]), "あ"),
        ("text/javascript;charset=unknown", "unknown", script_bytes(&[0x82,0xa0]), "あ"),
        ("text/javascript;charset=windows-1252", "Shift_JIS", script_bytes(&[0x81]), "\u{81}"),
        ("text/javascript;charset=utf-8", "Shift_JIS", script_bytes(&[0xe2,0x82]), "�"),
        ("text/javascript;charset=windows-1252", "Shift_JIS", utf16(std::str::from_utf8(&script_bytes("あ".as_bytes())).unwrap(),true), "あ"),
    ] {
        let source=document("", "waiting", false)+&format!("<script charset='{attr}' src='/source'></script>");
        let site=Site::new(vec![
            Response{path:"/page",content_type:"text/html;charset=Shift_JIS",headers:"",body:source.into_bytes()},
            Response{path:"/source",content_type:header,headers:"",body:bytes},
        ]);
        let mut app=open(&site,"/page");
        assert_eq!(app.document.title,expected,"{}",app.activity_report());
        assert_eq!(app.document.encoding.document_name(),"Shift_JIS");
        assert_painted_document(&app,expected);
        exercise_editor_and_link(&mut app);
        assert_eq!(site.requests(),["/page","/source"]);
    }
}

#[test]
fn linked_script_app_dynamic_preparation_captures_charset_and_document_fallback() {
    for (header, attr, bytes, expected) in [
        ("text/javascript", "Shift_JIS", script_bytes(&[0x82,0xa0]), "あ"),
        ("text/javascript;charset=unknown", "unknown", script_bytes(&[0x82,0xa0]), "あ"),
        ("text/javascript;charset=windows-1252", "Shift_JIS", script_bytes(&[0x81]), "\u{81}"),
        ("text/javascript;charset=Shift_JIS", "windows-1252", utf16(std::str::from_utf8(&script_bytes("Ω".as_bytes())).unwrap(),false), "Ω"),
    ] {
        let prepare=format!("<script>var s=document.createElement('script');s.setAttribute('charset','{attr}');s.src='/source';document.body.appendChild(s);s.setAttribute('charset','windows-1252');</script>");
        let source=document("", "waiting", false)+&prepare;
        let site=Site::new(vec![
            Response{path:"/page",content_type:"text/html;charset=Shift_JIS",headers:"",body:source.into_bytes()},
            Response{path:"/source",content_type:header,headers:"",body:bytes},
        ]);
        let mut app=open(&site,"/page");
        assert_eq!(app.document.title,expected,"{}",app.activity_report());
        assert_painted_document(&app,expected);
        exercise_editor_and_link(&mut app);
        assert_eq!(site.requests(),["/page","/source"]);
    }
}

#[test]
fn linked_script_app_local_files_inherit_without_synthetic_transport_charset() {
    let folder=Folder::new();
    folder.file("source.js",&script_bytes(&[0x82,0xa0]));
    let source=document("<meta charset=Shift_JIS>","waiting",false)+"<script src=source.js></script>";
    let path=folder.file("page.html",source.as_bytes());
    let mut app=app();app.open_address(path.to_str().unwrap());wait(&mut app);
    assert!(!app.failed,"{}",app.activity_report());
    assert_eq!(app.document.title,"あ","{}",app.activity_report());
    assert_painted_document(&app,"あ");exercise_editor_and_link(&mut app);
}

#[test]
fn native_app_integrated_utf16_codecs_slicing_and_anchor_protocol_effects() {
    let source=document("", "waiting", false)+r#"<script>
var value='A\uD800B\uDC00C';
if(value.slice(1,2).charCodeAt(0)!==55296||value.substring(3,4).charCodeAt(0)!==56320||value.substr(1,1).length!==1)throw 'slice';
var bytes=new TextEncoder().encode(value);
if(new TextDecoder().decode(bytes)!=='A\uFFFDB\uFFFDC')throw 'codec';
var a=document.getElementById('go');
a.href='http://user:pass@example.test:8080/path?x=1#old';
a.protocol='file:';
if(a.getAttribute('href')!=='http://user:pass@example.test:8080/path?x=1#old')throw 'guarded file';
a.protocol='mailto:';
if(a.href!=='http://user:pass@example.test:8080/path?x=1#old')throw 'non-special';
a.href='#done';document.getElementById('probe').textContent='native integrated';document.title='native integrated';
</script>"#;
    let site=Site::new(vec![Response{path:"/page",content_type:"text/html;charset=utf-8",headers:"",body:source.into_bytes()}]);
    let mut app=open(&site,"/page");
    assert_eq!(app.document.title,"native integrated","{}",app.activity_report());
    assert_painted_document(&app,"native integrated");exercise_editor_and_link(&mut app);
}

#[test]
fn script_charset_domstring_app_exact_metadata_identity_and_conversion_observation() {
    let source=document("","waiting",false)+r#"<script>
var s=document.createElement('script');s.charset='Shift_JIS';var attr=s.getAttributeNode('charset');
var observer=new MutationObserver(function(){});observer.observe(s,{attributes:true,attributeOldValue:true,attributeFilter:['charset']});
var calls=0;s.charset={toString:function(){calls++;s.setAttribute('charset','during');return 'x\ud800';}};
var records=observer.takeRecords();
if(calls!==1||s.charset!=='x\ud800'||attr.value!==s.charset||s.getAttribute('charset')!==s.charset||records.length!==2||records[0].oldValue!=='Shift_JIS'||records[1].oldValue!=='during')throw 'exact script charset conversion';
attr.value='\udfff';records=observer.takeRecords();if(s.charset!=='\udfff'||records.length!==1||records[0].oldValue!=='x\ud800')throw 'attached script Attr';
var replacement=document.createAttribute('charset');replacement.value='\ud800\ufffd';s.setAttributeNode(replacement);records=observer.takeRecords();
if(s.charset!=='\ud800\ufffd'||replacement.ownerElement!==s||attr.ownerElement!==null||records.length!==1||records[0].oldValue!=='\udfff')throw 'script charset Attr transfer';
s.removeAttribute('charset');records=observer.takeRecords();if(s.charset!==''||replacement.ownerElement!==null||records.length!==1||records[0].oldValue!=='\ud800\ufffd')throw 'raw charset removal';
document.getElementById('probe').textContent='Exact script metadata';document.title='Exact script metadata';
</script>"#;
    let site=Site::new(vec![Response{path:"/page",content_type:"text/html;charset=utf-8",headers:"",body:source.into_bytes()}]);
    let mut app=open(&site,"/page");assert_eq!(app.document.title,"Exact script metadata","{}",app.activity_report());
    assert_painted_document(&app,"Exact script metadata");exercise_editor_and_link(&mut app);assert_eq!(site.requests(),["/page"]);
}

#[test]
fn script_charset_domstring_app_unknown_raw_label_and_captured_fallback() {
    for (header,before,after,expected) in [
        ("text/html;charset=Shift_JIS",r"Shift_JIS\ud800","windows-1252","あ"),
        ("text/html;charset=windows-1252","Shift_JIS",r"Shift_JIS\ud800","あ"),
        ("text/html;charset=windows-1252",r"Shift_JIS\ud800","Shift_JIS","‚\u{a0}"),
    ] {
        let prepare=format!("<script>var s=document.createElement('script');s.charset='{before}';s.src='/source';document.body.appendChild(s);s.charset='{after}';</script>");
        let source=document("","waiting",false)+&prepare;
        let site=Site::new(vec![
            Response{path:"/page",content_type:header,headers:"",body:source.into_bytes()},
            Response{path:"/source",content_type:"text/javascript",headers:"",body:script_bytes(&[0x82,0xa0])},
        ]);
        let mut app=open(&site,"/page");assert_eq!(app.document.title,expected,"{}",app.activity_report());
        assert_painted_document(&app,expected);exercise_editor_and_link(&mut app);assert_eq!(site.requests(),["/page","/source"]);
    }
}


#[test]
fn parsed_script_charset_app_exact_metadata_attr_and_observer_without_activation() {
    let source=document("","waiting",false)+r#"<script id=parsed type=application/x-inert charset=seed>throw 'must stay inert';</script><script>
var s=document.getElementById('parsed');if(s.charset!=='seed'||!('charset' in s))throw 'parsed charset reflection';
var attr=s.getAttributeNode('charset'),observer=new MutationObserver(function(){});observer.observe(s,{attributes:true,attributeOldValue:true,attributeFilter:['charset']});
var calls=0;s.charset={toString:function(){calls++;s.setAttribute('charset','during');return '\ud800';}};
var records=observer.takeRecords();if(calls!==1||s.charset!=='\ud800'||s.getAttribute('charset')!==s.charset||attr.value!==s.charset||records.length!==2||records[0].oldValue!=='seed'||records[1].oldValue!=='during')throw 'parsed exact conversion and records';
attr.value='x\udfff';var replacement=document.createAttribute('charset');replacement.value='\ud800\ufffd';s.setAttributeNode(replacement);
records=observer.takeRecords();if(records.length!==2||records[0].oldValue!=='\ud800'||records[1].oldValue!=='x\udfff'||attr.ownerElement!==null||replacement.ownerElement!==s||s.charset!==replacement.value)throw 'parsed Attr identity';
s.removeAttributeNode(replacement);records=observer.takeRecords();if(s.charset!==''||replacement.ownerElement!==null||records.length!==1||records[0].oldValue!=='\ud800\ufffd')throw 'parsed removal';
s.charset=null;if(s.getAttribute('charset')!=='null')throw 'parsed nullable DOMString';
document.getElementById('probe').textContent='Parsed script metadata';document.title='Parsed script metadata';
</script>"#;
    let site=Site::new(vec![Response{path:"/page",content_type:"text/html;charset=utf-8",headers:"",body:source.into_bytes()}]);
    let mut app=open(&site,"/page");assert_eq!(app.document.title,"Parsed script metadata","{}",app.activity_report());
    assert_painted_document(&app,"Parsed script metadata");exercise_editor_and_link(&mut app);assert_eq!(site.requests(),["/page"]);
}

#[test]
fn parsed_script_charset_app_metadata_preserves_loaded_source_execution() {
    let source=document("","waiting",false)+r#"<script>var runs=0;</script><script id=parsed charset=Shift_JIS src=/source></script><script>
var s=document.getElementById('parsed'),originalSource=s.src;if(runs!==1||document.title!=='あ'||s.charset!=='Shift_JIS')throw 'original parsed execution';
s.charset='windows-1252';s.setAttribute('charset','\ud800');var a=s.getAttributeNode('charset');a.value='Shift_JIS';
if(runs!==1||s.charset!=='Shift_JIS'||s.src!==originalSource)throw 'parsed authority changed';
document.getElementById('probe').textContent='Parsed source retained';document.title='Parsed source retained';
</script>"#;
    let site=Site::new(vec![
        Response{path:"/page",content_type:"text/html;charset=utf-8",headers:"",body:source.into_bytes()},
        Response{path:"/source",content_type:"text/javascript",headers:"",body:b"runs++;document.title='\x82\xa0';".to_vec()},
    ]);
    let mut app=open(&site,"/page");assert_eq!(app.document.title,"Parsed source retained","{}",app.activity_report());
    assert_painted_document(&app,"Parsed source retained");exercise_editor_and_link(&mut app);assert_eq!(site.requests(),["/page","/source"]);
}
