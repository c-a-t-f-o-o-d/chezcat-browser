use super::*;

#[test]
fn regexp_inline_modifiers_enable_disable_and_restore_locally() {
    for (pattern,flags,text,yes) in [
        (r"^(?i:a)b$","","Ab",true),(r"^(?i:a)b$","","AB",false),
        (r"^(?-i:a)b$","i","aB",true),(r"^(?-i:a)b$","i","AB",false),
        (r"^(?i:a(?-i:b)c)d$","","AbCd",true),(r"^(?i:a(?-i:b)c)d$","","ABCD",false),
        (r"^(?i-:a)$","","A",true),(r"^(?:a)$","","a",true),
        (r"(?m:^a$)","","x\na\ny",true),(r"(?-m:^a$)","m","x\na\ny",false),
        (r"^(?s:.)$","","\n",true),(r"^(?-s:.)$","s","\n",false),
        (r"^(?ims:^a.$)$","","A\n",true),(r"^(?i:a|b)c$","","Bc",true),
        (r"(?m:^a$)\n^b$","","a\nb",false),(r"^(?s:.)\n.$","","\n\n\n",false),
        (r"^(?i-ms:a.\n)(?ms:^b$)$","ms","Ax\nb",true),
        (r"^(?i:a(?-i:b)c|d)e$","","De",true),
    ] {
        let re=Regex::compile(pattern,flags,&mut 1_000_000).unwrap();
        assert_eq!(re.flags.text(),flags);
        assert_eq!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 1_000_000).unwrap().is_some(),yes,"{pattern} {flags} {text:?}");
    }
}

#[test]
fn regexp_inline_modifiers_unicode_classes_properties_strings_and_words() {
    for flags in ["u","v"] {
        for (pattern,text,yes) in [(r"^(?i:[A-Z])$","K",true),(r"^(?i:[^k])$","K",false),(r"^(?i:\b\w\b)$","ſ",true),(r"^(?i:𐐀)$","𐐨",true),(r"^(?i:É)(?-i:É)$","éÉ",true),(r"^(?i:É)(?-i:É)$","éé",false)] {
            let re=Regex::compile(pattern,flags,&mut 20_000_000).unwrap();assert_eq!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().is_some(),yes,"{pattern} {flags}");
        }
        let re=Regex::compile(r"^(?-i:[A-Z])$",&format!("i{flags}"),&mut 20_000_000).unwrap();assert!(re.find(&[0x212a],0,false,&mut 20_000_000).unwrap().is_none());
    }
    for (flags,yes) in [("u",true),("v",false)] {assert_eq!(Regex::compile(r"^(?i:\P{Lowercase_Letter})$",flags,&mut 20_000_000).unwrap().find(&[97],0,false,&mut 20_000_000).unwrap().is_some(),yes);}
    assert!(Regex::compile(r"^(?i:[\q{AB|cd}&&\q{ab}])$","v",&mut 20_000_000).unwrap().find(&[65,98],0,false,&mut 20_000_000).unwrap().is_some());
}

#[test]
fn regexp_inline_modifiers_backtracking_assertions_references_and_captures() {
    for (pattern,flags,text,yes) in [
        (r"^(?:(?i:a)c|aB)$","","AB",false),
        (r"^(?i:a+)a$","","Aa",true),(r"^(?i:a+)a$","","AA",false),
        (r"^(?i:a)*b$","","Aab",true),(r"^(?i:a)*b$","","AaB",false),
        (r"^(?=(?i:a))a$","","A",false),(r"^(?i:(?=a)a)b$","","Ab",true),
        (r"(?<=(?i:a))b","","Ab",true),(r"(?i:(?<=a)b)c","","ABc",true),
        (r"^(?i:(a))\1$","","Aa",false),(r"^(a)(?i:\1)$","","aA",true),
        (r"^(?i:(𐐀)\1)$","v","𐐨𐐀",true),(r"^(?i:(?!b)a)b$","","Ab",true),
    ] {assert_eq!(Regex::compile(pattern,flags,&mut 1_000_000).unwrap().find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 1_000_000).unwrap().is_some(),yes,"{pattern} {text}");}
    let re=Regex::compile(r"^(?i:(a)|(b))+$","",&mut 1_000_000).unwrap();
    assert_eq!(re.find(&[65,66],0,false,&mut 1_000_000).unwrap().unwrap().captures,vec![Some((0,2)),None,Some((1,2))]);
}

