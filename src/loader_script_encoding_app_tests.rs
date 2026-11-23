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
