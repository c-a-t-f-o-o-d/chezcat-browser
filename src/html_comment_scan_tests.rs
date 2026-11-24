//! Original scanner boundary and cancellation checks for native comment data.
use super::{comments, RAW_TEXT_CHECK_BYTES};

#[test]
fn comment_scan_closing_delimiters_cross_chunks_without_losing_literal_data() {
    for closing in ["-->", "--!>"] {
        for overlap in 0..=closing.len() {
            let length = RAW_TEXT_CHECK_BYTES - overlap;
            let prefix = "雪🙂>literal<!";
            let data = format!("{prefix}{}", "x".repeat(length - prefix.len()));
            let source = format!("{data}{closing}tail");
            let (actual, consumed) = comments::token(&source, None).unwrap();
            assert_eq!(actual, data, "{closing:?}, overlap {overlap}");
            assert_eq!(consumed, data.len() + closing.len());
            assert_eq!(&source[consumed..], "tail");
            assert_eq!(actual.as_ptr(), source.as_ptr());
        }
    }
    let data = format!("{}雪🙂>literal", "x".repeat(RAW_TEXT_CHECK_BYTES - 2));
    let source = format!("{data}-->tail");
    let (actual, consumed) = comments::token(&source, None).unwrap();
    assert_eq!(actual, data);
    assert_eq!(&source[consumed..], "tail");
}

#[test]
fn comment_scan_eof_keeps_only_committed_data_and_bogus_comments_keep_punctuation() {
    for (source, expected) in [
        ("", ""), ("-", ""), ("--", ""), ("--!", ""),
        ("雪<", "雪<"), ("雪<!--", "雪<!"),
        ("雪--!-", "雪--!"), ("雪--!x", "雪--!x"),
        ("雪----", "雪--"),
    ] {
        assert_eq!(comments::token(source, None).unwrap(), (expected, source.len()), "{source:?}");
        assert_eq!(comments::bogus(source, None).unwrap(), (source, source.len()), "{source:?}");
    }
    for (source, expected, consumed) in [
        (">tail", "", 1), ("--!>tail", "--!", 4),
        ("雪'quoted>tail", "雪'quoted", "雪'quoted>".len()),
    ] {
        let (actual, used) = comments::bogus(source, None).unwrap();
        assert_eq!((actual, used), (expected, consumed));
        assert_eq!(&source[used..], "tail");
    }
}

#[test]
fn comment_scan_long_normal_and_bogus_data_can_cancel_and_retry() {
    let data = "雪".repeat(RAW_TEXT_CHECK_BYTES);
    let source = format!("{data}-->tail");
    for bogus in [false, true] {
        let calls = std::cell::Cell::new(0);
        let check = || {
            calls.set(calls.get() + 1);
            if calls.get() == 2 { Err("cancel comment scan".into()) } else { Ok(()) }
        };
        let result = if bogus { comments::bogus(&source, Some(&check)) }
            else { comments::token(&source, Some(&check)) };
        assert_eq!(result.unwrap_err(), "cancel comment scan");
        assert_eq!(calls.get(), 2);
        let (actual, consumed) = if bogus { comments::bogus(&source, None).unwrap() }
            else { comments::token(&source, None).unwrap() };
        let expected = if bogus { format!("{data}--") } else { data.clone() };
        assert_eq!(actual, expected);
        assert_eq!(&source[consumed..], "tail");
    }
}

#[test]
fn comment_scan_copy_refuses_after_sizing_then_recovers_exact_unicode_and_nulls() {
    let prefix = "x".repeat(RAW_TEXT_CHECK_BYTES - 2);
    let source = format!("{prefix}雪\0🙂{}&amp;<!--literal-->", "é\0".repeat(RAW_TEXT_CHECK_BYTES));
    let sizing_checks = source.len().div_ceil(RAW_TEXT_CHECK_BYTES);
    let calls = std::cell::Cell::new(0);
    let check = || {
        calls.set(calls.get() + 1);
        if calls.get() == sizing_checks + 2 { Err("cancel comment copy".into()) } else { Ok(()) }
    };
    assert_eq!(comments::copy_data(&source, Some(&check)).unwrap_err(), "cancel comment copy");
    assert_eq!(calls.get(), sizing_checks + 2);
    let expected = format!("{prefix}雪\u{fffd}🙂{}&amp;<!--literal-->", "é\u{fffd}".repeat(RAW_TEXT_CHECK_BYTES));
    assert_eq!(comments::copy_data(&source, None).unwrap(), expected);
    assert_eq!(comments::copy_data("", None).unwrap(), "");
}