#[test]
fn regexp_inline_modifiers_grammar_work_refusal_and_fresh_recovery() {
    for flags in ["","u","v"] {
        for pattern in ["(?-:a)","(?ii:a)","(?i-i:a)","(?i-ii:a)","(?mm:a)","(?g:a)","(?u:a)","(?v:a)","(?d:a)","(?y:a)","(?I:a)","(?i)","(?i--m:a)","(?i a)","(?i:a"] {
            assert!(matches!(Regex::compile(pattern,flags,&mut 1_000_000),Err(Error::Syntax(_))),"{pattern}");
        }
    }
    assert!(matches!(Regex::compile(r"(?i:[\p{Lu}])","v",&mut 100),Err(Error::Limit(_))));
    let re=Regex::compile(r"^(?i:a)+$","",&mut 1_000_000).unwrap();assert!(matches!(re.find(&[65],0,false,&mut 0),Err(Error::Limit(_))));assert!(re.find(&[65],0,false,&mut 1_000_000).unwrap().is_some());
    assert!(matches!(Regex::compile("(?i:é)","",&mut 1_000_000),Err(Error::Unsupported(_))));
    assert!(Regex::compile("(?-i:é)","i",&mut 1_000_000).unwrap().find(&[0xe9],0,false,&mut 1_000_000).unwrap().is_some());
    let deep=format!("{}a{}","(?i:".repeat(MAX_DEPTH),")".repeat(MAX_DEPTH));
    assert!(matches!(Regex::compile(&deep,"",&mut 1_000_000),Err(Error::Limit(_))));
}

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
    for flags in ["d"] {
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
fn utf16_regexp_group_names_reject_isolated_surrogates_and_accept_unicode_names() {
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
            assert!(compile(&name).is_ok(), "{name:x?}");
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
    for (pattern, flags) in [(".","d")] {
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
    for (pattern,flags) in [(".","d")] {
        assert!(matches!(Regex::compile(pattern,flags,&mut 100_000),Err(Error::Unsupported(_))));
    }
    let pattern=r"(?<=(.))\1";assert!(matches!(Regex::compile(pattern,"u",&mut 0),Err(Error::Limit(_))));
    let re=Regex::compile(pattern,"u",&mut 100_000).unwrap();
    assert!(matches!(re.find(&[0xd83d,0xde00,0xd83d,0xde00],0,false,&mut 0),Err(Error::Limit(_))));
    assert_eq!(re.find(&[0xd83d,0xde00,0xd83d,0xde00],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((2,4)));
}

#[test]
fn unicode_set_algebra_nested_union_intersection_and_subtraction_are_exact() {
    for (pattern,kind) in [(r"^[[a-z]&&[^aeiou]]$",0),(r"^[[a-z]--[aeiou]--[x-z]]$",1),(r"^[[a-c][x-z]😀]$",2),(r"^[\d&&[^5]]$",3)] {
        let re=Regex::compile(pattern,"v",&mut 100_000).unwrap();
        for unit in (0u32..=127).chain([0xd800,0xdc00,0x1f600]) {
            let input=if unit>0xffff {vec![0xd83d,0xde00]}else{vec![unit as u16]};
            let vowel=matches!(unit,97|101|105|111|117);
            let expected=match kind {0=>(97..=122).contains(&unit)&&!vowel,1=>(97..=119).contains(&unit)&&!vowel,2=>matches!(unit,97..=99|120..=122|0x1f600),_=>matches!(unit,48..=52|54..=57)};
            assert_eq!(re.find(&input,0,false,&mut 100_000).unwrap().is_some(),expected,"{pattern} U+{unit:x}");
        }
    }
    for pattern in [r"^[[😀-😃]--[😁]]+$",r"^[[^\uD800-\uDFFF]&&[😀-😃]]+$"] {
        let re=Regex::compile(pattern,"v",&mut 100_000).unwrap();
        let input:Vec<_>="😀😃".encode_utf16().collect();assert_eq!(re.find(&input,0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,4)));
        assert!(re.find(&[0xd800],0,false,&mut 100_000).unwrap().is_none());
    }
}

#[test]
fn unicode_set_algebra_grammar_flags_and_classic_boundaries_are_preserved() {
    for pattern in ["[a&&]","[&&a]","[a&&b--c]","[ab&&c]","[a-z&&x]","[a&&b-c]","[a--]","[a-]","[-a]","[a|b]","[(]","[{]","[/]","[!!]","[a&&&b]",r"[\d-a]"] {
        assert!(matches!(Regex::compile(pattern,"v",&mut 100_000),Err(Error::Syntax(_))),"{pattern}");
    }
    for flags in ["uv","vu","vv"] {assert!(matches!(Regex::compile("a",flags,&mut 100_000),Err(Error::Syntax(_))),"{flags}");}
    assert!(Regex::compile("a","iv",&mut 100_000).unwrap().find(&[65],0,false,&mut 100_000).unwrap().is_some());
    let re=Regex::compile(r"^[\&\-\!\#\%\,\:\;\<\=\>\@\`\~\(\)\[\]\{\}\/\|]$","v",&mut 100_000).unwrap();
    for unit in b"&-!#%,:;<=>@`~()[]{}/|" {assert!(re.find(&[u16::from(*unit)],0,false,&mut 100_000).unwrap().is_some());}
    let strings=Regex::compile(r"^[\q{ab|cd}]$","v",&mut 100_000).unwrap();
    assert!(strings.find(&[97,98],0,false,&mut 100_000).unwrap().is_some());
    assert!(strings.find(&[97,99],0,false,&mut 100_000).unwrap().is_none());
    for pattern in [r"^[\p{Letter}]$",r"^[\P{ASCII}]$"] {
        let property=Regex::compile(pattern,"v",&mut 100_000).unwrap();
        assert!(property.find(&[0xe9],0,false,&mut 100_000).unwrap().is_some());
        assert!(property.find(&[0],0,false,&mut 100_000).unwrap().is_none());
    }
    assert!(Regex::compile(r"\p{Basic_Emoji}","v",&mut 20_000_000).unwrap().find(&[0xd83d,0xde00],0,false,&mut 20_000_000).unwrap().is_some());
    let classic=Regex::compile("^[a&&b]$","u",&mut 100_000).unwrap();assert!(classic.find(&[38],0,false,&mut 100_000).unwrap().is_some());
    let flags=Flags::parse("yvmsg").unwrap();assert_eq!(flags.text(),"gmsvy");
}

#[test]
fn unicode_set_algebra_capture_direction_work_and_fresh_recovery_are_preserved() {
    let re=Regex::compile(r"(?<=([[😀A]--[B]]+))!","v",&mut 100_000).unwrap();let input:Vec<_>="😀A!".encode_utf16().collect();
    assert_eq!(re.find(&input,0,false,&mut 100_000).unwrap().unwrap().captures,vec![Some((3,4)),Some((0,3))]);
    let pattern=format!("[{}]",["[a-z]";80].join("&&"));
    assert!(matches!(Regex::compile(&pattern,"v",&mut 1_000),Err(Error::Limit(_))));
    let re=Regex::compile(&pattern,"v",&mut 100_000).unwrap();assert!(re.retained_bytes>0);
    assert!(matches!(re.find(&[97],0,false,&mut 0),Err(Error::Limit(_))));
    assert!(re.find(&[97],0,false,&mut 100_000).unwrap().is_some());
    let nested=format!("{}a{}","[".repeat(MAX_DEPTH+2),"]".repeat(MAX_DEPTH+2));assert!(matches!(Regex::compile(&nested,"v",&mut 100_000),Err(Error::Limit(_))));
    assert!(Regex::compile(r"^[\uD800--a]$","v",&mut 100_000).unwrap().find(&[0xd800],0,false,&mut 100_000).unwrap().is_some());
}

#[test]
fn unicode_set_algebra_predefined_sets_agree_at_character_boundaries() {
    for pattern in [r"^[\d]$",r"^[\D]$",r"^[\w]$",r"^[\W]$",r"^[\s]$",r"^[\S]$",r"^[^\D]$",r"^[\d\S]$"] {
        let unicode=Regex::compile(pattern,"u",&mut 100_000).unwrap();let sets=Regex::compile(pattern,"v",&mut 100_000).unwrap();
        for unit in (0u32..=128).chain([0xa0,0x1680,0x1fff,0x2000,0x200a,0x200b,0x2027,0x2028,0x2029,0x202a,0x202f,0x2030,0x205f,0x2060,0x3000,0x3001,0xd800,0xdc00,0xfeff,0xff00,0xffff,0x10000,0x1f600,0x10ffff]) {
            let input=if unit<=0xffff{vec![unit as u16]}else{vec![0xd800+((unit-0x10000)>>10)as u16,0xdc00+((unit-0x10000)&1023)as u16]};
            assert_eq!(sets.find(&input,0,false,&mut 100_000).unwrap().is_some(),unicode.find(&input,0,false,&mut 100_000).unwrap().is_some(),"{pattern} U+{unit:x}");
        }
    }
    assert!(matches!(Regex::compile(r"[\q]","v",&mut 100_000),Err(Error::Syntax(_))));
    let re=Regex::compile(r"^[\u{0}-\u{10ffff}--[\u{1}-\u{10fffe}]]$","v",&mut 100_000).unwrap_err();assert!(matches!(re,Error::Syntax(_)));
    let re=Regex::compile(r"^[[\u{0}-\u{10ffff}]--[\u{1}-\u{10fffe}]]$","v",&mut 100_000).unwrap();
    for input in [vec![0],vec![0xdbff,0xdfff]]{assert!(re.find(&input,0,false,&mut 100_000).unwrap().is_some());}
    assert!(re.find(&[1],0,false,&mut 100_000).unwrap().is_none());
}

#[test]
fn unicode_string_set_order_backtracking_and_direction_are_exact() {
    for (pattern,text,whole,capture) in [(r"([\q{a|ab|abc}])","abc",(0,3),(0,3)),(r"^([\q{a|ab|abc}])bc$","abc",(0,3),(0,1)),(r"(?<=([\q{a|ba|cba}]))!","cba!",(3,4),(0,3)),(r"([\q{😀|😀A}])","😀A",(0,3),(0,3))] {
        let re=Regex::compile(pattern,"v",&mut 100_000).unwrap();let input:Vec<_>=text.encode_utf16().collect();
        assert_eq!(re.find(&input,0,false,&mut 100_000).unwrap().unwrap().captures,vec![Some(whole),Some(capture)],"{pattern}");
    }
    let re=Regex::compile(r"^[\q{a|ab|}]*$","v",&mut 100_000).unwrap();assert!(re.find(&[97,98,97],0,false,&mut 100_000).unwrap().is_some());
    assert!(Regex::compile(r"^[\q{}]$","v",&mut 100_000).unwrap().find(&[],0,false,&mut 100_000).unwrap().is_some());
}

#[test]
fn unicode_string_set_algebra_keeps_characters_strings_and_empty_distinct() {
    for (pattern,accepted,rejected) in [(r"^[[\q{ab|a|😀|}]&&[\q{ab|😀|z|}]]$",vec!["ab","😀",""],vec!["a","z"]),(r"^[[\q{ab|a|}]--[\q{ab}]]$",vec!["a",""],vec!["ab","b"]),(r"^[[\q{ab|a}]a-c]$",vec!["ab","a","b","c"],vec!["abc",""]),(r"^[[\q{a|ab}]&&[a-z]]$",vec!["a"],vec!["ab","b"]),(r"^[^\q{a|b}]$",vec!["c","😀"],vec!["a","b",""]),(r"^[^[\q{ab}]&&[a]]$",vec!["a","b","😀"],vec!["ab",""])] {
        let re=Regex::compile(pattern,"v",&mut 100_000).unwrap();
        for text in accepted {assert!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 100_000).unwrap().is_some(),"{pattern} {text}");}
        for text in rejected {assert!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 100_000).unwrap().is_none(),"{pattern} {text}");}
    }
}

