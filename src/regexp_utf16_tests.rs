use super::*;

#[test]
fn unicode_regexp_subset_preserves_syntax_and_unsupported_boundaries() {
    for pattern in [r"[\d-q]", r"[q-\d]", "[z-a]", r"\q", "]"] {
        assert!(matches!(Regex::compile(pattern, "u", &mut 100_000), Err(Error::Syntax(_))), "{pattern}");
    }
    for pattern in [r"\u{d800}", r"\ud800"] {
        let re = Regex::compile(pattern, "u", &mut 100_000).unwrap();
        assert_eq!(re.find(&[0xd800], 0, false, &mut 100_000).unwrap().unwrap().captures[0], Some((0,1)));
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
    let unicode = Regex::compile_units(&[0xd842],"u",&mut 100_000).unwrap();
    assert_eq!(unicode.source_units.as_deref(),Some([0xd842].as_slice()));
    assert_eq!(unicode.find(&[0xd842],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,1)));
    for flags in ["v","d"] {
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
    for (pattern, flags) in [(r"\p{Letter}","u"),(".","iu"),(".","v"),(".","d")] {
        assert!(matches!(Regex::compile(pattern,flags,&mut 100_000),Err(Error::Unsupported(_))));
    }
}

#[test]
fn unicode_surrogate_pattern_literals_match_only_actual_code_points() {
    for unit in [0xd800u16,0xdbff,0xdc00,0xdfff] {
        for pattern in [vec![unit], format!(r"\u{unit:04x}").encode_utf16().collect(), format!(r"\u{{{unit:x}}}").encode_utf16().collect()] {
            let re=Regex::compile_units(&pattern,"u",&mut 100_000).unwrap();
            let found=re.find(&[unit],0,false,&mut 100_000).unwrap().unwrap();
            assert_eq!(found.captures[0],Some((0,1)));
            assert!(re.find(&[0xd800,0xdc00],0,false,&mut 100_000).unwrap().is_none());
        }
    }
    for pattern in [r"\uD83D+",r"[\uD83D]+",r"[\uD800-\uDBFF]+"] {
        let re=Regex::compile(pattern,"u",&mut 100_000).unwrap();
        assert!(re.find(&[0xd83d,0xde00],0,false,&mut 100_000).unwrap().is_none(),"{pattern}");
        let found=re.find(&[0xd83d,0xd83d,0xde00],0,false,&mut 100_000).unwrap().unwrap();
        assert_eq!(found.captures[0],Some((0,1)),"{pattern}");
    }
}

#[test]
fn unicode_surrogate_pattern_escape_pairs_quantify_whole_characters() {
    let pair=[0xd83d,0xde00];let two=[0xd83d,0xde00,0xd83d,0xde00];
    for pattern in [r"^(\uD83D\uDE00+)$",r"^([\uD83D\uDE00]+)$",r"^([\uD83D\uDE00-\uD83D\uDE03]+)$"] {
        let re=Regex::compile(pattern,"u",&mut 100_000).unwrap();
        assert_eq!(re.find(&two,0,false,&mut 100_000).unwrap().unwrap().captures,vec![Some((0,4)),Some((0,4))]);
        assert!(re.find(&[0xd83d],0,false,&mut 100_000).unwrap().is_none());
    }
    for pattern in [r"^\u{d83d}\u{de00}$",r"^\uD83D\u{de00}$",r"^\u{d83d}\uDE00$"] {
        assert!(Regex::compile(pattern,"u",&mut 100_000).unwrap().find(&pair,0,false,&mut 100_000).unwrap().is_none(),"{pattern}");
    }
    let re=Regex::compile(r"\uD800\uD83D\uDE00","u",&mut 100_000).unwrap();
    assert!(re.find(&[0xd800,0xd83d,0xde00],0,false,&mut 100_000).unwrap().is_some());
}

#[test]
fn unicode_surrogate_pattern_syntax_work_and_classic_boundaries_recover() {
    for pattern in [r"\uD800\u12xz",r"\uD800\u{}",r"[\uDFFF-\uD800]",r"[\uD83D\uDE03-\uD83D\uDE00]"] {
        assert!(matches!(Regex::compile(pattern,"u",&mut 100_000),Err(Error::Syntax(_))),"{pattern}");
    }
    let pattern=r"[\uD83D\uDE00-\uD83D\uDE03]+";
    assert!(matches!(Regex::compile(pattern,"u",&mut 0),Err(Error::Limit(_))));
    let re=Regex::compile(pattern,"u",&mut 100_000).unwrap();
    assert!(matches!(re.find(&[0xd83d,0xde00],0,false,&mut 0),Err(Error::Limit(_))));
    assert_eq!(re.find(&[0xd83d,0xde00],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,2)));
    let classic=Regex::compile(r"\uD83D+","",&mut 100_000).unwrap();
    assert_eq!(classic.find(&[0xd83d,0xde00],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,1)));
}

