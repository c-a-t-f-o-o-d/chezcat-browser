use super::*;

#[test]
fn unicode_regexp_subset_preserves_syntax_and_unsupported_boundaries() {
    for pattern in [r"[\d-q]", r"[q-\d]", "[z-a]", r"\q", "]"] {
        assert!(matches!(Regex::compile(pattern, "u", &mut 100_000), Err(Error::Syntax(_))), "{pattern}");
    }
    for pattern in [r"\u{d800}", r"\ud800"] {
        assert!(matches!(Regex::compile(pattern, "u", &mut 100_000), Err(Error::Unsupported(_))), "{pattern}");
    }
    let range = Regex::compile(r"[a-\uE001]+", "u", &mut 100_000).unwrap();
    let input = [0xd800, 0xd800, 0xdc00, 0xd800];
    let found = range.find(&input, 0, false, &mut 100_000).unwrap().unwrap();
    assert_eq!((found.start(), found.end()), (0, 1));
}

#[test]
fn utf16_regexp_raw_compile_matches_independent_class_unit_ranges() {
    let pattern = [91,0xd842,0xd845,45,0xd87a,0xd87d,45,0xdbff,93];
    let regex = Regex::compile_units(&pattern, "g", &mut 100_000).unwrap();
    assert_eq!(regex.source_units.as_deref(), Some(pattern.as_slice()));
    for unit in [0xd842,0xd845,0xd87a,0xd87d,0xdbff] {
        let found = regex.find(&[unit,0xdc00], 0, false, &mut 100_000).unwrap().unwrap();
        assert_eq!(found.captures[0], Some((0,1)));
    }
    for unit in [0xd800,0xd841,0xd843,0xd844,0xd87b,0xd87c,0xdc00] {
        assert!(regex.find(&[unit,0xdc00], 0, false, &mut 100_000).unwrap().is_none(), "{unit:x}");
    }
}

#[test]
fn utf16_regexp_raw_and_scalar_compilation_keep_sources_costs_and_invalid_grammar() {
    for pattern in ["a/b\n", "(a)[b-d]+", "😀", "", r"\uD845"] {
        let units: Vec<_> = pattern.encode_utf16().collect();
        let mut scalar_budget = 100_000;
        let mut unit_budget = 100_000;
        let scalar = Regex::compile(pattern, "g", &mut scalar_budget).unwrap();
        let raw = Regex::compile_units(&units, "g", &mut unit_budget).unwrap();
        assert_eq!(raw.source, scalar.source);
        assert!(raw.source_units.is_none());
        assert_eq!(raw.retained_bytes, scalar.retained_bytes);
        assert_eq!(unit_budget, scalar_budget);
    }
    for pattern in [vec![91,0xd87a,45,0xd845,93], "[\\u12xz]".encode_utf16().collect()] {
        assert!(matches!(Regex::compile_units(&pattern,"",&mut 100_000),Err(Error::Syntax(_))));
    }
    for flags in ["u","v","d"] {
        assert!(matches!(Regex::compile_units(&[0xd842],flags,&mut 100_000),Err(Error::Unsupported(_))));
    }
    let mut empty = 0;
    assert!(matches!(Regex::compile_units(&[0xd842],"",&mut empty),Err(Error::Limit(_))));
    assert_eq!(empty,0);
    let over = vec![0xd842; MAX_PATTERN / 3 + 1];
    assert!(matches!(
        Regex::compile_units(&over, "", &mut 100_000),
        Err(Error::Limit(PATTERN_LIMIT))
    ));
}

#[test]
fn utf16_regexp_group_names_reject_isolated_surrogates_but_keep_astral_names_unsupported() {
    let units = |s: &str| s.encode_utf16().collect::<Vec<_>>();
    // U+10428 is an astral letter. Raw pairs and consecutive four-digit
    // escapes can designate it; mixing those two forms cannot.
    let high = 0xd801;
    let low = 0xdc28;
    for forward_reference in [false, true] {
        let compile = |name: &[u16]| {
            let mut pattern = Vec::new();
            if forward_reference {
                pattern.extend(r"\k<".encode_utf16());
                pattern.extend_from_slice(name);
                pattern.push(b'>' as u16);
            }
            pattern.extend("(?<".encode_utf16());
            pattern.extend_from_slice(name);
            pattern.extend(">x)".encode_utf16());
            Regex::compile_units(&pattern, "", &mut 100_000)
        };
        let mut raw_then_escape = vec![high];
        raw_then_escape.extend(r"\uDC28".encode_utf16());
        let mut escape_then_raw = units(r"\uD801");
        escape_then_raw.push(low);
        for name in [
            vec![high], vec![low], units(r"\uD801"), units(r"\uDC28"),
            units(r"\u{D801}"), units(r"\u{DC28}"), raw_then_escape, escape_then_raw,
        ] {
            assert!(matches!(compile(&name), Err(Error::Syntax(_))), "{name:x?}");
        }
        for name in [vec![high, low], units(r"\uD801\uDC28"), units(r"\u{10428}"), units("é")] {
            assert!(matches!(compile(&name), Err(Error::Unsupported(_))), "{name:x?}");
        }
    }
}