#[test]
fn unicode_string_set_escapes_preserve_code_points_and_lone_units() {
    for pattern in [r"^[\q{\uD83D\uDE00A}]$",r"^[\q{😀A}]$"] {
        let re=Regex::compile(pattern,"v",&mut 100_000).unwrap();assert!(re.find(&[0xd83d,0xde00,65],0,false,&mut 100_000).unwrap().is_some());
    }
    let re=Regex::compile(r"^[\q{\u{d83d}\u{de00}A}]$","v",&mut 100_000).unwrap();assert!(re.find(&[0xd83d,0xde00,65],0,false,&mut 100_000).unwrap().is_none());
    let re=Regex::compile(r"^[\q{\ud800A|a\|b|\-\!}]$","v",&mut 100_000).unwrap();
    for input in [vec![0xd800,65],vec![97,124,98],vec![45,33]] {assert!(re.find(&input,0,false,&mut 100_000).unwrap().is_some());}
    assert!(re.find(&[0xd800,0xdc00,65],0,false,&mut 100_000).unwrap().is_none());
}

#[test]
fn unicode_string_set_syntax_work_refusal_and_fresh_recovery_are_preserved() {
    for pattern in [r"[^\q{ab}]",r"[^\q{}]",r"[^\q{ab}--\q{ab}]",r"[^\q{ab}&&\q{ab}]",r"[\q{a}-z]",r"[a-\q{z}]",r"[\q{a-b}]",r"[\q{\d}]",r"[\q{a]",r"[\q]",r"[\q{!!}]"] {
        assert!(matches!(Regex::compile(pattern,"v",&mut 100_000),Err(Error::Syntax(_))),"{pattern}");
    }
    for (pattern,flags) in [(".","d")] {assert!(matches!(Regex::compile(pattern,flags,&mut 100_000),Err(Error::Unsupported(_))));}
    assert!(matches!(Regex::compile(r"[\q{ab}]","u",&mut 100_000),Err(Error::Syntax(_))));
    let pattern=format!(r"[\q{{{}}}]",["abcdef";64].join("|"));assert!(matches!(Regex::compile(&pattern,"v",&mut 1_000),Err(Error::Limit(_))));
    let re=Regex::compile(&pattern,"v",&mut 100_000).unwrap();assert!(matches!(re.find(&[97,98,99,100,101,102],0,false,&mut 0),Err(Error::Limit(_))));
    assert_eq!(re.find(&[97,98,99,100,101,102],0,false,&mut 100_000).unwrap().unwrap().captures[0],Some((0,6)));
}

