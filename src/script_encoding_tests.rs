use super::*;

fn decode(bytes: &[u8], header: &str, fallback: Encoding, expected: &str) {
    assert_eq!(decode_script(bytes,header,fallback,expected.len(),&||Ok(())).unwrap(),expected);
    if !expected.is_empty() {
        assert!(decode_script(bytes,header,fallback,expected.len()-1,&||Ok(())).is_err());
    }
}

#[test]
fn script_encoding_response_charset_uses_shared_labels_and_replacement() {
    for (bytes,header,expected) in [
        (&b"\x82\xa0"[..],"text/javascript; charset=ms_kanji","あ"),
        (&b"\x81"[..],"text/javascript; charset=windows-1252","\u{81}"),
        (&b"\x82 "[..],"text/javascript; charset=Shift_JIS","� "),
        (&b"\xe2\x82"[..],"text/javascript; charset=utf-8","�"),
        (&b"\0\xd8"[..],"text/javascript; charset=utf-16le","�"),
        (&b"abc"[..],"text/javascript; charset=iso-2022-kr","�"),
        (&b"\x80"[..],"text/javascript; charset=x-user-defined","\u{f780}"),
        (&b"\x1b(Babc"[..],"text/javascript; charset=iso-2022-jp","abc"),
    ] { decode(bytes,header,Encoding::UTF8,expected); }
}

#[test]
fn script_encoding_response_mime_and_unknown_label_fallback() {
    let sjis=Encoding::label("shift_jis").unwrap();
    for header in ["", "charset=utf-8", "text/javascript", "text/javascript;charset=unknown",
        "text/javascript;charset=", "not a MIME", "text/javascript;charset=unknown;charset=utf-8"] {
        decode(&[0x82,0xa0],header,sjis,"あ");
    }
    decode(&[0x82,0xa0],"text/javascript;charset=\"Shift_JIS\";charset=utf-8",Encoding::UTF8,"あ");
    decode(&[0x81],"application/octet-stream;charset=windows-1252",Encoding::UTF8,"\u{81}");
}

#[test]
fn script_encoding_bom_overrides_header_and_fallback_and_consumes_one_signature() {
    for (bytes,expected) in [
        (&b"\xef\xbb\xbf\xe3\x81\x82"[..],"あ"),
        (&b"\xff\xfeB0"[..],"あ"),
        (&b"\xfe\xff0B"[..],"あ"),
        (&b"\xef\xbb\xbf\xef\xbb\xbf"[..],"\u{feff}"),
        (&b"\xff\xfe\xff\xfe"[..],"\u{feff}"),
        (&b"\xfe\xff\xfe\xff"[..],"\u{feff}"),
        (&b"\xff\xfe"[..],""),
    ] { decode(bytes,"text/javascript;charset=windows-1252",Encoding::REPLACEMENT,expected); }
}

#[test]
fn script_encoding_fallback_is_document_identity_without_utf8_sniffing() {
    decode("é".as_bytes(),"text/javascript",Encoding::WINDOWS1252,"Ã©");
    decode(b"A\0B\0","",Encoding::UTF16LE,"AB");
    for (attr,expected) in [(Some("Shift_JIS"),"shift_jis"),(Some("unknown"),"windows-1252"),(Some(""),"windows-1252"),(None,"windows-1252")] {
        let mut doc=crate::html::parse(&format!("<script {}></script>",attr.map(|v|format!("charset='{v}'")).unwrap_or_default()));
        doc.encoding=Encoding::WINDOWS1252;
        let node=(0..doc.nodes.len()).find(|&n|matches!(&doc.nodes[n].kind,crate::html::NodeKind::Element{tag,..} if tag=="script")).unwrap();
        assert_eq!(script_fallback(&doc,node).name(),expected);
    }
}

#[test]
fn script_encoding_output_refusal_cancellation_and_fresh_decode_recovery() {
    use std::cell::Cell;
    let bytes=vec![0x80;4096];
    assert!(decode_script(&bytes,"",Encoding::WINDOWS1252,12287,&||Ok(())).is_err());
    assert_eq!(decode_script(&bytes,"",Encoding::WINDOWS1252,12288,&||Ok(())).unwrap().len(),12288);
    for stop in [1,3,5] {
        let calls=Cell::new(0);
        let check=||{calls.set(calls.get()+1);if calls.get()>=stop{Err("canceled original script".into())}else{Ok(())}};
        assert!(decode_script(&bytes,"",Encoding::WINDOWS1252,12288,&check).unwrap_err().contains("canceled original"));
    }
    decode(b"recovered","",Encoding::UTF8,"recovered");
}

#[test]
fn script_encoding_initial_encoded_and_decoded_limits_are_independent() {
    use crate::loader::{local_with_options,LoadOptions};
    let path=std::env::temp_dir().join(format!("chezcat-script-encoding-{}",std::process::id()));
    std::fs::create_dir(&path).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup { fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);} }
    let _cleanup=Cleanup(path.clone());
    let encoded:Vec<_>=b"1;1;1;1;1;".iter().flat_map(|&b|[b,0]).collect();
    std::fs::write(path.join("source.js"),&encoded).unwrap();
    std::fs::write(path.join("page.html"),format!("<script>{}</script><script charset=utf-16le src=source.js></script>","1;".repeat(45))).unwrap();
    let page=crate::navigation::explicit(&path.join("page.html")).unwrap();
    let mut capacity=crate::javascript::limits::REGRESSION_CAPACITIES;
    capacity.source_bytes=100;capacity.retained_source=100;
    let load=||local_with_options(&page,LoadOptions{script_capacity:capacity,..LoadOptions::default()}).unwrap();
    let loaded=load();assert_eq!(loaded.scripts.len(),2,"{:?}",loaded.events);
    assert_eq!(loaded.scripts[1].source,"1;1;1;1;1;");
    std::fs::write(path.join("source.js"),vec![b' ';102]).unwrap();
    let refused=load();assert_eq!(refused.scripts.len(),1);assert_eq!(refused.skipped,1);
    std::fs::write(path.join("source.js"),&encoded).unwrap();
    assert_eq!(load().scripts.len(),2);
}