#[test]
fn unicode_character_sets_dot_and_complements_consume_one_code_point() {
    let samples: &[(&[u16], bool, bool, bool, bool)] = &[
        (&[0x41], false, false, true, false),
        (&[0x35], false, true, true, false),
        (&[0x5f], false, false, true, false),
        (&[0x0a], true, false, false, true),
        (&[0x0d], true, false, false, true),
        (&[0x2028], true, false, false, true),
        (&[0x2029], true, false, false, true),
        (&[0xfeff], false, false, false, true),
        (&[0x85], false, false, false, false),
        (&[0x200b], false, false, false, false),
        (&[0xd83d, 0xde00], false, false, false, false),
        (&[0xd800], false, false, false, false),
        (&[0xdc00], false, false, false, false),
    ];
    for &(input, is_line, digit, word, whitespace) in samples {
        for (pattern, flags, expected) in [
            ("^.$", "u", !is_line), ("^.$", "su", true),
            (r"^\D$", "u", !digit), (r"^\W$", "u", !word),
            (r"^\S$", "u", !whitespace), (r"^[^\D]$", "u", digit),
            (r"^[^\W]$", "u", word), (r"^[^\S]$", "u", whitespace),
            ("^[^]$", "u", true), ("^[]$", "u", false),
            (r"^[\D5]$", "u", !digit || input == [0x35]),
            (r"^[^\u{1f600}]$", "u", input != [0xd83d, 0xde00]),
        ] {
            let re = Regex::compile(pattern, flags, &mut 100_000).unwrap();
            let found = re.find(input, 0, false, &mut 100_000).unwrap();
            assert_eq!(found.is_some(), expected, "{pattern}/{flags} {input:x?}");
            if let Some(found) = found { assert_eq!(found.captures[0], Some((0, input.len()))); }
        }
    }
}

#[test]
fn unicode_character_sets_repetition_backtracks_at_code_point_boundaries() {
    let input: Vec<_> = "😀A🚀A".encode_utf16().collect();
    for pattern in [r"^(.+)A$", r"^(.+?)A$", r"^([^!]+)A$", r"^(\D+)A$", r"^([\W\w]+)A$", r"^(.{2,4})A$"] {
        let re = Regex::compile(pattern, "u", &mut 100_000).unwrap();
        let found = re.find(&input, 0, false, &mut 100_000).unwrap().unwrap();
        assert_eq!(found.captures, vec![Some((0, 6)), Some((0, 5))], "{pattern}");
    }
    for pattern in ["^..$", r"^[^!]{2}$", r"^\D{2}$"] {
        let re = Regex::compile(pattern, "u", &mut 100_000).unwrap();
        assert!(re.find(&[0xd83d,0xde00,0x41],0,false,&mut 100_000).unwrap().is_some());
        assert!(re.find(&[0xd83d,0xde00],0,false,&mut 100_000).unwrap().is_none());
    }
}

#[test]
fn unicode_character_sets_work_refusal_preserves_compiled_recovery() {
    let re = Regex::compile(r"^([^!]+)A$", "u", &mut 100_000).unwrap();
    let input: Vec<_> = "😀🚀A".encode_utf16().collect();
    assert!(matches!(re.find(&input,0,false,&mut 0),Err(Error::Limit(_))));
    let mut minimum = None;
    for available in 1..500 {
        match re.find(&input,0,false,&mut available.clone()) {
            Ok(Some(found)) => { assert_eq!(found.captures[1],Some((0,4))); minimum=Some(available); break; }
            Err(Error::Limit(_)) => (),
            result => panic!("unexpected refusal/recovery: {result:?}"),
        }
    }
    assert!(minimum.is_some());
    let found = re.find(&input,0,false,&mut 100_000).unwrap().unwrap();
    assert_eq!(found.captures[0], Some((0,5)));
    for pattern in [r"[^\d-q]", r"[q-\D]", r"\q", "[z-a]"] {
        assert!(matches!(Regex::compile(pattern,"u",&mut 100_000),Err(Error::Syntax(_))));
    }
    for (pattern, flags) in [(r"\p{Letter}","u"),(".","iu"),(".","v"),(".","d"),(r"\ud800","u")] {
        assert!(matches!(Regex::compile(pattern,flags,&mut 100_000),Err(Error::Unsupported(_))));
    }
}
