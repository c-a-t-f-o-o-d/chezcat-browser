//! Original initial-document selection, byte-prescan and admission witnesses.
use super::*;

fn decoded(bytes: &[u8], content_type: &str, html: bool) -> String {
    decode(bytes, content_type, html, 64 * 1024, &|| Ok(())).unwrap().0
}

#[test]
fn document_encoding_transport_bom_labels_and_fallback_use_original_decoders() {
    let cases: &[(&[u8], &str, &str)] = &[
        (&[0x82, 0xa0], "text/html;charset=Shift_JIS", "あ"),
        (&[0xa4, 0xa2], "text/html;charset=euc-jp", "あ"),
        (b"\x1b$B$\"\x1b(B", "text/plain;charset=ISO-2022-JP", "あ"),
        (&[0xa4, 0x40], "text/html;charset=Big5", "一"),
        (&[0x88, 0x62], "text/html;charset=big5", "Ê\u{304}"),
        (&[0xd6, 0xd0], "text/plain;charset=gbk", "中"),
        (&[0x81, 0x30, 0x81, 0x30], "text/plain;charset=gb18030", "\u{80}"),
        (&[0xb0, 0xa1], "text/html;charset=euc-kr", "가"),
        (&[0xc0], "text/html;charset=windows-1251", "А"),
        (&[0x80, 0x81, 0x8d], "text/html;charset=windows-1252", "€\u{81}\u{8d}"),
        (&[0x80], "text/html;charset=x-user-defined", "\u{f780}"),
        (b"source must not appear", "text/html;charset=replacement", "\u{fffd}"),
        (b"", "text/html;charset=replacement", ""),
        (&[65, 0], "text/html;charset=utf-16", "A"),
        (&[0, 65], "text/html;charset=utf-16be", "A"),
        (&[0xef, 0xbb, 0xbf, 0xc3, 0xa9], "text/html;charset=replacement", "é"),
        (&[0xff, 0xfe, 65, 0], "text/html;charset=windows-1252", "A"),
        (&[0xfe, 0xff, 0, 65], "text/html;charset=unknown", "A"),
        (&[0xc3, 0xa9], "text/html;charset=unknown", "é"),
        (&[0x80], "text/html;charset=unknown", "€"),
        (&[0xc3, 0xa9], "text/html;charset='windows-1252'", "é"),
        (&[0xc3, 0xa9], "text/html;charset=\"\u{a0}windows-1252\"", "é"),
        (&[0x82, 0xa0], "text/html;charset=\" \tSHIFT_JIS \"", "あ"),
        (&[0xc3, 0xa9], "text/html;charset=utf-8;charset=windows-1252", "é"),
    ];
    for &(bytes, content_type, expected) in cases {
        assert_eq!(decoded(bytes, content_type, true), expected, "{content_type}");
    }
    assert_eq!(decoded(b"\xef\xbb\xbf\xef\xbb\xbfA", "", true), "\u{feff}A");
    assert_eq!(decoded(b"\xff\xfe\xff\xfeA\0", "", true), "\u{feff}A");
    assert_eq!(decoded(b"\xfe\xff\xfe\xff\0A", "", false), "\u{feff}A");
}

