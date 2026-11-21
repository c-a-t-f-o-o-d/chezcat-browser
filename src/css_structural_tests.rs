//! Real child/type positions, strict shared parsing and finite matching work.
use super::*;
fn id(d: &Document, name: &str) -> usize {
    (0..d.nodes.len())
        .find(|&n| d.attr(n, "id") == Some(name))
        .unwrap()
}
fn fixture() -> Document {
    crate::html::parse("<ul id=parent>text<!--c--><li id=a></li> <span id=x></span><li id=b> </li><li id=c><!--c--></li></ul><div id=single><p id=only></p></div>")
}
#[test]
fn structural_selectors_count_only_real_element_siblings_and_share_the_cascade() {
    for (selector, wanted) in [
        ("li:first-child", vec!["a"]),
        ("li:last-child", vec!["c"]),
        ("li:nth-child(3)", vec!["b"]),
        ("li:nth-child(2n + 1)", vec!["a", "b"]),
        ("li:nth-last-child(-n+2)", vec!["b", "c"]),
        ("li:nth-of-type(even)", vec!["b"]),
        ("li:nth-last-of-type(2)", vec!["b"]),
        ("span:only-of-type", vec!["x"]),
        ("p:only-child", vec!["only"]),
        ("li:empty", vec!["a", "c"]),
        ("li:not(:first-of-type):last-of-type", vec!["c"]),
    ] {
        let css = format!("{selector}{{width:37px}}");
        let external = HashMap::new();
        let d=crate::html::parse(&format!("<style>{css}</style><ul id=parent>text<!--c--><li id=a></li> <span id=x></span><li id=b> </li><li id=c><!--c--></li></ul><div id=single><p id=only></p></div>"));
        let candidates = ["a", "x", "b", "c", "only"].map(|name| id(&d, name));
        let mut work = 100_000;
        let got = query_all(&d, selector, &candidates, None, &mut work).unwrap();
        assert_eq!(
            got,
            wanted.iter().map(|name| id(&d, name)).collect::<Vec<_>>(),
            "{selector}"
        );
        let styles = compute(&d, &external);
        for &node in &candidates {
            assert_eq!(
                styles[node].width == Length::Px(37.0),
                got.contains(&node),
                "cascade {selector}"
            );
        }
    }
}
#[test]
fn structural_selector_formula_syntax_refuses_whole_invalid_lists() {
    for invalid in [
        ":nth-child(2 n)",
        ":nth-child(n + -1)",
        ":nth-child()",
        ":nth-child(2n of .x)",
        ":first-child(1)",
        ":nth-of-type(1.5)",
        ":nth-last-child(n++)",
    ] {
        assert!(!valid_query_selector(&format!("li,{invalid}")), "{invalid}");
    }
    for valid in [
        ":nth-child(ODD)",
        ":nth-child(-n + 3)",
        ":nth-last-of-type(+2n-1)",
        ":only-child",
        ":empty",
    ] {
        assert!(valid_query_selector(valid), "{valid}");
    }
}
#[test]
fn structural_selector_huge_integer_coefficients_keep_exact_cancellation() {
    let d = fixture();
    let a = format!("1{}", "0".repeat(60));
    let b = format!("{}7", "9".repeat(59));
    let selector = format!("li:nth-child({a}n - {b})");
    let mut work = 100_000;
    assert_eq!(
        query_all(
            &d,
            &selector,
            &[id(&d, "a"), id(&d, "b"), id(&d, "c")],
            None,
            &mut work
        )
        .unwrap(),
        vec![id(&d, "b")]
    );
}
#[test]
fn structural_selector_exhaustion_cannot_be_inverted_by_negation() {
    let d = fixture();
    let mut work = d.nodes.len() * 3 + 3;
    let result = query_all(
        &d,
        "li:not(:nth-last-child(2n+1))",
        &[id(&d, "a"), id(&d, "b")],
        None,
        &mut work,
    );
    assert!(result.is_err());
    assert_eq!(work, 0);
}

#[test]
fn structural_selector_signed_formulas_agree_with_finite_integer_progressions() {
    let d = fixture();
    let candidates = ["a", "x", "b", "c"].map(|name| id(&d, name));
    for a in -3i32..=3 {
        for b in -12i32..=20 {
            let selector = format!(
                ":nth-child({a}n {} {})",
                if b < 0 { "-" } else { "+" },
                b.abs()
            );
            let mut work = 100_000;
            let got = query_all(&d, &selector, &candidates, None, &mut work).unwrap();
            let expected = candidates
                .iter()
                .enumerate()
                .filter_map(|(index, &node)| {
                    (0..40)
                        .any(|n| a * n + b == index as i32 + 1)
                        .then_some(node)
                })
                .collect::<Vec<_>>();
            assert_eq!(got, expected, "{selector}");
        }
    }
}
#[test]
fn structural_selector_expanded_types_and_syntax_copy_refusal_are_real() {
    let mut d = fixture();
    let x = id(&d, "x");
    if let NodeKind::Element { tag, .. } = &mut d.nodes[x].kind {
        *tag = "li".into();
    }
    d.nodes[x].namespace = crate::html::Namespace::Svg;
    let mut work = 100_000;
    assert_eq!(
        query_all(
            &d,
            "li:nth-of-type(2)",
            &[id(&d, "a"), x, id(&d, "b"), id(&d, "c")],
            None,
            &mut work
        )
        .unwrap(),
        vec![id(&d, "b")]
    );
    let (mut work, mut copies) = (100_000, 0);
    let parents = vec![None; d.nodes.len()];
    let slots = vec![0; d.nodes.len()];
    let error = query_with_topology(
        &d,
        ":nth-child(2n+1)",
        vec![id(&d, "a")],
        None,
        &mut work,
        &parents,
        &slots,
        false,
        &mut copies,
    )
    .unwrap_err();
    assert!(error.contains("syntax copy allowance"));
    assert_eq!(copies, 0);
}
