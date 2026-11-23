//! Initial stylesheet bytes through real document loading, layout and paint.
use super::*;

fn sheet() -> Vec<u8> {
    replace_bytes(b"#probe[data-name='_VALUE_']{width:211px;background:#123456}",b"_VALUE_",&[0x82,0xa0])
}

fn page(attr: &str) -> String {
    document("", "Styled text", false).replace("id=probe", "id=probe data-name='&#x3042;'")
        +&format!("<link rel=stylesheet charset='{attr}' href='/source.css'>")
}

fn assert_style(app: &mut App) {
    let probe=id(app,"probe");let rect=app.page.border_rects[probe].unwrap();
    assert_eq!(rect.width,211,"{}",app.activity_report());
    let canvas=app.draw();let x=rect.x+rect.width-8;let y=app.content_top()+rect.y+rect.height-8-app.scroll;
    assert_eq!(canvas.pixels[y as usize*canvas.width+x as usize]&0xffffff,0x123456);
    exercise_editor_and_link(app);
}

#[test]
fn stylesheet_encoding_app_initial_transport_declaration_environment_and_bom() {
    let mut declared=b"@charset \"Shift_JIS\";".to_vec();declared.extend(sheet());
    let unicode="#probe[data-name='あ']{width:211px;background:#123456}";
    for (header,doc_header,attr,body) in [
        ("text/css;charset=Shift_JIS","text/html;charset=utf-8","windows-1252",sheet()),
        ("text/css;charset=unknown","text/html;charset=utf-8","windows-1252",declared),
        ("text/css","text/html;charset=utf-8","Shift_JIS",sheet()),
        ("text/css","text/html;charset=Shift_JIS","unknown",sheet()),
        ("text/css;charset=windows-1252","text/html;charset=utf-8","windows-1252",utf16(unicode,true)),
    ] {
        let site=Site::new(vec![
            Response{path:"/page",content_type:doc_header,headers:"",body:page(attr).into_bytes()},
            Response{path:"/source.css",content_type:header,headers:"",body},
        ]);
        let mut app=open(&site,"/page");assert_style(&mut app);
        assert_eq!(site.requests(),["/page","/source.css"]);
    }
}

#[test]
fn stylesheet_encoding_app_initial_local_declaration_and_document_fallback() {
    let folder=Folder::new();
    let mut declared=b"@charset \"Shift_JIS\";".to_vec();declared.extend(sheet());
    for (meta,bytes) in [("",declared),("<meta charset=Shift_JIS>",sheet())] {
        folder.file("source.css",&bytes);
        let source=page("unknown").replace("href='/source.css'","href='source.css'").replace("<head>",&format!("<head>{meta}"));
        let path=folder.file("page.html",source.as_bytes());
        let mut app=app();app.open_address(path.to_str().unwrap());wait(&mut app);
        assert!(!app.failed,"{}",app.activity_report());assert_style(&mut app);
    }
}