#[test]
fn document_encoding_prescan_uses_byte_attributes_duplicates_pragma_and_meta_remaps() {
    let cases: &[(&[u8], Option<&str>)] = &[
        (b"<MeTa\tCHARSET='ShIfT_JiS'>", Some("shift_jis")),
        (b"<meta/charset=euc-kr>", Some("euc-kr")),
        (b"<meta charset=unknown><meta charset=big5>", Some("big5")),
        (b"<meta charset=unknown charset=big5>", None),
        (b"<meta charset=big5 charset=unknown>", Some("big5")),
        (b"<meta charset=\"\"><meta charset=euc-jp>", Some("euc-jp")),
        (b"<meta content='charset=gbk'>", None),
        (b"<meta content=\"text/html; charset='gbk'\" http-equiv=CONTENT-TYPE>", Some("gbk")),
        (b"<meta http-equiv=content-type content='text/html;charset=gbk'>", Some("gbk")),
        (b"<meta http-equiv=' content-type' content='charset=gbk'>", None),
        (b"<meta http-equiv=wrong http-equiv=content-type content='charset=gbk'>", None),
        (b"<meta http-equiv=content-type content='charset=gbk' charset=big5>", Some("big5")),
        (b"<meta charset=unknown http-equiv=content-type content='charset=gbk'>", None),
        (b"<meta charset=utf-16>", Some("utf-8")),
        (b"<meta charset=utf-16be>", Some("utf-8")),
        (b"<meta charset=x-user-defined>", Some("windows-1252")),
        (b"<meta charset=iso-2022-cn>", Some("replacement")),
        (b"<!--<meta charset=big5>--><meta charset=gbk>", Some("gbk")),
        (b"<!--><meta charset=big5>", Some("big5")),
        (b"<!--<meta charset=big5>", None),
        (b"<div value='<meta charset=big5>'><meta charset=gbk>", Some("gbk")),
        (b"<script>'<meta charset=big5>'</script>", Some("big5")),
        (b"<metadata charset=big5>", None),
        (b"<meta charset=big5/>", None),
        (b"<meta charset=big5", None),
        (b"<meta charset='big5'", None),
        (b"<meta charset='big5", None),
        (b"<meta charset=&#117;tf-8>", None),
        (b"<meta charset='\xa0utf-8'>", None),
        (b"<!not-a-comment><meta charset=big5>", Some("big5")),
        (b"<?ignored?><meta charset=big5>", Some("big5")),
    ];
    for &(bytes, expected) in cases {
        assert_eq!(prescan(bytes).map(Encoding::name), expected, "{:?}", String::from_utf8_lossy(bytes));
    }
    let input = b"<meta charset=windows-1252>\xc3\xa9";
    assert!(decoded(input, "text/html;charset=unknown", true).ends_with("Ã©"));
    assert!(decoded(input, "text/html;charset=utf-8", true).ends_with("é"));
    assert!(decoded(input, "text/plain", false).ends_with("é"));
    assert!(decoded(b"<meta charset=x-user-defined>\x80", "", true).ends_with('€'));
}

#[test]
fn document_encoding_meta_content_extraction_is_distinct_from_http_mime() {
    for (content, expected) in [
        (b"text/html; CHARSET = 'big5'; ignored".as_slice(), Some("big5")),
        (b"charsetx; charset=gbk".as_slice(), Some("gbk")),
        (b"charsetcharset=euc-kr".as_slice(), Some("euc-kr")),
        (b"charset=\"big5\"".as_slice(), Some("big5")),
        (b"charset=unknown; charset=big5".as_slice(), None),
        (b"charset='big5".as_slice(), None),
        (b"charset=".as_slice(), None),
        (b"charset=big5;later".as_slice(), Some("big5")),
        (b"charset=big5\x0blater".as_slice(), None),
    ] {
        assert_eq!(meta_content_encoding(content).map(Encoding::name), expected);
    }
}

