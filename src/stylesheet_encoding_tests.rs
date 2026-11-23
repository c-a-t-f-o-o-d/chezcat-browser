use super::*;

#[test]
fn stylesheet_encoding_css_signature_is_exact_and_bounded() {
    for bytes in [
        &b" @charset \"Shift_JIS\";"[..],b"@Charset \"Shift_JIS\";",b"@charset  \"Shift_JIS\";",
        b"@charset 'Shift_JIS';",b"/**/@charset \"Shift_JIS\";",b"@charset \"Shift_JIS\" ;",
        b"@charset \"Shift_JIS\"",b"@charset \"Sh\xffift_JIS\";",b"@charset \"unknown\";",
    ] { assert_eq!(declaration(bytes),None,"{bytes:?}"); }
    assert_eq!(declaration(b"@charset \"Shift_JIS\";").unwrap().name(),"shift_jis");
    for label in ["utf-16le","utf-16be"] {assert_eq!(declaration(format!("@charset \"{label}\";").as_bytes()),Some(Encoding::UTF8));}
    let exact=format!("@charset \"{}utf-8\";"," ".repeat(1007));
    assert_eq!(exact.len(),1024);assert_eq!(declaration(exact.as_bytes()),Some(Encoding::UTF8));
    let over=format!("@charset \"{}utf-8\";"," ".repeat(1008));
    assert_eq!(declaration(over.as_bytes()),None);
}

#[test]
fn stylesheet_encoding_transport_declaration_environment_and_utf8_default() {
    let sjis=Encoding::label("Shift_JIS").unwrap();
    assert_eq!(decode(&[0x82,0xa0],"text/css;charset=ms_kanji",Some(Encoding::UTF8),3,&||Ok(())).unwrap(),"あ");
    let mut declared=b"@charset \"Shift_JIS\";".to_vec();declared.extend([0x82,0xa0]);
    assert_eq!(decode(&declared,"text/css;charset=unknown",Some(Encoding::WINDOWS1252),100,&||Ok(())).unwrap(),"@charset \"Shift_JIS\";あ");
    assert_eq!(decode(&[0x81],"text/css;charset=windows-1252",Some(sjis),2,&||Ok(())).unwrap(),"\u{81}");
    assert_eq!(decode(&[0x82,0xa0],"text/css",Some(sjis),3,&||Ok(())).unwrap(),"あ");
    assert_eq!(decode(&[0x81],"text/css",None,3,&||Ok(())).unwrap(),"�");
    assert_eq!(decode("é".as_bytes(),"text/css",Some(Encoding::WINDOWS1252),4,&||Ok(())).unwrap(),"Ã©");
}

#[test]
fn stylesheet_encoding_bom_and_utf16_declaration_remap_are_separate() {
    for bytes in [&b"\xef\xbb\xbfA"[..],b"\xff\xfeA\0",b"\xfe\xff\0A"] {
        assert_eq!(decode(bytes,"text/css;charset=windows-1252",Some(Encoding::REPLACEMENT),1,&||Ok(())).unwrap(),"A");
    }
    for label in ["utf-16le","utf-16be"] {
        let source=format!("@charset \"{label}\";é");
        assert_eq!(decode(source.as_bytes(),"text/css",Some(Encoding::WINDOWS1252),source.len(),&||Ok(())).unwrap(),source);
    }
    assert_eq!(decode(b"A\0","text/css;charset=utf-16le",None,1,&||Ok(())).unwrap(),"A");
}

#[test]
fn stylesheet_encoding_output_admission_cancellation_and_recovery() {
    use std::cell::Cell;
    let bytes=vec![0x80;4096];
    assert!(decode(&bytes,"text/css;charset=windows-1252",None,12287,&||Ok(())).is_err());
    assert_eq!(decode(&bytes,"text/css;charset=windows-1252",None,12288,&||Ok(())).unwrap().len(),12288);
    for stop in [1,3,5] {
        let count=Cell::new(0);let check=||{count.set(count.get()+1);if count.get()==stop{Err("CSS canceled".into())}else{Ok(())}};
        assert!(decode(&bytes,"text/css;charset=windows-1252",None,12288,&check).unwrap_err().contains("CSS canceled"));
    }
    assert_eq!(decode(b"A\0B\0","text/css;charset=utf-16le",None,2,&||Ok(())).unwrap(),"AB");
}

#[test]
fn stylesheet_encoding_initial_reuse_distinguishes_link_environment() {
    use crate::loader::{local,local_with_options,LoadOptions};
    let path=std::env::temp_dir().join(format!("chezcat-css-encoding-{}",std::process::id()));
    std::fs::create_dir(&path).unwrap();
    struct Cleanup(std::path::PathBuf);impl Drop for Cleanup {fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    let _cleanup=Cleanup(path.clone());
    std::fs::write(path.join("sheet.css"),b"/*\x82\xa0*/p{width:17px}").unwrap();
    std::fs::write(path.join("page.html"),"<link rel=stylesheet charset=Shift_JIS href=sheet.css><link rel=stylesheet charset=windows-1252 href=sheet.css><link rel=stylesheet charset=Shift_JIS href=sheet.css>").unwrap();
    let location=crate::navigation::explicit(&path.join("page.html")).unwrap();
    let loaded=local(&location).unwrap();let mut sources:Vec<_>=loaded.styles.iter().collect();sources.sort_by_key(|(node,_)|**node);
    assert_eq!(sources.len(),3);assert_eq!(sources[0].1,"/*あ*/p{width:17px}");assert_eq!(sources[1].1,"/*‚\u{a0}*/p{width:17px}");assert_eq!(sources[2].1,sources[0].1);
    assert!(loaded.events.iter().any(|s|s.contains("reused stylesheet")));
    // Encoded input fits; decoded expansion is refused before attachment.
    std::fs::write(path.join("sheet.css"),[0x80;20]).unwrap();
    let options=LoadOptions{stylesheet_limits:crate::css::StylesheetLimits{sheet_bytes:32,document_bytes:128,rules:8},..LoadOptions::default()};
    let refused=local_with_options(&location,options).unwrap();assert!(refused.styles.is_empty());assert!(refused.events.iter().any(|s|s.contains("Decoded text exceeds")));
}