#[test]
fn unicode_property_category_aliases_cover_supplementary_and_non_scalar_units() {
    for flags in ["u","v"] {
        for pattern in [r"^\p{L}+$",r"^\p{General_Category=Letter}+$",r"^\p{gc=L}+$"] {
            let re=Regex::compile(pattern,flags,&mut 1_000_000).unwrap();
            assert!(re.find(&"Aéあ𐐀".encode_utf16().collect::<Vec<_>>(),0,false,&mut 1_000_000).unwrap().is_some());
            for input in [vec![49],vec![0xd83d,0xde00],vec![0xd800]] {assert!(re.find(&input,0,false,&mut 1_000_000).unwrap().is_none());}
        }
        for (property,unit) in [("Lu",65),("digit",0x660),("LC",0x1c5),("Cs",0xd800),("Cn",0x378),("Co",0xe000),("Zs",0xa0)] {
            let re=Regex::compile(&format!(r"^\p{{{property}}}$"),flags,&mut 1_000_000).unwrap();assert!(re.find(&[unit],0,false,&mut 1_000_000).unwrap().is_some(),"{property}");
        }
    }
}

#[test]
fn unicode_property_script_extensions_and_binary_membership_are_exact() {
    for flags in ["u","v"] {
        for (property,yes,no) in [("Script=Hiragana",0x3042,0x30fc),("scx=Hira",0x30fc,65),("Script_Extensions=Katakana",0x30fc,0x3042),("Script=Common",0x30fc,0x3042),("scx=Common",33,0x30fc),("sc=Unknown",0x378,65),("ASCII",65,0xe9),("AHex",70,71),("Alpha",0xe9,49),("White_Space",0x85,0xfeff),("Assigned",0xd800,0x378),("Bidi_M",40,65),("Join_C",0x200c,65)] {
            let re=Regex::compile(&format!(r"^\p{{{property}}}$"),flags,&mut 1_000_000).unwrap();assert!(re.find(&[yes],0,false,&mut 1_000_000).unwrap().is_some(),"{property}");assert!(re.find(&[no],0,false,&mut 1_000_000).unwrap().is_none(),"{property}");
        }
        let re=Regex::compile(r"^\p{Emoji}$",flags,&mut 1_000_000).unwrap();assert!(re.find(&[0xd83d,0xde00],0,false,&mut 1_000_000).unwrap().is_some());
        assert!(Regex::compile(r"^\p{Any}$",flags,&mut 1_000_000).unwrap().find(&[0xd800],0,false,&mut 1_000_000).unwrap().is_some());
    }
}

