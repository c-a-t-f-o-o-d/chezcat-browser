use super::*;

#[test]
fn unicode_regexp_subset_preserves_syntax_and_unsupported_boundaries() {
    for pattern in [r"[\d-q]", r"[q-\d]", "[z-a]", r"\q", "]"] {
        assert!(matches!(Regex::compile(pattern, "u", &mut 100_000), Err(Error::Syntax(_))), "{pattern}");
    }
    for pattern in [".", "[^a]", r"\D", r"\u{1f642}", r"[a-\uE001]", "🙂"] {
        assert!(matches!(Regex::compile(pattern, "u", &mut 100_000), Err(Error::Unsupported(_))), "{pattern}");
    }
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
    assert!(matches!(Regex::compile_units(&vec![0xd842;1366],"",&mut 100_000),Err(Error::Limit(_))));
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
