//! Shared DOM/CSS attribute operations, strict grammar and metered scans.
use super::*;

fn target(doc: &Document) -> usize {
    (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some("target")).unwrap()
}

#[test]
fn attribute_substring_operators_share_css_dom_empty_and_ascii_case_rules() {
    for (attributes, selector, expected) in [
        ("data-key='before-middle-after'", "[data-key^=before]", true),
        ("data-key='before-middle-after'", "[data-key^=middle]", false),
        ("data-key='before-middle-after'", "[data-key$=after]", true),
        ("data-key='before-middle-after'", "[data-key$=middle]", false),
        ("data-key='before-middle-after'", "[data-key*=middle]", true),
        ("data-key='before-middle-after'", "[data-key*=absent]", false),
        ("data-key='abc'", "[data-key^=abcd]", false),
        ("data-key='abc'", "[data-key$=abcd]", false),
        ("data-key='abc'", "[data-key*=abcd]", false),
        ("data-key='abc'", "[data-key^='']", false),
        ("data-key='abc'", "[data-key$='']", false),
        ("data-key='abc'", "[data-key*='']", false),
        ("data-key=''", "[data-key^='']", false),
        ("data-key=''", "[data-key$='']", false),
        ("data-key=''", "[data-key*='']", false),
        ("", "[data-key*='']", false),
        ("data-key=''", "[data-key='']", true),
        ("data-key=''", "[data-key~='']", false),
        ("data-key='en'", "[data-key|=en]", true),
        ("data-key='en-US'", "[data-key|=en]", true),
        ("data-key='en--US'", "[data-key|=en]", true),
        ("data-key='english'", "[data-key|=en]", false),
        ("data-key='en_US'", "[data-key|=en]", false),
        ("data-key=''", "[data-key|='']", true),
        ("data-key='-leading'", "[data-key|='']", true),
        ("data-key='leading'", "[data-key|='']", false),
        ("data-key='Camel-Case'", "[data-key^=camel]", false),
        ("data-key='Camel-Case'", "[data-key^=camel i]", true),
        ("data-key='Camel-Case'", "[data-key$=case I]", true),
        ("data-key='Camel-Case'", "[data-key*=MEL-c i]", true),
        ("data-key='Camel-Case'", "[data-key*=MEL-c s]", false),
        ("data-key='Camel-Case'", "[data-key|=camel I]", true),
        ("type=CHECKBOX", "[type^=check]", true),
        ("type=CHECKBOX", "[type$=box]", true),
        ("type=CHECKBOX", "[type*=eck]", true),
        ("type=CHECKBOX", "[type|=checkbox]", true),
        ("type=CHECKBOX", "[type^=check s]", false),
        ("type=CHECKBOX", "[type$=box s]", false),
        ("lang=EN-US", "[lang|=en]", true),
        ("lang=EN-US", "[lang|=en s]", false),
        ("data-key='Ä-mid-界'", "[data-key^='ä' i]", false),
        ("data-key='Ä-mid-界'", "[data-key*='Ä-MID' i]", true),
        ("data-key='Ä-mid-界'", "[data-key$='界']", true),
        ("data-key='x,y] > + ~ .*$'", "[data-key*=\"y] > + ~ .*$\"]", true),
        ("data-key='say &quot;yes&quot;'", "[data-key*='\"yes\"']", true),
        ("data-key='left ready right'", "[data-key~=ready]", true),
        ("data-key='already'", "[data-key~=ready]", false),
        ("data-key='before-middle-after'", "[data-key=middle]", false),
    ] {
        let doc = crate::html::parse(&format!(
            "<style>.target{{background:#aa2233}}{selector}{{background:#123456}}</style><div id=target class=target {attributes}></div>"
        ));
        let node = target(&doc);
        let mut budget = 100_000;
        assert_eq!(query_first(&doc, selector, &[node], None, &mut budget).unwrap(), expected.then_some(node), "DOM {attributes} {selector}");
        assert_eq!(compute(&doc, &HashMap::new())[node].background, Some(if expected { 0x123456 } else { 0xaa2233 }), "CSS {attributes} {selector}");
    }
}