#[test]
fn unicode_property_complements_algebra_captures_and_directions_are_preserved() {
    for (pattern,text,expected) in [(r"^([\p{L}&&[^\p{ASCII}]])\1$","éé",true),(r"^[[\p{L}]--[\p{Lu}]]+$","éあ",true),(r"^[[\p{L}]--[\p{Lu}]]+$","Aé",false),(r"^[\p{ASCII}\q{😀A}]$","😀A",true),(r"^\P{ASCII}+$","😀é",true),(r"^\P{Cs}+$","😀",true),(r"^\P{Cs}+$","A",true)] {
        let re=Regex::compile(pattern,"v",&mut 1_000_000).unwrap();assert_eq!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 1_000_000).unwrap().is_some(),expected,"{pattern}");
    }
    let re=Regex::compile(r"(?<=(\p{L}+))!","u",&mut 1_000_000).unwrap();assert_eq!(re.find(&"𐐀é!".encode_utf16().collect::<Vec<_>>(),0,false,&mut 1_000_000).unwrap().unwrap().captures,vec![Some((3,4)),Some((0,3))]);
    assert!(Regex::compile(r"^\P{Cs}$","u",&mut 1_000_000).unwrap().find(&[0xd800],0,false,&mut 1_000_000).unwrap().is_none());
}