#[test]
fn unicode_group_reference_assertions_keep_code_point_direction_and_atomic_captures() {
    let emoji:Vec<_>="😀".encode_utf16().collect();
    let re=Regex::compile(r"(?=(.))\1","u",&mut 100_000).unwrap();
    assert_eq!(re.find(&emoji,0,false,&mut 100_000).unwrap().unwrap().captures,vec![Some((0,2)),Some((0,2))]);
    let input:Vec<_>="😀A".encode_utf16().collect();
    let re=Regex::compile(r"(?<=(.))A","u",&mut 100_000).unwrap();
    assert_eq!(re.find(&input,0,false,&mut 100_000).unwrap().unwrap().captures,vec![Some((2,3)),Some((0,2))]);
    assert!(Regex::compile(r"(?<!😀)A","u",&mut 100_000).unwrap().find(&input,0,false,&mut 100_000).unwrap().is_none());
    let re=Regex::compile(r"(?!😀).","u",&mut 100_000).unwrap();
    assert_eq!(re.find(&input,0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((2,3)));
    assert!(Regex::compile(r"(?=(.+))\1A","u",&mut 100_000).unwrap().find(&input,0,false,&mut 100_000).unwrap().is_none());
    let input:Vec<_>="😀AB🚀!".encode_utf16().collect();
    let re=Regex::compile(r"(?<=([😀A]+)([🚀B]+))!","u",&mut 100_000).unwrap();
    assert_eq!(re.find(&input,0,false,&mut 100_000).unwrap().unwrap().captures,vec![Some((6,7)),Some((0,3)),Some((3,6))]);
}

#[test]
fn unicode_group_reference_numbered_named_forward_and_unmatched_are_exact() {
    for pattern in [r"^(.+?)\1$",r"^(?<x>.)\k<x>$",r"^(?:(?<x>😀)|(?<x>A))\k<x>$"] {
        let re=Regex::compile(pattern,"u",&mut 100_000).unwrap();
        let same:Vec<_>="😀😀".encode_utf16().collect();let other:Vec<_>="😀🚀".encode_utf16().collect();
        assert_eq!(re.find(&same,0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,4)));
        assert!(re.find(&other,0,false,&mut 100_000).unwrap().is_none());
    }
    let re=Regex::compile(r"^\1(.)$","u",&mut 100_000).unwrap();
    assert_eq!(re.find(&[0xd83d,0xde00],0,false,&mut 100_000).unwrap().unwrap().captures[1],Some((0,2)));
    let re=Regex::compile(r"^(a|(b))\2c$","u",&mut 100_000).unwrap();
    let found=re.find(&[97,99],0,false,&mut 100_000).unwrap().unwrap();assert_eq!(found.captures[2],None);
    assert!(Regex::compile(r"^(a\1)$","u",&mut 100_000).unwrap().find(&[97],0,false,&mut 100_000).unwrap().is_some());
}

#[test]
fn unicode_group_reference_lone_surrogates_cannot_consume_half_an_input_pair() {
    for pattern in [r"(\ud800)\1",r"(?<x>\ud800)\k<x>"] {
        let re=Regex::compile(pattern,"u",&mut 100_000).unwrap();
        assert!(re.find(&[0xd800,0xd800,0xdc00],0,false,&mut 100_000).unwrap().is_none(),"{pattern}");
        assert_eq!(re.find(&[0xd800,0xd800],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,2)));
    }
    let re=Regex::compile(r"(?<=\1(\udc00))A","u",&mut 100_000).unwrap();
    assert!(re.find(&[0xd800,0xdc00,0xdc00,65],0,false,&mut 100_000).unwrap().is_none());
    assert_eq!(re.find(&[0xdc00,0xdc00,65],0,false,&mut 100_000).unwrap().unwrap().captures,vec![Some((2,3)),Some((1,2))]);
    let classic=Regex::compile(r"(\ud800)\1","",&mut 100_000).unwrap();
    assert_eq!(classic.find(&[0xd800,0xd800,0xdc00],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,2)));
}

#[test]
fn unicode_group_reference_grammar_syntax_and_work_refusal_preserve_recovery() {
    for pattern in [r"(?=a)*",r"(?!a)+",r"(?<=a)?",r"\1",r"(a)\2",r"\01",r"[\1]",r"[\01]",r"(?<x>a)\k<y>"] {
        assert!(matches!(Regex::compile(pattern,"u",&mut 100_000),Err(Error::Syntax(_))),"{pattern}");
    }
    assert!(Regex::compile(r"(?=a)?a","",&mut 100_000).unwrap().find(&[97],0,false,&mut 100_000).unwrap().is_some());
    assert!(matches!(Regex::compile(r"\1","",&mut 100_000),Err(Error::Unsupported(_))));
    for (pattern,flags) in [("(?<é>a)","u"),(r"\p{Letter}","u"),(".","iu"),(".","v"),(".","d")] {
        assert!(matches!(Regex::compile(pattern,flags,&mut 100_000),Err(Error::Unsupported(_))));
    }
    let pattern=r"(?<=(.))\1";assert!(matches!(Regex::compile(pattern,"u",&mut 0),Err(Error::Limit(_))));
    let re=Regex::compile(pattern,"u",&mut 100_000).unwrap();
    assert!(matches!(re.find(&[0xd83d,0xde00,0xd83d,0xde00],0,false,&mut 0),Err(Error::Limit(_))));
    assert_eq!(re.find(&[0xd83d,0xde00,0xd83d,0xde00],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((2,4)));
}