#[test]
fn attribute_substring_invalid_members_and_existing_escape_boundary_refuse_whole_lists() {
    let doc = crate::html::parse("<div id=target data-key=prefix-suffix></div>");
    for invalid in [
        "[data-key^ =prefix]", "[data-key$==suffix]", "[data-key**=fix]",
        "[data-key| =prefix]", "[data-key*=]", "[data-key*=1]",
        "[data-key*='prefix' q]", "[data-key*='prefix' i s]",
        "[data-key*='prefix'", "[ns|data-key*=prefix]", "[*|data-key*=prefix]",
        "[data-key*='pre\\66 ix']", "[data-key*='pre\nfix']",
    ] {
        let source = format!("#target,{invalid}");
        assert!(!valid_query_selector(&source), "{source}");
        let mut rules = Vec::new();
        parse_sheet(&format!("{source}{{background:red}}"), &mut rules);
        assert!(rules.is_empty(), "invalid CSS member broadened a list: {source}");
        let mut budget = 100_000;
        assert!(query_all(&doc, &source, &[target(&doc)], None, &mut budget).is_err());
        assert_eq!(budget, 100_000, "invalid list must fail before partial matching");
    }
    let oversized = format!("[data-key*='{}']", "a".repeat(MAX_SELECTOR_BYTES));
    assert!(!valid_query_selector(&oversized));
}

#[test]
fn attribute_substring_repeated_prefixes_are_linear_precharged_and_never_invert_exhaustion() {
    let value = format!("{}b", "a".repeat(100_000));
    let operand = format!("{}b", "a".repeat(200));
    let doc = crate::html::parse(&format!("<div id=target data-key='{value}'></div>"));
    let node = target(&doc);
    let selector = format!("[data-key*='{operand}' i]");
    let mut budget = 10_000;
    assert_eq!(query_all(&doc, &selector, &[node], None, &mut budget).unwrap(), vec![node]);
    assert!(budget > 6_000 && budget < 7_000, "linear scan debit: {budget}");
    let missing = format!("[data-key*='{}c' i]", "a".repeat(200));
    let mut budget = 10_000;
    assert!(query_all(&doc, &missing, &[node], None, &mut budget).unwrap().is_empty());
    assert!(budget > 6_000 && budget < 7_000);
    for source in [selector.clone(), format!(":not({missing})")] {
        let mut budget = doc.nodes.len() * 3 + 10;
        let error = query_all(&doc, &source, &[node], None, &mut budget).unwrap_err();
        assert!(error.contains("work limit"), "{error}");
        assert_eq!(budget, 0);
    }
    for source in ["[data-key^=a]", "[data-key$=b]"] {
        let mut budget = 1_000;
        assert_eq!(query_first(&doc, source, &[node], None, &mut budget).unwrap(), Some(node));
        assert!(budget > 950, "edge matching must not scan the unrelated middle");
    }
    let mut fresh = 10_000;
    assert_eq!(query_first(&doc, &selector, &[node], None, &mut fresh).unwrap(), Some(node));
}

#[test]
fn attribute_substring_lists_scope_tree_order_and_specificity_remain_shared() {
    let doc = crate::html::parse(concat!(
        "<style>.candidate{background:red}[data-key*=mid]{background:blue}",
        "#target{background:#112233}[data-key^=pre][data-key$=end]{background:green}</style>",
        "<div id=scope><span id=target class=candidate data-key=pre-mid-end></span>",
        "<span id=second data-key=pre-mid-end></span></div><span id=outside data-key=mid></span>"
    ));
    let id = |name| (0..doc.nodes.len()).find(|&node| doc.attr(node, "id") == Some(name)).unwrap();
    let mut budget = 100_000;
    assert_eq!(query_all(&doc, "#scope > [data-key*=mid],#target", &[id("target"), id("second"), id("outside")], None, &mut budget).unwrap(), vec![id("target"), id("second")]);
    let styles = compute(&doc, &HashMap::new());
    assert_eq!(styles[id("target")].background, Some(0x112233), "an attribute operation retains ordinary class-level specificity");
    assert_eq!(styles[id("second")].background, Some(0x008000));
    assert_eq!(styles[id("outside")].background, Some(0x0000ff));
}