#[test]
fn unicode_property_strict_names_syntax_refusal_and_fresh_work_are_exact() {
    for flags in ["u","v"] {
        for pattern in [r"\p",r"\p{}",r"\p{letter}",r"\p{Script=latin}",r"\p{sc =Latn}",r"\p{Age=1.1}",r"\p{Alphabetic=Yes}",r"\p{Unknown_Property}",r"\p{gc=ASCII}",r"\p{Script=}",r"[\p{L}-z]"] {assert!(matches!(Regex::compile(pattern,flags,&mut 1_000_000),Err(Error::Syntax(_))),"{pattern}");}
    }
    assert!(matches!(Regex::compile(r"\p{RGI_Emoji}","u",&mut 1_000_000),Err(Error::Syntax(_))));
    assert!(matches!(Regex::compile(r"\P{RGI_Emoji}","v",&mut 1_000_000),Err(Error::Syntax(_))));
    assert!(Regex::compile(r"\p{Basic_Emoji}","v",&mut 20_000_000).unwrap().find(&[0xd83d,0xde00],0,false,&mut 20_000_000).unwrap().is_some());
    let pattern=r"[\p{L}&&\p{ASCII}]";assert!(matches!(Regex::compile(pattern,"v",&mut 100),Err(Error::Limit(_))));
    let re=Regex::compile(pattern,"v",&mut 1_000_000).unwrap();assert!(matches!(re.find(&[65],0,false,&mut 0),Err(Error::Limit(_))));assert!(re.find(&[65],0,false,&mut 1_000_000).unwrap().is_some());
}
#[test]
fn unicode_emoji_properties_seven_sets_match_whole_sequences() {
    for (property,text) in [("Basic_Emoji","😀"),("Basic_Emoji","❤\u{fe0f}"),("Emoji_Keycap_Sequence","1\u{fe0f}\u{20e3}"),("RGI_Emoji_Modifier_Sequence","👍🏽"),("RGI_Emoji_Flag_Sequence","🇺🇸"),("RGI_Emoji_Tag_Sequence","🏴\u{e0067}\u{e0062}\u{e0065}\u{e006e}\u{e0067}\u{e007f}"),("RGI_Emoji_ZWJ_Sequence","👩‍💻"),("RGI_Emoji","👩‍💻")] {
        for pattern in [format!(r"^(\p{{{property}}})$"),format!(r"^([\p{{{property}}}])$")] {
            let re=Regex::compile(&pattern,"v",&mut 20_000_000).unwrap();let input:Vec<_>=text.encode_utf16().collect();
            assert_eq!(re.find(&input,0,false,&mut 20_000_000).unwrap().unwrap().captures,vec![Some((0,input.len())),Some((0,input.len()))],"{property}");
            assert!(re.find(&[65],0,false,&mut 20_000_000).unwrap().is_none());
        }
    }
}

#[test]
fn unicode_emoji_properties_algebra_and_syntactic_strings_are_exact() {
    for (pattern,text,yes) in [(r"^[\p{RGI_Emoji}&&\q{😀|👩‍💻|A}]$","👩‍💻",true),(r"^[\p{RGI_Emoji}&&\q{😀|👩‍💻|A}]$","A",false),(r"^[\p{Basic_Emoji}--\q{😀}]$","😀",false),(r"^[\p{Basic_Emoji}--\q{😀}]$","❤\u{fe0f}",true),(r"^[\p{Basic_Emoji}&&\p{Emoji}]$","😀",true),(r"^[\p{Basic_Emoji}&&\p{Emoji}]$","❤\u{fe0f}",false),(r"^[^\p{Basic_Emoji}&&\p{ASCII}]$","A",true),(r"^\p{Basic_Emoji}$","❤",false)] {
        let re=Regex::compile(pattern,"v",&mut 20_000_000).unwrap();assert_eq!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().is_some(),yes,"{pattern}");
    }
    for pattern in [r"[^\p{Basic_Emoji}]",r"[^\p{Basic_Emoji}--\p{Basic_Emoji}]",r"[^\p{Basic_Emoji}&&\q{AB}]",r"[\p{Basic_Emoji}-z]",r"[a-\p{Basic_Emoji}]"] {assert!(matches!(Regex::compile(pattern,"v",&mut 20_000_000),Err(Error::Syntax(_))),"{pattern}");}
}

#[test]
fn unicode_emoji_properties_longest_backtracking_and_reverse_captures_work() {
    for (pattern,text,captures) in [(r"(\p{RGI_Emoji})","👩‍💻",vec![Some((0,5)),Some((0,5))]),(r"^(\p{RGI_Emoji})‍💻$","👩‍💻",vec![Some((0,5)),Some((0,2))]),(r"(?<=(\p{RGI_Emoji}))!","👩‍💻!",vec![Some((5,6)),Some((0,5))]),(r"^(\p{Emoji_Keycap_Sequence})\1$","1\u{fe0f}\u{20e3}1\u{fe0f}\u{20e3}",vec![Some((0,6)),Some((0,3))])] {
        let re=Regex::compile(pattern,"v",&mut 20_000_000).unwrap();assert_eq!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().unwrap().captures,captures,"{pattern}");
    }
}