#[test]
fn document_encoding_prescan_xml_compatibility_and_1024_byte_end_are_bounded() {
    let cases: &[(&[u8], Option<&str>)] = &[
        (b"<\0?\0x\0m\0l\0", Some("utf-16le")),
        (b"\0<\0?\0x\0m\0l", Some("utf-16be")),
        (b"<?xml encoding='shift_jis'?>", Some("shift_jis")),
        (b"<?xml encoding='utf-16'?>", Some("utf-8")),
        (b"<?xml encoding='x-user-defined'?>", Some("x-user-defined")),
        (b"<?xml encoding='big5'?><meta charset=gbk>", Some("gbk")),
        (b"<?xml encoding\x01=\x02'big5'?>", Some("big5")),
        (b"<?XML encoding='big5'?>", None),
        (b"<?xml ENCODING='big5'?>", None),
        (b"<?xml encoding=' big5'?>", None),
        (b"<?xml encoding=big5?>", None),
        (b"<?xml encoding='big5'", None),
    ];
    for &(bytes, expected) in cases {
        assert_eq!(prescan(bytes).map(Encoding::name), expected);
    }
    let declaration = b"<meta charset=big5>";
    for (end, expected) in [(1023, Some("big5")), (1024, Some("big5")), (1025, None)] {
        let mut bytes = vec![b' '; end - declaration.len()];
        bytes.extend_from_slice(declaration);
        assert_eq!(prescan(&bytes).map(Encoding::name), expected, "end={end}");
    }
    let mut late = vec![b' '; 1024];
    late.extend_from_slice(declaration);
    assert!(prescan(&late).is_none());
    let mut xml = b"<?xml encoding='big5'".to_vec();
    xml.resize(1024, b' ');
    xml.push(b'>');
    assert!(prescan(&xml).is_none());
    // Truncating any prefix must terminate without speculative tree parsing.
    let input = b"<!--><div data='ignored><meta charset=unknown>'></div><meta http-equiv=content-type content=\"charset='big5'\">";
    for length in 0..=input.len() {
        let _ = prescan(&input[..length]);
    }
}

#[test]
fn document_encoding_malformed_sequences_preserve_ascii_markup_delimiters() {
    for (bytes, header, expected) in [
        (b"\x82\x22<script>".as_slice(), "text/html;charset=shift_jis", "�\"<script>"),
        (b"\xa4\x3cscript>".as_slice(), "text/html;charset=big5", "�<script>"),
        (b"\xc3\x3cscript>".as_slice(), "text/html;charset=utf-8", "�<script>"),
        (b"\xff\xfe\x00\xd8\x3c\0".as_slice(), "text/html", "�<"),
        (b"<script>secret</script>".as_slice(), "text/html;charset=iso-2022-kr", "�"),
    ] {
        assert_eq!(decoded(bytes, header, true), expected);
    }
}

#[test]
fn document_encoding_output_admission_and_cancellation_do_not_publish_partial_text() {
    for (bytes, content_type, expected) in [
        (b"A\x80".as_slice(), "text/html;charset=windows-1252", "A€"),
        (b"\x88\x62".as_slice(), "text/html;charset=big5", "Ê\u{304}"),
        (b"\xff\xfeA\0".as_slice(), "text/html", "A"),
        (b"secret".as_slice(), "text/html;charset=replacement", "�"),
        (b"\xef\xbb\xbfA\xff".as_slice(), "text/html", "A�"),
    ] {
        assert_eq!(decode(bytes, content_type, true, expected.len(), &|| Ok(())).unwrap().0, expected);
        assert!(decode(bytes, content_type, true, expected.len() - 1, &|| Ok(())).is_err());
        assert_eq!(decoded(b"recovered", "", true), "recovered");
    }
    assert_eq!(decode(b"", "text/html", true, 0, &|| Ok(())).unwrap().0, "");
    assert_eq!(decode(b"\xef\xbb\xbf", "", true, 0, &|| Ok(())).unwrap().0, "");
    assert!(decode(b"valid", "", true, 99, &|| Err("navigation cancelled".into()))
        .unwrap_err().contains("navigation cancelled"));
    let calls = std::cell::Cell::new(0usize);
    let bytes = vec![b'a'; 32 * 1024];
    let stopped = decode(&bytes, "", true, bytes.len(), &|| {
        let count = calls.get() + 1;
        calls.set(count);
        if count >= 3 { Err("navigation cancelled during decode".into()) } else { Ok(()) }
    });
    assert!(stopped.unwrap_err().contains("navigation cancelled"));
    assert_eq!(decoded(b"fresh", "", true), "fresh");
}