#[test]
fn unicode_emoji_properties_work_syntax_and_fresh_recovery_are_real() {
    for name in ["Basic_Emoji","Emoji_Keycap_Sequence","RGI_Emoji_Modifier_Sequence","RGI_Emoji_Flag_Sequence","RGI_Emoji_Tag_Sequence","RGI_Emoji_ZWJ_Sequence","RGI_Emoji"] {
        assert!(matches!(Regex::compile(&format!(r"\p{{{name}}}"),"u",&mut 20_000_000),Err(Error::Syntax(_))));
        assert!(matches!(Regex::compile(&format!(r"\P{{{name}}}"),"v",&mut 20_000_000),Err(Error::Syntax(_))));
    }
    for name in ["basic_emoji","RGI_emoji","RGIEmoji","RGI_Emoji=Yes"] {assert!(matches!(Regex::compile(&format!(r"\p{{{name}}}"),"v",&mut 20_000_000),Err(Error::Syntax(_))));}
    assert!(matches!(Regex::compile(r"\p{RGI_Emoji}","v",&mut 100),Err(Error::Limit(_))));
    let re=Regex::compile(r"\p{RGI_Emoji}","v",&mut 20_000_000).unwrap();
    assert!(matches!(re.find(&[0xd83d,0xde00],0,false,&mut 0),Err(Error::Limit(_))));
    assert_eq!(re.find(&[0xd83d,0xde00],0,false,&mut 20_000_000).unwrap().unwrap().captures[0],Some((0,2)));
}
#[test]
fn unicode_capture_names_decode_identifiers_and_reference_identity() {
    for flags in ["","u","v"] {
        for (spelling,name) in [("猫","猫"),("é","é"),("e\u{301}","e\u{301}"),("a\u{200c}","a\u{200c}"),("a\u{200d}","a\u{200d}"),("𐐨","𐐨"),(r"\uD801\uDC28","𐐨"),(r"\u{10428}","𐐨"),(r"\u00e9","é"),("__proto__","__proto__")] {
            let re=Regex::compile(&format!(r"^(?<{spelling}>a)\k<{name}>$"),flags,&mut 100_000).unwrap();
            assert_eq!(re.find(&[97,97],0,false,&mut 100_000).unwrap().unwrap().named_captures,vec![(name.into(),Some((0,1)))]);
        }
    }
}

#[test]
fn unicode_capture_names_duplicates_forward_reverse_and_no_normalization() {
    for flags in ["","u","v"] {
        let re=Regex::compile(r"^(?:(?<猫>a)|(?<\u732b>b))\k<猫>$",flags,&mut 100_000).unwrap();
        for input in [[97,97],[98,98]] {assert!(re.find(&input,0,false,&mut 100_000).unwrap().is_some());}
        let re=Regex::compile(r"^\k<é>(?<\u00e9>a)$",flags,&mut 100_000).unwrap();assert!(re.find(&[97],0,false,&mut 100_000).unwrap().is_some());
        let re=Regex::compile(r"(?<=(?<猫>a))\k<\u732b>",flags,&mut 100_000).unwrap();assert_eq!(re.find(&[97,97],0,false,&mut 100_000).unwrap().unwrap().named_captures,vec![("猫".into(),Some((0,1)))]);
        assert!(matches!(Regex::compile(r"(?<é>a)(?<\u00e9>b)",flags,&mut 100_000),Err(Error::Syntax(_))));
        assert!(matches!(Regex::compile("(?<é>a)\\k<e\u{301}>",flags,&mut 100_000),Err(Error::Syntax(_))));
        assert!(Regex::compile("(?<é>a)(?<e\u{301}>b)",flags,&mut 100_000).is_ok());
    }
}

#[test]
fn unicode_capture_names_invalid_scalars_grammar_work_and_fresh_recovery() {
    for flags in ["","u","v"] {
        for name in ["😀","1猫","\u{301}x","\u{200c}x",r"\u{d800}",r"\uD801\u{DC28}",r"\u{110000}",r"\x61",r"\u{}",r"\uD801",r"\uDC28",r"\u{3e}"] {
            assert!(matches!(Regex::compile(&format!("(?<{name}>a)"),flags,&mut 100_000),Err(Error::Syntax(_))),"{name}");
        }
    }
    assert!(matches!(Regex::compile("(?<猫>a)","v",&mut 30),Err(Error::Limit(_))));
    let re=Regex::compile("(?<猫>a)","v",&mut 100_000).unwrap();assert!(matches!(re.find(&[97],0,false,&mut 0),Err(Error::Limit(_))));assert!(re.find(&[97],0,false,&mut 100_000).unwrap().is_some());
}
#[test]
fn unicode_case_folding_literals_ranges_and_simple_mapping_are_exact() {
    for flags in ["iu","iv"] {
        for (pattern,text,yes) in [("^É$","é",true),("^Σ$","ς",true),("^σ$","Σ",true),("^K$","K",true),("^s$","ſ",true),("^ẞ$","ß",true),("^𐐀$","𐐨",true),("^ß$","ss",false),("^İ$","i",false),("^I$","ı",false),("^[A-Z]$","K",true),("^[A-Z]$","ſ",true),("^[A-z]$","_",true),("^[A-z]$","[",true),("^[Ā-č]$","Ă",true),("^[^k]$","K",false)] {
            let re=Regex::compile(pattern,flags,&mut 20_000_000).unwrap();assert_eq!(re.find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().is_some(),yes,"{pattern} {flags} {text}");
        }
    }
}

#[test]
fn unicode_case_folding_property_complements_and_v_algebra_keep_order() {
    for (pattern,flags,text,yes) in [(r"^\P{Lowercase_Letter}$","iu","a",true),(r"^\P{Lowercase_Letter}$","iv","a",false),(r"^\P{Lowercase_Letter}$","iv","A",false),(r"^\P{ASCII}$","iu","k",true),(r"^\P{ASCII}$","iv","k",false),(r"^[\p{Lu}&&[a-z]]$","iv","K",true),(r"^[[A-Z]--k]$","iv","K",false),(r"^[[A-Z]--k]$","iv","ſ",true),(r"^[^[a]]$","iv","A",false),(r"^[[^a]&&[A-Z]]$","iv","a",false),(r"^[[^a]&&[A-Z]]$","iv","B",true),(r"^[\q{ab|AB|a}]$","iv","Ab",true),(r"^[\q{AB|cd}]&&\q{ab}]$","iv","ab",false),(r"^[\q{AB|cd}&&\q{ab}]$","iv","aB",true)] {
        let result=Regex::compile(pattern,flags,&mut 20_000_000);if pattern==r"^[\q{AB|cd}]&&\q{ab}]$" {assert!(matches!(result,Err(Error::Syntax(_))));continue;}
        assert_eq!(result.unwrap().find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().is_some(),yes,"{pattern} {flags}");
    }
}

#[test]
fn unicode_case_folding_word_boundaries_references_and_captures_are_exact() {
    for flags in ["iu","iv"] {
        for text in ["K","ſ","K","s"] {
            assert!(Regex::compile(r"^\b\w\b$",flags,&mut 20_000_000).unwrap().find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().is_some());
            assert!(Regex::compile(r"^\W$",flags,&mut 20_000_000).unwrap().find(&text.encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().is_none());
        }
        let re=Regex::compile(r"^(?<x>𐐀)\k<x>$",flags,&mut 20_000_000).unwrap();assert_eq!(re.find(&"𐐨𐐀".encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().unwrap().captures,vec![Some((0,4)),Some((0,2))]);
        assert!(Regex::compile(r"(?<=(𐐀))\1",flags,&mut 20_000_000).unwrap().find(&"𐐀𐐨".encode_utf16().collect::<Vec<_>>(),0,false,&mut 20_000_000).unwrap().is_some());
        let re=Regex::compile(r"^([\q{a|ab|AB}])b$","iv",&mut 20_000_000).unwrap();assert_eq!(re.find(&[65,66],0,false,&mut 20_000_000).unwrap().unwrap().captures[1],Some((0,1)));
    }
}

#[test]
fn unicode_case_folding_compile_match_refusal_and_fresh_recovery_are_real() {
    assert!(matches!(Regex::compile(r"[\p{Lu}]","iv",&mut 100),Err(Error::Limit(_))));
    let re=Regex::compile(r"^\p{Lu}+$","iv",&mut 20_000_000).unwrap();assert!(matches!(re.find(&[0x212a],0,false,&mut 0),Err(Error::Limit(_))));assert!(re.find(&[0x212a],0,false,&mut 20_000_000).unwrap().is_some());
    assert!(matches!(Regex::compile("é","i",&mut 20_000_000),Err(Error::Unsupported(_))));
    assert!(Regex::compile(r"^\uD800$","iu",&mut 20_000_000).unwrap().find(&[0xd800],0,false,&mut 20_000_000).unwrap().is_some());
}
